use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::QueryBuilder;
use tokio::sync::mpsc;

const FALLBACK_ERROR_CODE: &str = "upstream_error";

pub fn normalize_error_code(raw: Option<&str>) -> Option<String> {
    let code = raw?.trim();
    if code.is_empty()
        || code.len() > 64
        || !code.as_bytes()[0].is_ascii_lowercase()
        || !code
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    {
        return Some(FALLBACK_ERROR_CODE.to_owned());
    }
    Some(code.to_owned())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogType {
    #[default]
    Chat,
    Voice,
    WebSearch,
}

impl LogType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Voice => "voice",
            Self::WebSearch => "web_search",
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct UsageRow {
    pub key_id: Option<String>,
    pub user_id: Option<String>,
    pub plan_id: Option<String>,
    pub model: String,
    pub requested_model: Option<String>,
    pub provider: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub status: i32,
    pub latency_ms: Option<i64>,
    pub error_code: Option<String>,
    pub log_type: LogType,
    pub details: serde_json::Value,
}

struct Stamped {
    event_id: String,
    at: DateTime<Utc>,
    row: UsageRow,
}

const CHUNK: usize = 500;
const FLUSH_AT: usize = 100;
const MAX_PENDING: usize = 5_000;

#[derive(Clone)]
pub struct UsageRecorder {
    tx: mpsc::Sender<Stamped>,
    pool: PgPool,
}

impl UsageRecorder {
    pub fn new(pool: PgPool) -> Self {
        let (tx, mut rx) = mpsc::channel::<Stamped>(4096);
        let worker_pool = pool.clone();
        tokio::spawn(async move {
            let mut buffer: Vec<Stamped> = Vec::with_capacity(128);
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    received = rx.recv() => match received {
                        Some(item) => {
                            buffer.push(item);
                            if buffer.len() >= FLUSH_AT {
                                flush(&worker_pool, &mut buffer).await;
                            }
                        }
                        None => {
                            flush(&worker_pool, &mut buffer).await;
                            break;
                        }
                    },
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            flush(&worker_pool, &mut buffer).await;
                        }
                    }
                }
            }
        });
        Self { tx, pool }
    }

    pub fn record(&self, row: UsageRow) {
        let item = Stamped {
            event_id: next_event_id(),
            at: Utc::now(),
            row,
        };
        if let Err(error) = self.tx.try_send(item) {
            tracing::error!(%error, "usage row dropped: recorder queue is full or closed");
        }
    }

    pub async fn settle_generation(
        &self,
        reservation_id: &str,
        row: UsageRow,
    ) -> Result<(), sqlx::Error> {
        settle_generation(&self.pool, reservation_id, row).await
    }
}

fn next_event_id() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("usage-{:x}-{nanos:x}-{sequence:x}", std::process::id())
}

async fn flush(pool: &PgPool, buffer: &mut Vec<Stamped>) {
    while !buffer.is_empty() {
        let take = buffer.len().min(CHUNK);
        match insert(pool, &buffer[..take]).await {
            Ok(()) => {
                buffer.drain(..take);
            }
            Err(error) => {
                tracing::error!(%error, pending = buffer.len(), "usage_logs insert failed; will retry");
                if buffer.len() > MAX_PENDING {
                    let dropped = buffer.len() - MAX_PENDING;
                    buffer.drain(..dropped);
                    tracing::error!(dropped, "usage rows dropped: database unreachable too long");
                }
                return;
            }
        }
    }
}

async fn insert(pool: &PgPool, rows: &[Stamped]) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut qb = QueryBuilder::new(
        "INSERT INTO usage_logs (event_id, created_at, key_id, user_id, plan_id, model, requested_model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code, log_type, details) ",
    );
    qb.push_values(rows.iter(), |mut b, item| {
        let row = &item.row;
        b.push_bind(item.event_id.clone())
            .push_bind(item.at)
            .push_bind(row.key_id.clone())
            .push_bind(row.user_id.clone())
            .push_bind(row.plan_id.clone())
            .push_bind(row.model.clone())
            .push_bind(row.requested_model.clone())
            .push_bind(row.provider.clone())
            .push_bind(row.input_tokens)
            .push_bind(row.output_tokens)
            .push_bind(row.cache_read_tokens)
            .push_bind(row.cost_usd)
            .push_bind(row.status)
            .push_bind(row.latency_ms)
            .push_bind(normalize_error_code(row.error_code.as_deref()))
            .push_bind(row.log_type.as_str())
            .push_bind(row.details.to_string());
    });
    qb.push(" ON CONFLICT (event_id) DO NOTHING");
    qb.build().execute(tx.as_mut()).await?;
    tx.commit().await
}

async fn settle_generation(
    pool: &PgPool,
    reservation_id: &str,
    row: UsageRow,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let reserved = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS (SELECT 1 FROM generation_reservations WHERE id = $1)",
    )
    .bind(reservation_id)
    .fetch_one(tx.as_mut())
    .await?;
    if !reserved {
        let exists = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM usage_logs WHERE event_id = $1)",
        )
        .bind(reservation_id)
        .fetch_one(tx.as_mut())
        .await?;
        tx.rollback().await?;
        return if exists {
            Ok(())
        } else {
            Err(sqlx::Error::RowNotFound)
        };
    }

    let error_code = normalize_error_code(row.error_code.as_deref());
    let usage_inserted = sqlx::query(
        "INSERT INTO usage_logs
           (event_id, created_at, key_id, user_id, plan_id, model, requested_model, provider,
             input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code,
             log_type, details)
          VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
          ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(reservation_id)
    .bind(Utc::now())
    .bind(row.key_id)
    .bind(row.user_id)
    .bind(row.plan_id)
    .bind(row.model)
    .bind(row.requested_model)
    .bind(row.provider)
    .bind(row.input_tokens)
    .bind(row.output_tokens)
    .bind(row.cache_read_tokens)
    .bind(row.cost_usd)
    .bind(row.status)
    .bind(row.latency_ms)
    .bind(error_code)
    .bind(row.log_type.as_str())
    .bind(row.details.to_string())
    .execute(tx.as_mut())
    .await?
    .rows_affected();
    if usage_inserted != 1 {
        tx.rollback().await?;
        return Err(sqlx::Error::RowNotFound);
    }

    sqlx::query("DELETE FROM generation_reservations WHERE id = $1")
        .bind(reservation_id)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_error_code;

    #[test]
    fn normalizes_error_codes() {
        assert_eq!(normalize_error_code(None), None);
        assert_eq!(
            normalize_error_code(Some("  upstream_error ")),
            Some("upstream_error".to_owned())
        );
        assert_eq!(
            normalize_error_code(Some("Brave search error (429): {\"message\":\"slow down\"}")),
            Some("upstream_error".to_owned())
        );
        assert_eq!(normalize_error_code(Some("")), Some("upstream_error".to_owned()));
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use sqlx::QueryBuilder;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct UsageRow {
    pub key_id: Option<String>,
    pub user_id: Option<String>,
    pub plan_id: Option<String>,
    pub model: String,
    pub provider: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cost_usd: f64,
    pub status: i32,
    pub latency_ms: Option<i64>,
    pub error_code: Option<String>,
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
        "INSERT INTO usage_logs (event_id, created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code) ",
    );
    qb.push_values(rows.iter(), |mut b, item| {
        let row = &item.row;
        b.push_bind(item.event_id.clone())
            .push_bind(item.at)
            .push_bind(row.key_id.clone())
            .push_bind(row.user_id.clone())
            .push_bind(row.plan_id.clone())
            .push_bind(row.model.clone())
            .push_bind(row.provider.clone())
            .push_bind(row.input_tokens)
            .push_bind(row.output_tokens)
            .push_bind(row.cache_read_tokens)
            .push_bind(row.cost_usd)
            .push_bind(row.status)
            .push_bind(row.latency_ms)
            .push_bind(row.error_code.clone());
    });
    qb.push(" ON CONFLICT (event_id) DO NOTHING");
    qb.build().execute(tx.as_mut()).await?;
    tx.commit().await
}

fn cost_micros(cost_usd: f64) -> i64 {
    if !cost_usd.is_finite() || cost_usd <= 0.0 {
        return 0;
    }
    (cost_usd * 1_000_000.0).round().clamp(0.0, i64::MAX as f64) as i64
}

async fn settle_generation(
    pool: &PgPool,
    reservation_id: &str,
    row: UsageRow,
) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    let reservation = sqlx::query(
        "SELECT user_id, held_micros, credits_paid
         FROM generation_reservations
         WHERE id = $1
         FOR UPDATE",
    )
    .bind(reservation_id)
    .fetch_optional(tx.as_mut())
    .await?;
    let Some(reservation) = reservation else {
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
    };
    let user_id = sqlx::Row::try_get::<String, _>(&reservation, "user_id")?;
    let held_micros = sqlx::Row::try_get::<i64, _>(&reservation, "held_micros")?;
    let credits_paid = sqlx::Row::try_get::<bool, _>(&reservation, "credits_paid")?;

    let usage_inserted = sqlx::query(
        "INSERT INTO usage_logs
           (event_id, created_at, key_id, user_id, plan_id, model, provider, input_tokens,
            output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         ON CONFLICT (event_id) DO NOTHING",
    )
    .bind(reservation_id)
    .bind(Utc::now())
    .bind(row.key_id)
    .bind(row.user_id)
    .bind(row.plan_id)
    .bind(row.model)
    .bind(row.provider)
    .bind(row.input_tokens)
    .bind(row.output_tokens)
    .bind(row.cache_read_tokens)
    .bind(row.cost_usd)
    .bind(row.status)
    .bind(row.latency_ms)
    .bind(row.error_code)
    .execute(tx.as_mut())
    .await?
    .rows_affected();
    if usage_inserted != 1 {
        tx.rollback().await?;
        return Err(sqlx::Error::RowNotFound);
    }

    if credits_paid {
        let target_charge = if row.cost_usd > 0.0 || row.status == 200 {
            cost_micros(row.cost_usd).max(1)
        } else {
            0
        };
        let wallet_balance = sqlx::query_scalar::<_, i64>(
            "SELECT balance_micros FROM credit_wallet WHERE user_id = $1 FOR UPDATE",
        )
        .bind(&user_id)
        .fetch_optional(tx.as_mut())
        .await?
        .unwrap_or(0);
        let charged = target_charge.min(held_micros.saturating_add(wallet_balance));
        let adjustment = held_micros.saturating_sub(charged);
        if adjustment != 0 {
            sqlx::query(
                "UPDATE credit_wallet
                 SET balance_micros = balance_micros + $2, updated_at = NOW()
                 WHERE user_id = $1",
            )
            .bind(&user_id)
            .bind(adjustment)
            .execute(tx.as_mut())
            .await?;
        }
        if charged > 0 {
            let ledger_inserted = sqlx::query(
                "INSERT INTO credit_transactions (user_id, delta_micros, reason, reference)
                 VALUES ($1, $2, 'usage', $3)
                 ON CONFLICT (reason, reference) DO NOTHING",
            )
            .bind(&user_id)
            .bind(-charged)
            .bind(reservation_id)
            .execute(tx.as_mut())
            .await?
            .rows_affected();
            if ledger_inserted != 1 {
                tx.rollback().await?;
                return Err(sqlx::Error::RowNotFound);
            }
        }
        if charged < target_charge {
            tracing::error!(
                reservation_id,
                target_charge,
                charged,
                "generation cost exceeded its credit reservation"
            );
        }
    }

    sqlx::query("DELETE FROM generation_reservations WHERE id = $1")
        .bind(reservation_id)
        .execute(tx.as_mut())
        .await?;
    tx.commit().await?;
    Ok(())
}

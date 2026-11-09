use std::time::Duration;

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

/// A row plus the moment the request was recorded. The timestamp is taken at
/// `record()` time, not at flush time, so rows batched for several seconds
/// still land in the hour/day bucket the request actually happened in.
struct Stamped {
    at: DateTime<Utc>,
    row: UsageRow,
}

/// Rows per INSERT statement (13 binds each, well under Postgres' 65535 cap).
const CHUNK: usize = 500;
/// Flush as soon as this many rows are buffered.
const FLUSH_AT: usize = 100;
/// While the database is unreachable, keep at most this many rows in memory;
/// beyond it the oldest are dropped (and counted in the error log).
const MAX_PENDING: usize = 5_000;

#[derive(Clone)]
pub struct UsageRecorder {
    tx: mpsc::Sender<Stamped>,
}

impl UsageRecorder {
    pub fn new(pool: PgPool) -> Self {
        let (tx, mut rx) = mpsc::channel::<Stamped>(4096);
        tokio::spawn(async move {
            let mut buffer: Vec<Stamped> = Vec::with_capacity(128);
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    received = rx.recv() => match received {
                        Some(item) => {
                            buffer.push(item);
                            if buffer.len() >= FLUSH_AT {
                                flush(&pool, &mut buffer).await;
                            }
                        }
                        // Every sender is gone (shutdown): write what is left.
                        None => {
                            flush(&pool, &mut buffer).await;
                            break;
                        }
                    },
                    _ = interval.tick() => {
                        if !buffer.is_empty() {
                            flush(&pool, &mut buffer).await;
                        }
                    }
                }
            }
        });
        Self { tx }
    }

    pub fn record(&self, row: UsageRow) {
        let item = Stamped {
            at: Utc::now(),
            row,
        };
        if let Err(error) = self.tx.try_send(item) {
            // Never block the request path, but never lose a row silently.
            tracing::error!(%error, "usage row dropped: recorder queue is full or closed");
        }
    }
}

/// Writes the buffer. On success the buffer is emptied; on failure the rows
/// stay buffered and are retried on the next tick (bounded by `MAX_PENDING`).
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
    let mut qb = QueryBuilder::new(
        "INSERT INTO usage_logs (created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code) ",
    );
    qb.push_values(rows.iter(), |mut b, item| {
        let row = &item.row;
        // `created_at` is TIMESTAMPTZ: bind a typed timestamp, not a string.
        b.push_bind(item.at)
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
    qb.build().execute(pool).await?;
    Ok(())
}
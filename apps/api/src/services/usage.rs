use std::time::Duration;

use chrono::Utc;
use sqlx::QueryBuilder;
use sqlx::PgPool;
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

#[derive(Clone)]
pub struct UsageRecorder {
    tx: mpsc::Sender<UsageRow>,
}

impl UsageRecorder {
    pub fn new(pool: PgPool) -> Self {
        let (tx, mut rx) = mpsc::channel::<UsageRow>(4096);
        tokio::spawn(async move {
            let mut buffer: Vec<UsageRow> = Vec::with_capacity(128);
            let mut interval = tokio::time::interval(Duration::from_secs(5));
            loop {
                tokio::select! {
                    Some(row) = rx.recv() => {
                        buffer.push(row);
                        if buffer.len() >= 100 {
                            flush(&pool, &mut buffer).await;
                        }
                    }
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
        let _ = self.tx.try_send(row);
    }
}

async fn flush(pool: &PgPool, rows: &mut Vec<UsageRow>) {
    if rows.is_empty() {
        return;
    }
    let mut qb = QueryBuilder::new(
        "INSERT INTO usage_logs (created_at, key_id, user_id, plan_id, model, provider, input_tokens, output_tokens, cache_read_tokens, cost_usd, status, latency_ms, error_code) ",
    );
    qb.push_values(rows.iter(), |mut b, row| {
        b.push_bind(Utc::now().to_rfc3339())
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
    let _ = qb.build().execute(pool).await;
    rows.clear();
}
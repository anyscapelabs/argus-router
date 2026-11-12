use sqlx::PgPool;

use super::AppDb;

impl AppDb {
    pub async fn from_pool(pool: PgPool) -> Result<Self, Box<dyn std::error::Error>> {
        let db = Self { pool };
        db.migrate().await?;
        Ok(db)
    }

    async fn migrate(&self) -> Result<(), sqlx::Error> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS plans (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                price TEXT NOT NULL DEFAULT '0',
                payload JSONB NOT NULL DEFAULT '{}',
                searches_per_month BIGINT NOT NULL DEFAULT 0,
                voice_minutes BIGINT NOT NULL DEFAULT 0,
                created_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS price TEXT NOT NULL DEFAULT '0'",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS payload JSONB NOT NULL DEFAULT '{}'",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS searches_per_month BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS voice_minutes BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS monthly_tokens BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS weekly_tokens BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS daily_requests BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "ALTER TABLE plans ADD COLUMN IF NOT EXISTS rate_per_min BIGINT NOT NULL DEFAULT 0",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_entitlements (
                user_id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL REFERENCES plans(id),
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_allowlist (
                plan_id TEXT NOT NULL REFERENCES plans(id),
                model_id TEXT NOT NULL,
                PRIMARY KEY (plan_id, model_id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_preferences (
                user_id TEXT NOT NULL,
                model_id TEXT NOT NULL,
                enabled BOOLEAN NOT NULL,
                PRIMARY KEY (user_id, model_id)
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS model_catalog (
                id TEXT PRIMARY KEY,
                payload JSONB NOT NULL,
                updated_at TEXT NOT NULL
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS usage_logs (
                id BIGSERIAL PRIMARY KEY,
                event_id TEXT UNIQUE,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                key_id TEXT,
                user_id TEXT,
                plan_id TEXT,
                model TEXT NOT NULL,
                provider TEXT,
                input_tokens BIGINT NOT NULL DEFAULT 0,
                output_tokens BIGINT NOT NULL DEFAULT 0,
                cache_read_tokens BIGINT NOT NULL DEFAULT 0,
                cost_usd DOUBLE PRECISION NOT NULL DEFAULT 0,
                status INTEGER NOT NULL,
                latency_ms BIGINT,
                error_code TEXT
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query("ALTER TABLE usage_logs ADD COLUMN IF NOT EXISTS event_id TEXT")
            .execute(&self.pool)
            .await?;
        sqlx::query("DROP INDEX IF EXISTS usage_logs_event_id")
            .execute(&self.pool)
            .await?;
        sqlx::query("CREATE UNIQUE INDEX usage_logs_event_id ON usage_logs (event_id)")
            .execute(&self.pool)
            .await?;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS usage_logs_key_created ON usage_logs (key_id, created_at)",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS usage_logs_user_created ON usage_logs (user_id, created_at)",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS usage_logs_search_quota              ON usage_logs (user_id, model, status, created_at)",
        )
        .execute(&self.pool)
        .await;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS usage_logs_voice_quota              ON usage_logs (user_id, provider, status, created_at)",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS search_monthly_usage (
                user_id TEXT NOT NULL,
                month_start DATE NOT NULL,
                used BIGINT NOT NULL DEFAULT 0 CHECK (used >= 0),
                PRIMARY KEY (user_id, month_start)
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "INSERT INTO search_monthly_usage (user_id, month_start, used)
             SELECT user_id, date_trunc('month', CURRENT_TIMESTAMP)::date, COUNT(*)
             FROM usage_logs
             WHERE user_id IS NOT NULL
               AND model = 'web-search'
               AND status = 200
               AND created_at >= date_trunc('month', CURRENT_TIMESTAMP)
             GROUP BY user_id
             ON CONFLICT (user_id, month_start) DO NOTHING",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS daily_request_usage (
                user_id TEXT NOT NULL,
                day_start DATE NOT NULL,
                used BIGINT NOT NULL DEFAULT 0 CHECK (used >= 0),
                PRIMARY KEY (user_id, day_start)
            )",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "DELETE FROM daily_request_usage WHERE day_start < CURRENT_DATE - INTERVAL '35 days'",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS voice_reservations (
                session_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS voice_reservations_user ON voice_reservations (user_id)",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS user_settings (
                user_id TEXT PRIMARY KEY,
                allow_free_training BOOLEAN NOT NULL DEFAULT TRUE,
                max_key_lifetime_days BIGINT NOT NULL DEFAULT 0,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_customers (
                user_id TEXT PRIMARY KEY,
                customer_id TEXT NOT NULL UNIQUE,
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_subscriptions (
                subscription_id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                plan_id TEXT NOT NULL REFERENCES plans(id),
                status TEXT NOT NULL,
                next_billing_date TEXT,
                past_due_ends_at TEXT,
                event_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "ALTER TABLE billing_subscriptions ADD COLUMN IF NOT EXISTS event_at TIMESTAMPTZ NOT NULL DEFAULT NOW()",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS billing_subscriptions_user ON billing_subscriptions (user_id)",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS credit_wallet (
                user_id TEXT PRIMARY KEY,
                balance_micros BIGINT NOT NULL DEFAULT 0 CHECK (balance_micros >= 0),
                updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS generation_reservations (
                id TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                reserved_tokens BIGINT NOT NULL CHECK (reserved_tokens >= 0),
                held_micros BIGINT NOT NULL CHECK (held_micros >= 0),
                credits_paid BOOLEAN NOT NULL,
                dispatched_at TIMESTAMPTZ,
                expires_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        sqlx::query(
            "ALTER TABLE generation_reservations ADD COLUMN IF NOT EXISTS dispatched_at TIMESTAMPTZ",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS generation_reservations_user_expiry ON generation_reservations (user_id, expires_at)",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS credit_transactions (
                id BIGSERIAL PRIMARY KEY,
                user_id TEXT NOT NULL,
                delta_micros BIGINT NOT NULL,
                reason TEXT NOT NULL,
                reference TEXT NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
                UNIQUE (reason, reference)
            )",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "CREATE INDEX IF NOT EXISTS credit_transactions_user ON credit_transactions (user_id, created_at)",
        )
        .execute(&self.pool)
        .await;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS webhook_events (
                webhook_id TEXT PRIMARY KEY,
                processed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS billing_checkout_sessions (
                checkout_key TEXT PRIMARY KEY,
                user_id TEXT NOT NULL,
                checkout_url TEXT,
                expires_at TIMESTAMPTZ NOT NULL,
                created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            )",
        )
        .execute(&self.pool)
        .await?;
        let _ = sqlx::query(
            "DELETE FROM webhook_events WHERE processed_at < NOW() - INTERVAL '90 days'",
        )
        .execute(&self.pool)
        .await;

        Ok(())
    }
}

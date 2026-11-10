use std::str::FromStr;

use chrono::Utc;
use serde::Serialize;
use sqlx::PgPool;
use sqlx::Row;

use crate::config::Settings;
use crate::services::catalog::ModelCatalogEntry;

#[derive(Clone, Debug)]
pub enum QuotaCheck {
    Allowed { used: i64, quota: i64 },
    NotAvailable,
    Exceeded { used: i64, quota: i64 },
}

/// App-owned database access — the auth tables are better-auth's domain (its
/// store), everything about *paid* Argus lives here: plans, entitlements,
/// the dynamic model catalog, and the per-plan model allowlist consumed by
/// the dashboard models page and the inference proxy.
#[derive(Clone)]
pub struct AppDb {
    pool: PgPool,
}

impl AppDb {
    pub fn pool(&self) -> PgPool {
        self.pool.clone()
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub id: String,
    pub name: String,
    pub price: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tagline: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<crate::services::plans::PlanLimits>,
    #[serde(default)]
    pub searches_per_month: i64,
    #[serde(default)]
    pub voice_minutes: i64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_names: Vec<String>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub model_count: String,
    pub models: Vec<String>,
}

pub const FREE_PLAN_ID: &str = "free";

impl AppDb {
    pub async fn connect(settings: &Settings) -> Result<Self, Box<dyn std::error::Error>> {
        // The same Postgres instance better-auth's store runs on (Neon in
        // production). `PgConnectOptions::from_str` honors `sslmode=require`
        // in the URL, negotiating TLS through rustls.
        let options = sqlx::postgres::PgConnectOptions::from_str(&settings.database_url)?;
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(5)
            .min_connections(1)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect_with(options)
            .await?;
        let db = Self { pool };
        db.migrate().await?;
        // Note: seed_plans() is called from main() after AppDb::connect(),
        // so there is no need for a redundant minimal seed() here.
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
        // Backfill columns on databases created before they existed.
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

        // Dynamic model catalog: one row per model, its seed entry stored as
        // a JSON payload. New rows (or edits) unload into the router on
        // `POST /api/v1/models/refresh` — no redeploy needed.
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
        // Covering indexes for search and voice quota queries.
        // Prevents full table scans on usage_logs as the table grows.
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

        Ok(())
    }

    /// Upserts plans from the plans catalog: stores the full spec in JSONB
    /// payload (for limits/taglines/model names) and also syncs quota columns
    /// used by quota enforcement.
    pub async fn seed_plans(
        &self,
        plans: &[crate::services::plans::PlanCatalogEntry],
    ) -> Result<(), sqlx::Error> {
        // Drop plans that are no longer in the catalog so removed plans don't
        // linger in the list shown to users.
        let new_ids: Vec<String> = plans.iter().map(|p| p.id.clone()).collect();
        if !new_ids.is_empty() {
            sqlx::query("DELETE FROM model_allowlist WHERE NOT (plan_id = ANY($1::text[]))")
                .bind(&new_ids)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM plans WHERE NOT (id = ANY($1::text[]))")
                .bind(&new_ids)
                .execute(&self.pool)
                .await?;
            // Reassign users on removed plans to the free plan.
            sqlx::query(
                "UPDATE user_entitlements SET plan_id = $1 WHERE NOT (plan_id = ANY($2::text[]))",
            )
            .bind(FREE_PLAN_ID)
            .bind(&new_ids)
            .execute(&self.pool)
            .await?;
        }
        for plan in plans {
            let payload =
                serde_json::to_string(plan).map_err(|error| sqlx::Error::ColumnDecode {
                    index: "payload".into(),
                    source: Box::new(error),
                })?;
            sqlx::query(
                "INSERT INTO plans (id, name, price, payload, searches_per_month, voice_minutes, created_at)
                 VALUES ($1, $2, $3, $4::jsonb, $5, $6, $7)
                 ON CONFLICT (id) DO UPDATE SET
                   name = EXCLUDED.name,
                   price = EXCLUDED.price,
                   payload = EXCLUDED.payload,
                   searches_per_month = EXCLUDED.searches_per_month,
                   voice_minutes = EXCLUDED.voice_minutes",
            )
            .bind(&plan.id)
            .bind(&plan.name)
            .bind(&plan.price)
            .bind(payload)
            .bind(plan.searches_per_month)
            .bind(plan.voice_minutes)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    /// Replaces each plan's allowlist with its configured models (fixed per plan).
    pub async fn seed_plan_allowlists(
        &self,
        plans: &[crate::services::plans::PlanCatalogEntry],
    ) -> Result<(), sqlx::Error> {
        for plan in plans {
            sqlx::query("DELETE FROM model_allowlist WHERE plan_id = $1")
                .bind(&plan.id)
                .execute(&self.pool)
                .await?;
            for model_id in &plan.models {
                sqlx::query(
                    "INSERT INTO model_allowlist (plan_id, model_id) VALUES ($1, $2) ON CONFLICT (plan_id, model_id) DO NOTHING",
                )
                .bind(&plan.id)
                .bind(model_id)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }

    /// Returns all plans with their full specs. The JSONB payload (seeded from
    /// plans.json) is parsed so limits/model names don't force a redeploy.
    /// Plans are ordered by price ascending so plans show Free → … → Max.
    pub async fn list_plans(&self) -> Result<Vec<PlanView>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT id, name, price, searches_per_month, voice_minutes, payload::text AS payload FROM plans",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut out = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let name: String = row.get("name");
            let price: String = row.try_get("price").unwrap_or_else(|_| "0".to_owned());
            let searches_per_month: i64 = row.try_get("searches_per_month").unwrap_or(0);
            let voice_minutes: i64 = row.try_get("voice_minutes").unwrap_or(0);
            let payload: String = row.try_get("payload").unwrap_or_default();
            let entry: Option<crate::services::plans::PlanCatalogEntry> =
                serde_json::from_str(&payload).ok();
            let models = self.allowlist_of(&id).await.unwrap_or_default();
            out.push(PlanView {
                id,
                name,
                price,
                tagline: entry
                    .as_ref()
                    .map(|e| e.tagline.clone())
                    .unwrap_or_default(),
                limits: entry.as_ref().and_then(|e| e.limits.clone()),
                searches_per_month,
                voice_minutes,
                model_names: entry
                    .as_ref()
                    .map(|e| e.model_names.clone())
                    .unwrap_or_default(),
                model_count: entry
                    .as_ref()
                    .map(|e| e.model_count.clone())
                    .unwrap_or_default(),
                models,
            });
        }
        // Sort by price ascending so plans show Free → Starter → … → Max
        // regardless of insertion order in the table.
        out.sort_by(|a, b| {
            let pa = a.price.parse::<f64>().unwrap_or(f64::MAX);
            let pb = b.price.parse::<f64>().unwrap_or(f64::MAX);
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    /// Atomically checks a user's monthly web-search quota under a per-user advisory
    /// transaction lock, preventing TOCTOU quota bypass under concurrent requests.
    pub async fn check_search_quota(
        &self,
        user_id: &str,
        plan_id: &str,
    ) -> Result<QuotaCheck, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("search-quota:{user_id}"))
            .execute(tx.as_mut())
            .await?;

        let quota_row = sqlx::query("SELECT searches_per_month FROM plans WHERE id = $1")
            .bind(plan_id)
            .fetch_optional(tx.as_mut())
            .await?;
        let quota = quota_row
            .and_then(|r| r.try_get::<i64, _>("searches_per_month").ok())
            .unwrap_or(0);
        if quota <= 0 {
            tx.commit().await?;
            return Ok(QuotaCheck::NotAvailable);
        }

        let used_row = sqlx::query(
            "SELECT COUNT(*) AS total FROM usage_logs
             WHERE user_id = $1
               AND model = 'web-search'
               AND status = 200
               AND created_at >= date_trunc('month', CURRENT_TIMESTAMP)",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await?;
        let used = used_row.try_get::<i64, _>("total").unwrap_or(0);

        tx.commit().await?;
        if used >= quota {
            Ok(QuotaCheck::Exceeded { used, quota })
        } else {
            Ok(QuotaCheck::Allowed { used, quota })
        }
    }

    /// Atomically checks a user's monthly voice-minutes quota under a per-user
    /// advisory transaction lock. DB errors are propagated (not treated as quota decisions).
    pub async fn check_voice_quota(
        &self,
        user_id: &str,
        plan_id: &str,
    ) -> Result<QuotaCheck, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("voice-quota:{user_id}"))
            .execute(tx.as_mut())
            .await?;

        let quota_row = sqlx::query("SELECT voice_minutes FROM plans WHERE id = $1")
            .bind(plan_id)
            .fetch_optional(tx.as_mut())
            .await?;
        let quota = quota_row
            .and_then(|r| r.try_get::<i64, _>("voice_minutes").ok())
            .unwrap_or(0);
        if quota <= 0 {
            tx.commit().await?;
            return Ok(QuotaCheck::NotAvailable);
        }

        let used_row = sqlx::query(
            "SELECT COALESCE(SUM(CEIL(latency_ms::numeric / 60000.0)), 0)::bigint as used_minutes
             FROM usage_logs
             WHERE user_id = $1
               AND status IN (200, 429)
               AND latency_ms > 0
               AND provider = 'gemini-live'
               AND created_at >= date_trunc('month', CURRENT_TIMESTAMP)",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await?;
        let used = used_row.try_get::<i64, _>("used_minutes").unwrap_or(0);

        tx.commit().await?;
        if used >= quota {
            Ok(QuotaCheck::Exceeded { used, quota })
        } else {
            Ok(QuotaCheck::Allowed { used, quota })
        }
    }

    pub async fn plan_of(&self, user_id: &str) -> Result<String, sqlx::Error> {
        let row = sqlx::query("SELECT plan_id FROM user_entitlements WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => Ok(row.get("plan_id")),
            None => {
                // Operators default onto the free plan at first contact.
                sqlx::query(
                    "INSERT INTO user_entitlements (user_id, plan_id, updated_at) VALUES ($1, $2, $3) ON CONFLICT (user_id) DO NOTHING",
                )
                .bind(user_id)
                .bind(FREE_PLAN_ID)
                .bind(Utc::now().to_rfc3339())
                .execute(&self.pool)
                .await?;
                Ok(FREE_PLAN_ID.to_owned())
            }
        }
    }

    pub async fn allowlist_of(&self, plan_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query("SELECT model_id FROM model_allowlist WHERE plan_id = $1")
            .bind(plan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|row| row.get("model_id")).collect())
    }

    /// Flips one model on/off the plan allowlist (dashboard models page).
    /// Object rows are left untouched so existing rows stay untouched.
    pub async fn allowlist_set(
        &self,
        plan_id: &str,
        model_id: &str,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        if enabled {
            sqlx::query(
                "INSERT INTO model_allowlist (plan_id, model_id) VALUES ($1, $2) ON CONFLICT (plan_id, model_id) DO NOTHING",
            )
            .bind(plan_id)
            .bind(model_id)
            .execute(&self.pool)
            .await?;
        } else {
            sqlx::query("DELETE FROM model_allowlist WHERE plan_id = $1 AND model_id = $2")
                .bind(plan_id)
                .bind(model_id)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Seeds the dynamic catalog from the embedded JSON (idempotent) and
    /// loads the table contents. When a seed is given, rows outside it are
    /// pruned — the embedded list is the catalog, so a table polluted by an
    /// earlier wide-open sync converges on boot even without network. Rows
    /// are returned id-ordered so the dashboard list is stable.
    pub async fn load_catalog(
        &self,
        seed: &[ModelCatalogEntry],
    ) -> Result<Vec<ModelCatalogEntry>, sqlx::Error> {
        for entry in seed {
            let payload =
                serde_json::to_string(entry).map_err(|error| sqlx::Error::ColumnDecode {
                    index: "payload".into(),
                    source: Box::new(error),
                })?;
            sqlx::query(
                "INSERT INTO model_catalog (id, payload, updated_at) VALUES ($1, $2::jsonb, $3)
                 ON CONFLICT (id) DO UPDATE SET payload = EXCLUDED.payload, updated_at = EXCLUDED.updated_at",
            )
            .bind(&entry.id)
            .bind(payload)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        if !seed.is_empty() {
            let keep: Vec<&str> = seed.iter().map(|entry| entry.id.as_str()).collect();
            sqlx::query("DELETE FROM model_catalog WHERE NOT (id = ANY($1))")
                .bind(&keep)
                .execute(&self.pool)
                .await?;
        }

        let rows = sqlx::query("SELECT payload::text AS payload FROM model_catalog ORDER BY id")
            .fetch_all(&self.pool)
            .await?;
        let mut parsed = Vec::new();
        for row in &rows {
            let text: String = row.try_get("payload")?;
            if let Ok(entry) = serde_json::from_str::<ModelCatalogEntry>(&text) {
                parsed.push(entry);
            }
        }
        Ok(parsed)
    }

    /// Upserts the catalog from an OpenRouter sync: rows are refreshed
    /// (payload + timestamp) while unknown ids are dropped so the table
    /// converges on what the origin currently serves.
    pub async fn catalog_sync(&self, entries: &[ModelCatalogEntry]) -> Result<(), sqlx::Error> {
        let seen: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
        for entry in entries {
            let payload =
                serde_json::to_string(entry).map_err(|error| sqlx::Error::ColumnDecode {
                    index: "payload".into(),
                    source: Box::new(error),
                })?;
            sqlx::query(
                "INSERT INTO model_catalog (id, payload, updated_at) VALUES ($1, $2::jsonb, $3)
                 ON CONFLICT (id) DO UPDATE SET payload = EXCLUDED.payload, updated_at = EXCLUDED.updated_at",
            )
            .bind(&entry.id)
            .bind(payload)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        if !seen.is_empty() {
            sqlx::query("DELETE FROM model_catalog WHERE NOT (id = ANY($1))")
                .bind(&seen)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

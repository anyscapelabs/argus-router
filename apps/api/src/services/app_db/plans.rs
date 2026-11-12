use chrono::Utc;
use sqlx::Row;
use std::collections::HashMap;

use super::{AppDb, FREE_PLAN_ID, PlanQuota, PlanView};
use crate::services::plans::PlanCatalogEntry;

impl AppDb {
    pub async fn seed_plans(&self, plans: &[PlanCatalogEntry]) -> Result<(), sqlx::Error> {
        let new_ids: Vec<String> = plans.iter().map(|p| p.id.clone()).collect();
        if !new_ids.is_empty() {
            sqlx::query(
                "UPDATE user_entitlements SET plan_id = $1 WHERE NOT (plan_id = ANY($2::text[]))",
            )
            .bind(FREE_PLAN_ID)
            .bind(&new_ids)
            .execute(&self.pool)
            .await?;
            sqlx::query("DELETE FROM model_allowlist WHERE NOT (plan_id = ANY($1::text[]))")
                .bind(&new_ids)
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM plans WHERE NOT (id = ANY($1::text[]))")
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
            let (monthly, weekly, daily, rpm) = plan
                .limits
                .as_ref()
                .map(|l| {
                    (
                        l.monthly_tokens_count(),
                        l.weekly_tokens_count(),
                        l.daily_requests_count(),
                        l.rate_per_min_count(),
                    )
                })
                .unwrap_or((0, 0, 0, 0));
            sqlx::query(
                "INSERT INTO plans (id, name, price, payload, searches_per_month, voice_minutes, monthly_tokens, weekly_tokens, daily_requests, rate_per_min, created_at)
                 VALUES ($1, $2, $3, $4::jsonb, $5, $6, $7, $8, $9, $10, $11)
                 ON CONFLICT (id) DO UPDATE SET
                   name = EXCLUDED.name,
                   price = EXCLUDED.price,
                   payload = EXCLUDED.payload,
                   searches_per_month = EXCLUDED.searches_per_month,
                   voice_minutes = EXCLUDED.voice_minutes,
                   monthly_tokens = EXCLUDED.monthly_tokens,
                   weekly_tokens = EXCLUDED.weekly_tokens,
                   daily_requests = EXCLUDED.daily_requests,
                   rate_per_min = EXCLUDED.rate_per_min",
            )
            .bind(&plan.id)
            .bind(&plan.name)
            .bind(&plan.price)
            .bind(payload)
            .bind(plan.searches_per_month)
            .bind(plan.voice_minutes)
            .bind(monthly)
            .bind(weekly)
            .bind(daily)
            .bind(rpm)
            .bind(Utc::now().to_rfc3339())
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn seed_plan_allowlists(
        &self,
        plans: &[PlanCatalogEntry],
    ) -> Result<(), sqlx::Error> {
        let plan_ids: Vec<&str> = plans.iter().map(|p| p.id.as_str()).collect();
        if plan_ids.is_empty() {
            return Ok(());
        }
        sqlx::query("DELETE FROM model_allowlist WHERE plan_id = ANY($1::text[])")
            .bind(&plan_ids)
            .execute(&self.pool)
            .await?;
        let (plan_col, model_col): (Vec<&str>, Vec<&str>) = plans
            .iter()
            .flat_map(|p| p.models.iter().map(move |m| (p.id.as_str(), m.as_str())))
            .unzip();
        if !plan_col.is_empty() {
            sqlx::query(
                "INSERT INTO model_allowlist (plan_id, model_id)
                 SELECT * FROM UNNEST($1::text[], $2::text[])
                 ON CONFLICT (plan_id, model_id) DO NOTHING",
            )
            .bind(&plan_col)
            .bind(&model_col)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    pub async fn list_plans(&self) -> Result<Vec<PlanView>, sqlx::Error> {
        let mut allowlists: HashMap<String, Vec<String>> = HashMap::new();
        for row in sqlx::query("SELECT plan_id, model_id FROM model_allowlist")
            .fetch_all(&self.pool)
            .await?
        {
            allowlists
                .entry(row.get("plan_id"))
                .or_default()
                .push(row.get("model_id"));
        }
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
            let entry: Option<PlanCatalogEntry> = serde_json::from_str(&payload).ok();
            out.push(PlanView {
                tagline: entry
                    .as_ref()
                    .map(|e| e.tagline.clone())
                    .unwrap_or_default(),
                limits: entry.as_ref().and_then(|e| e.limits.clone()),
                model_names: entry
                    .as_ref()
                    .map(|e| e.model_names.clone())
                    .unwrap_or_default(),
                model_count: entry
                    .as_ref()
                    .map(|e| e.model_count.clone())
                    .unwrap_or_default(),
                models: allowlists.remove(&id).unwrap_or_default(),
                id,
                name,
                price,
                searches_per_month,
                voice_minutes,
            });
        }
        out.sort_by(|a, b| {
            let pa = a.price.parse::<f64>().unwrap_or(f64::MAX);
            let pb = b.price.parse::<f64>().unwrap_or(f64::MAX);
            pa.partial_cmp(&pb).unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    pub async fn quota_of(&self, plan_id: &str) -> Result<PlanQuota, sqlx::Error> {
        let row = sqlx::query(
            "SELECT monthly_tokens, weekly_tokens, daily_requests, rate_per_min
             FROM plans WHERE id = $1",
        )
        .bind(plan_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some(row) => PlanQuota {
                monthly_tokens: row.try_get("monthly_tokens").unwrap_or(0),
                weekly_tokens: row.try_get("weekly_tokens").unwrap_or(0),
                daily_requests: row.try_get("daily_requests").unwrap_or(0),
                rate_per_min: row.try_get("rate_per_min").unwrap_or(0),
            },
            None => PlanQuota::default(),
        })
    }

    pub async fn plan_of(&self, user_id: &str) -> Result<String, sqlx::Error> {
        let row = sqlx::query("SELECT plan_id FROM user_entitlements WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;
        match row {
            Some(row) => {
                let stored: String = row.get("plan_id");
                let effective = self
                    .entitled_subscription_plan(user_id)
                    .await?
                    .unwrap_or_else(|| FREE_PLAN_ID.to_owned());
                if stored != effective {
                    self.set_user_plan(user_id, &effective).await?;
                }
                Ok(effective)
            }
            None => {
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
}

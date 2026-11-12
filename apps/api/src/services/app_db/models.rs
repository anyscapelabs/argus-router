use sqlx::Row;

use super::AppDb;

impl AppDb {
    pub async fn allowlist_of(&self, plan_id: &str) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query("SELECT model_id FROM model_allowlist WHERE plan_id = $1")
            .bind(plan_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.iter().map(|row| row.get("model_id")).collect())
    }

    pub async fn effective_allowlist_of(
        &self,
        user_id: &str,
        plan_id: &str,
    ) -> Result<Vec<String>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT a.model_id
             FROM model_allowlist a
             LEFT JOIN model_catalog c ON c.id = a.model_id
             WHERE a.plan_id = $1
               AND NOT EXISTS (
                   SELECT 1 FROM model_preferences p
                   WHERE p.user_id = $2
                     AND p.model_id = a.model_id
                     AND p.enabled = false
               )
               AND NOT EXISTS (
                   SELECT 1 FROM user_settings s
                   WHERE s.user_id = $2
                     AND s.allow_free_training = false
                     AND COALESCE((c.payload->>'free')::boolean, false)
               )",
        )
        .bind(plan_id)
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.iter().map(|row| row.get("model_id")).collect())
    }

    pub async fn set_model_preference(
        &self,
        user_id: &str,
        model_id: &str,
        enabled: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO model_preferences (user_id, model_id, enabled)
             VALUES ($1, $2, $3)
             ON CONFLICT (user_id, model_id) DO UPDATE SET enabled = EXCLUDED.enabled",
        )
        .bind(user_id)
        .bind(model_id)
        .bind(enabled)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

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
}

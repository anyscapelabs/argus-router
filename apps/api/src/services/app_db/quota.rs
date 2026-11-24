use chrono::Utc;
use sqlx::Row;

use super::period::load_usage_period;
use super::{AppDb, QuotaCheck};

impl AppDb {
    pub async fn search_usage(&self, user_id: &str, plan_id: &str) -> Result<(i64, i64), sqlx::Error> {
        let quota_row = sqlx::query("SELECT searches_per_month FROM plans WHERE id = $1")
            .bind(plan_id)
            .fetch_optional(&self.pool)
            .await?;
        let quota = quota_row
            .and_then(|row| row.try_get::<i64, _>("searches_per_month").ok())
            .unwrap_or(0);
        let period = self.usage_period(user_id).await?;
        let used = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(used), 0)::bigint FROM search_monthly_usage WHERE user_id = $1 AND month_start = $2",
        )
        .bind(user_id)
        .bind(period.monthly_start.date_naive())
        .fetch_one(&self.pool)
        .await?;
        Ok((used, quota))
    }

    pub async fn voice_usage(&self, user_id: &str, plan_id: &str) -> Result<(i64, i64), sqlx::Error> {
        let quota_row = sqlx::query("SELECT voice_minutes FROM plans WHERE id = $1")
            .bind(plan_id)
            .fetch_optional(&self.pool)
            .await?;
        let quota = quota_row
            .and_then(|row| row.try_get::<i64, _>("voice_minutes").ok())
            .unwrap_or(0);
        let period = self.usage_period(user_id).await?;
        let used_row = sqlx::query(
            "SELECT
               (SELECT COALESCE(SUM(CEIL(latency_ms::numeric / 60000.0)), 0)::bigint
                FROM usage_logs
                WHERE user_id = $1
                  AND status IN (200, 429)
                  AND latency_ms > 0
                  AND provider = 'gemini-live'
                  AND created_at >= $2)
             +
               (SELECT COUNT(*)::bigint FROM voice_reservations WHERE user_id = $1)
             AS used_minutes",
        )
        .bind(user_id)
        .bind(period.monthly_start)
        .fetch_one(&self.pool)
        .await?;
        Ok((used_row.try_get::<i64, _>("used_minutes").unwrap_or(0), quota))
    }

    pub async fn reserve_search_quota(
        &self,
        user_id: &str,
        plan_id: &str,
    ) -> Result<QuotaCheck, sqlx::Error> {
        let quota_row = sqlx::query("SELECT searches_per_month FROM plans WHERE id = $1")
            .bind(plan_id)
            .fetch_optional(&self.pool)
            .await?;
        let quota = quota_row
            .and_then(|r| r.try_get::<i64, _>("searches_per_month").ok())
            .unwrap_or(0);
        if quota <= 0 {
            return Ok(QuotaCheck::NotAvailable);
        }

        let period = self.usage_period(user_id).await?;
        let used = sqlx::query_scalar::<_, i64>(
            "INSERT INTO search_monthly_usage (user_id, month_start, used)
             VALUES ($1, $2, 1)
             ON CONFLICT (user_id, month_start) DO UPDATE
             SET used = search_monthly_usage.used + 1
             WHERE search_monthly_usage.used < $3
             RETURNING used",
        )
        .bind(user_id)
        .bind(period.monthly_start.date_naive())
        .bind(quota)
        .fetch_optional(&self.pool)
        .await?;
        match used {
            Some(used) => Ok(QuotaCheck::Allowed { used, quota }),
            None => Ok(QuotaCheck::Exceeded { used: quota, quota }),
        }
    }

    pub async fn release_search_quota(&self, user_id: &str) -> Result<(), sqlx::Error> {
        let period = self.usage_period(user_id).await?;
        sqlx::query(
            "UPDATE search_monthly_usage
             SET used = GREATEST(used - 1, 0)
             WHERE user_id = $1 AND month_start = $2",
        )
        .bind(user_id)
        .bind(period.monthly_start.date_naive())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn tokens_used(&self, user_id: &str) -> Result<(i64, i64), sqlx::Error> {
        let period = self.usage_period(user_id).await?;
        let row = sqlx::query(
            "SELECT
               COALESCE(SUM(input_tokens + output_tokens)
                   FILTER (WHERE created_at >= $2), 0)::bigint AS monthly,
               COALESCE(SUM(input_tokens + output_tokens)
                   FILTER (WHERE created_at >= $3), 0)::bigint AS weekly
             FROM usage_logs
             WHERE user_id = $1",
        )
        .bind(user_id)
        .bind(period.monthly_start)
        .bind(period.weekly_start)
        .fetch_one(&self.pool)
        .await?;
        Ok((
            row.try_get("monthly").unwrap_or(0),
            row.try_get("weekly").unwrap_or(0),
        ))
    }

    pub async fn reserve_voice_session(
        &self,
        user_id: &str,
        plan_id: &str,
        session_id: &str,
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

        sqlx::query(
            "DELETE FROM voice_reservations
             WHERE created_at < NOW() - INTERVAL '35 minutes'",
        )
        .execute(tx.as_mut())
        .await?;

        let period = load_usage_period(tx.as_mut(), user_id, Utc::now()).await?;
        let used_row = sqlx::query(
            "SELECT
               (SELECT COALESCE(SUM(CEIL(latency_ms::numeric / 60000.0)), 0)::bigint as used_minutes
                FROM usage_logs
                WHERE user_id = $1
                  AND status IN (200, 429)
                  AND latency_ms > 0
                  AND provider = 'gemini-live'
                  AND created_at >= $2)
             +
               (SELECT COUNT(*)::bigint FROM voice_reservations WHERE user_id = $1)
               AS used_minutes",
        )
        .bind(user_id)
        .bind(period.monthly_start)
        .fetch_one(tx.as_mut())
        .await?;
        let used = used_row.try_get::<i64, _>("used_minutes").unwrap_or(0);

        if used >= quota {
            tx.commit().await?;
            return Ok(QuotaCheck::Exceeded { used, quota });
        }

        sqlx::query(
            "INSERT INTO voice_reservations (session_id, user_id, created_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (session_id) DO NOTHING",
        )
        .bind(session_id)
        .bind(user_id)
        .execute(tx.as_mut())
        .await?;

        tx.commit().await?;
        Ok(QuotaCheck::Allowed { used, quota })
    }

    pub async fn release_voice_reservation(&self, session_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM voice_reservations WHERE session_id = $1")
            .bind(session_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

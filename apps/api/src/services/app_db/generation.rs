use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::Utc;
use sqlx::Row;

use super::period::load_usage_period;
use super::{AppDb, GenerationReservation, GenerationReservationOutcome};
use crate::services::plans::PlanCatalogEntry;

impl AppDb {
    pub async fn reserve_generation(
        &self,
        user_id: &str,
        plan_id: &str,
        tier: Option<&str>,
        desired_output_tokens: u64,
        prompt_token_bound: i64,
    ) -> Result<GenerationReservationOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("generation:{user_id}"))
            .execute(tx.as_mut())
            .await?;

        sqlx::query(
            "DELETE FROM generation_reservations
             WHERE user_id = $1 AND expires_at <= NOW() AND dispatched_at IS NULL",
        )
        .bind(user_id)
        .execute(tx.as_mut())
        .await?;

        let quota = sqlx::query(
            "SELECT monthly_tokens, weekly_tokens, payload::text AS payload
             FROM plans WHERE id = $1",
        )
        .bind(plan_id)
        .fetch_optional(tx.as_mut())
        .await?;
        let monthly_limit = quota
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("monthly_tokens").ok())
            .unwrap_or(0);
        let weekly_limit = quota
            .as_ref()
            .and_then(|row| row.try_get::<i64, _>("weekly_tokens").ok())
            .unwrap_or(0);
        let tier_limits: Option<HashMap<String, (i64, i64)>> = quota
            .as_ref()
            .and_then(|row| row.try_get::<String, _>("payload").ok())
            .and_then(|payload| serde_json::from_str::<PlanCatalogEntry>(&payload).ok())
            .and_then(|entry| entry.tier_limits)
            .map(|limits| {
                limits
                    .into_iter()
                    .map(|(name, tier)| {
                        (
                            name,
                            (tier.monthly_tokens_count(), tier.weekly_tokens_count()),
                        )
                    })
                    .collect()
            });
        let period = load_usage_period(tx.as_mut(), user_id, Utc::now()).await?;
        let used = sqlx::query(
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
        .fetch_one(tx.as_mut())
        .await?;
        let reserved = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(SUM(reserved_tokens), 0)::bigint
             FROM generation_reservations
             WHERE user_id = $1 AND expires_at > NOW()",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await?;

        let prompt_tokens = prompt_token_bound.max(1);
        let desired_output = desired_output_tokens.clamp(1, i64::MAX as u64) as i64;
        let monthly_used = used.try_get::<i64, _>("monthly").unwrap_or(0) + reserved;
        let weekly_used = used.try_get::<i64, _>("weekly").unwrap_or(0) + reserved;
        let (tier_monthly_room, tier_weekly_room) = match (&tier_limits, tier) {
            (Some(limits), Some(tier_name)) if limits.contains_key(tier_name) => {
                let tier_row = sqlx::query(
                    "SELECT
                       COALESCE(SUM(u.input_tokens + u.output_tokens)
                         FILTER (WHERE u.created_at >= $3), 0)::bigint
                           AS monthly,
                       COALESCE(SUM(u.input_tokens + u.output_tokens)
                         FILTER (WHERE u.created_at >= $4), 0)::bigint
                           AS weekly
                     FROM usage_logs u
                     LEFT JOIN model_catalog m ON m.id = u.model
                     WHERE u.user_id = $1 AND (m.payload->>'tier') = $2",
                )
                .bind(user_id)
                .bind(tier_name)
                .bind(period.monthly_start)
                .bind(period.weekly_start)
                .fetch_one(tx.as_mut())
                .await?;
                let tier_reserved = sqlx::query_scalar::<_, i64>(
                    "SELECT COALESCE(SUM(reserved_tokens), 0)::bigint
                     FROM generation_reservations
                     WHERE user_id = $1 AND expires_at > NOW() AND tier = $2",
                )
                .bind(user_id)
                .bind(tier_name)
                .fetch_one(tx.as_mut())
                .await?;
                let (tier_monthly, tier_weekly) = limits
                    .get(tier_name)
                    .copied()
                    .unwrap_or((0, 0));
                (
                    Some(remaining(
                        tier_monthly,
                        tier_row.try_get::<i64, _>("monthly").unwrap_or(0) + tier_reserved,
                    )),
                    Some(remaining(
                        tier_weekly,
                        tier_row.try_get::<i64, _>("weekly").unwrap_or(0) + tier_reserved,
                    )),
                )
            }
            _ => (None, None),
        };
        let monthly_room = min_opt(remaining(monthly_limit, monthly_used), tier_monthly_room);
        if monthly_room <= prompt_tokens {
            tx.rollback().await?;
            return Ok(GenerationReservationOutcome::MonthlyExhausted);
        }
        let weekly_room = min_opt(remaining(weekly_limit, weekly_used), tier_weekly_room);
        if weekly_room <= prompt_tokens {
            tx.rollback().await?;
            return Ok(GenerationReservationOutcome::WeeklyExhausted);
        }

        let output_tokens = desired_output
            .min(monthly_room - prompt_tokens)
            .min(weekly_room - prompt_tokens);
        let id = next_reservation_id();
        sqlx::query(
            "INSERT INTO generation_reservations
               (id, user_id, reserved_tokens, tier, expires_at)
             VALUES ($1, $2, $3, $4, NOW() + INTERVAL '2 hours')",
        )
        .bind(&id)
        .bind(user_id)
        .bind(prompt_tokens.saturating_add(output_tokens))
        .bind(tier)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(GenerationReservationOutcome::Reserved(GenerationReservation {
            id,
            max_output_tokens: output_tokens as u64,
            reserved_tokens: prompt_tokens.saturating_add(output_tokens),
        }))
    }

    pub async fn mark_generation_dispatched(
        &self,
        reservation_id: &str,
    ) -> Result<(), sqlx::Error> {
        let updated = sqlx::query(
            "UPDATE generation_reservations
             SET dispatched_at = COALESCE(dispatched_at, NOW())
             WHERE id = $1",
        )
        .bind(reservation_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        if updated == 1 {
            Ok(())
        } else {
            Err(sqlx::Error::RowNotFound)
        }
    }

    pub async fn reclaim_expired_generation_reservations(&self) -> Result<u64, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let expired = sqlx::query(
            "DELETE FROM generation_reservations
             WHERE expires_at <= NOW() AND dispatched_at IS NULL",
        )
        .execute(tx.as_mut())
        .await?
        .rows_affected();
        let dispatched = sqlx::query(
            "DELETE FROM generation_reservations
             WHERE expires_at <= NOW() AND dispatched_at IS NOT NULL
             RETURNING id, user_id, reserved_tokens",
        )
        .fetch_all(tx.as_mut())
        .await?;
        for row in &dispatched {
            let id = row.try_get::<String, _>("id").unwrap_or_default();
            let user_id = row.try_get::<String, _>("user_id").unwrap_or_default();
            let reserved_tokens = row.try_get::<i64, _>("reserved_tokens").unwrap_or(0);
            sqlx::query(
                "INSERT INTO usage_logs
                   (event_id, user_id, model, input_tokens, output_tokens, cost_usd, status, error_code)
                 VALUES ($1, $2, 'reservation-expired', $3, 0, 0, 500, 'settlement_expired')
                 ON CONFLICT (event_id) DO NOTHING",
            )
            .bind(&id)
            .bind(&user_id)
            .bind(reserved_tokens)
            .execute(tx.as_mut())
            .await?;
        }
        tx.commit().await?;
        Ok((expired + dispatched.len() as u64) as u64)
    }
}

fn remaining(limit: i64, used: i64) -> i64 {
    if limit <= 0 {
        i64::MAX
    } else {
        limit.saturating_sub(used)
    }
}

fn min_opt(value: i64, cap: Option<i64>) -> i64 {
    match cap {
        Some(cap) => value.min(cap),
        None => value,
    }
}

fn next_reservation_id() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("generation-{:x}-{nanos:x}-{sequence:x}", std::process::id())
}

#[cfg(test)]
mod tests {
    #[test]
    fn remaining_treats_unlimited_as_i64_max() {
        assert_eq!(super::remaining(0, 123), i64::MAX);
        assert_eq!(super::remaining(100, 40), 60);
    }
}

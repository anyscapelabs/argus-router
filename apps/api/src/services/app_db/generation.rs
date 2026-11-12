use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::Row;

use super::{AppDb, GenerationReservation, GenerationReservationOutcome};

impl AppDb {
    pub async fn reserve_generation(
        &self,
        user_id: &str,
        plan_id: &str,
        force_credits: bool,
        desired_output_tokens: u64,
        prompt_token_bound: i64,
        input_price_per_million: f64,
        output_price_per_million: f64,
    ) -> Result<GenerationReservationOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("generation:{user_id}"))
            .execute(tx.as_mut())
            .await?;

        let reclaimed = sqlx::query_scalar::<_, i64>(
            "WITH expired AS (
               DELETE FROM generation_reservations
               WHERE user_id = $1 AND expires_at <= NOW() AND dispatched_at IS NULL
               RETURNING held_micros
             )
             SELECT COALESCE(SUM(held_micros), 0)::bigint FROM expired",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await?;
        if reclaimed > 0 {
            sqlx::query(
                "INSERT INTO credit_wallet (user_id, balance_micros, updated_at)
                 VALUES ($1, $2, NOW())
                 ON CONFLICT (user_id) DO UPDATE
                   SET balance_micros = credit_wallet.balance_micros + $2,
                       updated_at = NOW()",
            )
            .bind(user_id)
            .bind(reclaimed)
            .execute(tx.as_mut())
            .await?;
        }

        let quota = sqlx::query("SELECT monthly_tokens, weekly_tokens FROM plans WHERE id = $1")
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
        let used = sqlx::query(
            "SELECT
               COALESCE(SUM(input_tokens + output_tokens)
                 FILTER (WHERE created_at >= date_trunc('month', CURRENT_TIMESTAMP)), 0)::bigint AS monthly,
               COALESCE(SUM(input_tokens + output_tokens)
                 FILTER (WHERE created_at >= date_trunc('week', CURRENT_TIMESTAMP)), 0)::bigint AS weekly
             FROM usage_logs
             WHERE user_id = $1",
        )
        .bind(user_id)
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
        let monthly_room = remaining(monthly_limit, monthly_used);
        if monthly_room <= prompt_tokens {
            tx.rollback().await?;
            return Ok(GenerationReservationOutcome::MonthlyExhausted);
        }
        let monthly_output = monthly_room - prompt_tokens;
        let weekly_room = remaining(weekly_limit, weekly_used);
        let weekly_output = weekly_room.saturating_sub(prompt_tokens);
        let mut credits_paid = force_credits || weekly_output <= 0;
        let mut output_tokens = desired_output.min(monthly_output);
        if !credits_paid {
            output_tokens = output_tokens.min(weekly_output);
        }
        if output_tokens <= 0 {
            credits_paid = true;
            output_tokens = desired_output.min(monthly_output);
        }

        let input_price = valid_price(input_price_per_million);
        let output_price = valid_price(output_price_per_million);
        let mut held_micros = 0;
        if credits_paid {
            let balance = sqlx::query_scalar::<_, i64>(
                "SELECT balance_micros FROM credit_wallet WHERE user_id = $1 FOR UPDATE",
            )
            .bind(user_id)
            .fetch_optional(tx.as_mut())
            .await?
            .unwrap_or(0);
            output_tokens = affordable_output_tokens(
                balance,
                prompt_tokens,
                output_tokens,
                input_price,
                output_price,
            );
            if output_tokens <= 0 {
                tx.rollback().await?;
                return Ok(GenerationReservationOutcome::CreditsExhausted);
            }
            held_micros =
                reservation_cost(prompt_tokens, output_tokens, input_price, output_price).max(1);
            let updated = sqlx::query(
                "UPDATE credit_wallet
                 SET balance_micros = balance_micros - $2, updated_at = NOW()
                 WHERE user_id = $1 AND balance_micros >= $2",
            )
            .bind(user_id)
            .bind(held_micros)
            .execute(tx.as_mut())
            .await?
            .rows_affected();
            if updated != 1 {
                tx.rollback().await?;
                return Ok(GenerationReservationOutcome::CreditsExhausted);
            }
        }

        let id = next_reservation_id();
        sqlx::query(
            "INSERT INTO generation_reservations
               (id, user_id, reserved_tokens, held_micros, credits_paid, expires_at)
             VALUES ($1, $2, $3, $4, $5, NOW() + INTERVAL '2 hours')",
        )
        .bind(&id)
        .bind(user_id)
        .bind(prompt_tokens.saturating_add(output_tokens))
        .bind(held_micros)
        .bind(credits_paid)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(GenerationReservationOutcome::Reserved(
            GenerationReservation {
                id,
                max_output_tokens: output_tokens as u64,
                reserved_tokens: prompt_tokens.saturating_add(output_tokens),
                held_micros,
            },
        ))
    }

    pub async fn reserve_credit_operation(
        &self,
        user_id: &str,
        amount_micros: i64,
    ) -> Result<GenerationReservationOutcome, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("generation:{user_id}"))
            .execute(tx.as_mut())
            .await?;
        let reclaimed = sqlx::query_scalar::<_, i64>(
            "WITH expired AS (
               DELETE FROM generation_reservations
               WHERE user_id = $1 AND expires_at <= NOW() AND dispatched_at IS NULL
               RETURNING held_micros
             )
             SELECT COALESCE(SUM(held_micros), 0)::bigint FROM expired",
        )
        .bind(user_id)
        .fetch_one(tx.as_mut())
        .await?;
        if reclaimed > 0 {
            sqlx::query(
                "UPDATE credit_wallet
                 SET balance_micros = balance_micros + $2, updated_at = NOW()
                 WHERE user_id = $1",
            )
            .bind(user_id)
            .bind(reclaimed)
            .execute(tx.as_mut())
            .await?;
        }
        let held_micros = amount_micros.max(1);
        let updated = sqlx::query(
            "UPDATE credit_wallet
             SET balance_micros = balance_micros - $2, updated_at = NOW()
             WHERE user_id = $1 AND balance_micros >= $2",
        )
        .bind(user_id)
        .bind(held_micros)
        .execute(tx.as_mut())
        .await?
        .rows_affected();
        if updated != 1 {
            tx.rollback().await?;
            return Ok(GenerationReservationOutcome::CreditsExhausted);
        }
        let id = next_reservation_id();
        sqlx::query(
            "INSERT INTO generation_reservations
               (id, user_id, reserved_tokens, held_micros, credits_paid, expires_at)
             VALUES ($1, $2, 0, $3, TRUE, NOW() + INTERVAL '2 hours')",
        )
        .bind(&id)
        .bind(user_id)
        .bind(held_micros)
        .execute(tx.as_mut())
        .await?;
        tx.commit().await?;
        Ok(GenerationReservationOutcome::Reserved(
            GenerationReservation {
                id,
                max_output_tokens: 0,
                reserved_tokens: 0,
                held_micros,
            },
        ))
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
        let refundable = sqlx::query(
            "DELETE FROM generation_reservations
             WHERE expires_at <= NOW() AND dispatched_at IS NULL
             RETURNING user_id, held_micros",
        )
        .fetch_all(tx.as_mut())
        .await?;
        for row in &refundable {
            let held = row.try_get::<i64, _>("held_micros").unwrap_or(0);
            if held <= 0 {
                continue;
            }
            let user_id = row.try_get::<String, _>("user_id").unwrap_or_default();
            sqlx::query(
                "INSERT INTO credit_wallet (user_id, balance_micros, updated_at)
                 VALUES ($1, $2, NOW())
                 ON CONFLICT (user_id) DO UPDATE
                   SET balance_micros = credit_wallet.balance_micros + $2,
                       updated_at = NOW()",
            )
            .bind(user_id)
            .bind(held)
            .execute(tx.as_mut())
            .await?;
        }
        let dispatched = sqlx::query(
            "DELETE FROM generation_reservations
             WHERE expires_at <= NOW() AND dispatched_at IS NOT NULL
             RETURNING id, user_id, reserved_tokens, held_micros",
        )
        .fetch_all(tx.as_mut())
        .await?;
        for row in &dispatched {
            let id = row.try_get::<String, _>("id").unwrap_or_default();
            let user_id = row.try_get::<String, _>("user_id").unwrap_or_default();
            let reserved_tokens = row.try_get::<i64, _>("reserved_tokens").unwrap_or(0);
            let held_micros = row.try_get::<i64, _>("held_micros").unwrap_or(0);
            sqlx::query(
                "INSERT INTO usage_logs
                   (event_id, user_id, model, input_tokens, output_tokens, cost_usd, status, error_code)
                 VALUES ($1, $2, 'reservation-expired', $3, 0, $4, 500, 'settlement_expired')
                 ON CONFLICT (event_id) DO NOTHING",
            )
            .bind(&id)
            .bind(&user_id)
            .bind(reserved_tokens)
            .bind(held_micros as f64 / 1_000_000.0)
            .execute(tx.as_mut())
            .await?;
            if held_micros > 0 {
                sqlx::query(
                    "INSERT INTO credit_transactions (user_id, delta_micros, reason, reference)
                     VALUES ($1, $2, 'usage', $3)
                     ON CONFLICT (reason, reference) DO NOTHING",
                )
                .bind(&user_id)
                .bind(-held_micros)
                .bind(&id)
                .execute(tx.as_mut())
                .await?;
            }
        }
        tx.commit().await?;
        Ok((refundable.len() + dispatched.len()) as u64)
    }
}

fn remaining(limit: i64, used: i64) -> i64 {
    if limit <= 0 {
        i64::MAX
    } else {
        limit.saturating_sub(used)
    }
}

fn valid_price(price: f64) -> f64 {
    if price.is_finite() && price > 0.0 {
        price
    } else {
        0.0
    }
}

fn reservation_cost(
    input_tokens: i64,
    output_tokens: i64,
    input_price: f64,
    output_price: f64,
) -> i64 {
    (input_tokens as f64 * input_price + output_tokens as f64 * output_price)
        .ceil()
        .clamp(0.0, i64::MAX as f64) as i64
}

fn affordable_output_tokens(
    balance: i64,
    input_tokens: i64,
    output_limit: i64,
    input_price: f64,
    output_price: f64,
) -> i64 {
    if balance <= 0 || output_limit <= 0 {
        return 0;
    }
    let mut low = 0;
    let mut high = output_limit;
    while low < high {
        let middle = low + (high - low + 1) / 2;
        if reservation_cost(input_tokens, middle, input_price, output_price).max(1) <= balance {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    low
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
    use super::{affordable_output_tokens, reservation_cost};

    #[test]
    fn caps_output_to_credit_balance() {
        assert_eq!(affordable_output_tokens(100, 100, 1000, 0.2, 0.8), 100);
        assert_eq!(reservation_cost(100, 100, 0.2, 0.8), 100);
    }

    #[test]
    fn requires_positive_balance_for_zero_cost_models() {
        assert_eq!(affordable_output_tokens(0, 100, 1000, 0.0, 0.0), 0);
        assert_eq!(affordable_output_tokens(1, 100, 1000, 0.0, 0.0), 1000);
    }
}

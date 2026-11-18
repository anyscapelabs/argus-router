use serde::Serialize;
use sqlx::Row;

use super::{AppDb, CheckoutClaim};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionView {
    pub plan_id: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_billing_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub past_due_ends_at: Option<String>,
    pub cancel_at_next_billing_date: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cancelled_at: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SubscriptionRow {
    pub subscription_id: String,
    pub user_id: String,
    pub plan_id: String,
    pub status: String,
}

const AUTO_RECHARGE_MAX_FAILURES: i32 = 3;
const AUTO_RECHARGE_COOLDOWN_MINUTES: i64 = 10;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoRechargeView {
    pub enabled: bool,
    pub amount_cents: i64,
    pub threshold_micros: i64,
    pub disabled_by_failures: bool,
}

impl AppDb {
    pub async fn claim_billing_checkout(
        &self,
        checkout_key: &str,
        user_id: &str,
    ) -> Result<CheckoutClaim, sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("SELECT pg_advisory_xact_lock(hashtext($1))")
            .bind(format!("billing-checkout:{checkout_key}"))
            .execute(tx.as_mut())
            .await?;
        sqlx::query(
            "DELETE FROM billing_checkout_sessions
             WHERE checkout_key = $1 AND expires_at <= NOW()",
        )
        .bind(checkout_key)
        .execute(tx.as_mut())
        .await?;
        let existing = sqlx::query_scalar::<_, Option<String>>(
            "SELECT checkout_url FROM billing_checkout_sessions WHERE checkout_key = $1",
        )
        .bind(checkout_key)
        .fetch_optional(tx.as_mut())
        .await?;
        let claim = match existing {
            Some(Some(url)) => CheckoutClaim::Existing(url),
            Some(None) => CheckoutClaim::InProgress,
            None => {
                sqlx::query(
                    "INSERT INTO billing_checkout_sessions
                       (checkout_key, user_id, expires_at)
                     VALUES ($1, $2, NOW() + INTERVAL '24 hours')",
                )
                .bind(checkout_key)
                .bind(user_id)
                .execute(tx.as_mut())
                .await?;
                CheckoutClaim::Claimed
            }
        };
        tx.commit().await?;
        Ok(claim)
    }

    pub async fn finish_billing_checkout(
        &self,
        checkout_key: &str,
        checkout_url: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE billing_checkout_sessions
             SET checkout_url = $2
             WHERE checkout_key = $1",
        )
        .bind(checkout_key)
        .bind(checkout_url)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_billing_checkout(&self, checkout_key: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM billing_checkout_sessions WHERE checkout_key = $1")
            .bind(checkout_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn clear_user_plan_checkouts(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM billing_checkout_sessions
             WHERE user_id = $1 AND checkout_key LIKE 'plan:%'",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clear_user_credit_checkouts(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "DELETE FROM billing_checkout_sessions
             WHERE user_id = $1 AND checkout_key LIKE 'credits:%'",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn billing_customer_id(&self, user_id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT customer_id FROM billing_customers WHERE user_id = $1")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn clear_billing_customer(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM billing_customers WHERE user_id = $1")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn billing_user_of_customer(
        &self,
        customer_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT user_id FROM billing_customers WHERE customer_id = $1")
            .bind(customer_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn upsert_billing_customer(
        &self,
        user_id: &str,
        customer_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO billing_customers (user_id, customer_id, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (user_id) DO UPDATE
               SET customer_id = EXCLUDED.customer_id, updated_at = NOW()",
        )
        .bind(user_id)
        .bind(customer_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn upsert_subscription(
        &self,
        subscription_id: &str,
        user_id: &str,
        plan_id: &str,
        status: &str,
        next_billing_date: Option<&str>,
        past_due_ends_at: Option<&str>,
        cancel_at_next_billing_date: bool,
        cancelled_at: Option<&str>,
        event_at: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO billing_subscriptions
                 (subscription_id, user_id, plan_id, status, next_billing_date, past_due_ends_at,
                  cancel_at_next_billing_date, cancelled_at, event_at, updated_at)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, COALESCE($9::text::timestamptz, NOW()), NOW())
             ON CONFLICT (subscription_id) DO UPDATE
               SET plan_id = EXCLUDED.plan_id,
                   status = EXCLUDED.status,
                   next_billing_date = COALESCE(EXCLUDED.next_billing_date, billing_subscriptions.next_billing_date),
                   past_due_ends_at = CASE
                     WHEN EXCLUDED.status = 'past_due' THEN COALESCE(EXCLUDED.past_due_ends_at, billing_subscriptions.past_due_ends_at)
                     ELSE NULL
                   END,
                   cancel_at_next_billing_date = EXCLUDED.cancel_at_next_billing_date,
                   cancelled_at = COALESCE(EXCLUDED.cancelled_at, billing_subscriptions.cancelled_at),
                   event_at = EXCLUDED.event_at,
                   updated_at = NOW()
             WHERE billing_subscriptions.event_at <= EXCLUDED.event_at",
        )
        .bind(subscription_id)
        .bind(user_id)
        .bind(plan_id)
        .bind(status)
        .bind(next_billing_date)
        .bind(past_due_ends_at)
        .bind(cancel_at_next_billing_date)
        .bind(cancelled_at)
        .bind(event_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn subscription_of(
        &self,
        user_id: &str,
    ) -> Result<Option<SubscriptionView>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT plan_id, status, next_billing_date, past_due_ends_at,
                    cancel_at_next_billing_date, cancelled_at
             FROM billing_subscriptions
             WHERE user_id = $1
             ORDER BY CASE
                        WHEN status IN ('active', 'past_due')
                          OR (status = 'cancelled' AND cancel_at_next_billing_date)
                        THEN 0 ELSE 1 END,
                      updated_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| SubscriptionView {
            plan_id: row.get("plan_id"),
            status: row.get("status"),
            next_billing_date: row.get("next_billing_date"),
            past_due_ends_at: row.get("past_due_ends_at"),
            cancel_at_next_billing_date: row.get("cancel_at_next_billing_date"),
            cancelled_at: row.get("cancelled_at"),
        }))
    }

    pub async fn subscription_owner(
        &self,
        subscription_id: &str,
    ) -> Result<Option<(String, String)>, sqlx::Error> {
        let row = sqlx::query(
            "SELECT user_id, plan_id FROM billing_subscriptions WHERE subscription_id = $1",
        )
        .bind(subscription_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|row| (row.get("user_id"), row.get("plan_id"))))
    }

    pub async fn subscription_id_of(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT subscription_id FROM billing_subscriptions
             WHERE user_id = $1 AND status IN ('active', 'pending', 'on_hold')
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn entitled_subscription_plan(
        &self,
        user_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT plan_id
             FROM billing_subscriptions
             WHERE user_id = $1
               AND (
                 status = 'active'
                 OR (
                   status = 'past_due'
                   AND past_due_ends_at IS NOT NULL
                   AND past_due_ends_at::timestamptz > NOW()
                 )
                 OR (
                   status = 'cancelled'
                   AND cancel_at_next_billing_date
                   AND next_billing_date IS NOT NULL
                   AND next_billing_date::timestamptz > NOW()
                 )
               )
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn active_subscriptions(&self) -> Result<Vec<SubscriptionRow>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT subscription_id, user_id, plan_id, status
             FROM billing_subscriptions
             WHERE status IN ('pending', 'active', 'past_due', 'on_hold', 'paused')
                OR (status = 'cancelled' AND cancel_at_next_billing_date)",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| SubscriptionRow {
                subscription_id: row.get("subscription_id"),
                user_id: row.get("user_id"),
                plan_id: row.get("plan_id"),
                status: row.get("status"),
            })
            .collect())
    }

    pub async fn duplicate_managed_subscription(
        &self,
        user_id: &str,
        exclude_subscription_id: &str,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT subscription_id
             FROM billing_subscriptions
             WHERE user_id = $1
               AND subscription_id <> $2
               AND status IN ('pending', 'active', 'past_due', 'on_hold', 'paused')
             ORDER BY updated_at DESC
             LIMIT 1",
        )
        .bind(user_id)
        .bind(exclude_subscription_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn set_user_plan(&self, user_id: &str, plan_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_entitlements (user_id, plan_id, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (user_id) DO UPDATE
               SET plan_id = EXCLUDED.plan_id, updated_at = NOW()",
        )
        .bind(user_id)
        .bind(plan_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn credit_balance(&self, user_id: &str) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT COALESCE(balance_micros, 0) FROM credit_wallet WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(|found: Option<i64>| found.unwrap_or(0))
    }

    pub async fn auto_recharge_of(&self, user_id: &str) -> Result<Option<AutoRechargeView>, sqlx::Error> {
        sqlx::query(
            "SELECT enabled, amount_cents, threshold_micros, consecutive_failures
             FROM auto_recharge WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await
        .map(|row| {
            row.map(|row| AutoRechargeView {
                enabled: row.try_get("enabled").unwrap_or(false),
                amount_cents: row.try_get("amount_cents").unwrap_or(0),
                threshold_micros: row.try_get("threshold_micros").unwrap_or(0),
                disabled_by_failures: row.try_get::<i32, _>("consecutive_failures")
                    .unwrap_or(0)
                    >= AUTO_RECHARGE_MAX_FAILURES,
            })
        })
    }

    pub async fn upsert_auto_recharge(
        &self,
        user_id: &str,
        enabled: bool,
        amount_cents: i64,
        threshold_micros: i64,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO auto_recharge
               (user_id, enabled, amount_cents, threshold_micros, consecutive_failures, updated_at)
             VALUES ($1, $2, $3, $4, 0, NOW())
             ON CONFLICT (user_id) DO UPDATE
               SET enabled = $2,
                   amount_cents = $3,
                   threshold_micros = $4,
                   consecutive_failures = 0,
                   updated_at = NOW()",
        )
        .bind(user_id)
        .bind(enabled)
        .bind(amount_cents)
        .bind(threshold_micros)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Claims the right to fire one auto-recharge charge, enforcing a cooldown
    /// so concurrent requests that hit the credit floor trigger a single charge.
    pub async fn claim_auto_recharge(
        &self,
        user_id: &str,
        amount_cents: &mut i64,
        threshold_micros: &mut i64,
    ) -> Result<bool, sqlx::Error> {
        let row = sqlx::query(
            "UPDATE auto_recharge
             SET last_attempt_at = NOW(), updated_at = NOW()
             WHERE user_id = $1
               AND enabled
               AND consecutive_failures < $2
               AND (last_attempt_at IS NULL
                    OR last_attempt_at < NOW() - $3 * INTERVAL '1 minute')
             RETURNING amount_cents, threshold_micros",
        )
        .bind(user_id)
        .bind(AUTO_RECHARGE_MAX_FAILURES)
        .bind(AUTO_RECHARGE_COOLDOWN_MINUTES)
        .fetch_optional(&self.pool)
        .await?;
        match row {
            Some(row) => {
                *amount_cents = row.try_get("amount_cents").unwrap_or(0);
                *threshold_micros = row.try_get("threshold_micros").unwrap_or(0);
                Ok(true)
            }
            None => Ok(false),
        }
    }

    pub async fn release_auto_recharge_claim(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE auto_recharge
             SET last_attempt_at = NULL, updated_at = NOW()
             WHERE user_id = $1 AND last_attempt_at IS NOT NULL",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_auto_recharge_success(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE auto_recharge
             SET consecutive_failures = 0, last_attempt_at = NULL, updated_at = NOW()
             WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn record_auto_recharge_failure(&self, user_id: &str) -> Result<bool, sqlx::Error> {
        let failures = sqlx::query_scalar::<_, i32>(
            "UPDATE auto_recharge
             SET consecutive_failures = consecutive_failures + 1, updated_at = NOW()
             WHERE user_id = $1
             RETURNING consecutive_failures",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        .unwrap_or(0);
        Ok(failures >= AUTO_RECHARGE_MAX_FAILURES)
    }

    pub async fn add_credits(
        &self,
        user_id: &str,
        delta_micros: i64,
        reason: &str,
        reference: &str,
    ) -> Result<(), sqlx::Error> {
        let mut tx = self.pool.begin().await?;
        let inserted = sqlx::query(
            "INSERT INTO credit_transactions (user_id, delta_micros, reason, reference)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (reason, reference) DO NOTHING",
        )
        .bind(user_id)
        .bind(delta_micros)
        .bind(reason)
        .bind(reference)
        .execute(&mut *tx)
        .await?
        .rows_affected();
        if inserted > 0 {
            sqlx::query(
                "INSERT INTO credit_wallet (user_id, balance_micros, updated_at)
                 VALUES ($1, GREATEST($2, 0), NOW())
                 ON CONFLICT (user_id) DO UPDATE
                   SET balance_micros = GREATEST(credit_wallet.balance_micros + $2, 0),
                       updated_at = NOW()",
            )
            .bind(user_id)
            .bind(delta_micros)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub async fn credit_product_of_amount(
        &self,
        amount_cents: i64,
    ) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT product_id FROM billing_credit_products WHERE amount_cents = $1")
            .bind(amount_cents)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn plan_product_of(&self, plan_id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT product_id FROM billing_plan_products WHERE plan_id = $1")
            .bind(plan_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn plan_for_product(&self, product_id: &str) -> Result<Option<String>, sqlx::Error> {
        sqlx::query_scalar("SELECT plan_id FROM billing_plan_products WHERE product_id = $1")
            .bind(product_id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn mapped_plan_products(&self) -> Result<Vec<String>, sqlx::Error> {
        sqlx::query("SELECT plan_id FROM billing_plan_products")
            .fetch_all(&self.pool)
            .await
            .map(|rows| rows.into_iter().map(|row| row.get("plan_id")).collect())
    }

    pub async fn upsert_plan_product(&self, plan_id: &str, product_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO billing_plan_products (plan_id, product_id)
             VALUES ($1, $2)
             ON CONFLICT (plan_id) DO UPDATE
               SET product_id = EXCLUDED.product_id",
        )
        .bind(plan_id)
        .bind(product_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn credit_amount_of_product(
        &self,
        product_id: &str,
    ) -> Result<Option<i64>, sqlx::Error> {
        sqlx::query_scalar(
            "SELECT amount_cents FROM billing_credit_products WHERE product_id = $1",
        )
        .bind(product_id)
        .fetch_optional(&self.pool)
        .await
    }

    pub async fn upsert_credit_product(
        &self,
        amount_cents: i64,
        product_id: &str,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO billing_credit_products (amount_cents, product_id)
             VALUES ($1, $2)
             ON CONFLICT (amount_cents) DO UPDATE
               SET product_id = EXCLUDED.product_id",
        )
        .bind(amount_cents)
        .bind(product_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_webhook_seen(&self, webhook_id: &str) -> Result<bool, sqlx::Error> {
        let inserted = sqlx::query(
            "INSERT INTO webhook_events (webhook_id) VALUES ($1)
             ON CONFLICT (webhook_id) DO NOTHING",
        )
        .bind(webhook_id)
        .execute(&self.pool)
        .await?
        .rows_affected();
        Ok(inserted > 0)
    }

    pub async fn webhook_seen(&self, webhook_id: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM webhook_events WHERE webhook_id = $1)",
        )
        .bind(webhook_id)
        .fetch_one(&self.pool)
        .await
    }
}

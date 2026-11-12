use sqlx::Row;

use super::{AppDb, UserSettings};

impl AppDb {
    pub async fn settings_of(&self, user_id: &str) -> Result<UserSettings, sqlx::Error> {
        let row = sqlx::query(
            "SELECT allow_free_training, max_key_lifetime_days
             FROM user_settings WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(match row {
            Some(row) => UserSettings {
                allow_free_training: row.get("allow_free_training"),
                max_key_lifetime_days: row.get("max_key_lifetime_days"),
            },
            None => UserSettings::default(),
        })
    }

    pub async fn set_allow_free_training(
        &self,
        user_id: &str,
        allow: bool,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_settings (user_id, allow_free_training, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (user_id) DO UPDATE
               SET allow_free_training = EXCLUDED.allow_free_training,
                   updated_at = NOW()",
        )
        .bind(user_id)
        .bind(allow)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn set_max_key_lifetime(&self, user_id: &str, days: i64) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_settings (user_id, max_key_lifetime_days, updated_at)
             VALUES ($1, $2, NOW())
             ON CONFLICT (user_id) DO UPDATE
               SET max_key_lifetime_days = EXCLUDED.max_key_lifetime_days,
                   updated_at = NOW()",
        )
        .bind(user_id)
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn clamp_key_lifetimes(&self, user_id: &str, days: i64) -> Result<(), sqlx::Error> {
        if days <= 0 {
            return Ok(());
        }
        sqlx::query(
            "UPDATE api_keys
             SET expires_at = NOW() + make_interval(days => $2),
                 updated_at = NOW()
             WHERE reference_id = $1 AND expires_at IS NULL",
        )
        .bind(user_id)
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

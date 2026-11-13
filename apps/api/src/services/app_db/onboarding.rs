use super::AppDb;

impl AppDb {
    pub async fn complete_onboarding(&self, user_id: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO user_onboarding (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

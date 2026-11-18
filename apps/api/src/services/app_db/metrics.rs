use super::AppDb;

impl AppDb {
    pub async fn count_users(&self) -> Result<i64, sqlx::Error> {
        sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(&self.pool)
            .await
    }
}

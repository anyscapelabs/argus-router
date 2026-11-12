use chrono::Utc;
use sqlx::Row;

use super::AppDb;
use crate::services::catalog::ModelCatalogEntry;

impl AppDb {
    async fn upsert_catalog(&self, entries: &[ModelCatalogEntry]) -> Result<(), sqlx::Error> {
        if entries.is_empty() {
            return Ok(());
        }
        let ids: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
        let payloads: Vec<String> = entries
            .iter()
            .map(|entry| serde_json::to_string(entry))
            .collect::<Result<_, _>>()
            .map_err(|error| sqlx::Error::ColumnDecode {
                index: "payload".into(),
                source: Box::new(error),
            })?;
        sqlx::query(
            "INSERT INTO model_catalog (id, payload, updated_at)
             SELECT u.id, u.payload::jsonb, $3
             FROM UNNEST($1::text[], $2::text[]) AS u(id, payload)
             ON CONFLICT (id) DO UPDATE SET payload = EXCLUDED.payload, updated_at = EXCLUDED.updated_at",
        )
        .bind(&ids)
        .bind(&payloads)
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn load_catalog(
        &self,
        seed: &[ModelCatalogEntry],
    ) -> Result<Vec<ModelCatalogEntry>, sqlx::Error> {
        self.upsert_catalog(seed).await?;
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

    pub async fn catalog_sync(&self, entries: &[ModelCatalogEntry]) -> Result<(), sqlx::Error> {
        self.upsert_catalog(entries).await?;
        let seen: Vec<&str> = entries.iter().map(|entry| entry.id.as_str()).collect();
        if !seen.is_empty() {
            sqlx::query("DELETE FROM model_catalog WHERE NOT (id = ANY($1))")
                .bind(&seen)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }
}

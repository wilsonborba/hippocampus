use super::store::Store;
use crate::domain::models::{
    error::Error,
    persona_schema::{PersonaSchema, SchemaDraft},
};
use sqlx::types::Json;
impl Store {
    pub async fn schema(&self, name: &str, version: Option<i64>) -> Result<PersonaSchema, Error> {
        let value:Option<serde_json::Value>=sqlx::query_scalar("SELECT row_to_json(s) FROM persona_schema s WHERE schema_name=$1 AND (($2::bigint IS NULL AND status='published') OR version=$2) ORDER BY version DESC LIMIT 1").bind(name).bind(version).fetch_optional(&self.pool).await?;
        value
            .map(|v| serde_json::from_value(v).expect("stored schema"))
            .ok_or_else(|| {
                Error::new(
                    axum::http::StatusCode::NOT_FOUND,
                    "schema_not_found",
                    "The persona schema version was not found",
                )
            })
    }
    pub async fn save_schema(&self, r: &SchemaDraft) -> Result<PersonaSchema, Error> {
        let row:Option<serde_json::Value>=sqlx::query_scalar("INSERT INTO persona_schema(schema_name,version,status,fields) VALUES ($1,$2,'draft',$3) ON CONFLICT(schema_name,version) DO UPDATE SET fields=excluded.fields WHERE persona_schema.status='draft' RETURNING row_to_json(persona_schema.*)").bind(&r.schema_name).bind(r.version).bind(Json(&r.fields)).fetch_optional(&self.pool).await?;
        row.map(|v| serde_json::from_value(v).expect("stored schema"))
            .ok_or_else(|| {
                Error::new(
                    axum::http::StatusCode::CONFLICT,
                    "schema_published",
                    "A published schema cannot change",
                )
            })
    }
    pub async fn publish_schema(&self, name: &str, version: i64) -> Result<PersonaSchema, Error> {
        sqlx::query("UPDATE persona_schema SET status='published' WHERE schema_name=$1 AND version=$2 AND status='draft'").bind(name).bind(version).execute(&self.pool).await?;
        self.schema(name, Some(version)).await
    }
}

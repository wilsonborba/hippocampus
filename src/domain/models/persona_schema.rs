use super::guardrail::SchemaField;
use serde::{Deserialize, Serialize};
#[derive(Clone, Serialize, Deserialize)]
pub struct PersonaSchema {
    pub schema_name: String,
    pub version: i64,
    pub status: String,
    pub fields: Vec<SchemaField>,
}
#[derive(Deserialize)]
pub struct SchemaDraft {
    pub schema_name: String,
    pub version: i64,
    pub fields: Vec<SchemaField>,
}

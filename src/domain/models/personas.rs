use super::scope::{ReadScope, WriteScope};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct PersonaCreate {
    #[serde(default, deserialize_with = "attributes")]
    pub attributes: Option<serde_json::Value>,
    #[serde(flatten)]
    pub scope: WriteScope,
    pub entity_type: String,
    pub canonical_name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub is_self: bool,
}
#[derive(Deserialize)]
pub struct PersonaUpdate {
    #[serde(default, deserialize_with = "attributes")]
    pub attributes: Option<serde_json::Value>,
    #[serde(flatten)]
    pub scope: WriteScope,
    pub canonical_name: Option<String>,
    pub aliases: Option<Vec<String>>,
    pub is_self: Option<bool>,
}
#[derive(Deserialize)]
pub struct PersonaMatch {
    #[serde(flatten)]
    pub scope: ReadScope,
    pub text: String,
}

fn attributes<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<serde_json::Value>, D::Error> {
    serde_json::Value::deserialize(d).map(Some)
}

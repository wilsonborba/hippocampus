use super::scope::{ReadScope, WriteScope};
use serde::Deserialize;
#[derive(Deserialize)]
pub struct PersonaCreate {
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

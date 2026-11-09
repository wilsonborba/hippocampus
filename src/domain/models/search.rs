use super::scope::ReadScope;
use serde::Deserialize;
#[derive(Debug, Deserialize)]
pub struct SearchRequest {
    #[serde(flatten)]
    pub scope: ReadScope,
    pub text: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entity_ids: Vec<String>,
    #[serde(default)]
    pub memory_types: Vec<String>,
    #[serde(default)]
    pub historical: bool,
    #[serde(default)]
    pub expand: u8,
    pub limit: Option<usize>,
}

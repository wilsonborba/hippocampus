use super::scope::WriteScope;
use serde::Deserialize;
#[derive(Deserialize)]
pub struct DeleteRequest {
    #[serde(flatten)]
    pub scope: WriteScope,
    pub memory_ids: Vec<String>,
}

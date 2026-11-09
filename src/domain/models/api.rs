use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryCreateRequest {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default = "memory_create_request_workspace_id_default")]
    pub workspace_id: String,
    #[serde(default)]
    pub memory_type: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub importance: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub entities: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub resources: Vec<BTreeMap<String, Value>>,
    #[serde(default)]
    pub context: BTreeMap<String, Value>,
    #[serde(default)]
    pub observed_at: Option<String>,
    #[serde(default)]
    pub valid_from: Option<String>,
    #[serde(default)]
    pub valid_until: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub metadata: BTreeMap<String, Value>,
    pub app_id: String,
    #[serde(default)]
    pub user_id: Option<String>,
}
fn memory_create_request_workspace_id_default() -> String {
    "default".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MemoryUpdateRequest {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub importance: Option<f64>,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub valid_from: Option<String>,
    #[serde(default)]
    pub valid_until: Option<String>,
    #[serde(default)]
    pub expires_at: Option<String>,
    #[serde(default)]
    pub metadata: Option<BTreeMap<String, Value>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TagRequest {
    pub tags: Vec<String>,
    #[serde(default = "tag_request_source_default")]
    pub source: String,
}
fn tag_request_source_default() -> String {
    "user".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EntityAssociateRequest {
    pub entity_type: String,
    pub canonical_name: String,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub confidence: Option<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceLinkRequest {
    pub resource_type: String,
    pub source_system: String,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default = "resource_link_request_relationship_default")]
    pub relationship: String,
}
fn resource_link_request_relationship_default() -> String {
    "references".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RelationshipCreateRequest {
    pub target_memory_id: String,
    pub relation_type: String,
    #[serde(default)]
    pub confidence: Option<f64>,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReinforceRequest {
    #[serde(default)]
    pub source: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SupersedeRequest {
    pub new_memory_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CorrectRequest {
    pub new_memory_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ForgetRequest {
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ResourceRegisterRequest {
    pub resource_type: String,
    pub source_system: String,
    #[serde(default)]
    pub external_id: Option<String>,
    #[serde(default)]
    pub uri: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub content_type: Option<String>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<i64>,
}

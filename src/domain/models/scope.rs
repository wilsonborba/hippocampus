use serde::{Deserialize, Deserializer, de::Error};

fn nonempty<'de, D: Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    let value = String::deserialize(deserializer)?;
    if value.trim().is_empty() {
        Err(D::Error::custom("The scope value must not be empty"))
    } else {
        Ok(value)
    }
}
#[derive(Debug, Clone, Deserialize)]
pub struct WriteScope {
    #[serde(deserialize_with = "nonempty")]
    pub workspace_id: String,
    #[serde(deserialize_with = "nonempty")]
    pub app_id: String,
    pub user_id: Option<String>,
}
/// Omission searches all users. Explicit null selects only A2A memories.
#[derive(Debug, Default, Clone, PartialEq)]
pub enum UserFilter {
    #[default]
    Any,
    AppOnly,
    User(String),
}
fn user_filter<'de, D: Deserializer<'de>>(deserializer: D) -> Result<UserFilter, D::Error> {
    Ok(match Option::<String>::deserialize(deserializer)? {
        Some(value) if !value.is_empty() => UserFilter::User(value),
        _ => UserFilter::AppOnly,
    })
}
#[derive(Debug, Clone, Deserialize)]
pub struct ReadScope {
    #[serde(deserialize_with = "nonempty")]
    pub workspace_id: String,
    pub app_id: Option<String>,
    #[serde(default, deserialize_with = "user_filter")]
    pub user_id: UserFilter,
}

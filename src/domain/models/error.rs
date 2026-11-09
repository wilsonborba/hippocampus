use axum::http::StatusCode;
#[derive(Debug)]
pub struct Error {
    pub details: serde_json::Value,
    pub status: StatusCode,
    pub name: &'static str,
    pub message: String,
}
impl Error {
    pub fn new(status: StatusCode, name: &'static str, message: impl Into<String>) -> Self {
        Self {
            details: serde_json::json!({}),
            status,
            name,
            message: message.into(),
        }
    }
    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = details;
        self
    }
    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "invalid_request", message)
    }
    pub fn not_found(kind: &'static str, id: &str) -> Self {
        let name = match kind {
            "memory" => "memory_not_found",
            "resource" => "resource_not_found",
            "entity" => "entity_not_found",
            _ => "tag_not_found",
        };
        Self::new(
            StatusCode::NOT_FOUND,
            name,
            format!("{kind} '{id}' not found"),
        )
    }
}
impl From<sqlx::Error> for Error {
    fn from(error: sqlx::Error) -> Self {
        if let Some(db) = error.as_database_error() {
            tracing::error!(sqlstate = ?db.code(), constraint = ?db.constraint(), "Database operation failed");
        }
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The database operation failed",
        )
    }
}

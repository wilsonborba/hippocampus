use crate::{
    dal::local::store::Store,
    domain::models::{
        error::Error,
        guardrail::{check_fields, validate_payload},
        persona_schema::{PersonaSchema, SchemaDraft},
    },
};
use serde_json::{Value, json};
pub async fn draft(store: &Store, r: SchemaDraft) -> Result<PersonaSchema, Error> {
    if !["person", "bot"].contains(&r.schema_name.as_str()) || r.version < 1 {
        return Err(Error::invalid(
            "Schema name must be person or bot and version must be positive",
        ));
    }
    let errors = check_fields(&r.fields);
    if !errors.is_empty() {
        return Err(Error::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_schema",
            "The schema fields are not valid",
        )
        .with_details(json!({"errors":errors})));
    }
    store.save_schema(&r).await
}
pub fn validate(schema: &PersonaSchema, attributes: &Value) -> Result<(), Error> {
    if schema.status != "published" {
        return Err(Error::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "schema_not_published",
            "The schema version is not published",
        ));
    }
    validate_payload(&schema.fields, attributes).map_err(|errors| {
        Error::new(
            axum::http::StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_attributes",
            "The attributes do not match the schema",
        )
        .with_details(json!({"errors":errors}))
    })
}

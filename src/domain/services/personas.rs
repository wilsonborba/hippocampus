use crate::{
    dal::{
        local::{personas::read_scope, store::Store},
        remote::documents::Documents,
    },
    domain::models::{
        error::Error,
        personas::{PersonaCreate, PersonaMatch, PersonaUpdate},
        scope::ReadScope,
    },
};
use serde_json::Value;
pub fn normalize(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}
fn validate_name(s: &str) -> Result<(), Error> {
    if s.trim().is_empty() || s.chars().count() > 256 {
        Err(Error::invalid(
            "Names and aliases must contain 1 to 256 characters",
        ))
    } else {
        Ok(())
    }
}
pub async fn create(
    store: &Store,
    documents: &Documents,
    r: PersonaCreate,
) -> Result<Value, Error> {
    if !["person", "bot"].contains(&r.entity_type.as_str()) {
        return Err(Error::invalid("Persona type must be person or bot"));
    }
    validate_name(&r.canonical_name)?;
    for alias in &r.aliases {
        validate_name(alias)?;
    }
    let schema = if let Some(attributes) = &r.attributes {
        let schema = store.schema(&r.entity_type, None).await?;
        super::persona_schemas::validate(&schema, attributes)?;
        Some(schema)
    } else {
        None
    };
    let mut operation = store.create_persona(&r).await?;
    if let Some(schema) = schema {
        documents.put_persona(&operation.id,serde_json::json!({"schema_name":schema.schema_name,"schema_version":schema.version,"attributes":r.attributes})).await?;
        operation.document().await?;
    }
    let id = operation.commit().await?;
    get(store, documents, &id, read_scope(&r.scope)).await
}
pub async fn update(
    store: &Store,
    documents: &Documents,
    id: &str,
    r: PersonaUpdate,
) -> Result<Value, Error> {
    if let Some(name) = &r.canonical_name {
        validate_name(name)?;
    }
    for alias in r.aliases.iter().flatten() {
        validate_name(alias)?;
    }
    let current = store
        .personas(&read_scope(&r.scope))
        .await?
        .into_iter()
        .find(|v| v["id"] == id)
        .ok_or_else(|| Error::not_found("entity", id))?;
    let previous = if current["document_store"] == "couchdb" {
        Some(documents.get_persona(id).await?.ok_or_else(|| {
            Error::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "document_store_error",
                "The persona attribute document is missing",
            )
        })?)
    } else {
        None
    };
    let schema = if let Some(attributes) = &r.attributes {
        let version = previous.as_ref().and_then(|v| v["schema_version"].as_i64());
        let schema = store
            .schema(current["entity_type"].as_str().unwrap(), version)
            .await?;
        super::persona_schemas::validate(&schema, attributes)?;
        Some(schema)
    } else {
        None
    };
    let mut operation = store.update_persona(id, &r).await?;
    if let Some(schema) = schema {
        documents.put_persona(id,serde_json::json!({"schema_name":schema.schema_name,"schema_version":schema.version,"attributes":r.attributes})).await?;
        operation.document().await?;
    }
    operation.commit().await?;
    get(store, documents, id, read_scope(&r.scope)).await
}
async fn present(documents: &Documents, mut persona: Value) -> Result<Value, Error> {
    persona["attributes"] = serde_json::json!({});
    if persona["document_store"] == "couchdb" {
        let document = documents
            .get_persona(persona["id"].as_str().unwrap())
            .await?
            .ok_or_else(|| {
                Error::new(
                    axum::http::StatusCode::BAD_GATEWAY,
                    "document_store_error",
                    "The persona attribute document is missing",
                )
            })?;
        for field in ["attributes", "schema_name", "schema_version"] {
            persona[field] = document[field].clone();
        }
    }
    Ok(Value::Object(
        [
            "id",
            "workspace_id",
            "app_id",
            "user_id",
            "entity_type",
            "canonical_name",
            "aliases",
            "is_self",
            "attributes",
            "schema_name",
            "schema_version",
            "created_at",
            "updated_at",
        ]
        .into_iter()
        .filter_map(|k| persona.get(k).map(|v| (k.to_owned(), v.clone())))
        .collect(),
    ))
}
pub async fn get(
    store: &Store,
    documents: &Documents,
    id: &str,
    mut scope: ReadScope,
) -> Result<Value, Error> {
    scope.workspace_id = super::memories::normalize(&scope.workspace_id);
    let row = store
        .personas(&scope)
        .await?
        .into_iter()
        .find(|v| v["id"] == id)
        .ok_or_else(|| Error::not_found("entity", id))?;
    present(documents, row).await
}
pub async fn list(
    store: &Store,
    documents: &Documents,
    mut scope: ReadScope,
) -> Result<Vec<Value>, Error> {
    scope.workspace_id = super::memories::normalize(&scope.workspace_id);
    let mut out = vec![];
    for row in store.personas(&scope).await? {
        out.push(present(documents, row).await?);
    }
    Ok(out)
}
fn words(s: &str) -> Vec<String> {
    normalize(s)
        .split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect()
}
pub fn named(text: &str, name: &str) -> bool {
    let input = words(text);
    let name = words(name);
    !name.is_empty() && input.windows(name.len()).any(|w| w == name)
}
pub async fn matches(
    store: &Store,
    documents: &Documents,
    mut r: PersonaMatch,
) -> Result<Vec<Value>, Error> {
    r.scope.workspace_id = super::memories::normalize(&r.scope.workspace_id);
    let mut out = vec![];
    for row in store.personas(&r.scope).await? {
        if named(&r.text, row["canonical_name"].as_str().unwrap())
            || row["aliases"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| named(&r.text, a.as_str().unwrap()))
        {
            out.push(present(documents, row).await?);
        }
    }
    Ok(out)
}

pub async fn delete(
    store: &Store,
    documents: &Documents,
    id: &str,
    scope: crate::domain::models::scope::WriteScope,
) -> Result<(), Error> {
    let operation = store.delete_persona(id, &scope).await?;
    if operation.document {
        documents.delete_persona(id).await?;
    }
    operation.commit().await
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundaries_and_case() {
        assert!(named("Ask JOÃO, then Ana Maria.", "João"));
        assert!(named("Ana   MARIA", "Ana Maria"));
        assert!(!named("Anatomy", "Ana"));
        assert!(!named("some_ana", "ana"));
        assert!(!named("anything", "!!!"));
    }
}

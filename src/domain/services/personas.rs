use crate::{
    dal::local::store::Store,
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
pub async fn create(store: &Store, r: PersonaCreate) -> Result<Value, Error> {
    if !["person", "bot"].contains(&r.entity_type.as_str()) {
        return Err(Error::invalid("Persona type must be person or bot"));
    }
    validate_name(&r.canonical_name)?;
    for alias in &r.aliases {
        validate_name(alias)?;
    }
    store.create_persona(&r).await
}
pub async fn update(store: &Store, id: &str, r: PersonaUpdate) -> Result<Value, Error> {
    if let Some(name) = &r.canonical_name {
        validate_name(name)?;
    }
    for alias in r.aliases.iter().flatten() {
        validate_name(alias)?;
    }
    store.update_persona(id, &r).await
}
pub async fn list(store: &Store, mut scope: ReadScope) -> Result<Vec<Value>, Error> {
    scope.workspace_id = super::memories::normalize(&scope.workspace_id);
    store.personas(&scope).await
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
pub async fn matches(store: &Store, r: PersonaMatch) -> Result<Vec<Value>, Error> {
    Ok(list(store, r.scope)
        .await?
        .into_iter()
        .filter(|p| {
            named(&r.text, p["canonical_name"].as_str().unwrap())
                || p["aliases"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| named(&r.text, a.as_str().unwrap()))
        })
        .collect())
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

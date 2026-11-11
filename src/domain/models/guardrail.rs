//! Schema guardrail: a received payload must match the fields of its published schema
//! version. Missing required fields, wrong types and undeclared fields are rejected before
//! anything reaches the document store.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Field types that a schema can declare.
pub const FIELD_TYPES: &[&str] = &[
    "string", "integer", "float", "boolean", "date", "datetime", "object", "array",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ValidationViolationType {
    MissingRequiredField,
    TypeMismatch { expected: String, found: String },
    UnknownField,
    SchemaNotFound,
    InvalidRoot,
    InvalidDefinition,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationError {
    pub field_path: String,
    pub message: String,
    pub error_type: ValidationViolationType,
}

impl ValidationError {
    pub fn definition(path: &str, message: impl Into<String>) -> Self {
        Self {
            field_path: path.to_string(),
            message: message.into(),
            error_type: ValidationViolationType::InvalidDefinition,
        }
    }
}

/// One declared field. `path` uses dots for nested objects (`input.tokens`) and `[]` for the
/// items of an array of objects (`steps[].name`).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchemaField {
    pub name: String,
    #[serde(rename = "type")]
    pub field_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub in_context: bool,
}

/// Checks a field list before it is stored: known types, valid paths, no duplicates, and a
/// parent of type object (or array for `[]`) for every nested path.
pub fn check_fields(fields: &[SchemaField]) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    let mut seen: BTreeMap<&str, &str> = BTreeMap::new();
    if fields.is_empty() {
        errors.push(ValidationError::definition(
            "$",
            "a schema needs at least one field",
        ));
    }
    for f in fields {
        if !FIELD_TYPES.contains(&f.field_type.as_str()) {
            errors.push(ValidationError::definition(
                &f.name,
                format!(
                    "unknown type '{}' (use {})",
                    f.field_type,
                    FIELD_TYPES.join(", ")
                ),
            ));
        }
        let valid_path = !f.name.is_empty()
            && f.name.split('.').all(|seg| {
                let name = seg.strip_suffix("[]").unwrap_or(seg);
                !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
        if !valid_path {
            errors.push(ValidationError::definition(&f.name, "invalid field path"));
        }
        if seen
            .insert(f.name.as_str(), f.field_type.as_str())
            .is_some()
        {
            errors.push(ValidationError::definition(&f.name, "duplicate field path"));
        }
    }
    for f in fields {
        if let Some((parent, _)) = f.name.rsplit_once('.') {
            let (parent_path, needs) = match parent.strip_suffix("[]") {
                Some(p) => (p, "array"),
                None => (parent, "object"),
            };
            if seen.get(parent_path).copied() != Some(needs) {
                errors.push(ValidationError::definition(
                    &f.name,
                    format!("parent '{parent_path}' must be declared with type {needs}"),
                ));
            }
        }
    }
    errors
}

/// Validates one payload against a published field list.
pub fn validate_payload(
    fields: &[SchemaField],
    payload: &Value,
) -> Result<(), Vec<ValidationError>> {
    let Some(root) = payload.as_object() else {
        return Err(vec![ValidationError {
            field_path: "$".into(),
            message: "Payload root must be a JSON object".into(),
            error_type: ValidationViolationType::InvalidRoot,
        }]);
    };
    let mut errors = Vec::new();
    validate_object(root, "", fields, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Fields directly below `prefix` ("" for the root, "a." or "a[]." for nested ones).
fn children<'a>(fields: &'a [SchemaField], prefix: &str) -> Vec<(&'a str, &'a SchemaField)> {
    fields
        .iter()
        .filter_map(|f| {
            let rest = f.name.strip_prefix(prefix)?;
            (!rest.contains('.')).then_some((rest, f))
        })
        .collect()
}

fn validate_object(
    obj: &Map<String, Value>,
    prefix: &str,
    fields: &[SchemaField],
    errors: &mut Vec<ValidationError>,
) {
    let expected = children(fields, prefix);
    for (name, f) in &expected {
        let missing = obj.get(*name).map(Value::is_null).unwrap_or(true);
        if f.required && missing {
            errors.push(ValidationError {
                field_path: f.name.clone(),
                message: format!("Required field '{}' is missing", f.name),
                error_type: ValidationViolationType::MissingRequiredField,
            });
        }
    }
    for (key, value) in obj {
        let path = format!("{prefix}{key}");
        match expected.iter().find(|(name, _)| name == key) {
            None => errors.push(ValidationError {
                field_path: path.clone(),
                message: format!("Undeclared field '{path}' is not defined in the schema"),
                error_type: ValidationViolationType::UnknownField,
            }),
            Some((_, f)) if value.is_null() => {
                let _ = f; // optional null values are allowed; required ones were reported above
            }
            Some((_, f)) => validate_value(value, f, fields, errors),
        }
    }
}

fn validate_value(
    value: &Value,
    f: &SchemaField,
    fields: &[SchemaField],
    errors: &mut Vec<ValidationError>,
) {
    let ok = match f.field_type.as_str() {
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "float" => value.is_number(),
        "boolean" => value.is_boolean(),
        "date" => value
            .as_str()
            .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").is_ok())
            .unwrap_or(false),
        "datetime" => value
            .as_str()
            .map(|s| chrono::DateTime::parse_from_rfc3339(s).is_ok())
            .unwrap_or(false),
        "object" => match value.as_object() {
            Some(child) => {
                validate_object(child, &format!("{}.", f.name), fields, errors);
                true
            }
            None => false,
        },
        "array" => match value.as_array() {
            Some(items) => {
                let item_prefix = format!("{}[].", f.name);
                if !children(fields, &item_prefix).is_empty() {
                    for (i, item) in items.iter().enumerate() {
                        match item.as_object() {
                            Some(obj) => validate_object(obj, &item_prefix, fields, errors),
                            None => errors.push(ValidationError {
                                field_path: format!("{}[{}]", f.name, i),
                                message: format!("Items of '{}' must be objects", f.name),
                                error_type: ValidationViolationType::TypeMismatch {
                                    expected: "object".into(),
                                    found: type_name(item).into(),
                                },
                            }),
                        }
                    }
                }
                true
            }
            None => false,
        },
        _ => false,
    };
    if !ok {
        errors.push(ValidationError {
            field_path: f.name.clone(),
            message: format!("Field '{}' must be of type {}", f.name, f.field_type),
            error_type: ValidationViolationType::TypeMismatch {
                expected: f.field_type.clone(),
                found: type_name(value).into(),
            },
        });
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
        Value::Number(_) => "float",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn nested_paths_and_definition_errors() {
        let fields: Vec<SchemaField> = serde_json::from_value(json!([
            {"name":"profile","type":"object","required":true,"in_context":true},
            {"name":"profile.name","type":"string","required":true,"in_context":true},
            {"name":"steps","type":"array","in_context":false},
            {"name":"steps[].done","type":"boolean","required":true,"in_context":false}
        ]))
        .unwrap();
        assert!(check_fields(&fields).is_empty());
        assert!(
            validate_payload(
                &fields,
                &json!({"profile":{"name":"Fixture"},"steps":[{"done":true}]})
            )
            .is_ok()
        );
        let errors = validate_payload(
            &fields,
            &json!({"profile":{"unknown":true},"steps":[{"done":"wrong"}]}),
        )
        .unwrap_err();
        for path in ["profile.name", "profile.unknown", "steps[].done"] {
            assert!(errors.iter().any(|e| e.field_path == path));
        }
        let bad: Vec<SchemaField> =
            serde_json::from_value(json!([{"name":"child.name","type":"wrong"}])).unwrap();
        assert!(!check_fields(&bad).is_empty());
    }
}

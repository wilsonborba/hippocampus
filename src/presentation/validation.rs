use axum::{
    Json,
    body::Bytes,
    extract::{FromRequest, Request},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use std::sync::OnceLock;

pub struct ApiJson<T>(pub T);
fn failure(errors: Vec<Value>) -> Response {
    (
        StatusCode::UNPROCESSABLE_ENTITY,
        Json(json!({"detail":errors})),
    )
        .into_response()
}
fn issue(kind: &str, loc: Value, message: &str, input: Value) -> Value {
    json!({"type":kind,"loc":loc,"msg":message,"input":input})
}
impl<S: Send + Sync, T: DeserializeOwned> FromRequest<S> for ApiJson<T> {
    type Rejection = Response;
    async fn from_request(request: Request, state: &S) -> Result<Self, Response> {
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(IntoResponse::into_response)?;
        if bytes.is_empty() {
            return Err(failure(vec![issue(
                "missing",
                json!(["body"]),
                "Field required",
                Value::Null,
            )]));
        }
        let mut input: Value = serde_json::from_slice(&bytes).map_err(|e| {
            let mut error = issue(
                "json_invalid",
                json!(["body", e.column().saturating_sub(1)]),
                "JSON decode error",
                json!({}),
            );
            error["ctx"] = json!({"error":e.to_string()});
            failure(vec![error])
        })?;
        let shapes = SHAPES.get_or_init(|| {
            serde_json::from_str(include_str!("request_shapes.json")).expect("request contract")
        });
        let name = std::any::type_name::<T>()
            .rsplit("::")
            .next()
            .expect("request name");
        let fields = shapes[name].as_array().expect("request fields");
        let mut errors = vec![];
        if !input.is_object() {
            errors.push(issue(
                "model_attributes_type",
                json!(["body"]),
                "Input should be a valid dictionary or object to extract fields from",
                input.clone(),
            ));
        } else {
            let original = input.clone();
            for field in fields {
                let key = field["name"].as_str().unwrap();
                if let Some(value) = input.get_mut(key) {
                    validate(value, &field["schema"], json!(["body", key]), &mut errors);
                } else if field["required"] == true {
                    errors.push(issue(
                        "missing",
                        json!(["body", key]),
                        "Field required",
                        original.clone(),
                    ));
                }
            }
        }
        if !errors.is_empty() {
            return Err(failure(errors));
        }
        serde_json::from_value(input).map(Self).map_err(|_| {
            failure(vec![issue(
                "value_error",
                json!(["body"]),
                "The request data is not valid",
                json!({}),
            )])
        })
    }
}
static SHAPES: OnceLock<Value> = OnceLock::new();
fn validate(value: &mut Value, schema: &Value, loc: Value, errors: &mut Vec<Value>) {
    if let Some(reference) = schema["$ref"].as_str() {
        static OPENAPI: OnceLock<Value> = OnceLock::new();
        let api = OPENAPI
            .get_or_init(|| serde_json::from_str(include_str!("openapi.json")).expect("OpenAPI"));
        let name = reference.rsplit('/').next().expect("schema reference");
        validate(value, &api["components"]["schemas"][name], loc, errors);
        return;
    }
    if let Some(options) = schema["anyOf"].as_array() {
        if value.is_null() && options.iter().any(|x| x["type"] == "null") {
            return;
        }
        if let Some(actual) = options.iter().find(|x| x["type"] != "null") {
            validate(value, actual, loc, errors);
        }
        return;
    }
    let kind = schema["type"].as_str().unwrap_or("");
    if ["number", "integer"].contains(&kind)
        && let Some(text) = value.as_str()
    {
        if kind == "integer" {
            if let Ok(number) = text.parse::<i64>() {
                *value = json!(number);
            }
        } else if let Ok(number) = text.parse::<f64>()
            && number.is_finite()
        {
            *value = json!(number);
        }
    }
    let valid = match kind {
        "string" => value.is_string(),
        "object" => value.is_object(),
        "array" => value.is_array(),
        "number" => value.is_number(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        _ => true,
    };
    if !valid {
        let (error, message) = match kind {
            "string" => ("string_type", "Input should be a valid string"),
            "object" => ("dict_type", "Input should be a valid dictionary"),
            "array" => ("list_type", "Input should be a valid list"),
            "number" => ("float_type", "Input should be a valid number"),
            "integer" => ("int_type", "Input should be a valid integer"),
            "boolean" => ("bool_type", "Input should be a valid boolean"),
            _ => ("value_error", "The value is not valid"),
        };
        errors.push(issue(error, loc, message, value.clone()));
        return;
    }
    if let Some(properties) = schema["properties"].as_object() {
        let original = value.clone();
        for (key, property) in properties {
            let mut path = loc.as_array().expect("location").clone();
            path.push(json!(key));
            if let Some(child) = value.get_mut(key) {
                validate(child, property, json!(path), errors);
            } else if schema["required"]
                .as_array()
                .is_some_and(|r| r.iter().any(|v| v == key))
            {
                errors.push(issue(
                    "missing",
                    json!(path),
                    "Field required",
                    original.clone(),
                ));
            }
        }
    }
    if let Some(allowed) = schema["enum"].as_array()
        && !allowed.contains(value)
    {
        let expected = allowed
            .iter()
            .map(|v| format!("'{}'", v.as_str().unwrap_or("")))
            .collect::<Vec<_>>()
            .join(" or ");
        let mut error = issue(
            "literal_error",
            loc.clone(),
            &format!("Input should be {expected}"),
            value.clone(),
        );
        error["ctx"] = json!({"expected":expected});
        errors.push(error);
    }
    if let Some(values) = value.as_array_mut() {
        for (index, item) in values.iter_mut().enumerate() {
            let mut path = loc.as_array().unwrap().clone();
            path.push(json!(index));
            validate(item, &schema["items"], json!(path), errors);
        }
    }
    if schema["format"] == "date-time" {
        let text = value.as_str().unwrap();
        if let Ok(date) = chrono::DateTime::parse_from_rfc3339(text) {
            *value = json!(date.to_rfc3339());
        } else if let Ok(date) = chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f")
        {
            *value = json!(date.and_utc().to_rfc3339());
        } else if let Ok(date) = chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d") {
            *value = json!(date.and_hms_opt(0, 0, 0).unwrap().and_utc().to_rfc3339());
        } else {
            let reason = if text.len() < 10 {
                "input is too short"
            } else {
                "invalid datetime"
            };
            let mut error = issue(
                "datetime_from_date_parsing",
                loc,
                &format!("Input should be a valid datetime or date, {reason}"),
                value.clone(),
            );
            error["ctx"] = json!({"error":reason});
            errors.push(error);
        }
    }
}

pub struct ApiQuery(pub crate::domain::services::memories::Filters);
impl<S: Send + Sync> axum::extract::FromRequestParts<S> for ApiQuery {
    type Rejection = Response;
    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Response> {
        let axum::extract::Query(mut filters) = axum::extract::Query::<
            crate::domain::services::memories::Filters,
        >::from_request_parts(parts, state)
        .await
        .map_err(IntoResponse::into_response)?;
        let mut errors = vec![];
        for field in ["depth", "max_nodes", "limit"] {
            if let Some(value) = filters.get(field)
                && value.parse::<i64>().is_err()
            {
                errors.push(issue(
                    "int_parsing",
                    json!(["query", field]),
                    "Input should be a valid integer, unable to parse string as an integer",
                    json!(value),
                ));
            }
        }
        for field in ["include_tags", "include_entities", "include_resources"] {
            if let Some(value) = filters.get_mut(field) {
                match value.to_lowercase().as_str() {
                    "0" | "off" | "f" | "false" | "n" | "no" => *value = "false".into(),
                    "1" | "on" | "t" | "true" | "y" | "yes" => *value = "true".into(),
                    _ => errors.push(issue(
                        "bool_parsing",
                        json!(["query", field]),
                        "Input should be a valid boolean, unable to interpret input",
                        json!(value),
                    )),
                }
            }
        }
        if errors.is_empty() {
            Ok(Self(filters))
        } else {
            Err(failure(errors))
        }
    }
}

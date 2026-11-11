use crate::{
    core::config::{EmbeddingBackend, EmbeddingConfig},
    domain::models::embedding::{EmbeddingSource, Pending},
};
use serde_json::json;
use std::sync::Arc;
#[derive(Default)]
pub struct DisabledEmbedding;
impl EmbeddingSource for DisabledEmbedding {
    fn name(&self) -> &str {
        "none"
    }
    fn embed<'a>(&'a self, _: &'a str) -> Pending<'a, Option<Vec<f64>>> {
        Box::pin(async { None })
    }
}
pub struct OllamaEmbedding {
    client: reqwest::Client,
    url: String,
    model: String,
}
impl EmbeddingSource for OllamaEmbedding {
    fn name(&self) -> &str {
        &self.model
    }
    fn embed<'a>(&'a self, text: &'a str) -> Pending<'a, Option<Vec<f64>>> {
        Box::pin(async move {
            let response = self
                .client
                .post(format!("{}/api/embeddings", self.url.trim_end_matches('/')))
                .json(&json!({"model":self.model,"prompt":text}))
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?;
            let value: serde_json::Value = response.json().await.ok()?;
            let vector: Vec<f64> = serde_json::from_value(value["embedding"].clone()).ok()?;
            if vector.is_empty() || vector.iter().any(|v| !v.is_finite()) {
                None
            } else {
                Some(vector)
            }
        })
    }
}
pub fn configured(config: &EmbeddingConfig) -> Arc<dyn EmbeddingSource> {
    if config.backend == EmbeddingBackend::Ollama
        && let Some(model) = config.model.as_ref().filter(|s| !s.trim().is_empty())
    {
        return Arc::new(OllamaEmbedding {
            client: reqwest::Client::builder()
                .timeout(config.timeout)
                .build()
                .expect("embedding HTTP client"),
            url: config.base_url.clone(),
            model: model.clone(),
        });
    }
    Arc::new(DisabledEmbedding)
}

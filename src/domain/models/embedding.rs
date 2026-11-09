use crate::domain::models::error::Error;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::{future::Future, pin::Pin};
pub type Pending<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[derive(Debug, Clone)]
pub struct EmbeddingMemory {
    pub id: String,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub content: Option<String>,
    pub current_hash: Option<String>,
}
#[derive(Debug, Default, Serialize)]
pub struct EmbeddingReport {
    pub processed: usize,
    pub embedded: usize,
    pub skipped_no_provider: usize,
    pub failed: usize,
}
pub trait EmbeddingSource: Send + Sync {
    fn name(&self) -> &str;
    fn embed<'a>(&'a self, text: &'a str) -> Pending<'a, Option<Vec<f64>>>;
}
pub trait EmbeddingStore: Send + Sync {
    fn batch<'a>(
        &'a self,
        model: &'a str,
        after: &'a str,
        limit: i64,
    ) -> Pending<'a, Result<Vec<EmbeddingMemory>, Error>>;
    fn save<'a>(
        &'a self,
        id: &'a str,
        model: &'a str,
        vector: &'a [f64],
        hash: &'a str,
        clock: DateTime<Utc>,
    ) -> Pending<'a, Result<(), Error>>;
}

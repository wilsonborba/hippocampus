use crate::domain::models::{
    embedding::{EmbeddingMemory, EmbeddingReport, EmbeddingSource, EmbeddingStore},
    error::Error,
};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
pub fn canonical_text(memory: &EmbeddingMemory) -> String {
    [&memory.title, &memory.summary, &memory.content]
        .iter()
        .filter_map(|s| s.as_deref())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_owned()
}
pub fn content_hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
pub async fn run(
    store: &dyn EmbeddingStore,
    provider: &dyn EmbeddingSource,
    batch_limit: i64,
    clock: DateTime<Utc>,
) -> Result<EmbeddingReport, Error> {
    let mut report = EmbeddingReport::default();
    if provider.name() == "none" {
        return Ok(report);
    }
    if batch_limit < 1 {
        return Err(Error::invalid("The embedding batch limit must be positive"));
    }
    let mut after = String::new();
    loop {
        let memories = store.batch(provider.name(), &after, batch_limit).await?;
        if memories.is_empty() {
            break;
        }
        for memory in memories {
            after = memory.id.clone();
            report.processed += 1;
            let text = canonical_text(&memory);
            if text.is_empty() {
                continue;
            }
            let hash = content_hash(&text);
            if memory.current_hash.as_ref() == Some(&hash) {
                continue;
            }
            if let Some(vector) = provider
                .embed(&text)
                .await
                .filter(|v| !v.is_empty() && v.iter().all(|n| n.is_finite()))
            {
                store
                    .save(&memory.id, provider.name(), &vector, &hash, clock)
                    .await?;
                report.embedded += 1;
            } else {
                report.failed += 1;
                tracing::warn!("Embedding generation failed");
            }
        }
    }
    Ok(report)
}

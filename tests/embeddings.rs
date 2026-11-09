use chrono::{DateTime, Utc};
use hippocampus::{
    dal::remote::embeddings::DisabledEmbedding,
    domain::{
        models::{embedding::*, error::Error},
        tasks::embeddings::{content_hash, run},
    },
};
use std::sync::{
    Mutex,
    atomic::{AtomicUsize, Ordering},
};
struct MemoryCopy {
    rows: Mutex<Vec<EmbeddingMemory>>,
    batches: AtomicUsize,
}
impl EmbeddingStore for MemoryCopy {
    fn batch<'a>(
        &'a self,
        _: &'a str,
        after: &'a str,
        limit: i64,
    ) -> Pending<'a, Result<Vec<EmbeddingMemory>, Error>> {
        Box::pin(async move {
            self.batches.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|m| m.id.as_str() > after)
                .take(limit as usize)
                .cloned()
                .collect())
        })
    }
    fn save<'a>(
        &'a self,
        id: &'a str,
        _: &'a str,
        vector: &'a [f64],
        hash: &'a str,
        _: DateTime<Utc>,
    ) -> Pending<'a, Result<(), Error>> {
        Box::pin(async move {
            assert_eq!(vector.len(), 2);
            self.rows
                .lock()
                .unwrap()
                .iter_mut()
                .find(|m| m.id == id)
                .unwrap()
                .current_hash = Some(hash.into());
            Ok(())
        })
    }
}
struct ModelDouble(AtomicUsize);
impl EmbeddingSource for ModelDouble {
    fn name(&self) -> &str {
        "fixture"
    }
    fn embed<'a>(&'a self, _: &'a str) -> Pending<'a, Option<Vec<f64>>> {
        Box::pin(async move {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some(vec![1.0, 0.0])
        })
    }
}
fn copy() -> MemoryCopy {
    MemoryCopy {
        rows: Mutex::new(
            (0..500)
                .map(|i| EmbeddingMemory {
                    id: format!("{i:04}"),
                    title: Some(format!("Memory {i}")),
                    summary: None,
                    content: None,
                    current_hash: None,
                })
                .collect(),
        ),
        batches: AtomicUsize::new(0),
    }
}
#[tokio::test]
async fn all_500_memories_cross_batch_boundaries() {
    let store = copy();
    let model = ModelDouble(AtomicUsize::new(0));
    let clock = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    let report = run(&store, &model, 50, clock).await.unwrap();
    assert_eq!(report.processed, 500);
    assert_eq!(report.embedded, 500);
    assert_eq!(store.batches.load(Ordering::SeqCst), 11);
    assert_eq!(model.0.load(Ordering::SeqCst), 500);
    assert!(
        store
            .rows
            .lock()
            .unwrap()
            .iter()
            .all(|m| m.current_hash.is_some())
    );
    let repeat = run(&store, &model, 50, clock).await.unwrap();
    assert_eq!(repeat.embedded, 0);
    assert_eq!(model.0.load(Ordering::SeqCst), 500);
    store.rows.lock().unwrap()[0].title = Some("Changed old memory".into());
    assert_eq!(run(&store, &model, 50, clock).await.unwrap().embedded, 1);
}
#[tokio::test]
async fn disabled_model_has_no_storage_or_network_work() {
    let store = copy();
    let report = run(&store, &DisabledEmbedding, 50, Utc::now())
        .await
        .unwrap();
    assert_eq!(report.processed, 0);
    assert_eq!(report.embedded, 0);
    assert_eq!(store.batches.load(Ordering::SeqCst), 0);
    assert_eq!(
        hippocampus::dal::remote::embeddings::configured(&Default::default()).name(),
        "none"
    );
    assert_eq!(
        content_hash("abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test"]
async fn postgres_vector_and_model_off_word_search() {
    use hippocampus::{
        dal::local::{store::Store, test_database},
        domain::{
            models::api::MemoryCreateRequest,
            services::memories::{Filters, MemoryService},
        },
    };
    use serde_json::json;
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let workspace = format!("embedding-{}", uuid::Uuid::new_v4().simple());
    let request: MemoryCreateRequest = serde_json::from_value(
        json!({"app_id":"fixture","workspace_id":workspace,"title":"needle"}),
    )
    .unwrap();
    let m = service.create(request).await.unwrap();
    let id = m["id"].as_str().unwrap();
    assert_eq!(
        service
            .list(&Filters::from([
                ("workspace_id".into(), workspace),
                ("text".into(), "needle".into())
            ]))
            .await
            .unwrap()
            .len(),
        1
    );
    let clock = DateTime::parse_from_rfc3339("2030-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&Utc);
    store
        .save(id, "fixture", &[1.0, 0.0], "first", clock)
        .await
        .unwrap();
    store
        .save(id, "fixture", &[0.0, 1.0], "second", clock)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM memory_embedding WHERE memory_id=$1")
        .bind(id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(count, 1);
    let vector: String =
        sqlx::query_scalar("SELECT embedding::text FROM memory_embedding WHERE memory_id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(vector, "[0,1]");
    sqlx::query("DELETE FROM memory_embedding WHERE memory_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM memory_event WHERE memory_id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM memory WHERE id=$1")
        .bind(id)
        .execute(&pool)
        .await
        .unwrap();
}

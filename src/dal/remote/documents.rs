use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct Documents {
    fallback: Arc<RwLock<HashMap<String, Value>>>,
    couch: Option<(reqwest::Client, String, String, String)>,
}
impl Documents {
    pub async fn from_environment() -> Self {
        let mut result = Self::default();
        if let (Ok(url), Ok(user), Ok(password)) = (
            std::env::var("HIPPOCAMPUS_COUCHDB_URL"),
            std::env::var("HIPPOCAMPUS_COUCHDB_USERNAME"),
            std::env::var("HIPPOCAMPUS_COUCHDB_PASSWORD"),
        ) {
            let client = reqwest::Client::builder()
                .timeout(crate::core::config::Config::default().document_timeout)
                .build()
                .expect("HTTP client");
            let base = format!(
                "{}/{}",
                url.trim_end_matches('/'),
                crate::core::config::Config::default().document_database
            );
            if client
                .get(&base)
                .basic_auth(&user, Some(&password))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                result.couch = Some((client, base, user, password));
            }
        }
        result
    }
    pub async fn get(&self, id: &str) -> Option<Value> {
        if let Some((client, base, user, password)) = &self.couch {
            client
                .get(format!("{base}/memory:{id}"))
                .basic_auth(user, Some(password))
                .send()
                .await
                .ok()?
                .error_for_status()
                .ok()?
                .json()
                .await
                .ok()
        } else {
            self.fallback.read().await.get(id).cloned()
        }
    }
    pub async fn available(&self) -> bool {
        if let Some((client, base, user, password)) = &self.couch {
            client
                .get(base)
                .basic_auth(user, Some(password))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
        } else {
            true
        }
    }
}

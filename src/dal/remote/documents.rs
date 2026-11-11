use serde_json::Value;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[derive(Clone, Default)]
pub struct Documents {
    delete_unavailable: bool,
    fallback: Arc<RwLock<HashMap<String, Value>>>,
    couch: Option<(reqwest::Client, String, String, String)>,
}
impl Documents {
    pub fn couchdb(base: String, user: String, password: String) -> Self {
        let client = reqwest::Client::builder()
            .timeout(crate::core::config::Config::default().document_timeout)
            .build()
            .expect("document client");
        Self {
            couch: Some((client, base, user, password)),
            ..Self::default()
        }
    }
    pub fn persistent(&self) -> bool {
        self.couch.is_some()
    }
    pub async fn from_environment() -> Self {
        Self::from_config(&crate::core::config::Config::default()).await
    }
    pub async fn from_config(config: &crate::core::config::Config) -> Self {
        let mut result = Self {
            delete_unavailable: true,
            ..Self::default()
        };
        if let (Ok(url), Ok(user), Ok(password)) = (
            std::env::var("HIPPOCAMPUS_COUCHDB_URL"),
            std::env::var("HIPPOCAMPUS_COUCHDB_USERNAME"),
            std::env::var("HIPPOCAMPUS_COUCHDB_PASSWORD"),
        ) {
            let client = reqwest::Client::builder()
                .timeout(crate::core::config::Config::default().document_timeout)
                .build()
                .expect("HTTP client");
            let base = format!("{}/{}", url.trim_end_matches('/'), config.document_database);
            if client
                .get(&base)
                .basic_auth(&user, Some(&password))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                result.couch = Some((client, base, user, password));
                result.delete_unavailable = false;
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
    pub async fn delete(&self, id: &str) -> bool {
        if self.delete_unavailable {
            return false;
        }
        if let Some((client, base, user, password)) = &self.couch {
            let url = format!("{base}/memory:{id}");
            let Ok(response) = client
                .get(&url)
                .basic_auth(user, Some(password))
                .send()
                .await
            else {
                return false;
            };
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return true;
            }
            if !response.status().is_success() {
                return false;
            }
            let Ok(document) = response.json::<Value>().await else {
                return false;
            };
            let Some(revision) = document["_rev"].as_str() else {
                return false;
            };
            client
                .delete(url)
                .query(&[("rev", revision)])
                .basic_auth(user, Some(password))
                .send()
                .await
                .is_ok_and(|r| {
                    r.status().is_success() || r.status() == reqwest::StatusCode::NOT_FOUND
                })
        } else {
            self.fallback.write().await.remove(id);
            true
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

    pub async fn put(&self, id: &str, mut document: Value) -> bool {
        if let Some((client, base, user, password)) = &self.couch {
            let url = format!("{base}/memory:{id}");
            if let Some(existing) = self.get(id).await
                && let Some(rev) = existing.get("_rev")
            {
                document["_rev"] = rev.clone();
            }
            document["_id"] = serde_json::json!(format!("memory:{id}"));
            document["memory_id"] = serde_json::json!(id);
            client
                .put(url)
                .basic_auth(user, Some(password))
                .json(&document)
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
        } else {
            self.fallback.write().await.insert(id.to_owned(), document);
            true
        }
    }
}

impl Documents {
    pub async fn get_persona(
        &self,
        id: &str,
    ) -> Result<Option<Value>, crate::domain::models::error::Error> {
        if self.delete_unavailable {
            return Err(persona_failure());
        }
        if let Some((client, base, user, password)) = &self.couch {
            let response = client
                .get(format!("{base}/{id}"))
                .basic_auth(user, Some(password))
                .send()
                .await
                .map_err(|_| persona_failure())?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Ok(None);
            }
            if !response.status().is_success() {
                return Err(persona_failure());
            }
            Ok(Some(response.json().await.map_err(|_| persona_failure())?))
        } else {
            Ok(self
                .fallback
                .read()
                .await
                .get(&format!("persona:{id}"))
                .cloned())
        }
    }
    pub async fn put_persona(
        &self,
        id: &str,
        mut document: Value,
    ) -> Result<(), crate::domain::models::error::Error> {
        let existing = self.get_persona(id).await?;
        if let Some((client, base, user, password)) = &self.couch {
            if let Some(revision) = existing.as_ref().and_then(|v| v.get("_rev")) {
                document["_rev"] = revision.clone();
            }
            document["_id"] = serde_json::json!(id);
            let response = client
                .put(format!("{base}/{id}"))
                .basic_auth(user, Some(password))
                .json(&document)
                .send()
                .await
                .map_err(|_| persona_failure())?;
            if !response.status().is_success() {
                return Err(persona_failure());
            }
        } else {
            document["_id"] = serde_json::json!(id);
            self.fallback
                .write()
                .await
                .insert(format!("persona:{id}"), document);
        }
        Ok(())
    }
    pub async fn delete_persona(
        &self,
        id: &str,
    ) -> Result<(), crate::domain::models::error::Error> {
        let document = self.get_persona(id).await?;
        let Some(document) = document else {
            return Ok(());
        };
        if let Some((client, base, user, password)) = &self.couch {
            let revision = document["_rev"].as_str().ok_or_else(persona_failure)?;
            let response = client
                .delete(format!("{base}/{id}"))
                .query(&[("rev", revision)])
                .basic_auth(user, Some(password))
                .send()
                .await
                .map_err(|_| persona_failure())?;
            if !response.status().is_success()
                && response.status() != reqwest::StatusCode::NOT_FOUND
            {
                return Err(persona_failure());
            }
        } else {
            self.fallback.write().await.remove(&format!("persona:{id}"));
        }
        Ok(())
    }
}
fn persona_failure() -> crate::domain::models::error::Error {
    crate::domain::models::error::Error::new(
        axum::http::StatusCode::BAD_GATEWAY,
        "document_store_error",
        "The persona document store operation failed",
    )
}

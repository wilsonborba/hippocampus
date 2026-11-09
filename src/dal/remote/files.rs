use crate::{
    core::config::Config,
    domain::models::{
        embedding::Pending,
        error::Error,
        files::{FileBytes, FileStore, StoredFile},
    },
};
use axum::http::StatusCode;
use sha2::{Digest, Sha256};
#[derive(Clone)]
pub struct FsmClient {
    base_url: Option<String>,
    key: Option<String>,
    app: String,
    client: reqwest::Client,
}
fn failure(message: impl Into<String>) -> Error {
    Error::new(StatusCode::BAD_GATEWAY, "filestore_error", message)
}
impl Default for FsmClient {
    fn default() -> Self {
        let config = Config::default();
        Self {
            base_url: None,
            key: None,
            app: config.filestore_app,
            client: reqwest::Client::builder()
                .timeout(config.filestore_timeout)
                .build()
                .expect("FSM client"),
        }
    }
}
impl FsmClient {
    pub fn from_environment() -> Self {
        Self {
            base_url: std::env::var("HIPPOCAMPUS_FILESTORE_URL")
                .ok()
                .filter(|s| !s.is_empty()),
            key: std::env::var("HIPPOCAMPUS_FILESTORE_API_KEY")
                .ok()
                .filter(|s| !s.is_empty()),
            ..Self::default()
        }
    }
    pub fn new(base_url: String, key: String, app: String) -> Self {
        Self {
            base_url: Some(base_url),
            key: Some(key),
            app,
            ..Self::default()
        }
    }
    fn url(&self, key: Option<&str>) -> Result<reqwest::Url, Error> {
        let base = self
            .base_url
            .as_ref()
            .zip(self.key.as_ref())
            .ok_or_else(|| {
                Error::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "filestore_not_configured",
                    "File Store (FSM) is not configured or missing API key",
                )
            })?
            .0;
        let mut url = reqwest::Url::parse(base)
            .map_err(|_| failure("The File Store address is not valid"))?;
        {
            let mut path = url
                .path_segments_mut()
                .map_err(|_| failure("The File Store address is not valid"))?;
            path.pop_if_empty().push(&self.app).push("media");
            if let Some(key) = key {
                path.push(key);
            }
        }
        Ok(url)
    }
}
impl FileStore for FsmClient {
    fn upload<'a>(
        &'a self,
        album: &'a str,
        filename: &'a str,
        content_type: &'a str,
        bytes: &'a [u8],
    ) -> Pending<'a, Result<StoredFile, Error>> {
        Box::pin(async move {
            let url = self.url(None)?;
            let part = reqwest::multipart::Part::bytes(bytes.to_vec())
                .file_name(filename.to_owned())
                .mime_str(content_type)
                .map_err(|_| Error::invalid("The file content type is not valid"))?;
            let form = reqwest::multipart::Form::new()
                .text("album", album.to_owned())
                .text("force", "false")
                .part("file", part);
            let response = self
                .client
                .post(url)
                .bearer_auth(self.key.as_deref().expect("configured key"))
                .multipart(form)
                .send()
                .await
                .map_err(|_| failure("File Store upload failed"))?;
            if !response.status().is_success() {
                return Err(failure(format!(
                    "File Store rejected upload (status {})",
                    response.status().as_u16()
                )));
            }
            let data: serde_json::Value = response
                .json()
                .await
                .map_err(|_| failure("File Store returned invalid upload response"))?;
            let key = data["item"]["key"]
                .as_str()
                .ok_or_else(|| failure("File Store returned invalid upload response"))?
                .to_owned();
            let mime = data["item"]["content_type"]
                .as_str()
                .unwrap_or(content_type)
                .to_owned();
            Ok(StoredFile {
                uri: self.url(Some(&key))?.to_string(),
                key,
                content_type: mime,
                size_bytes: bytes.len() as i64,
                checksum: format!("{:x}", Sha256::digest(bytes)),
            })
        })
    }
    fn download<'a>(&'a self, key: &'a str) -> Pending<'a, Result<FileBytes, Error>> {
        Box::pin(async move {
            let response = self
                .client
                .get(self.url(Some(key))?)
                .bearer_auth(self.key.as_deref().expect("configured key"))
                .send()
                .await
                .map_err(|_| failure("File Store download failed"))?;
            if response.status() == reqwest::StatusCode::NOT_FOUND {
                return Err(Error::new(
                    StatusCode::NOT_FOUND,
                    "resource_not_found",
                    "The file content was not found",
                ));
            }
            if !response.status().is_success() {
                return Err(failure("File Store download failed"));
            }
            let mime = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("application/octet-stream")
                .to_owned();
            Ok(FileBytes {
                content_type: mime,
                bytes: response
                    .bytes()
                    .await
                    .map_err(|_| failure("File Store download failed"))?
                    .to_vec(),
            })
        })
    }
}

use crate::domain::models::{embedding::Pending, error::Error, scope::WriteScope};
use serde::Serialize;
pub struct FileUpload {
    pub scope: WriteScope,
    pub filename: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
    pub extracted_text: String,
}
pub struct StoredFile {
    pub key: String,
    pub uri: String,
    pub content_type: String,
    pub size_bytes: i64,
    pub checksum: String,
}
pub struct FileBytes {
    pub bytes: Vec<u8>,
    pub content_type: String,
}
#[derive(Serialize)]
pub struct FileIds {
    pub memory_id: String,
    pub resource_id: String,
}
pub struct Download {
    pub bytes: Vec<u8>,
    pub content_type: String,
    pub filename: String,
}
pub trait FileStore: Send + Sync {
    fn delete<'a>(&'a self, key: &'a str) -> Pending<'a, Result<(), Error>>;
    fn upload<'a>(
        &'a self,
        album: &'a str,
        filename: &'a str,
        content_type: &'a str,
        bytes: &'a [u8],
    ) -> Pending<'a, Result<StoredFile, Error>>;
    fn download<'a>(&'a self, key: &'a str) -> Pending<'a, Result<FileBytes, Error>>;
}

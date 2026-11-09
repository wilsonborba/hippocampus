use crate::{
    dal::{local::store::Store, remote::documents::Documents},
    domain::models::{deletion::DeleteRequest, error::Error, files::FileStore},
};
pub async fn delete(
    store: &Store,
    files: &dyn FileStore,
    documents: &Documents,
    request: DeleteRequest,
) -> Result<Vec<String>, Error> {
    let deletion = store.begin_memory_delete(&request).await?;
    for id in &deletion.documents {
        if !documents.delete(id).await {
            return Err(Error::new(
                axum::http::StatusCode::BAD_GATEWAY,
                "document_store_error",
                "The memory document could not be deleted",
            ));
        }
    }
    for file in &deletion.files {
        if file["source_system"] == "fsm" && file["ownership"] == "hippocampus" {
            let key = file["external_id"]
                .as_str()
                .ok_or_else(|| Error::invalid("The owned file has no storage key"))?;
            files.delete(key).await?;
        }
    }
    deletion.commit().await
}

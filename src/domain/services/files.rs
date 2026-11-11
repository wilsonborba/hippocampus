use crate::{
    dal::local::store::{Table, id},
    domain::{
        models::{
            error::Error,
            files::{Download, FileIds, FileStore, FileUpload},
        },
        services::memories::{Filters, MemoryService, normalize},
    },
};
impl MemoryService {
    pub async fn upload_file(
        &self,
        mut file: FileUpload,
        fs: &dyn FileStore,
    ) -> Result<FileIds, Error> {
        file.scope.workspace_id = normalize(&file.scope.workspace_id);
        for (name, value, limit) in [
            ("workspace_id", file.scope.workspace_id.as_str(), 64),
            ("app_id", file.scope.app_id.as_str(), 64),
        ] {
            if value.trim().is_empty() || value.chars().count() > limit {
                return Err(Error::invalid(format!("{name} is not valid")));
            }
        }
        if file
            .scope
            .user_id
            .as_ref()
            .is_some_and(|s| s.chars().count() > 128)
        {
            return Err(Error::invalid("user_id is not valid"));
        }
        file.scope.user_id = file.scope.user_id.filter(|s| !s.is_empty());
        if file.filename.is_empty() {
            file.filename = "upload.bin".into();
        }
        if file.filename.chars().count() > 256 {
            return Err(Error::invalid("The file name is too long"));
        }
        let memory_id = id();
        let stored = fs
            .upload(
                &format!("memory-{memory_id}"),
                &file.filename,
                &file.content_type,
                &file.bytes,
            )
            .await?;
        self.store
            .save_file_record(
                &memory_id,
                &file.scope,
                &file.filename,
                &file.extracted_text,
                &stored,
            )
            .await
    }
    pub async fn download_file(
        &self,
        id: &str,
        scope: &Filters,
        fs: &dyn FileStore,
    ) -> Result<Download, Error> {
        let resource = self
            .store
            .get(Table::Resource, id)
            .await?
            .ok_or_else(|| Error::not_found("resource", id))?;
        let mut filters = scope.clone();
        if let Some(workspace) = filters.get_mut("workspace_id") {
            *workspace = normalize(workspace);
        }
        if ["workspace_id", "app_id", "user_id"]
            .iter()
            .any(|key| filters.contains_key(*key))
            && !self.store.resource_in_scope(id, &filters).await?
        {
            return Err(Error::not_found("resource", id));
        }
        if resource["source_system"] != "fsm" {
            return Err(Error::invalid("The resource is not an FSM file"));
        }
        let key = resource["external_id"]
            .as_str()
            .ok_or_else(|| Error::invalid("The file has no FSM key"))?;
        let file = fs.download(key).await?;
        Ok(Download {
            bytes: file.bytes,
            content_type: file.content_type,
            filename: resource["title"].as_str().unwrap_or("download").into(),
        })
    }
}

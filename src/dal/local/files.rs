use crate::{
    dal::local::store::{Store, id},
    domain::{
        models::{
            error::Error,
            files::{FileIds, StoredFile},
            scope::WriteScope,
        },
        services::memories::Filters,
    },
};
use sqlx::{Postgres, QueryBuilder};
impl Store {
    pub async fn save_file_record(
        &self,
        memory_id: &str,
        scope: &WriteScope,
        filename: &str,
        text: &str,
        file: &StoredFile,
    ) -> Result<FileIds, Error> {
        let resource_id = id();
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO memory (id,workspace_id,app_id,user_id,memory_type,status,title,content,access_count,reinforcement_count,metadata_json,created_at,updated_at) VALUES ($1,$2,$3,$4,'file','active',$5,$6,0,0,'{}',now(),now())").bind(memory_id).bind(&scope.workspace_id).bind(&scope.app_id).bind(&scope.user_id).bind(filename).bind(text).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO memory_event (id,memory_id,event_type,actor_type,payload,created_at) VALUES ($1,$2,'created','user','{}',now())").bind(id()).bind(memory_id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO resource (id,resource_type,source_system,external_id,uri,title,content_type,checksum,size_bytes,ownership,status,metadata_json,created_at,updated_at) VALUES ($1,'file','fsm',$2,$3,$4,$5,$6,$7,'hippocampus','available','{}',now(),now())").bind(&resource_id).bind(&file.key).bind(&file.uri).bind(filename).bind(&file.content_type).bind(&file.checksum).bind(file.size_bytes).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO memory_resource (id,memory_id,resource_id,relationship,source,created_at) VALUES ($1,$2,$3,'references','user',now())").bind(id()).bind(memory_id).bind(&resource_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(FileIds {
            memory_id: memory_id.into(),
            resource_id,
        })
    }
    pub async fn resource_in_scope(&self, id: &str, scope: &Filters) -> Result<bool, Error> {
        let mut query = QueryBuilder::<Postgres>::new(
            "SELECT EXISTS(SELECT 1 FROM memory_resource mr JOIN memory m ON m.id=mr.memory_id WHERE mr.resource_id=",
        );
        query.push_bind(id).push(" AND m.status != 'deleted'");
        for key in ["workspace_id", "app_id"] {
            if let Some(value) = scope.get(key) {
                query.push(format!(" AND m.{key}=")).push_bind(value);
            }
        }
        if let Some(user) = scope.get("user_id") {
            if user.is_empty() {
                query.push(" AND m.user_id IS NULL");
            } else {
                query.push(" AND m.user_id=").push_bind(user);
            }
        }
        query.push(")");
        Ok(query.build_query_scalar().fetch_one(&self.pool).await?)
    }
}

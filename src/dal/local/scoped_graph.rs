use super::store::{Store, Table};
use crate::domain::models::error::Error;
use serde_json::Value;
impl Store {
    pub async fn graph_links(&self, ids: &[String], table: Table) -> Result<Vec<Value>, Error> {
        let sql = match table {
            Table::Tag => {
                "SELECT row_to_json(t) FROM (SELECT mt.memory_id,t.* FROM memory_tag mt JOIN tag t ON t.id=mt.tag_id WHERE mt.memory_id=ANY($1)) t"
            }
            Table::Entity => {
                "SELECT row_to_json(e) FROM (SELECT me.memory_id,e.* FROM memory_entity me JOIN entity e ON e.id=me.entity_id WHERE me.memory_id=ANY($1)) e"
            }
            Table::Resource => {
                "SELECT row_to_json(r) FROM (SELECT mr.memory_id,mr.relationship,r.* FROM memory_resource mr JOIN resource r ON r.id=mr.resource_id WHERE mr.memory_id=ANY($1)) r"
            }
            Table::Relationship => {
                "SELECT row_to_json(r) FROM memory_relationship r WHERE source_memory_id=ANY($1) AND target_memory_id=ANY($1)"
            }
            _ => return Err(Error::invalid("Unsupported graph link")),
        };
        Ok(sqlx::query_scalar(sql)
            .bind(ids)
            .fetch_all(&self.pool)
            .await?)
    }
}

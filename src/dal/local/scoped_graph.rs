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
    /// Tag or entity names linked to a memory inside the scope (15.3), filtered,
    /// ordered (byte order, like the earlier Rust sort) and limited in SQL. Loading
    /// the whole scope graph was too slow (#33).
    pub async fn scoped_names(
        &self,
        r: &crate::domain::models::search::SearchRequest,
        table: Table,
        needle: &str,
        limit: usize,
    ) -> Result<Vec<Value>, Error> {
        let (rows, link, key) = match table {
            Table::Tag => ("tag", "memory_tag", "tag_id"),
            Table::Entity => ("entity", "memory_entity", "entity_id"),
            _ => return Err(Error::invalid("Unsupported name kind")),
        };
        let mut q = sqlx::QueryBuilder::new(format!(
            "SELECT row_to_json(x) FROM (SELECT DISTINCT ON (n.canonical_name COLLATE \"C\",n.id) n.* FROM {rows} n \
             JOIN {link} l ON l.{key}=n.id WHERE l.memory_id IN (SELECT m.id"
        ));
        super::search::push_scope(&mut q, r);
        q.push(") AND position(")
            .push_bind(needle.to_lowercase())
            .push(" in lower(n.canonical_name))>0 ORDER BY n.canonical_name COLLATE \"C\",n.id LIMIT ")
            .push_bind(limit as i64)
            .push(") x");
        Ok(q.build_query_scalar::<sqlx::types::Json<Value>>()
            .fetch_all(&self.pool)
            .await?
            .into_iter()
            .map(|v| v.0)
            .collect())
    }
}

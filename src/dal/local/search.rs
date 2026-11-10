use crate::{
    dal::local::store::Store,
    domain::{
        models::{error::Error, scope::UserFilter, search::SearchRequest},
        services::memories::{normalize, tag_name},
    },
};
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder};
fn filtered<'a>(r: &'a SearchRequest, prefix: &str) -> QueryBuilder<'a, Postgres> {
    let mut q = QueryBuilder::new(prefix.to_owned());
    push_scope(&mut q, r);
    q
}
/// `FROM memory m WHERE <scope and filters>` of a search request.
pub(crate) fn push_scope<'a>(q: &mut QueryBuilder<'a, Postgres>, r: &'a SearchRequest) {
    q.push(" FROM memory m WHERE workspace_id=");
    q.push_bind(normalize(&r.scope.workspace_id));
    if let Some(app) = &r.scope.app_id {
        q.push(" AND app_id=").push_bind(app);
    }
    match &r.scope.user_id {
        UserFilter::Any => {}
        UserFilter::AppOnly => {
            q.push(" AND (user_id IS NULL OR user_id='')");
        }
        UserFilter::User(user) => {
            q.push(" AND user_id=").push_bind(user);
        }
    }
    q.push(if r.historical {
        " AND status IN ('active','superseded')"
    } else {
        " AND status='active'"
    });
    for tag in &r.tags {
        let (a, b) = tag_name(tag);
        q.push(" AND EXISTS (SELECT 1 FROM memory_tag mt JOIN tag t ON t.id=mt.tag_id WHERE mt.memory_id=m.id AND t.canonical_name=").push_bind(format!("{a}:{b}")).push(")");
    }
    if !r.entity_ids.is_empty() {
        q.push(" AND EXISTS (SELECT 1 FROM memory_entity me WHERE me.memory_id=m.id AND me.entity_id=ANY(").push_bind(&r.entity_ids).push("))");
    }
    if !r.memory_types.is_empty() {
        q.push(" AND memory_type=ANY(")
            .push_bind(&r.memory_types)
            .push(")");
    }
}
impl Store {
    pub async fn search_words(&self, r: &SearchRequest) -> Result<Vec<String>, Error> {
        let mut q = filtered(r, "SELECT m.id");
        if let Some(text) = r.text.as_ref().filter(|s| !s.trim().is_empty()) {
            q.push(" AND search_document @@ plainto_tsquery('simple',")
                .push_bind(text)
                .push(") ORDER BY ts_rank(search_document,plainto_tsquery('simple',")
                .push_bind(text)
                .push(")) DESC,id");
        } else {
            q.push(" ORDER BY id");
        }
        Ok(q.build_query_scalar().fetch_all(&self.pool).await?)
    }
    pub async fn search_vectors(
        &self,
        r: &SearchRequest,
        model: &str,
        vector: &[f64],
    ) -> Result<Vec<String>, Error> {
        let mut q = filtered(r, "WITH filtered AS (SELECT m.*");
        q.push(") SELECT m.id FROM filtered m JOIN LATERAL (SELECT embedding FROM memory_embedding WHERE memory_id=m.id AND model=");
        q.push_bind(model.to_owned())
            .push(" AND model_version IS NULL AND dimensions=")
            .push_bind(vector.len() as i32)
            .push(" ORDER BY created_at DESC LIMIT 1) e ON true ORDER BY e.embedding <=> ")
            .push_bind(serde_json::to_string(vector).expect("finite vector"))
            .push("::vector,m.id");
        Ok(q.build_query_scalar().fetch_all(&self.pool).await?)
    }
    pub async fn search_candidates(
        &self,
        r: &SearchRequest,
        ids: Option<&[String]>,
    ) -> Result<Vec<Value>, Error> {
        let mut q = filtered(r, super::store::MEMORY_JSON);
        if let Some(ids) = ids {
            q.push(" AND id=ANY(").push_bind(ids).push(")");
        }
        Ok(q.build_query_scalar().fetch_all(&self.pool).await?)
    }
}

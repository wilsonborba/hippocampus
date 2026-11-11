use crate::domain::models::scope::{ReadScope, UserFilter};
use sqlx::{PgPool, Postgres, QueryBuilder};

pub fn scope_filter<'a>(query: &mut QueryBuilder<'a, Postgres>, scope: &'a ReadScope) {
    query.push("workspace_id = ").push_bind(&scope.workspace_id);
    if let Some(app) = &scope.app_id {
        query.push(" AND app_id = ").push_bind(app);
    }
    match &scope.user_id {
        UserFilter::Any => {}
        UserFilter::AppOnly => {
            query.push(" AND user_id IS NULL");
        }
        UserFilter::User(user) => {
            query.push(" AND user_id = ").push_bind(user);
        }
    }
}

pub async fn scoped_memory_ids(
    pool: &PgPool,
    scope: &ReadScope,
) -> Result<Vec<String>, sqlx::Error> {
    let mut query = QueryBuilder::new("SELECT id FROM memory WHERE ");
    scope_filter(&mut query, scope);
    query.push(" ORDER BY id");
    query.build_query_scalar().fetch_all(pool).await
}

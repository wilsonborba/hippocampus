use hippocampus::{
    dal::local::{
        store::{Store, Table},
        test_database,
    },
    domain::{
        models::api::MemoryCreateRequest,
        services::{
            memories::{Filters, MemoryService},
            scoped_graph::{graph, names},
        },
    },
};
use serde_json::{Value, json};
fn filters(w: &str, user: Option<&str>) -> Filters {
    let mut f = Filters::from([
        ("workspace_id".into(), w.into()),
        ("app_id".into(), "fixture".into()),
    ]);
    if let Some(u) = user {
        f.insert("user_id".into(), u.into());
    }
    f
}
fn ids(g: &Value) -> Vec<&str> {
    g["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["id"].as_str().unwrap())
        .collect()
}
#[tokio::test]
#[ignore = "requires hippocampus_v2_test"]
async fn shared_names_never_cross_scope() {
    dotenvy::dotenv().ok();
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let w = format!("graph-{}", uuid::Uuid::new_v4().simple());
    let mut memories = vec![];
    let mut entities = vec![];
    for (user, name, app) in [
        (Some("alice"), "Alice", "fixture"),
        (Some("bob"), "Bob", "fixture"),
        (None, "Machine", "fixture"),
        (Some("alice"), "OtherApp", "other"),
    ] {
        let req:MemoryCreateRequest=serde_json::from_value(json!({"workspace_id":w,"app_id":app,"user_id":user,"title":name,"tags":[format!("shared:{w}")]})).unwrap();
        let memory = service.create(req).await.unwrap();
        let id = memory["id"].as_str().unwrap().to_owned();
        let entity = service
            .associate_entity(
                &id,
                serde_json::from_value(
                    json!({"entity_type":"concept","canonical_name":format!("{name}-{w}")}),
                )
                .unwrap(),
                None,
            )
            .await
            .unwrap();
        entities.push(entity["id"].as_str().unwrap().to_owned());
        memories.push(id);
    }
    for i in 1..4 {
        service
            .relationship(
                &memories[0],
                serde_json::from_value(
                    json!({"target_memory_id":memories[i],"relation_type":"related_to"}),
                )
                .unwrap(),
                true,
            )
            .await
            .unwrap();
    }
    let file=service.associate_resource(&memories[0],serde_json::from_value(json!({"resource_type":"file","source_system":"fsm","external_id":format!("file-{w}"),"uri":"fixture://no-network"})).unwrap()).await.unwrap();
    let resource_id = file["id"].as_str().unwrap().to_owned();
    let tag = store.memory_tags(&memories[0]).await.unwrap()[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (index, user) in [(0, "alice"), (1, "bob"), (2, "")] {
        let f = filters(&w, Some(user));
        let g = graph(&store, &f, None).await.unwrap();
        let nodes = ids(&g);
        assert!(nodes.contains(&memories[index].as_str()));
        for (i, id) in memories.iter().enumerate() {
            assert_eq!(nodes.contains(&id.as_str()), i == index);
        }
        assert_eq!(
            g["nodes"].as_array().unwrap().len(),
            if index == 0 { 4 } else { 3 }
        );
        assert_eq!(
            g["edges"].as_array().unwrap().len(),
            if index == 0 { 3 } else { 2 }
        );
        let tags = names(&store, &f, Table::Tag).await.unwrap();
        assert_eq!(tags.as_array().unwrap().len(), 1);
        let names = names(&store, &f, Table::Entity).await.unwrap();
        assert_eq!(names.as_array().unwrap().len(), 1);
        assert_eq!(names[0]["id"], entities[index]);
        let context = graph(&store, &f, Some(&format!("tag:{tag}")))
            .await
            .unwrap();
        assert_eq!(context["node"]["node_type"], "tag");
        assert_eq!(ids(&context).len(), if index == 0 { 4 } else { 3 });
        let other = (index + 1) % 3;
        assert!(graph(&store, &f, Some(&memories[other])).await.is_err());
        assert!(
            graph(&store, &f, Some(&format!("entity:{}", entities[other])))
                .await
                .is_err()
        );
    }
    let file_node = format!("resource:{resource_id}");
    assert_eq!(
        graph(&store, &filters(&w, Some("alice")), Some(&file_node))
            .await
            .unwrap()["node"]["node_type"],
        "file"
    );
    assert!(
        graph(&store, &filters(&w, Some("bob")), Some(&file_node))
            .await
            .is_err()
    );
    let f = filters(&w, None);
    let whole = graph(&store, &f, None).await.unwrap();
    assert_eq!(whole["nodes"].as_array().unwrap().len(), 8);
    assert_eq!(whole["edges"].as_array().unwrap().len(), 9);
    let mut zero = f.clone();
    zero.insert("depth".into(), "0".into());
    let context = graph(&store, &zero, Some(&memories[0])).await.unwrap();
    assert_eq!(ids(&context).len(), 4);
    let mut one = f.clone();
    one.insert("depth".into(), "1".into());
    let context = graph(&store, &one, Some(&memories[0])).await.unwrap();
    assert_eq!(ids(&context).len(), 8);
    let mut limited = f.clone();
    limited.insert("limit".into(), "1".into());
    assert_eq!(
        graph(&store, &limited, None).await.unwrap()["truncated"],
        true
    );
    let mut query = f.clone();
    query.insert("q".into(), "Bob".into());
    assert_eq!(
        names(&store, &query, Table::Entity)
            .await
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        1
    );
    query.insert("user_id".into(), "alice".into());
    assert_eq!(
        names(&store, &query, Table::Entity).await.unwrap(),
        json!([])
    );
    assert!(graph(&store, &Filters::new(), None).await.is_err());
    assert!(names(&store, &Filters::new(), Table::Tag).await.is_err());
    {
        use axum::{
            body::{Body, to_bytes},
            http::{Request, StatusCode},
        };
        use hippocampus::{
            dal::remote::{documents::Documents, embeddings::DisabledEmbedding, files::FsmClient},
            presentation::api::{AppState, router},
        };
        use std::sync::Arc;
        use tower::ServiceExt;
        let app = router(AppState {
            embedding: Arc::new(DisabledEmbedding),
            filestore: Arc::new(FsmClient::default()),
            service: service.clone(),
            documents: Documents::default(),
            api_keys: Default::default(),
        });
        for path in [
            "graph".to_owned(),
            "tags/search".into(),
            "entities/search".into(),
            format!("graph/nodes/tag:{tag}"),
        ] {
            let uri = format!("/api/v1/{path}?workspace_id={w}&app_id=fixture&user_id=alice");
            let response = app
                .clone()
                .oneshot(Request::get(uri).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            let output: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap())
                    .unwrap();
            assert!(!output.to_string().contains(&entities[1]));
            assert!(!output.to_string().contains(&memories[1]));
        }
        let response = app
            .oneshot(
                Request::get(format!(
                    "/api/v1/graph/nodes/{}?workspace_id={w}&app_id=fixture&user_id=alice",
                    memories[1]
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    sqlx::query("DELETE FROM memory_relationship WHERE source_memory_id=ANY($1) OR target_memory_id=ANY($1)").bind(&memories).execute(&pool).await.unwrap();
    for t in [
        "memory_tag",
        "memory_entity",
        "memory_event",
        "memory_resource",
    ] {
        sqlx::query(&format!("DELETE FROM {t} WHERE memory_id=ANY($1)"))
            .bind(&memories)
            .execute(&pool)
            .await
            .unwrap();
    }
    sqlx::query("DELETE FROM memory WHERE id=ANY($1)")
        .bind(&memories)
        .execute(&pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn graph_and_names_require_scope_before_database_access() {
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use hippocampus::{
        dal::remote::{documents::Documents, embeddings::DisabledEmbedding, files::FsmClient},
        presentation::api::{AppState, router},
    };
    use std::sync::Arc;
    use tower::ServiceExt;
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://localhost/hippocampus_v2_test")
        .unwrap();
    let app = router(AppState {
        embedding: Arc::new(DisabledEmbedding),
        filestore: Arc::new(FsmClient::default()),
        service: MemoryService {
            store: Store { pool },
        },
        documents: Documents::default(),
        api_keys: Default::default(),
    });
    for path in [
        "graph",
        "tags/search",
        "entities/search",
        "graph/nodes/private",
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!("/api/v1/{path}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }
}

#[tokio::test]
#[ignore = "requires hippocampus_v2_test; unique workspace only"]
async fn scoped_depth_three_preserves_legacy_limit_and_user_boundary() {
    dotenvy::dotenv().ok();
    let config = hippocampus::core::config::Config::default();
    assert_eq!(config.graph_max_depth, 2);
    assert_eq!(config.scope_graph_max_depth, 3);
    let pool = test_database::connect().await.unwrap();
    let store = Store { pool: pool.clone() };
    let service = MemoryService {
        store: store.clone(),
    };
    let w = format!("depth-{}", uuid::Uuid::new_v4().simple());
    let mut memories = vec![];
    for i in 0..5 {
        let request:MemoryCreateRequest=serde_json::from_value(json!({"workspace_id":w,"app_id":"fixture","user_id":if i==4 {"bob"}else{"alice"},"title":format!("node-{i}")})).unwrap();
        memories.push(
            service.create(request).await.unwrap()["id"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    for pair in memories.windows(2) {
        service
            .relationship(
                &pair[0],
                serde_json::from_value(
                    json!({"target_memory_id":pair[1],"relation_type":"related_to"}),
                )
                .unwrap(),
                true,
            )
            .await
            .unwrap();
    }
    let mut f = filters(&w, Some("alice"));
    f.insert("depth".into(), "2".into());
    let two = graph(&store, &f, Some(&memories[0])).await.unwrap();
    assert_eq!(ids(&two).len(), 3);
    f.insert("depth".into(), "3".into());
    let three = graph(&store, &f, Some(&memories[0])).await.unwrap();
    assert_eq!(ids(&three).len(), 4);
    assert!(ids(&three).contains(&memories[3].as_str()));
    assert!(!ids(&three).contains(&memories[4].as_str()));
    assert!(graph(&store, &f, None).await.is_ok());
    f.insert("depth".into(), "4".into());
    assert!(graph(&store, &f, None).await.is_err());
    sqlx::query("DELETE FROM memory_relationship WHERE source_memory_id=ANY($1) OR target_memory_id=ANY($1)").bind(&memories).execute(&pool).await.unwrap();
    sqlx::query("DELETE FROM memory_event WHERE memory_id=ANY($1)")
        .bind(&memories)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM memory WHERE id=ANY($1)")
        .bind(&memories)
        .execute(&pool)
        .await
        .unwrap();
}

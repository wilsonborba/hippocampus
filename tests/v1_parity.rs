use reqwest::Client;
use serde_json::{Value, json};
async fn call(
    c: &Client,
    base: &str,
    method: &str,
    path: &str,
    body: Option<Value>,
    expected: u16,
) -> Value {
    let mut request = c.request(method.parse().unwrap(), format!("{base}{path}"));
    if let Some(body) = body {
        request = request.json(&body);
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let text = response.text().await.unwrap();
    assert_eq!(status, expected, "{base} {method} {path}: {text}");
    if status == 204 {
        return Value::Null;
    }
    normalize_timestamps(serde_json::from_str(&text).unwrap())
}
#[tokio::test]
#[ignore = "requires isolated v1 on 28001 and v2 on 18001"]
async fn unchanged_http_contracts() {
    for base in ["http://127.0.0.1:28001", "http://127.0.0.1:18001"] {
        let c = Client::new();
        let ready = call(&c, base, "GET", "/ready", None, 200).await;
        assert_eq!(ready["status"], "ready");
        assert_eq!(ready["dependencies"]["postgresql"], "ok");
        let workspace = format!("parity-{}", uuid::Uuid::new_v4().simple());
        assert_eq!(
            call(&c, base, "GET", "/health", None, 200).await,
            json!({"status":"ok"})
        );
        let a=call(&c,base,"POST","/api/v1/memories",Some(json!({"workspace_id":workspace,"app_id":"codex","content":"Immutable text","title":"First","tags":["Topic:Rust"],"entities":[{"entity_type":"project","canonical_name":"Cortex"}],"resources":[{"resource_type":"url","source_system":"web","uri":format!("https://example.com/{workspace}") }]})),201).await;
        assert_eq!(a["data"]["memory_type"], "semantic");
        assert_eq!(a["data"]["access_count"], 0);
        assert!(a["pagination"].is_null());
        let id = a["data"]["id"].as_str().unwrap();
        let path = format!("/api/v1/memories/{id}");
        let b=call(&c,base,"POST","/api/v1/memories",Some(json!({"workspace_id":workspace,"app_id":"codex","title":"Second","memory_type":"decision"})),201).await;
        let bid = b["data"]["id"].as_str().unwrap();
        assert_eq!(
            call(&c, base, "GET", &path, None, 200).await["data"]["access_count"],
            0
        );
        assert_eq!(
            call(&c, base, "GET", &path, None, 200).await["data"]["access_count"],
            1
        );
        let list = call(
            &c,
            base,
            "GET",
            &format!("/api/v1/memories?workspace_id={workspace}&tag=topic:rust"),
            None,
            200,
        )
        .await;
        assert_eq!(list["data"].as_array().unwrap().len(), 1);
        let updated = call(
            &c,
            base,
            "PATCH",
            &path,
            Some(json!({"title":"Renamed","content":"Cannot replace","importance":0.7})),
            200,
        )
        .await;
        assert_eq!(updated["data"]["content"], "Immutable text");
        assert_eq!(updated["data"]["title"], "Renamed");
        let tags = call(
            &c,
            base,
            "POST",
            &format!("{path}/tags"),
            Some(json!({"tags":["Machine Learning"]})),
            200,
        )
        .await;
        assert_eq!(
            tags["data"][0]["canonical_name"],
            "general:machine-learning"
        );
        let tid = tags["data"][0]["id"].as_str().unwrap();
        call(&c, base, "DELETE", &format!("{path}/tags/{tid}"), None, 204).await;
        let entity = call(
            &c,
            base,
            "POST",
            &format!("{path}/entities"),
            Some(json!({"entity_type":"project","canonical_name":"Parity"})),
            200,
        )
        .await;
        let eid = entity["data"]["id"].as_str().unwrap();
        assert_eq!(
            call(
                &c,
                base,
                "GET",
                &format!("/api/v1/entities/{eid}"),
                None,
                200
            )
            .await["data"]["canonical_name"],
            "Parity"
        );
        let body = json!({"resource_type":"url","source_system":"web","uri":format!("https://example.com/extra/{workspace}")});
        let resource = call(
            &c,
            base,
            "POST",
            "/api/v1/resources",
            Some(body.clone()),
            201,
        )
        .await;
        let duplicate = call(
            &c,
            base,
            "POST",
            "/api/v1/resources",
            Some(body.clone()),
            201,
        )
        .await;
        assert_eq!(resource["data"]["id"], duplicate["data"]["id"]);
        let rid = resource["data"]["id"].as_str().unwrap();
        assert_eq!(
            call(
                &c,
                base,
                "GET",
                &format!("/api/v1/resources/{rid}"),
                None,
                200
            )
            .await["data"]["id"],
            resource["data"]["id"]
        );
        call(
            &c,
            base,
            "POST",
            &format!("{path}/resources"),
            Some(body),
            200,
        )
        .await;
        let relation = json!({"target_memory_id":bid,"relation_type":"related_to"});
        let edge = call(
            &c,
            base,
            "POST",
            &format!("{path}/relationships"),
            Some(relation.clone()),
            201,
        )
        .await;
        assert_eq!(
            edge,
            call(
                &c,
                base,
                "POST",
                &format!("{path}/relationships"),
                Some(relation),
                201
            )
            .await
        );
        assert_eq!(
            call(&c, base, "GET", &format!("{path}/neighbors"), None, 200).await["data"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            call(
                &c,
                base,
                "POST",
                &format!("{path}/reinforce"),
                Some(json!({"reason":"test"})),
                200
            )
            .await["data"]["reinforcement_count"],
            1
        );
        call(
            &c,
            base,
            "POST",
            &format!("{path}/supersede"),
            Some(json!({"new_memory_id":bid})),
            200,
        )
        .await;
        assert_eq!(
            call(&c, base, "GET", &path, None, 200).await["data"]["status"],
            "superseded"
        );
        call(&c, base, "POST", &format!("{path}/restore"), None, 200).await;
        call(
            &c,
            base,
            "POST",
            &format!("{path}/correct"),
            Some(json!({"new_memory_id":bid})),
            200,
        )
        .await;
        call(&c, base, "POST", &format!("{path}/archive"), None, 200).await;
        call(
            &c,
            base,
            "POST",
            &format!("{path}/forget?workspace_id=other"),
            Some(json!({})),
            404,
        )
        .await;
        call(
            &c,
            base,
            "POST",
            &format!("{path}/forget?workspace_id={workspace}"),
            Some(json!({"reason":"test"})),
            200,
        )
        .await;
        let visible = call(
            &c,
            base,
            "GET",
            &format!("/api/v1/memories?workspace_id={workspace}"),
            None,
            200,
        )
        .await;
        assert!(
            visible["data"]
                .as_array()
                .unwrap()
                .iter()
                .all(|m| m["id"] != id)
        );
        assert_eq!(
            call(
                &c,
                base,
                "GET",
                &format!("/api/v1/memories?workspace_id={workspace}&status=forgotten"),
                None,
                200
            )
            .await["data"][0]["id"],
            id
        );
        assert_eq!(
            call(&c, base, "POST", &format!("{path}/restore"), None, 200).await["data"]["status"],
            "active"
        );
        let events = call(&c, base, "GET", &format!("{path}/events"), None, 200).await;
        assert_eq!(events["data"].as_array().unwrap().len(), 13);
        assert_eq!(
            events["data"][1]["payload"]["fields"],
            json!(["title", "importance"])
        );
        assert!(
            call(&c, base, "GET", &format!("{path}/document"), None, 200).await["data"].is_null()
        );
        let error = call(
            &c,
            base,
            "GET",
            "/api/v1/memories/missing-parity-id",
            None,
            404,
        )
        .await;
        assert_eq!(
            error,
            json!({"error":{"code":"memory_not_found","message":"memory 'missing-parity-id' not found","details":{}}})
        );
    }
}

// SQLite drops timezone offsets when it reads v1 rows. Compare UTC values.
fn normalize_timestamps(mut value: Value) -> Value {
    match &mut value {
        Value::Object(map) => {
            for (key, v) in map {
                if key.ends_with("_at") {
                    if let Some(text) = v.as_str() {
                        *v = json!(text.trim_end_matches('Z').trim_end_matches("+00:00"));
                    }
                } else {
                    *v = normalize_timestamps(v.take());
                }
            }
        }
        Value::Array(values) => {
            for v in values {
                *v = normalize_timestamps(v.take());
            }
        }
        _ => {}
    }
    value
}
#[tokio::test]
#[ignore = "requires isolated v1 on 28001 and v2 on 18001"]
async fn graph_formats_and_validation() {
    for base in ["http://127.0.0.1:28001", "http://127.0.0.1:18001"] {
        let c = Client::new();
        let memory=call(&c,base,"POST","/api/v1/memories",Some(json!({"app_id":"codex","workspace_id":"parity-graphs","title":"Graph root","tags":["topic:rust"]})),201).await;
        let id = memory["data"]["id"].as_str().unwrap();
        let path = format!("/api/v1/memories/{id}/graph");
        let graph = call(&c, base, "GET", &path, None, 200).await;
        assert_eq!(graph["data"]["root_ids"], json!([id]));
        assert_eq!(graph["data"]["nodes"].as_array().unwrap().len(), 2);
        assert_eq!(graph["data"]["edges"][0]["edge_type"], "tagged_with");
        assert_eq!(graph["data"]["truncated"], false);
        for (format, content_type) in [
            ("d2", "text/vnd.d2"),
            ("mermaid", "text/vnd.mermaid"),
            ("svg", "image/svg+xml"),
            ("png", "image/png"),
        ] {
            let response = c
                .get(format!("{base}{path}?format={format}"))
                .send()
                .await
                .unwrap();
            assert_eq!(response.status().as_u16(), 200, "{base} {format}");
            assert!(
                response.headers()["content-type"]
                    .to_str()
                    .unwrap()
                    .starts_with(content_type)
            );
            let bytes = response.bytes().await.unwrap();
            assert!(!bytes.is_empty());
            if format == "png" {
                assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            }
            if format == "svg" {
                assert!(std::str::from_utf8(&bytes).unwrap().contains("<svg"));
            }
            if format == "d2" {
                assert!(
                    std::str::from_utf8(&bytes)
                        .unwrap()
                        .contains("shape: diamond")
                );
            }
            if format == "mermaid" {
                assert!(std::str::from_utf8(&bytes).unwrap().starts_with("graph TD"));
            }
        }
        let error = call(
            &c,
            base,
            "GET",
            &format!("{path}?format=unsupported"),
            None,
            400,
        )
        .await;
        assert_eq!(error["error"]["code"], "unsupported_render_format");
        assert_eq!(
            call(
                &c,
                base,
                "GET",
                &format!("{path}?include_tags=False"),
                None,
                200
            )
            .await["data"]["nodes"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        let invalid = call(
            &c,
            base,
            "POST",
            "/api/v1/memories",
            Some(json!({"app_id":"codex","content":4})),
            422,
        )
        .await;
        assert_eq!(
            invalid,
            json!({"detail":[{"type":"string_type","loc":["body","content"],"msg":"Input should be a valid string","input":4}]})
        );
        let invalid = call(&c, base, "GET", &format!("{path}?depth=bad"), None, 422).await;
        assert_eq!(invalid["detail"][0]["type"], "int_parsing");
        assert_eq!(
            call(&c, base, "GET", "/not-a-route", None, 404).await,
            json!({"detail":"Not Found"})
        );
        assert_eq!(
            call(&c, base, "POST", "/health", None, 405).await,
            json!({"detail":"Method Not Allowed"})
        );
        assert_eq!(
            c.head(format!("{base}/health"))
                .send()
                .await
                .unwrap()
                .status()
                .as_u16(),
            405
        );
        let response = c
            .request(reqwest::Method::OPTIONS, format!("{base}/api/v1/memories"))
            .header("origin", "http://example.com")
            .header("access-control-request-method", "POST")
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(
            response.headers()["access-control-allow-origin"],
            "http://example.com"
        );
    }
}

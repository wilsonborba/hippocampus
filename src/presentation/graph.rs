use crate::domain::models::error::Error;
use axum::{
    body::Body,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
};
use serde_json::Value;
use std::process::Stdio;
use tokio::io::AsyncWriteExt;
fn quoted(s: &str) -> String {
    serde_json::to_string(s).expect("string")
}
pub async fn render(graph: Value, format: &str) -> Result<Response, Error> {
    let nodes = graph["nodes"].as_array().expect("nodes");
    let edges = graph["edges"].as_array().expect("edges");
    if format == "json" {
        return Ok(axum::Json(serde_json::json!({"data":graph,"pagination":null})).into_response());
    }
    if format == "d2" {
        let mut lines = vec![];
        for n in nodes {
            let shape = match n["node_type"].as_str().unwrap() {
                "entity" => "oval",
                "tag" => "diamond",
                "resource" => "hexagon",
                "cluster" => "package",
                _ => "rectangle",
            };
            lines.push(format!(
                "{}: {} {{shape: {shape}}}",
                quoted(n["id"].as_str().unwrap()),
                quoted(n["label"].as_str().unwrap())
            ));
        }
        for e in edges {
            lines.push(format!(
                "{} -> {}: {}",
                quoted(e["source_id"].as_str().unwrap()),
                quoted(e["target_id"].as_str().unwrap()),
                e["edge_type"].as_str().unwrap()
            ));
        }
        return Ok((
            [(header::CONTENT_TYPE, "text/vnd.d2; charset=utf-8")],
            lines.join("\n"),
        )
            .into_response());
    }
    if format == "mermaid" {
        let ids: std::collections::HashMap<_, _> = nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n["id"].as_str().unwrap(), format!("n{i}")))
            .collect();
        let mut lines = vec!["graph TD".to_owned()];
        for n in nodes {
            let (a, b) = match n["node_type"].as_str().unwrap() {
                "entity" => ("([", "])"),
                "tag" => ("{", "}"),
                "resource" => ("[[", "]]"),
                "cluster" => ("[/", "/]"),
                _ => ("[", "]"),
            };
            lines.push(format!(
                "{}{a}\"{}\"{b}",
                ids[n["id"].as_str().unwrap()],
                n["label"].as_str().unwrap().replace('"', "'")
            ));
        }
        for e in edges {
            if let (Some(a), Some(b)) = (
                ids.get(e["source_id"].as_str().unwrap()),
                ids.get(e["target_id"].as_str().unwrap()),
            ) {
                lines.push(format!("{a} -->|{}| {b}", e["edge_type"].as_str().unwrap()));
            }
        }
        return Ok((
            [(header::CONTENT_TYPE, "text/vnd.mermaid; charset=utf-8")],
            lines.join("\n"),
        )
            .into_response());
    }
    if !["svg", "png"].contains(&format) {
        return Err(Error::new(
            StatusCode::BAD_REQUEST,
            "unsupported_render_format",
            format!("unsupported format: '{format}'"),
        ));
    }
    // Graphviz is a native graph renderer. Pass graph data on stdin, never to a shell.
    let mut source = String::from(
        "digraph memory { graph [rankdir=TB]; node [style=filled,fontname=\"sans-serif\",fontsize=12];\n",
    );
    for n in nodes {
        let (shape, fill, stroke) = match n["node_type"].as_str().unwrap() {
            "entity" => ("ellipse", "#eafbea", "#16a34a"),
            "tag" => ("diamond", "#fff4e0", "#d97706"),
            "resource" => ("hexagon", "#f3e8ff", "#7c3aed"),
            "cluster" => ("box", "#f1f5f9", "#64748b"),
            _ => ("box", "#e6f0ff", "#2563eb"),
        };
        source += &format!(
            "{} [label={},shape={shape},fillcolor=\"{fill}\",color=\"{stroke}\"];\n",
            quoted(n["id"].as_str().unwrap()),
            quoted(n["label"].as_str().unwrap())
        );
    }
    for e in edges {
        source += &format!(
            "{} -> {} [color=\"#2563eb\"];\n",
            quoted(e["source_id"].as_str().unwrap()),
            quoted(e["target_id"].as_str().unwrap())
        );
    }
    source += "}";
    let mut child = tokio::process::Command::new("dot")
        .arg(format!("-T{format}"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| {
            Error::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The graph renderer could not start",
            )
        })?;
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(source.as_bytes())
        .await
        .map_err(|_| {
            Error::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal_error",
                "The graph renderer could not read the graph",
            )
        })?;
    let output = tokio::time::timeout(
        crate::core::config::Config::default().graph_render_timeout,
        child.wait_with_output(),
    )
    .await
    .map_err(|_| {
        Error::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The graph renderer timed out",
        )
    })?
    .map_err(|_| {
        Error::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The graph renderer failed",
        )
    })?;
    if !output.status.success() {
        return Err(Error::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "The graph renderer failed",
        ));
    }
    let content_type = if format == "svg" {
        "image/svg+xml"
    } else {
        "image/png"
    };
    Ok((
        [(header::CONTENT_TYPE, content_type)],
        Body::from(output.stdout),
    )
        .into_response())
}

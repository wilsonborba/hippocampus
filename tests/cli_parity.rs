use serde_json::{Value, json};
use std::process::Command;
fn invoke(v1: bool, args: &[&str]) -> Value {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut cmd = if v1 {
        let mut cmd = Command::new(root.parent().unwrap().join("hippocampus/.venv/bin/python"));
        cmd.args(["-c", "from lib.dal.local.database import Base, get_engine; import lib.dal.models; from lib.dal.migrations import get_alembic_config; from alembic.command import stamp; Base.metadata.create_all(get_engine()); stamp(get_alembic_config(), 'head'); from lib.presentation.cli.main import app; app()"]);
        cmd.env(
            "HIPPOCAMPUS_DATABASE_URL",
            format!(
                "sqlite:///{}",
                std::env::temp_dir()
                    .join(format!("hippocampus-cli-{}.sqlite", std::process::id()))
                    .display()
            ),
        )
        .env("HIPPOCAMPUS_REDIS_URL", "")
        .env("HIPPOCAMPUS_COUCHDB_URL", "")
        .env("HIPPOCAMPUS_FILESTORE_URL", "")
        .env("HIPPOCAMPUS_API_KEYS", "{}");
        cmd
    } else {
        Command::new(env!("CARGO_BIN_EXE_hippocampus"))
    };
    let out = cmd
        .current_dir(root)
        .arg("--json")
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "CLI command failed, v1={v1}, command={args:?}, exit={}",
        out.status
    );
    if args.starts_with(&["db", "upgrade"]) {
        return Value::Null;
    }
    serde_json::from_slice(&out.stdout).expect("CLI output must be JSON")
}
#[test]
#[ignore = "requires the isolated PostgreSQL test database and the local v1 Python environment"]
fn unchanged_cli_json_contracts() {
    for v1 in [true, false] {
        invoke(v1, &["db", "upgrade"]);
        let workspace = format!("cli-parity-{}", uuid::Uuid::new_v4().simple());
        let a = invoke(
            v1,
            &[
                "memory",
                "remember",
                "--workspace",
                &workspace,
                "--content",
                "Immutable CLI text",
                "--title",
                "First",
                "--tag",
                "topic:rust",
            ],
        );
        assert_eq!(a["workspace_id"], workspace);
        assert_eq!(a["memory_type"], "semantic");
        let id = a["id"].as_str().unwrap();
        let b = invoke(
            v1,
            &[
                "memory",
                "remember",
                "--workspace",
                &workspace,
                "--title",
                "Second",
            ],
        );
        let bid = b["id"].as_str().unwrap();
        let memory = invoke(v1, &["memory", "get", id]);
        assert_eq!(memory["content"], "Immutable CLI text");
        assert_eq!(memory.as_object().unwrap().len(), 9);
        assert_eq!(
            invoke(v1, &["memory", "list", "--workspace", &workspace])
                .as_array()
                .unwrap()
                .len(),
            2
        );
        assert_eq!(
            invoke(v1, &["memory", "tag", id, "Machine Learning"]),
            json!(["general:machine-learning"])
        );
        let graph = invoke(v1, &["memory", "graph", id]);
        let tag = graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|n| n["label"] == "general:machine-learning")
            .unwrap()["id"]
            .as_str()
            .unwrap()
            .trim_start_matches("tag:");
        assert_eq!(
            invoke(v1, &["memory", "untag", id, tag]),
            json!({"removed":true})
        );
        assert_eq!(
            invoke(v1, &["memory", "link", id, "related_to", bid])["relation_type"],
            "related_to"
        );
        assert_eq!(
            invoke(v1, &["memory", "reinforce", id, "--reason", "Fixture"])["reinforcement_count"],
            1
        );
        assert_eq!(
            invoke(v1, &["memory", "supersede", id, "--with", bid]),
            json!({"memory_id":id,"status":"superseded"})
        );
        assert_eq!(
            invoke(v1, &["memory", "correct", id, "--with", bid]),
            json!({"memory_id":id,"status":"superseded"})
        );
        assert_eq!(
            invoke(v1, &["memory", "archive", id]),
            json!({"memory_id":id,"status":"archived"})
        );
        assert_eq!(
            invoke(v1, &["memory", "forget", id]),
            json!({"memory_id":id,"status":"forgotten"})
        );
        assert_eq!(
            invoke(v1, &["memory", "list", "--workspace", &workspace])
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(invoke(v1, &["health"])["status"], "ready");
        assert_eq!(
            invoke(
                v1,
                &[
                    "consolidate",
                    "--tag",
                    "topic:rust",
                    "--workspace",
                    &workspace
                ]
            ),
            json!({"candidate_duplicates":[],"candidate_contradictions":[],"applied_changes":[]})
        );
        let expired = invoke(v1, &["admin", "expire"]);
        assert!(expired["expired_memory_ids"].is_array());
        let synced = invoke(v1, &["admin", "sync-documents", "--limit", "1"]);
        for key in ["processed", "synced", "skipped_no_document", "failed"] {
            assert!(synced[key].is_number());
        }
        let reconciled = invoke(v1, &["admin", "reconcile", "--dry-run"]);
        assert!(reconciled["missing_document"].is_array());
        assert!(reconciled["missing_embedding"].is_array());
    }
}

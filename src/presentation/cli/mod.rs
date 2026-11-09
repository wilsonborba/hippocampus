use crate::{
    core::config::Config,
    domain::{
        models::{api::*, error::Error},
        services::memories::Filters,
        tasks::{
            scheduler::{ScheduledJob, run as run_scheduler},
            workers,
        },
    },
    presentation::api::AppState,
};
use clap::{Args, Parser, Subcommand};
use serde_json::{Value, json};
use std::{
    io::{self, IsTerminal, Read},
    path::PathBuf,
    sync::Arc,
};

#[derive(Parser)]
#[command(
    name = "hippocampus",
    version,
    about = "Dedicated memory service",
    arg_required_else_help = true
)]
pub struct Cli {
    #[arg(long = "json", global = true)]
    json: bool,
    #[arg(long, short = 'v', global = true)]
    verbose: bool,
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    Search(SearchArgs),
    Recall(RecallArgs),
    Health,
    Db {
        #[command(subcommand)]
        command: DbCommand,
    },
    Scheduler {
        #[command(subcommand)]
        command: SchedulerCommand,
    },
    Admin {
        #[command(subcommand)]
        command: AdminCommand,
    },
    Consolidate(ConsolidateArgs),
}
#[derive(Subcommand)]
enum DbCommand {
    Upgrade,
}
#[derive(Subcommand)]
enum SchedulerCommand {
    Run,
}
#[derive(Subcommand)]
enum AdminCommand {
    Reembed {
        #[arg(long,default_value_t=Config::default().embedding.batch_limit)]
        limit: i64,
    },
    Reconcile {
        #[arg(long, conflicts_with = "apply")]
        dry_run: bool,
        #[arg(long)]
        apply: bool,
    },
    Expire,
    SyncDocuments {
        #[arg(long, default_value_t = 50)]
        limit: i64,
    },
}
#[derive(Args)]
struct Id {
    memory_id: String,
}
#[derive(Args)]
struct Replace {
    old_memory_id: String,
    #[arg(long = "with")]
    with_id: String,
}
#[derive(Args)]
struct ListArgs {
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long)]
    status: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long, default_value_t = 20)]
    limit: i64,
}
#[derive(Args)]
struct SearchArgs {
    query: String,
    #[arg(long)]
    workspace: String,
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long, default_value_t = 20)]
    limit: usize,
}
#[derive(Args)]
struct RecallArgs {
    query: String,
    #[arg(long)]
    workspace: String,
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    historical: bool,
    #[arg(long, default_value_t = 10)]
    limit: usize,
}
#[derive(Args)]
struct ConsolidateArgs {
    #[arg(long)]
    tag: Option<String>,
    #[arg(long)]
    workspace: Option<String>,
    #[arg(long = "type")]
    kind: Option<String>,
    #[arg(long, default_value_t = 50)]
    limit: i64,
    #[arg(long)]
    apply: bool,
}
#[derive(Subcommand)]
enum MemoryCommand {
    Remember {
        #[arg(long)]
        content: Option<String>,
        #[arg(long)]
        content_file: Option<PathBuf>,
        #[arg(long)]
        stdin: bool,
        #[arg(long = "type")]
        kind: Option<String>,
        #[arg(long, default_value = "default")]
        workspace: String,
        #[arg(long)]
        title: Option<String>,
        #[arg(long)]
        summary: Option<String>,
        #[arg(long)]
        importance: Option<f64>,
        #[arg(long)]
        confidence: Option<f64>,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    Delete {
        memory_id: String,
        #[arg(long)]
        hard: bool,
        #[arg(long)]
        yes: bool,
        #[arg(long)]
        workspace: String,
        #[arg(long, default_value = "hippocampus")]
        app: String,
        #[arg(long)]
        user: Option<String>,
    },
    Get(Id),
    List(ListArgs),
    Tag {
        memory_id: String,
        #[arg(required = true)]
        tags: Vec<String>,
    },
    Untag {
        memory_id: String,
        tag_id: String,
    },
    Link {
        source_memory_id: String,
        relation: String,
        target_memory_id: String,
    },
    Reinforce {
        memory_id: String,
        #[arg(long)]
        reason: Option<String>,
    },
    Supersede(Replace),
    Correct(Replace),
    Archive(Id),
    Forget(Id),
    Graph {
        memory_id: String,
        #[arg(long)]
        depth: Option<i64>,
        #[arg(long)]
        relation: Option<String>,
        #[arg(long,action=clap::ArgAction::SetTrue,overrides_with="no_entities")]
        include_entities: bool,
        #[arg(long,action=clap::ArgAction::SetTrue)]
        no_entities: bool,
        #[arg(long,action=clap::ArgAction::SetTrue,overrides_with="no_tags")]
        include_tags: bool,
        #[arg(long,action=clap::ArgAction::SetTrue)]
        no_tags: bool,
        #[arg(long,action=clap::ArgAction::SetTrue,overrides_with="no_resources")]
        include_resources: bool,
        #[arg(long,action=clap::ArgAction::SetTrue)]
        no_resources: bool,
        #[arg(long, default_value_t = 300)]
        max_nodes: i64,
        #[arg(long, default_value = "json")]
        format: String,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}
fn pick(memory: &Value, fields: &[&str]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|k| (k.to_string(), memory[*k].clone()))
            .collect(),
    )
}
fn emit(value: &Value, json_output: bool, title: Option<&str>) {
    if json_output {
        println!(
            "{}",
            serde_json::to_string_pretty(value).expect("JSON output")
        );
    } else if let Some(title) = title {
        println!("{title}");
        let rows = if let Some(array) = value.as_array() {
            array.clone()
        } else {
            vec![value.clone()]
        };
        for row in rows {
            match row {
                Value::Object(map) => println!(
                    "{}",
                    map.values()
                        .map(|v| v
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()))
                        .collect::<Vec<_>>()
                        .join(" | ")
                ),
                _ => println!("{row}"),
            }
        }
    }
}
fn io_error(_: impl std::fmt::Display) -> Error {
    Error::invalid("The input or output operation failed")
}

pub async fn execute(cli: Cli, state: AppState) -> Result<(), Error> {
    let json_output = cli.json || !io::stdout().is_terminal();
    let service = &state.service;
    let config = Config::default();
    match cli.command {
        Command::Search(args) => {
            let request = serde_json::from_value(json!({"workspace_id": args.workspace, "text": args.query, "tags": args.tag.into_iter().collect::<Vec<_>>(), "memory_types": args.kind.into_iter().collect::<Vec<_>>(), "limit": args.limit})).expect("search request");
            let rows = crate::domain::services::search::search(
                &service.store,
                state.embedding.as_ref(),
                request,
            )
            .await?;
            let output = Value::Array(
                rows.as_array()
                    .expect("search array")
                    .iter()
                    .map(|r| {
                        pick(
                            &r["memory"],
                            &["id", "workspace_id", "memory_type", "status"],
                        )
                    })
                    .collect(),
            );
            emit(&output, json_output, Some("Search results"));
        }
        Command::Recall(args) => {
            let request = serde_json::from_value(json!({"workspace_id": args.workspace, "text": args.query, "tags": args.tag.into_iter().collect::<Vec<_>>(), "historical": args.historical, "limit": args.limit})).expect("recall request");
            let rows = crate::domain::services::search::search(
                &service.store,
                state.embedding.as_ref(),
                request,
            )
            .await?;
            let output = Value::Array(rows.as_array().expect("search array").iter().map(|r| json!({"id":r["memory"]["id"], "score":r["score"], "match_reasons":r["match_reasons"]})).collect());
            emit(&output, json_output, Some("Recall results"));
        }
        Command::Memory { command } => match command {
            MemoryCommand::Remember {
                mut content,
                content_file,
                stdin,
                kind,
                workspace,
                title,
                summary,
                importance,
                confidence,
                tags,
            } => {
                if stdin {
                    let mut text = String::new();
                    io::stdin().read_to_string(&mut text).map_err(io_error)?;
                    content = Some(text);
                } else if let Some(path) = content_file {
                    content = Some(tokio::fs::read_to_string(path).await.map_err(io_error)?);
                }
                let request:MemoryCreateRequest=serde_json::from_value(json!({"content":content,"memory_type":kind,"workspace_id":workspace,"app_id":config.cli_app_id,"title":title,"summary":summary,"importance":importance,"confidence":confidence,"tags":tags})).expect("CLI request");
                let memory = service.create(request).await?;
                emit(
                    &pick(&memory, &["id", "workspace_id", "memory_type", "status"]),
                    json_output,
                    Some("Memory created"),
                );
            }
            MemoryCommand::Delete {
                memory_id,
                hard,
                yes,
                workspace,
                app,
                user,
            } => {
                if !hard {
                    return Err(Error::invalid("Use memory forget or pass --hard"));
                }
                if !yes {
                    if !io::stdin().is_terminal() {
                        return Err(Error::invalid("Pass --yes for non-interactive hard delete"));
                    }
                    eprintln!("Permanently delete memory {memory_id}? [y/N]");
                    let mut answer = String::new();
                    io::stdin().read_line(&mut answer).map_err(io_error)?;
                    if !["y", "yes"].contains(&answer.trim().to_lowercase().as_str()) {
                        return Err(Error::invalid("Delete cancelled"));
                    }
                }
                let request = crate::domain::models::deletion::DeleteRequest {
                    scope: crate::domain::models::scope::WriteScope {
                        workspace_id: workspace,
                        app_id: app,
                        user_id: user,
                    },
                    memory_ids: vec![memory_id.clone()],
                };
                crate::domain::services::deletion::delete(
                    &service.store,
                    state.filestore.as_ref(),
                    &state.documents,
                    request,
                )
                .await?;
                emit(
                    &json!({"memory_id":memory_id,"status":"deleted"}),
                    json_output,
                    Some("Memory deleted"),
                );
            }
            MemoryCommand::Get(Id { memory_id }) => {
                let memory = service.get(&memory_id, &Filters::new(), true).await?;
                emit(
                    &pick(
                        &memory,
                        &[
                            "id",
                            "memory_type",
                            "status",
                            "workspace_id",
                            "title",
                            "summary",
                            "content",
                            "importance",
                            "confidence",
                        ],
                    ),
                    json_output,
                    Some("Memory"),
                );
            }
            MemoryCommand::List(args) => {
                let mut filters = Filters::from([("limit".into(), args.limit.to_string())]);
                for (key, val) in [
                    ("workspace_id", args.workspace),
                    ("memory_type", args.kind),
                    ("status", args.status),
                    ("tag", args.tag),
                ] {
                    if let Some(val) = val {
                        filters.insert(key.into(), val);
                    }
                }
                let rows = service.list(&filters).await?;
                emit(
                    &json!(
                        rows.iter()
                            .map(|m| pick(m, &["id", "workspace_id", "memory_type", "status"]))
                            .collect::<Vec<_>>()
                    ),
                    json_output,
                    Some("Memories"),
                );
            }
            MemoryCommand::Tag { memory_id, tags } => {
                let tags = service
                    .tag(
                        &memory_id,
                        TagRequest {
                            tags,
                            source: "user".into(),
                        },
                    )
                    .await?;
                emit(
                    &json!(
                        tags.iter()
                            .map(|t| t["canonical_name"].clone())
                            .collect::<Vec<_>>()
                    ),
                    json_output,
                    Some("Tagged"),
                );
            }
            MemoryCommand::Untag { memory_id, tag_id } => {
                let removed = service.store.remove_tag(&memory_id, &tag_id).await?;
                if removed {
                    service
                        .store
                        .event(&memory_id, "untagged", json!({"tag_id":tag_id}), None)
                        .await?;
                }
                emit(&json!({"removed":removed}), json_output, None);
            }
            MemoryCommand::Link {
                source_memory_id,
                relation,
                target_memory_id,
            } => {
                let link = service
                    .relationship(
                        &source_memory_id,
                        RelationshipCreateRequest {
                            target_memory_id,
                            relation_type: relation,
                            source: None,
                            confidence: None,
                        },
                        true,
                    )
                    .await?;
                emit(&pick(&link, &["id", "relation_type"]), json_output, None);
            }
            MemoryCommand::Reinforce { memory_id, reason } => {
                let memory = service
                    .reinforce(
                        &memory_id,
                        ReinforceRequest {
                            reason,
                            source: None,
                        },
                    )
                    .await?;
                emit(
                    &pick(&memory, &["id", "reinforcement_count"]),
                    json_output,
                    None,
                );
            }
            MemoryCommand::Supersede(args) => {
                service
                    .replace(&args.old_memory_id, &args.with_id, false)
                    .await?;
                emit(
                    &json!({"memory_id":args.old_memory_id,"status":"superseded"}),
                    json_output,
                    None,
                );
            }
            MemoryCommand::Correct(args) => {
                service
                    .replace(&args.old_memory_id, &args.with_id, true)
                    .await?;
                emit(
                    &json!({"memory_id":args.old_memory_id,"status":"superseded"}),
                    json_output,
                    None,
                );
            }
            MemoryCommand::Archive(Id { memory_id }) => {
                service
                    .status(&memory_id, "archived", json!({}), &Filters::new())
                    .await?;
                emit(
                    &json!({"memory_id":memory_id,"status":"archived"}),
                    json_output,
                    None,
                );
            }
            MemoryCommand::Forget(Id { memory_id }) => {
                service
                    .status(
                        &memory_id,
                        "forgotten",
                        json!({"reason":null}),
                        &Filters::new(),
                    )
                    .await?;
                emit(
                    &json!({"memory_id":memory_id,"status":"forgotten"}),
                    json_output,
                    None,
                );
            }
            MemoryCommand::Graph {
                memory_id,
                depth,
                relation,
                no_entities,
                no_tags,
                no_resources,
                max_nodes,
                format,
                output,
                ..
            } => {
                let mut filters = Filters::from([
                    ("include_entities".into(), (!no_entities).to_string()),
                    ("include_tags".into(), (!no_tags).to_string()),
                    ("include_resources".into(), (!no_resources).to_string()),
                    ("max_nodes".into(), max_nodes.to_string()),
                ]);
                if let Some(depth) = depth {
                    filters.insert("depth".into(), depth.to_string());
                }
                if let Some(relation) = relation {
                    filters.insert("relation_types".into(), relation);
                }
                let graph = service.graph(&memory_id, &filters).await?;
                if ["svg", "png"].contains(&format.as_str()) && output.is_none() {
                    return Err(Error::invalid(format!(
                        "--format {format} requires --output <path>"
                    )));
                }
                if format == "json" {
                    emit(&graph, json_output, None);
                } else {
                    let response = crate::presentation::graph::render(graph, &format).await?;
                    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
                        .await
                        .map_err(io_error)?;
                    if ["svg", "png"].contains(&format.as_str()) {
                        let path = output.expect("output path");
                        tokio::fs::write(&path, bytes).await.map_err(io_error)?;
                        println!("wrote {}", path.display());
                    } else {
                        println!("{}", String::from_utf8_lossy(&bytes));
                    }
                }
            }
        },
        Command::Health => {
            let value = crate::presentation::api::ready(axum::extract::State(state))
                .await
                .0;
            emit(&value, json_output, Some("Dependencies"));
        }
        Command::Db {
            command: DbCommand::Upgrade,
        } => {
            sqlx::migrate!()
                .run(&service.store.pool)
                .await
                .map_err(io_error)?;
            println!("Database upgraded to head.");
        }
        Command::Admin { command } => {
            let kind = match &command {
                AdminCommand::Expire => "expire",
                AdminCommand::Reembed { .. } => "reembed",
                AdminCommand::SyncDocuments { .. } => "sync-documents",
                AdminCommand::Reconcile { .. } => "reconcile",
            };
            let value = match command {
                AdminCommand::Reembed { limit } => json!(
                    crate::domain::tasks::embeddings::run(
                        &service.store,
                        crate::dal::remote::embeddings::configured(&config.embedding).as_ref(),
                        limit,
                        chrono::Utc::now()
                    )
                    .await?
                ),
                AdminCommand::Expire => json!(
                    workers::expire(
                        &service.store,
                        config.expiration_batch_limit,
                        chrono::Utc::now()
                    )
                    .await?
                ),
                AdminCommand::SyncDocuments { limit } => {
                    json!(workers::sync_documents(&service.store, &state.documents, limit).await?)
                }
                AdminCommand::Reconcile { .. } => json!(
                    workers::reconcile(
                        &service.store,
                        &state.documents,
                        config.reconciliation_batch_limit
                    )
                    .await?
                ),
            };
            if json_output {
                emit(&value, true, None);
            } else if let Some(text) = maintenance_text(kind, &value) {
                println!("{text}");
            }
        }
        Command::Consolidate(args) => {
            if args.tag.is_none() && args.kind.is_none() {
                return Err(Error::invalid(
                    "consolidation requires at least --tag or --type (never the whole corpus, spec Part 5 §64)",
                ));
            }
            let request = ConsolidationRequest {
                memory_ids: vec![],
                filters: Some(ConsolidationFilters {
                    workspace_id: args.workspace,
                    memory_types: args.kind.into_iter().collect(),
                    tags: args.tag.into_iter().collect(),
                    created_after: None,
                    created_before: None,
                    limit: args.limit,
                }),
                mode: if args.apply {
                    ConsolidationMode::ApplySafe
                } else {
                    ConsolidationMode::Analyze
                },
            };
            emit(
                &service.consolidate(request).await?,
                json_output,
                Some("Consolidation"),
            );
        }
        Command::Scheduler {
            command: SchedulerCommand::Run,
        } => {
            if !config.scheduler_enabled {
                println!("Scheduler disabled.");
                return Ok(());
            }
            let mut jobs = vec![];
            for (name, interval) in [
                ("expire_memories", config.expiration_interval),
                ("sync_documents", config.document_sync_interval),
                ("generate_embeddings", config.embedding.interval),
                ("reconcile", config.reconciliation_interval),
            ] {
                let store = service.store.clone();
                let docs = state.documents.clone();
                let cfg = config.clone();
                jobs.push(ScheduledJob {
                    name,
                    interval,
                    run: Arc::new(move || {
                        let store = store.clone();
                        let docs = docs.clone();
                        let cfg = cfg.clone();
                        Box::pin(async move {
                            match name {
                                "expire_memories" => {
                                    workers::expire(
                                        &store,
                                        cfg.expiration_batch_limit,
                                        chrono::Utc::now(),
                                    )
                                    .await?;
                                }
                                "sync_documents" => {
                                    workers::sync_documents(
                                        &store,
                                        &docs,
                                        cfg.document_batch_limit,
                                    )
                                    .await?;
                                }
                                "generate_embeddings" => {
                                    crate::domain::tasks::embeddings::run(
                                        &store,
                                        crate::dal::remote::embeddings::configured(&cfg.embedding)
                                            .as_ref(),
                                        cfg.embedding.batch_limit,
                                        chrono::Utc::now(),
                                    )
                                    .await?;
                                }
                                _ => {
                                    workers::reconcile(
                                        &store,
                                        &docs,
                                        cfg.reconciliation_batch_limit,
                                    )
                                    .await?;
                                }
                            }
                            Ok(())
                        })
                    }),
                });
            }
            println!(
                "Starting scheduler with {} job(s): {}",
                jobs.len(),
                jobs.iter().map(|j| j.name).collect::<Vec<_>>().join(", ")
            );
            let (tx, rx) = tokio::sync::watch::channel(false);
            tokio::spawn(async move {
                let mut term =
                    tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                        .expect("SIGTERM handler");
                tokio::select! {_=tokio::signal::ctrl_c()=>{},_=term.recv()=>{}}
                let _ = tx.send(true);
            });
            run_scheduler(jobs, rx).await;
            println!("Scheduler stopped.");
        }
    }
    Ok(())
}

impl Cli {
    pub fn verbose(&self) -> bool {
        self.verbose
    }
}

fn maintenance_text(kind: &str, value: &Value) -> Option<String> {
    match kind {
        "reconcile" => Some(format!(
            "missing_document: {}\nmissing_embedding: {}",
            value["missing_document"].as_array().map_or(0, Vec::len),
            value["missing_embedding"].as_array().map_or(0, Vec::len)
        )),
        "reembed" => Some(
            ["processed", "embedded", "skipped_no_provider", "failed"]
                .into_iter()
                .map(|key| format!("{key}: {}", value[key]))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        "sync-documents" => Some(
            ["processed", "synced", "skipped_no_document", "failed"]
                .into_iter()
                .map(|key| format!("{key}: {}", value[key]))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        _ => None,
    }
}
#[cfg(test)]
mod output_tests {
    use super::*;
    #[test]
    fn interactive_maintenance_reports_remain_visible() {
        assert_eq!(
            maintenance_text(
                "reconcile",
                &json!({"missing_document":["a"],"missing_embedding":[]})
            )
            .unwrap(),
            "missing_document: 1\nmissing_embedding: 0"
        );
        assert!(
            maintenance_text(
                "sync-documents",
                &json!({"processed":1,"synced":1,"skipped_no_document":0,"failed":0})
            )
            .unwrap()
            .contains("synced: 1")
        );
        assert!(maintenance_text("expire", &json!({"expired_memory_ids":[]})).is_none());
    }
}

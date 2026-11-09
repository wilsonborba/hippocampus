# hippocampus

Dedicated memory service for the Cortex/Plane ecosystem.

```text
Cortex      → Think
Hippocampus → Remember
Plane       → Track Work
File Store  → Preserve large/raw artifacts
```

Hippocampus is not a vector database wrapper. It encodes, classifies, associates, and
recalls memory: it owns memory identity, lifecycle, tags, entities, relationships, and
provenance, and exposes them through an HTTP API and a CLI backed by the same use cases.

## Persistence

```text
PostgreSQL → authoritative structured memory (identity, lifecycle, tags, entities,
             relationships, provenance, embeddings via pgvector)
Redis      → transient working memory / cache (disposable, not required for durability)
CouchDB    → durable flexible MemoryDocument content
File Store → raw/large artifact bytes (referenced, never duplicated)
```

Redis and CouchDB are optional at runtime: durable memory operations keep working when
either is unavailable, degrading only the features that depend on them.

## Setup

```bash
python3 -m venv .venv
source .venv/bin/activate
pip install -e ".[dev]"
cp .env.example .env
hippocampus db upgrade
```

## Usage & HTTP API Reference

### Running the API Server

```bash
hippocampus-api  # Serves the HTTP API on port 8001 (see /health, /ready, /docs)
```

Interactive OpenAPI documentation is available at `http://localhost:8001/docs`.

### Multitenant Isolation Tag Convention

Hippocampus uses tag-based multitenancy:
- Add `tenant:<tenant_id>` in the `tags` array to isolate memories per tenant/app.
- When recalling memories, pass `tags: ["tenant:<tenant_id>"]` to query within that tenant's namespace.

### REST API Endpoints & `curl` Examples

#### 1. Store Memory (`POST /api/v1/memories`)
```bash
curl -X POST http://localhost:8001/api/v1/memories \
  -H "Content-Type: application/json" \
  -d '{
    "title": "cortex_db_choice",
    "content": "Cortex uses SQLite local for logs and PostgreSQL for multi-tenant production.",
    "tags": ["architecture", "tenant:cortex-app-1"],
    "metadata": {"key": "cortex_db_choice", "tenant_id": "cortex-app-1"}
  }'
```

#### 2. Recall / Vector Search Memories (`POST /api/v1/recall`)
```bash
curl -X POST http://localhost:8001/api/v1/recall \
  -H "Content-Type: application/json" \
  -d '{
    "query": "Qual banco de dados o Cortex utiliza?",
    "tags": ["tenant:cortex-app-1"],
    "limit": 5
  }'
```

#### 3. Delete Memory Node (`DELETE /api/v1/memories/{id}`)
```bash
curl -X DELETE http://localhost:8001/api/v1/memories/<memory-id>
```

### CLI Usage

```bash
hippocampus memory remember --type decision --content "CouchDB is the selected document store." --tag project:hippocampus
hippocampus memory get <memory-id>
hippocampus search "mapper"
hippocampus recall "What did we decide about the mapper?"
```

## Status

MVP scope tracked in issue #1. Full architecture/domain specification lives locally in `docs/` (untracked, per working rules).

## Rust development

The Rust app uses an isolated test database before cutover. It does not open the v1 production database.

- Use the `hippocampus_dev` Git worktree.
- Set `HIPPOCAMPUS_TEST_DATABASE_URL` in the shared, ignored `.env` file.
- The database name must be `hippocampus_v2_test`.
- Run `cargo run --bin migrate_test` after restoring the v1 backup to the test database.
- Run `scripts/app_dev_run.sh`. The test API listens on `127.0.0.1:18001`.
- Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test`.

The graph image formats require Graphviz (`dot`). JSON, D2, and Mermaid output need no external renderer.
The Rust graph renderer preserves node types and edges. Image coordinates can differ from the Python renderer.
All behavior settings are in `src/core/config.rs`. Keep private connection values in `.env`.

Build the CLI with `cargo build --bin hippocampus`. Use `target/debug/hippocampus --help` for commands.
The CLI writes A2A memories with app id `hippocampus`. It uses the same test database guard as the API.
The scheduler runs in the foreground with `hippocampus scheduler run`. It stops on SIGINT or SIGTERM.
Expiration runs every 300 seconds. Document sync runs every 120 seconds. Reconciliation runs every 1800 seconds.
The embedding job has its own area contract. The scheduler does not run consolidation automatically.
Logs rotate at 5 MB, with three backup files. CLI logs go to stderr and preserve JSON stdout.

Run `scripts/test_v1_parity.sh` for isolated HTTP and CLI contract checks.
It needs the original checkout's Python environment and free ports 18001 and 28001.
The v1 CLI fixture starts from its known model schema and revision. See issue #29 for the empty SQLite migration defect.
Run `cargo test --test workers -- --ignored` for worker checks on the test database.

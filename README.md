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

Embedding settings are in `Config.embedding`. The default backend is `None`, and no model is selected.
The owner supplies the model name before enabling Ollama. The worker scans all memories in batches.
It skips current vectors and refreshes vectors after canonical text changes. No vector index is created.
`hippocampus admin reembed --limit 50` runs a complete pass with batches of 50 memories.
The scheduler checks this job every 60 seconds. A disabled provider does no storage or network work.
Embedding tests use a double. They do not contact Pong or install a model.

`POST /api/v1/files` accepts multipart fields `file`, `workspace_id`, `app_id`, `user_id`, and `extracted_text`.
An empty or omitted `user_id` means A2A memory. Extracted text can be empty.
The response contains `data.memory_id` and `data.resource_id`.
FSM keeps raw bytes. PostgreSQL keeps the file memory and resource metadata. CouchDB receives no file bytes.
`GET /api/v1/resources/{id}/content` returns the bytes. Scope query fields limit metadata and content reads.
FSM upload completes before the PostgreSQL transaction. A database failure can leave an FSM object.
Run `cargo test --test files -- --ignored` to check uploads and downloads with a local FSM double.

### Rust search

`POST /api/v1/search` is the only Rust search route. Send `workspace_id`.
Optional filters are `app_id`, `user_id`, `tags` (all match), `entity_ids`,
`memory_types`, and `historical`. Omit `user_id` for all users, or send null
for A2A. Cortex must always send its app and user filters.

`text` uses PostgreSQL `simple` full-text terms with AND. The GIN document
includes title, summary, content, tags and entities. Link and name changes
refresh it. Thai word search is weak without spaces. With no text, the word
list uses stable memory id order. Default limit is 20.

When the configured embedding model returns a vector, all filtered vectors
of that model are ranked by cosine distance. RRF uses k=60. With the model
off or unavailable, only the word list is used. There is no vector index.
Each result has `memory`, `score`, and `match_reasons` (word, semantic, or
relationship). `expand` is 0 to 3, default 0. Expansion adds linked memories
after the ranked limit and keeps all hard filters. No match returns an empty
array. Historical search adds superseded memories, not deleted ones.

The Rust CLI restores `search` and `recall` with a required `--workspace`.
Both use the same new ranking. Recall no longer returns the removed v1
weighted signals. The v1 examples above do not define the Rust search API.

### Scoped graph and names

`GET /api/v1/graph` returns nodes and edges for active memories in a scope.
`GET /api/v1/graph/nodes/{node_id}` returns that node and its scoped context.
Both require `workspace_id`. Optional `app_id` and `user_id` have the search
scope rules. Empty query `user_id` selects A2A. Limit bounds the returned
memories (default 300). Depth is 0 to 2 (default 2). Whole graph seeds are
the scoped list in stable id order. Node context starts with memories
linked to that node and follows relationships up to depth and limit.

Node ids are a memory id, `tag:id`, `entity:id`, or `resource:id`.
File resources use node type `file`. Names and edges come only from scoped
memories. An absent or private node returns 404. A shared tag never adds
another user's memory or entity.

`GET /api/v1/tags/search` and `GET /api/v1/entities/search` require the same
scope. They keep `q` and `limit` (default 20) from v1. Only names linked to
active memories in that scope are returned.

### Hard delete

`POST /api/v1/memories/delete` accepts `memory_ids`, `workspace_id`, `app_id`
and optional `user_id` (empty or null for A2A). All existing ids must match
the exact scope. Missing ids are ignored. The data array lists removed ids.

The transaction removes memory rows, joins, events, vectors and unused tags.
It removes a file resource only after the last memory link. For files owned
by Hippocampus in FSM, it calls FSM DELETE with the configured app and key.
It never follows a resource URI to delete files. External resource metadata
can be removed, but files owned by other apps are not deleted.

Memory document cleanup and owned FSM cleanup run before the PostgreSQL
commit. A remote failure rolls back PostgreSQL. External deletes are safe
to repeat, including a 404. The stores do not share an atomic transaction.
There is no background retry or retry table. The caller retries the request.

The CLI keeps `memory delete ID --hard --yes`, with required `--workspace`,
optional `--app` (default hippocampus), and optional `--user` (default A2A).
`memory forget` remains the logical operation.

### Scoped personas

Personas are entities of type `person` or `bot`. `POST /api/v1/entities`
accepts the write scope, `entity_type`, `canonical_name`, `aliases`, and
`is_self`. `PATCH /api/v1/entities/{id}` accepts the exact write scope and
changed name, aliases or is_self. `GET /api/v1/entities` lists personas in a
required read scope, with optional `q` and `type`. Reading one persona also
requires a read scope. Existing non-persona entity reads stay compatible.

`POST /api/v1/entities/match` accepts a required read scope and `text`.
It matches normalized Unicode words in names and aliases. It does not match
part of a word and makes no LLM call. Each scope has its own names and aliases.
`is_self` is unique per user and app across workspaces. A2A has at most one
per workspace and app. Another scope cannot change a persona.

Migration 0004 splits old shared entities by the scopes of their linked
memories. It preserves aliases and links. Unlinked old entities use workspace
default, app legacy, and no user. Only the test copy runs this migration now.

### Persona schemas and attribute documents

The LAN schema routes are:

- `POST /api/v1/persona-schemas`: save a draft with schema_name, version and fields.
- `POST /api/v1/persona-schemas/{name}/versions/{version}/publish`: publish it.
- `GET /api/v1/persona-schemas/{name}/versions/{version}`: read one version.
- `GET /api/v1/persona-schemas/{name}`: read the latest published version.

Schema names are person and bot. Each field has name, type, required and
in_context. The guardrail follows the Reports pattern, including nested
field paths. It rejects missing required fields, wrong types and unknown
fields. Errors include each field_path. Published versions cannot change.
No attribute field definitions are installed by default.

Entity create and update accept `attributes`. The first attribute document
uses the latest published schema for that persona type. Later attribute
changes keep that document's schema version. A new schema version does not
change old documents. A persona can have a name and aliases before it has
attributes. The first attribute write requires a published schema.

The CouchDB document has _id equal to the persona id, schema_name,
schema_version and attributes. The entity has document_store and document_id
links. Persona list, read and match return the saved attributes and version.
Read a version's schema fields to select in_context fields for chat context.
The runtime document client fails when the configured store is unavailable.
The in-memory document store is a local test double.

`DELETE /api/v1/entities/{id}` requires the exact write scope as query fields.
It removes persona links and aliases, deletes the entity and its document,
and leaves referenced memories intact. A missing id or document is safe.
Document failure rolls back PostgreSQL. The caller can retry. There is no
shared transaction between PostgreSQL and CouchDB. No background retry runs.

All current migrations and document tests use hippocampus_v2_test. Production
Hippocampus remains read-only until the approved cutover.

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

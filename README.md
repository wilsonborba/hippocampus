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

## Usage

```bash
hippocampus memory remember --type decision --content "CouchDB is the selected document store." --tag project:hippocampus
hippocampus memory get <memory-id>
hippocampus search "mapper"
hippocampus recall "What did we decide about the mapper?"
```

```bash
hippocampus-api  # serves the HTTP API (see /health, /ready)
```

## Status

Early implementation — MVP scope tracked in issue #1. Full architecture/domain
specification lives locally in `docs/` (untracked, per working rules).

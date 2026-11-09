# Changelog


## Unreleased


### Chores

- Merge v1.0.0 back to release

- Merge v1.0.0 back to development

- Merge v1.0.0 back to feature/main

- Add Rust project skeleton (#13)

- Merge #13 add Rust project skeleton

- Map the complete v1 behavior (#14)


## Unreleased


### Chores

- Merge v1.0.0 back to release

- Merge v1.0.0 back to development

- Merge v1.0.0 back to feature/main


## v1.0.0 - 2026-11-09


### Bug Fixes

- Strip behavior/config variables out of .env.example

- Strip behavior/config variables out of .env.example

- Install postgres/redis extras in install-service.sh

- Improve memory tagging and validation

- Enforce workspace_id on single-memory get and graph traversal

- Scope forget() by workspace_id

- Exclude forgotten memories from the default search/list


### Chores

- Scaffold Python project (pyproject, env template, gitignore, README)

- Document HIPPOCAMPUS_FILESTORE_API_KEY in the env template (#2)

- Add install/uninstall scripts for the systemd deployment

- Move install/uninstall into scripts/, matching monorepo convention


### Documentation

- Interactive API reference via Scalar (#7)

- Interactive API reference via Scalar (#7)

- Update README with API reference and curl examples


### Features

- Persistence layer (PostgreSQL schema, migrations, repositories, Redis/CouchDB/File Store adapters)

- Domain services (remember/recall/lifecycle use cases, background workers)

- HTTP API and CLI (same use cases, shared error/response envelope)

- Merge Hippocampus MVP scaffold (#1)

- Verify real LAN infra integration (Postgres+pgvector, Valkey, CouchDB) (#2)

- Native pgvector column for memory_embedding (#3)

- Native pgvector column for memory_embedding (#3)

- API authentication and authorization (#4)

- API authentication and authorization (#4)

- Consolidation, exact-duplicate detection, and contradiction candidates (#5)

- Consolidation, exact-duplicate detection, and contradiction candidates (#5)

- Automatic scheduler for background workers (#6)

- Automatic scheduler for background workers (#6)

- Memory graph route (nodes/edges + D2/Mermaid/SVG/PNG rendering)

- Add workspace isolation

- Implement direct file upload to FSM and memory resource association (#11)

- Include created_at in memory-graph node metadata

- Include a content preview in memory-graph node metadata


### Tests

- Domain and API coverage for lifecycle, tagging, relationships, and recall ranking



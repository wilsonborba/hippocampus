# Changelog


## Unreleased


### Bug Fixes

- Keep interactive maintenance reports visible (#16)

- Cli-readable-summary (#30)

- Scoped-graph-depth (#32)


### Chores

- Merge v1.0.0 back to release

- Merge v1.0.0 back to development

- Merge v1.0.0 back to feature/main

- Add Rust project skeleton (#13)

- Merge #13 add Rust project skeleton

- Map the complete v1 behavior (#14)

- Merge #14 map the complete v1 behavior

- Add forced memory scope columns (#18)

- Merge #18 add forced memory scope columns

- Port the unchanged memory API to Rust (#15)

- Merge #15 port the unchanged memory API to Rust

- Preserve forgotten memory list filters (#15)

- Merge #15 preserve forgotten memory list filters

- Merge #16 port workers CLI consolidation and Scalar docs

- Merge #19 migrate the isolated v1 memory copy

- Merge #16 keep interactive maintenance reports visible

- Merge #17 embed all memories through an optional provider

- Merge #20 store files and their extracted text through FSM

- Merge #21 unified-search

- Merge #22 scoped-graph

- Merge #23 bulk-delete

- Merge #24 scoped-personas

- Merge #25 persona-schemas

- Merge #30 cli-readable-summary


### Features

- Port workers CLI consolidation and Scalar docs (#16)

- Migrate the isolated v1 memory copy (#19)

- Embed all memories through an optional provider (#17)

- Store files and their extracted text through FSM (#20)

- Unified-search (#21)

- Scoped-graph (#22)

- Bulk-delete (#23)

- Scoped-personas (#24)

- Persona-schemas (#25)


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

- Prepare v1.0.0 (#12)

- Consolidate v1.0.0

- Prepare development for v1.0.0

- Release v1.0.0


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


### merge

- Promote feature/main to development



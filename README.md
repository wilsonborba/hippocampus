# Hippocampus

Rust memory service and CLI. PostgreSQL holds memory relations and scope.
CouchDB holds schema-checked persona attributes and linked documents. File bytes
are stored through FSM. Embeddings stay disabled until separately configured.

Private credentials belong in `.env`. Non-secret settings are in
`src/core/config.rs`. No credentials belong in arguments, logs or Git.

## Run and release

- `cargo run --bin hippocampus-api`: test database and development port 18001.
- `cargo run --bin hippocampus-api -- --production`: production database,
  CouchDB `hippocampus_v2`, and port 8001.
- `cargo run --bin hippocampus -- --production health`: production CLI.
- `cargo run --bin hippocampus -- --production db upgrade`: explicit migration.
- `cargo run --bin hippocampus -- --production scheduler run`: background tasks.
- `./scripts/install-service.sh --prepare`: locked release build, staged binaries and units.
  `CARGO_TARGET_DIR` is supported. Run this step in the production checkout.
- `./scripts/install-service.sh --check`: verify staged artifact hashes and source files.
- `./scripts/install-service.sh --activate`: administrator step. Installs the staged
  binaries before restart, then verifies each running executable hash.
  Missing, changed or stale prepared artifacts are rejected before service changes.
  Run only in the coordinated deployment window.

Installation does not migrate a database. Before a production migration, stop
both legacy APIs and the legacy scheduler, take and verify fresh PostgreSQL and
CouchDB backups, and preserve project-memory content and metadata. The migration
removes the aligned Cortex v1 user workspaces. Coordinate the API switch with
Cortex. Rollback requires the pre-migration database backup and `v1.0.0` sources.

## Checks

Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings` and `cargo test`.
Ignored integration tests use only the isolated test database. Reports, backups
and benchmark data stay under ignored `docs/`. Never run the test migration
checker against production. Local and production launches select their stores
explicitly. No migration runs implicitly at API startup.

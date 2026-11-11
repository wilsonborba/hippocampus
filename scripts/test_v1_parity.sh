#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
repo_path="$PWD"
python_path="${repo_path%_dev}/.venv/bin/python"
test_dir="$(mktemp -d /tmp/hippocampus-parity.XXXXXX)"
v1_pid=''
v2_pid=''
cleanup() {
  if [[ -n "$v1_pid" ]]; then kill "$v1_pid" 2>/dev/null || true; fi
  if [[ -n "$v2_pid" ]]; then kill "$v2_pid" 2>/dev/null || true; fi
  rm -rf "$test_dir"
}
trap cleanup EXIT
for port in 18001 28001; do
  if [[ -n "$(ss -H -ltn "sport = :$port")" ]]; then
    echo "The test port $port is in use" >&2
    exit 1
  fi
done
cargo build --bin hippocampus-api
HIPPOCAMPUS_DATABASE_URL="sqlite:///$test_dir/v1.sqlite" \
HIPPOCAMPUS_REDIS_URL='' HIPPOCAMPUS_COUCHDB_URL='' \
HIPPOCAMPUS_FILESTORE_URL='' HIPPOCAMPUS_API_KEYS='{}' \
HIPPOCAMPUS_EMBEDDING_PROVIDER=none HIPPOCAMPUS_SCHEDULER_ENABLED=false \
"$python_path" -m uvicorn lib.presentation.api.app:create_app --factory \
  --host 127.0.0.1 --port 28001 > "$test_dir/v1.log" 2>&1 &
v1_pid=$!
target/debug/hippocampus-api > "$test_dir/v2.log" 2>&1 &
v2_pid=$!
for port in 18001 28001; do
  ready=false
  for attempt in {1..30}; do
    if curl --fail --silent --output /dev/null "http://127.0.0.1:$port/health"; then ready=true; break; fi
    sleep 1
  done
  if [[ "$ready" != true ]]; then echo "The isolated test API did not start on $port" >&2; exit 1; fi
done
cargo test --test v1_parity -- --ignored
cargo test --test cli_parity -- --ignored

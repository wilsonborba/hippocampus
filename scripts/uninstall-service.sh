#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
API_SERVICE_NAME="hippocampus-api"
SCHEDULER_SERVICE_NAME="hippocampus-scheduler"
API_SERVICE_FILE="/etc/systemd/system/${API_SERVICE_NAME}.service"
SCHEDULER_SERVICE_FILE="/etc/systemd/system/${SCHEDULER_SERVICE_NAME}.service"
PURGE_DATA=false

for arg in "$@"; do
  case "$arg" in
    --purge-data) PURGE_DATA=true ;;
    *)
      printf 'Unknown argument: %s\n' "$arg" >&2
      exit 1
      ;;
  esac
done

log() {
  printf '[hippocampus-uninstall] %s\n' "$1"
}

stop_and_disable() {
  local service_name="$1"
  local service_file="$2"

  if sudo systemctl list-unit-files | grep -q "^${service_name}.service"; then
    sudo systemctl stop "$service_name" || true
    sudo systemctl disable "$service_name" || true
  fi

  if [ -f "$service_file" ]; then
    sudo rm -f "$service_file"
  fi
}

remove_ufw_rules() {
  if ! command -v ufw >/dev/null 2>&1; then
    return
  fi

  if ! sudo ufw status | grep -q 'Status: active'; then
    return
  fi

  while read -r rule_number; do
    sudo ufw --force delete "$rule_number"
  done < <(sudo ufw status numbered | grep 'hippocampus-api-' | sed -n 's/^\[ \{0,1\}\([0-9]\+\)\].*/\1/p' | sort -rn)
}

main() {
  stop_and_disable "$SCHEDULER_SERVICE_NAME" "$SCHEDULER_SERVICE_FILE"
  stop_and_disable "$API_SERVICE_NAME" "$API_SERVICE_FILE"
  sudo systemctl daemon-reload

  remove_ufw_rules

  if [ "$PURGE_DATA" = true ]; then
    rm -rf "$ROOT_DIR/var"
    log "Removed var directory (local sqlite/log data)."
  fi

  log "Uninstall complete."
}

main

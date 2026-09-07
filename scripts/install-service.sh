#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
VENV_DIR="$ROOT_DIR/.venv"
ENV_FILE="$ROOT_DIR/.env"
ENV_EXAMPLE="$ROOT_DIR/.env.example"
PYTHON_BIN=${PYTHON_BIN:-python3}
API_SERVICE_NAME="hippocampus-api"
SCHEDULER_SERVICE_NAME="hippocampus-scheduler"
API_SERVICE_FILE="/etc/systemd/system/${API_SERVICE_NAME}.service"
SCHEDULER_SERVICE_FILE="/etc/systemd/system/${SCHEDULER_SERVICE_NAME}.service"
CURRENT_USER=$(id -un)
CURRENT_GROUP=$(id -gn)

log() {
  printf '[hippocampus-install] %s\n' "$1"
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    printf 'Missing required command: %s\n' "$1" >&2
    exit 1
  fi
}

install_api_service() {
  local host port
  host=$(grep '^HIPPOCAMPUS_API_HOST=' "$ENV_FILE" | cut -d= -f2-)
  port=$(grep '^HIPPOCAMPUS_API_PORT=' "$ENV_FILE" | cut -d= -f2-)

  sudo tee "$API_SERVICE_FILE" >/dev/null <<EOF
[Unit]
Description=Hippocampus API Service
After=network.target

[Service]
Type=simple
User=${CURRENT_USER}
Group=${CURRENT_GROUP}
WorkingDirectory=${ROOT_DIR}
EnvironmentFile=${ENV_FILE}
ExecStart=${VENV_DIR}/bin/hippocampus-api
Restart=always
RestartSec=3
NoNewPrivileges=yes
PrivateTmp=yes
UMask=027

[Install]
WantedBy=multi-user.target
EOF

  sudo systemctl daemon-reload
  sudo systemctl enable "$API_SERVICE_NAME"
  sudo systemctl restart "$API_SERVICE_NAME"

  log "host/port in use: ${host}:${port}"
}

install_scheduler_service() {
  sudo tee "$SCHEDULER_SERVICE_FILE" >/dev/null <<EOF
[Unit]
Description=Hippocampus Scheduler Service
After=network.target ${API_SERVICE_NAME}.service

[Service]
Type=simple
User=${CURRENT_USER}
Group=${CURRENT_GROUP}
WorkingDirectory=${ROOT_DIR}
EnvironmentFile=${ENV_FILE}
ExecStart=${VENV_DIR}/bin/hippocampus scheduler run
Restart=always
RestartSec=3
NoNewPrivileges=yes
PrivateTmp=yes
UMask=027

[Install]
WantedBy=multi-user.target
EOF

  sudo systemctl daemon-reload
  sudo systemctl enable "$SCHEDULER_SERVICE_NAME"
  sudo systemctl restart "$SCHEDULER_SERVICE_NAME"
}

configure_ufw() {
  if ! command -v ufw >/dev/null 2>&1; then
    log "ufw not installed; skipping firewall configuration."
    return
  fi

  if ! sudo ufw status | grep -q 'Status: active'; then
    log "ufw installed but inactive; skipping firewall configuration."
    return
  fi

  local host port
  host=$(grep '^HIPPOCAMPUS_API_HOST=' "$ENV_FILE" | cut -d= -f2-)
  port=$(grep '^HIPPOCAMPUS_API_PORT=' "$ENV_FILE" | cut -d= -f2-)

  if [ "$host" = "127.0.0.1" ] || [ "$host" = "localhost" ]; then
    log "API bound to ${host}; not LAN-reachable, skipping firewall rules."
    return
  fi

  sudo ufw allow proto tcp from 10.0.0.0/8 to any port "$port" comment 'hippocampus-api-lan-10' >/dev/null || true
  sudo ufw allow proto tcp from 172.16.0.0/12 to any port "$port" comment 'hippocampus-api-lan-172' >/dev/null || true
  sudo ufw allow proto tcp from 192.168.0.0/16 to any port "$port" comment 'hippocampus-api-lan-192' >/dev/null || true
  sudo ufw deny "$port"/tcp comment 'hippocampus-api-deny-public' >/dev/null || true
  log "ufw LAN-only rules ensured for port ${port}."
}

run_healthcheck() {
  local host port
  host=$(grep '^HIPPOCAMPUS_API_HOST=' "$ENV_FILE" | cut -d= -f2-)
  port=$(grep '^HIPPOCAMPUS_API_PORT=' "$ENV_FILE" | cut -d= -f2-)
  [ "$host" = "0.0.0.0" ] && host=127.0.0.1
  "$VENV_DIR/bin/python" - <<PY
import time
from urllib.error import URLError
from urllib.request import urlopen

host = "${host}"
port = ${port}
last_error = None
for _ in range(10):
    try:
        with urlopen(f"http://{host}:{port}/health", timeout=5) as response:
            print(response.read().decode())
            break
    except URLError as exc:
        last_error = exc
        time.sleep(1)
else:
    raise SystemExit(f"health check failed: {last_error}")
PY
}

main() {
  require_command "$PYTHON_BIN"
  require_command sudo
  require_command systemctl

  log "Creating Python virtual environment."
  if [ ! -d "$VENV_DIR" ]; then
    "$PYTHON_BIN" -m venv "$VENV_DIR"
  fi

  log "Installing Python dependencies."
  "$VENV_DIR/bin/pip" install --upgrade pip >/dev/null
  # `postgres`/`redis` extras: a real deployment's .env points at real
  # Postgres/Redis (psycopg/redis are lazy-imported everywhere except the
  # SQLAlchemy engine, which needs psycopg present at import time).
  "$VENV_DIR/bin/pip" install -e "$ROOT_DIR[postgres,redis]" >/dev/null

  mkdir -p "$ROOT_DIR/var"

  if [ ! -f "$ENV_FILE" ]; then
    log "No .env found; copying .env.example. Edit it with real deployment values before relying on this service."
    cp "$ENV_EXAMPLE" "$ENV_FILE"
  else
    log "Using existing .env file."
  fi

  log "Applying database migrations."
  "$VENV_DIR/bin/hippocampus" db upgrade

  log "Installing systemd units."
  install_api_service
  install_scheduler_service

  log "Configuring firewall when available."
  configure_ufw

  log "Running health check."
  run_healthcheck

  log "Installation complete."
  log "Services: ${API_SERVICE_NAME}, ${SCHEDULER_SERVICE_NAME}"
}

main "$@"

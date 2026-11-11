#!/usr/bin/env bash
# Prepare Rust units. Migrations are a separate, explicit cutover step.
set -euo pipefail
cd "$(dirname "${BASH_SOURCE[0]}")/.."
root=$(pwd)
mode=${1:---prepare}
case "$mode" in --prepare|--activate) ;; *) printf 'Use install-service.sh [--prepare|--activate]\n' >&2; exit 1;; esac
if [ "$mode" = --activate ] && [ "$(id -u)" != 0 ]; then
  printf 'Administrator rights are required to activate the system services.\n' >&2
  exit 1
fi
if [ "$mode" = --prepare ]; then
  cargo build --release --locked --bins -j2
  mkdir -p var/systemd
  for name in api scheduler; do
    if [ "$name" = api ]; then command="$root/target/release/hippocampus-api --production";
    else command="$root/target/release/hippocampus --production scheduler run"; fi
    cat > "var/systemd/hippocampus-$name.service" <<UNIT
[Unit]
Description=Hippocampus Rust $name
After=network-online.target
Wants=network-online.target

[Service]
Type=simple
User=$(id -un)
Group=$(id -gn)
WorkingDirectory=$root
EnvironmentFile=$root/.env
ExecStart=$command
Restart=on-failure
RestartSec=3
NoNewPrivileges=yes
PrivateTmp=yes
UMask=027

[Install]
WantedBy=multi-user.target
UNIT
  done
  printf 'Build and service files ready in var/systemd.\n'
else
  for name in api scheduler; do
    test -f "var/systemd/hippocampus-$name.service"
    install -m 644 "var/systemd/hippocampus-$name.service" "/etc/systemd/system/hippocampus-$name.service"
  done
  systemctl daemon-reload
  systemctl enable hippocampus-api.service hippocampus-scheduler.service
  systemctl restart hippocampus-api.service hippocampus-scheduler.service
fi

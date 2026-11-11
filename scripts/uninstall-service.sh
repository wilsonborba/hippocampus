#!/usr/bin/env bash
# Keep all databases, documents and backups.
set -euo pipefail
if [ "$(id -u)" != 0 ]; then
  printf 'Administrator rights are required to remove the system services.\n' >&2
  exit 1
fi
systemctl stop hippocampus-api.service hippocampus-scheduler.service
systemctl disable hippocampus-api.service hippocampus-scheduler.service
rm -f /etc/systemd/system/hippocampus-api.service /etc/systemd/system/hippocampus-scheduler.service
systemctl daemon-reload

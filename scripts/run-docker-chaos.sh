#!/usr/bin/env bash
set -euo pipefail

if [ "${SUSUN_DOCKER_CHAOS:-0}" != "1" ]; then
  printf '%s\n' "skipped Docker chaos scenarios: set SUSUN_DOCKER_CHAOS=1"
  exit 0
fi

SUSUN_DOCKER_REQUIRED=1 cargo test -p susun-engine-bollard --test adapter_contract -- --nocapture

if command -v systemctl >/dev/null 2>&1 && systemctl is-active --quiet docker; then
  sudo systemctl restart docker
elif command -v service >/dev/null 2>&1; then
  sudo service docker restart
else
  printf '%s\n' "skipped daemon restart: no supported service manager found"
  exit 0
fi

SUSUN_DOCKER_REQUIRED=1 cargo test -p susun-engine-bollard --test adapter_contract bollard_satisfies_basic_engine_contract -- --nocapture

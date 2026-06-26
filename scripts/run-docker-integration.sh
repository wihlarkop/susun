#!/usr/bin/env bash
set -euo pipefail

SUSUN_DOCKER_REQUIRED=1 cargo test -p susun-engine-bollard --test adapter_contract -- --nocapture

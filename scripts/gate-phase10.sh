#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase9.sh
bash scripts/check-release-policy.sh
bash scripts/check-release-readiness.sh
bash scripts/generate-release-docs.sh
git diff --exit-code -- docs/generated/capability-and-compatibility.md

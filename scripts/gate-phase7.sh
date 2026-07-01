#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase5.sh
bash scripts/check-architecture.sh
bash scripts/check-diagnostics.sh
bash scripts/check-schemas.sh
bash scripts/check-release-policy.sh
bash scripts/check-real-world-catalog.sh
bash scripts/check-release-readiness.sh
bash scripts/generate-release-docs.sh
git diff --exit-code -- docs/generated/capability-and-compatibility.md

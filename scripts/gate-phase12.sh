#!/usr/bin/env bash
set -euo pipefail

bash scripts/gate-phase11.sh
bash scripts/check-package-readiness.sh
bash scripts/check-release-readiness.sh
bash scripts/generate-release-docs.sh
git diff --exit-code -- docs/generated/capability-and-compatibility.md

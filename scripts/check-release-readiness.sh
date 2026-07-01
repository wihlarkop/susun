#!/usr/bin/env bash
set -euo pipefail

python - <<'PY'
import json
import sys
from pathlib import Path

manifest_path = Path("fixtures/compatibility/release-readiness.json")
workspace = Path("Cargo.toml").read_text()
changelog = Path("CHANGELOG.md").read_text()
manifest = json.loads(manifest_path.read_text())

errors = []

schema = manifest.get("schema_version", {})
if schema.get("major") != 1:
    errors.append("release readiness schema_version.major must be 1")

version = manifest.get("release_version")
if not version:
    errors.append("release_version is required")
elif f'version = "{version}"' not in workspace:
    errors.append(f"release_version {version} must match workspace version")
elif f"## {version}" not in changelog:
    errors.append(f"CHANGELOG.md must contain section for {version}")

if manifest.get("phase") != 7:
    errors.append("phase must be 7")

gates = manifest.get("required_gates", [])
if not gates:
    errors.append("required_gates must not be empty")

seen = set()
for index, gate in enumerate(gates):
    prefix = f"required_gates[{index}]"
    gate_id = gate.get("id")
    if not gate_id:
        errors.append(f"{prefix}.id is required")
    elif gate_id in seen:
        errors.append(f"duplicate gate id: {gate_id}")
    else:
        seen.add(gate_id)

    command = gate.get("command")
    if not command:
        errors.append(f"{prefix}.command is required")
    elif not Path(command).exists():
        errors.append(f"{prefix}.command does not exist: {command}")

    if not gate.get("purpose"):
        errors.append(f"{prefix}.purpose is required")

if not manifest.get("deferred"):
    errors.append("deferred must record known non-goals")

if errors:
    for error in errors:
        print(f"release readiness error: {error}", file=sys.stderr)
    sys.exit(1)

print(f"validated release readiness for {version}")
PY

#!/usr/bin/env bash
set -euo pipefail

python - <<'PY'
import json
import sys
from pathlib import Path

catalog_path = Path("fixtures/compatibility/real-world-catalog.json")
corpus_path = Path("fixtures/compatibility/corpus.json")

catalog = json.loads(catalog_path.read_text())
corpus = json.loads(corpus_path.read_text())

errors = []

schema = catalog.get("schema_version", {})
if schema.get("major") != 1:
    errors.append("catalog schema_version.major must be 1")

patterns = catalog.get("patterns")
if not isinstance(patterns, list) or not patterns:
    errors.append("catalog must contain at least one pattern")
    patterns = []

fixture_ids = {fixture["id"] for fixture in corpus.get("fixtures", [])}
valid_support = {"supported", "supported_subset", "experimental", "unsupported"}
valid_operations = {"config", "analyze", "plan", "build", "run", "exec", "events", "wait", "cp", "port", "watch"}
seen = set()

for index, pattern in enumerate(patterns):
    prefix = f"patterns[{index}]"
    pattern_id = pattern.get("id")
    if not pattern_id:
        errors.append(f"{prefix}.id is required")
    elif pattern_id in seen:
        errors.append(f"duplicate pattern id: {pattern_id}")
    else:
        seen.add(pattern_id)

    if pattern.get("support") not in valid_support:
        errors.append(f"{prefix}.support must be one of {sorted(valid_support)}")

    fixtures = pattern.get("fixtures", [])
    if not fixtures:
        errors.append(f"{prefix}.fixtures must not be empty")
    for fixture in fixtures:
        if fixture not in fixture_ids:
            errors.append(f"{prefix}.fixtures references unknown fixture {fixture!r}")

    operations = pattern.get("operations", [])
    if not operations:
        errors.append(f"{prefix}.operations must not be empty")
    for operation in operations:
        if operation not in valid_operations:
            errors.append(f"{prefix}.operations contains unknown operation {operation!r}")

    support = pattern.get("support")
    deferred = pattern.get("deferred", [])
    if support in {"supported_subset", "experimental", "unsupported"} and not deferred:
        errors.append(f"{prefix}.deferred must explain gaps for {support}")

    if not pattern.get("evidence"):
        errors.append(f"{prefix}.evidence is required")

if errors:
    for error in errors:
        print(f"real-world catalog error: {error}", file=sys.stderr)
    sys.exit(1)

print(f"validated {len(patterns)} real-world compatibility patterns")
PY

#!/usr/bin/env bash
set -eu

manifest="schemas/manifest.json"
failures=0

fail() {
  printf 'schema package violation: %s\n' "$1" >&2
  failures=$((failures + 1))
}

if [ ! -f "$manifest" ]; then
  fail "schema manifest missing: $manifest"
  exit 1
fi

python -m json.tool "$manifest" >/dev/null

for schema in schemas/*.schema.json; do
  [ -f "$schema" ] || continue
  python -m json.tool "$schema" >/dev/null

  grep -q '"\$schema"' "$schema" || fail "$schema must declare \$schema"
  grep -q '"\$id"' "$schema" || fail "$schema must declare \$id"
  grep -q '"x-susun-artifact"' "$schema" || fail "$schema must declare x-susun-artifact"
  grep -q '"x-susun-version"' "$schema" || fail "$schema must declare x-susun-version"
  grep -q '"x-susun-secret-policy"' "$schema" || fail "$schema must declare x-susun-secret-policy"

  if grep -Eq '"(password|token|credential|secret_value|private_key|cleartext)"[[:space:]]*:' "$schema"; then
    fail "$schema defines a prohibited cleartext credential field"
  fi

  grep -q "\"path\": \"$schema\"" "$manifest" || fail "$schema is not listed in $manifest"
done

while IFS= read -r path; do
  [ -n "$path" ] || continue
  [ -f "$path" ] || fail "$manifest references missing schema: $path"
done <<EOF
$(grep -E '"path": "schemas/[^"]+\.schema\.json"' "$manifest" | sed -E 's/.*"path": "([^"]+)".*/\1/')
EOF

if [ "$failures" -ne 0 ]; then
  exit 1
fi

printf 'json schema package checks passed\n'

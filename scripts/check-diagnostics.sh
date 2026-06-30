#!/usr/bin/env bash
set -eu

catalog="crates/susun-diagnostics/catalog.toml"
failures=0

fail() {
  printf 'diagnostic catalog violation: %s\n' "$1" >&2
  failures=$((failures + 1))
}

if [ ! -f "$catalog" ]; then
  fail "diagnostic catalog missing: $catalog"
  exit 1
fi

codes="$(grep -E '^[[:space:]]*code[[:space:]]*=' "$catalog" | sed -E 's/.*"([^"]+)".*/\1/')"

if [ -z "$codes" ]; then
  fail "$catalog does not register any diagnostic codes"
fi

while IFS= read -r code; do
  [ -n "$code" ] || continue
  if ! printf '%s\n' "$code" | grep -Eq '^SUS-[A-Z0-9]+(-[A-Z0-9]+)+$'; then
    fail "$catalog contains invalid diagnostic code format: $code"
  fi
done <<EOF
$codes
EOF

duplicates="$(printf '%s\n' "$codes" | sort | uniq -d)"
if [ -n "$duplicates" ]; then
  while IFS= read -r code; do
    [ -n "$code" ] && fail "$catalog contains duplicate diagnostic code: $code"
  done <<EOF
$duplicates
EOF
fi

used="$(find crates -type f -path '*/src/*' -name '*.rs' -print0 \
  | xargs -0 grep -Eho 'SUS-[A-Z0-9]+(-[A-Z0-9]+)+' \
  | sort -u || true)"

if [ -n "$used" ]; then
  while IFS= read -r code; do
    [ -n "$code" ] || continue
    if ! printf '%s\n' "$codes" | grep -qx "$code"; then
      source="$(grep -Rsl "$code" crates --include '*.rs' | grep '/src/' | head -n 1 || true)"
      fail "${source:-production source} emits undocumented diagnostic code: $code"
    fi
  done <<EOF
$used
EOF
fi

if [ "$failures" -ne 0 ]; then
  exit 1
fi

printf 'diagnostic catalog checks passed\n'

#!/usr/bin/env bash
set -eu

failures=0

fail() {
  printf 'architecture violation: %s\n' "$1" >&2
  failures=$((failures + 1))
}

dependency_names() {
  local manifest="$1"
  local section="$2"
  awk -v wanted="$section" '
    /^\[/ {
      active = ($0 == wanted)
      next
    }
    active && /^[A-Za-z0-9_-]+[[:space:]]*=/ {
      name = $1
      gsub(/[[:space:]]/, "", name)
      print name
    }
  ' "$manifest"
}

crate_name() {
  awk '
    $1 == "name" && $2 == "=" {
      value = $3
      gsub(/"/, "", value)
      print value
      exit
    }
  ' "$1"
}

production_deps() {
  dependency_names "$1" "[dependencies]"
}

check_public_api_leaks() {
  for source in crates/*/src/*.rs crates/*/src/**/*.rs; do
    [ -f "$source" ] || continue
    case "$source" in
      crates/susun-engine-bollard/src/*)
        continue
        ;;
    esac

    public_lines="$(grep -nE '^[[:space:]]*pub([({[:space:]]|$)' "$source" || true)"
    [ -n "$public_lines" ] || continue

    if printf '%s\n' "$public_lines" | grep -Eq 'bollard::|Bollard[A-Za-z0-9_]*'; then
      fail "$source public API must not mention Bollard adapter/backend types"
    fi

    if printf '%s\n' "$public_lines" | grep -Eq 'tokio::sync|tokio::task|JoinHandle'; then
      fail "$source public API must not mention Tokio channel/task handle types"
    fi

    if printf '%s\n' "$public_lines" | grep -Eiq 'buildkit.*(client|proto|transport)|buildx.*(client|proto)|tonic::'; then
      fail "$source public API must not mention raw BuildKit transport types"
    fi

    if printf '%s\n' "$public_lines" | grep -Eiq 'registry.*(client|token|credential)|oci_distribution|reqwest::'; then
      fail "$source public API must not mention raw registry client types"
    fi
  done
}

for manifest in crates/*/Cargo.toml; do
  crate="$(crate_name "$manifest")"
  deps="$(production_deps "$manifest")"

  if [ "$crate" != "susun-engine-bollard" ] && printf '%s\n' "$deps" | grep -qx 'bollard'; then
    fail "$crate must not depend on bollard; keep Bollard isolated in susun-engine-bollard"
  fi

  case "$crate" in
    susun-source|susun-diagnostics|susun-model|susun-normalize|susun-loader|susun-validation|susun-graph|susun-engine|susun-planner)
      if printf '%s\n' "$deps" | grep -qx 'tokio'; then
        fail "$crate must remain pure/synchronous and must not depend on tokio"
      fi
      ;;
  esac

  if [ "$crate" = "susun-planner" ] && printf '%s\n' "$deps" | grep -qx 'susun-runtime'; then
    fail "susun-planner must not depend on susun-runtime"
  fi

  if [ "$crate" = "susun-engine" ] && printf '%s\n' "$deps" | grep -qx 'susun-cli'; then
    fail "susun-engine must not depend on susun-cli"
  fi

  if printf '%s\n' "$deps" | grep -qx 'susun-testkit'; then
    fail "$crate must not use susun-testkit as a production dependency"
  fi
done

check_public_api_leaks

if [ "$failures" -ne 0 ]; then
  exit 1
fi

printf 'architecture dependency checks passed\n'

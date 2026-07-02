#!/usr/bin/env bash
set -euo pipefail

python - <<'PY'
import json
import subprocess
import sys
from pathlib import Path

metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"]))
workspace_root = Path(metadata["workspace_root"]).resolve()
crates_root = workspace_root / "crates"
errors = []

for license_file in ("LICENSE-MIT", "LICENSE-APACHE"):
    if not (workspace_root / license_file).exists():
        errors.append(f"{license_file} is required for MIT OR Apache-2.0 publication")

root_manifest = (workspace_root / "Cargo.toml").read_text()
for field in ("description", "repository", "homepage", "readme", "keywords", "categories"):
    if f"{field} =" not in root_manifest:
        errors.append(f"workspace.package.{field} is required")

publishable = []
for package in metadata["packages"]:
    manifest_path = Path(package["manifest_path"]).resolve()
    if crates_root not in manifest_path.parents:
        continue

    publish_disabled = package.get("publish") == []
    contents = manifest_path.read_text()
    for field in ("description", "repository", "homepage", "readme", "keywords", "categories"):
        if f"{field}.workspace = true" not in contents:
            errors.append(f"{package['name']} must inherit {field}.workspace")

    if not publish_disabled:
        publishable.append(package["name"])
        for dependency in package["dependencies"]:
            # Path-only dev-dependencies are exempt: cargo strips them from
            # the packaged crate entirely, so they never need a registry
            # version. Requiring one here would force circular internal
            # dependencies (e.g. a crate's tests depending on a crate that
            # depends on it normally) to be permanently unpublishable.
            if dependency.get("kind") == "dev":
                continue
            dep_path = dependency.get("path")
            if dependency.get("source") is None and dep_path and crates_root in Path(dep_path).resolve().parents:
                if dependency.get("req") == "*":
                    errors.append(
                        f"{package['name']} depends on {dependency['name']} without a publishable version requirement"
                    )

if not publishable:
    errors.append("no publishable crates found under crates/")

if errors:
    for error in errors:
        print(f"package readiness error: {error}", file=sys.stderr)
    sys.exit(1)

print("\n".join(sorted(set(publishable))))
PY

if [ "${SUSUN_SKIP_PACKAGE_DRY_RUN:-}" = "1" ]; then
  printf '%s\n' "package readiness metadata checks passed; package assembly skipped by SUSUN_SKIP_PACKAGE_DRY_RUN=1"
  exit 0
fi

packages="$(
python - <<'PY'
import json
import subprocess
from pathlib import Path

metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--no-deps", "--format-version", "1"]))
crates_root = Path(metadata["workspace_root"]).resolve() / "crates"
names = []
for package in metadata["packages"]:
    manifest_path = Path(package["manifest_path"]).resolve()
    if crates_root in manifest_path.parents and package.get("publish") != []:
        names.append(package["name"])
for name in sorted(set(names)):
    print(name)
PY
)"

while IFS= read -r package; do
  [ -n "$package" ] || continue
  printf 'checking package assembly for %s\n' "$package"
  cargo package -p "$package" --allow-dirty --no-verify --list >/dev/null
done <<< "$packages"

printf '%s\n' "package readiness checks passed"

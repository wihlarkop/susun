$ErrorActionPreference = "Stop"

$failures = New-Object System.Collections.Generic.List[string]
$manifest = Get-Content -LiteralPath "Cargo.toml" -Raw

if ($manifest -notmatch 'rust-version\s*=\s*"([^"]+)"') {
    $failures.Add("workspace rust-version is missing")
    $msrv = $null
} else {
    $msrv = $Matches[1]
}

if ($manifest -notmatch 'version\s*=\s*"([^"]+)"') {
    $failures.Add("workspace version is missing")
    $version = $null
} else {
    $version = $Matches[1]
}

if ($msrv -and $msrv -ne "1.85") {
    $failures.Add("workspace rust-version must remain 1.85 unless the MSRV policy is intentionally updated")
}

$ci = Get-Content -LiteralPath ".github/workflows/ci.yml" -Raw
if ($msrv -and $ci -notmatch "dtolnay/rust-toolchain@$([regex]::Escape($msrv))") {
    $failures.Add("CI MSRV toolchain must match workspace rust-version $msrv")
}

foreach ($crateManifest in Get-ChildItem -Path crates -Directory | ForEach-Object { Join-Path $_.FullName "Cargo.toml" }) {
    $relative = Resolve-Path -LiteralPath $crateManifest -Relative
    $contents = Get-Content -LiteralPath $crateManifest -Raw
    if ($contents -notmatch 'version\.workspace\s*=\s*true') {
        $failures.Add("$relative must inherit version.workspace")
    }
    if ($contents -notmatch 'rust-version\.workspace\s*=\s*true') {
        $failures.Add("$relative must inherit rust-version.workspace")
    }
}

$changelog = Get-Content -LiteralPath "CHANGELOG.md" -Raw
if ($changelog -notmatch '(?m)^## Unreleased\s*$') {
    $failures.Add("CHANGELOG.md must contain an Unreleased section")
}
if ($version -and $changelog -notmatch [regex]::Escape($version)) {
    $failures.Add("CHANGELOG.md must mention workspace version $version before release")
}

if ($ci -notmatch 'cargo-semver-checks --version 0\.42\.0 --locked') {
    $failures.Add("CI must install pinned cargo-semver-checks 0.42.0")
}
if ($ci -notmatch 'cargo semver-checks check-release --workspace --baseline-rev origin/main') {
    $failures.Add("CI must run cargo semver-checks check-release --workspace --baseline-rev origin/main")
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        Write-Error "release policy violation: $failure"
    }
    exit 1
}

Write-Host "release policy checks passed"

$ErrorActionPreference = "Stop"

$manifestPath = "fixtures/compatibility/release-readiness.json"
$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
$workspace = Get-Content -LiteralPath "Cargo.toml" -Raw
$changelog = Get-Content -LiteralPath "CHANGELOG.md" -Raw
$errors = New-Object System.Collections.Generic.List[string]

if ($manifest.schema_version.major -ne 1) {
    $errors.Add("release readiness schema_version.major must be 1")
}

$version = $manifest.release_version
if (-not $version) {
    $errors.Add("release_version is required")
} elseif ($workspace -notmatch "version\s*=\s*`"$([regex]::Escape($version))`"") {
    $errors.Add("release_version $version must match workspace version")
} elseif ($changelog -notmatch "## $([regex]::Escape($version))") {
    $errors.Add("CHANGELOG.md must contain section for $version")
}

if ($manifest.phase -ne 10) {
    $errors.Add("phase must be 10")
}

if ($manifest.status -ne "ready_for_0_1_0_release_candidate") {
    $errors.Add("status must be ready_for_0_1_0_release_candidate")
}

$gates = @($manifest.required_gates)
if ($gates.Count -eq 0) {
    $errors.Add("required_gates must not be empty")
}

$seen = @{}
for ($i = 0; $i -lt $gates.Count; $i++) {
    $gate = $gates[$i]
    $prefix = "required_gates[$i]"

    if (-not $gate.id) {
        $errors.Add("$prefix.id is required")
    } elseif ($seen.ContainsKey($gate.id)) {
        $errors.Add("duplicate gate id: $($gate.id)")
    } else {
        $seen[$gate.id] = $true
    }

    if (-not $gate.command) {
        $errors.Add("$prefix.command is required")
    } elseif (-not (Test-Path -LiteralPath $gate.command)) {
        $errors.Add("$prefix.command does not exist: $($gate.command)")
    }

    if (-not $gate.purpose) {
        $errors.Add("$prefix.purpose is required")
    }
}

if (@($manifest.deferred).Count -eq 0) {
    $errors.Add("deferred must record known non-goals")
}

if ($errors.Count -gt 0) {
    foreach ($errorItem in $errors) {
        Write-Error "release readiness error: $errorItem"
    }
    exit 1
}

Write-Host "validated release readiness for $version"

$ErrorActionPreference = "Stop"

$manifestPath = "schemas/manifest.json"
$failures = New-Object System.Collections.Generic.List[string]

if (-not (Test-Path -LiteralPath $manifestPath)) {
    Write-Error "schema manifest missing: $manifestPath"
    exit 1
}

try {
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
} catch {
    Write-Error "schema manifest is invalid JSON: $_"
    exit 1
}

$manifestEntries = @($manifest.artifacts)
$manifestPaths = @{}
foreach ($entry in $manifestEntries) {
    $manifestPaths[$entry.path] = $entry
}

$schemaFiles = @(Get-ChildItem -Path schemas -Filter *.schema.json | Sort-Object Name)
foreach ($schemaFile in $schemaFiles) {
    $relative = "schemas/$($schemaFile.Name)"
    if (-not $manifestPaths.ContainsKey($relative)) {
        $failures.Add("$relative is not listed in $manifestPath")
    }

    try {
        $schema = Get-Content -LiteralPath $schemaFile.FullName -Raw | ConvertFrom-Json
    } catch {
        $failures.Add("$relative is invalid JSON: $_")
        continue
    }

    if (-not $schema.'$schema') {
        $failures.Add("$relative must declare `$schema")
    }
    if (-not $schema.'$id') {
        $failures.Add("$relative must declare `$id")
    }
    if (-not $schema.'x-susun-artifact') {
        $failures.Add("$relative must declare x-susun-artifact")
    }
    if (-not $schema.'x-susun-version') {
        $failures.Add("$relative must declare x-susun-version")
    }
    if (-not $schema.'x-susun-secret-policy') {
        $failures.Add("$relative must declare x-susun-secret-policy")
    }

    $raw = Get-Content -LiteralPath $schemaFile.FullName -Raw
    if ($raw -match '"(password|token|credential|secret_value|private_key|cleartext)"\s*:') {
        $failures.Add("$relative defines a prohibited cleartext credential field")
    }
}

foreach ($path in $manifestPaths.Keys) {
    if (-not (Test-Path -LiteralPath $path)) {
        $failures.Add("$manifestPath references missing schema: $path")
    }
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        Write-Error "schema package violation: $failure"
    }
    exit 1
}

Write-Host "json schema package checks passed"

$ErrorActionPreference = "Stop"

$failures = New-Object System.Collections.Generic.List[string]

function Get-CrateName {
    param([string]$Manifest)

    $inPackage = $false
    foreach ($line in Get-Content -LiteralPath $Manifest) {
        if ($line -match '^\[package\]') {
            $inPackage = $true
            continue
        }
        if ($line -match '^\[') {
            $inPackage = $false
        }
        if ($inPackage -and $line -match '^name\s*=\s*"([^"]+)"') {
            return $Matches[1]
        }
    }

    throw "could not determine crate name for $Manifest"
}

function Get-DependencyNames {
    param(
        [string]$Manifest,
        [string]$Section
    )

    $active = $false
    $deps = New-Object System.Collections.Generic.List[string]
    foreach ($line in Get-Content -LiteralPath $Manifest) {
        if ($line -match '^\[') {
            $active = ($line.Trim() -eq $Section)
            continue
        }
        if ($active -and $line -match '^([A-Za-z0-9_-]+)\s*=') {
            $deps.Add($Matches[1])
        }
    }
    return $deps
}

foreach ($manifest in Get-ChildItem -Path crates -Directory | ForEach-Object { Join-Path $_.FullName "Cargo.toml" }) {
    $crate = Get-CrateName -Manifest $manifest
    $deps = @(Get-DependencyNames -Manifest $manifest -Section "[dependencies]")

    if ($crate -ne "susun-engine-bollard" -and $deps -contains "bollard") {
        $failures.Add("$crate must not depend on bollard; keep Bollard isolated in susun-engine-bollard")
    }

    $pureCrates = @(
        "susun-source",
        "susun-diagnostics",
        "susun-model",
        "susun-normalize",
        "susun-loader",
        "susun-validation",
        "susun-graph",
        "susun-engine",
        "susun-planner"
    )
    if ($pureCrates -contains $crate -and $deps -contains "tokio") {
        $failures.Add("$crate must remain pure/synchronous and must not depend on tokio")
    }

    if ($crate -eq "susun-planner" -and $deps -contains "susun-runtime") {
        $failures.Add("susun-planner must not depend on susun-runtime")
    }

    if ($crate -eq "susun-engine" -and $deps -contains "susun-cli") {
        $failures.Add("susun-engine must not depend on susun-cli")
    }

    if ($deps -contains "susun-testkit") {
        $failures.Add("$crate must not use susun-testkit as a production dependency")
    }
}

foreach ($sourceDir in Get-ChildItem -Path crates -Directory | ForEach-Object { Join-Path $_.FullName "src" }) {
    if (-not (Test-Path -LiteralPath $sourceDir)) {
        continue
    }
    foreach ($source in Get-ChildItem -Path $sourceDir -Recurse -Filter *.rs) {
    $relative = Resolve-Path -LiteralPath $source.FullName -Relative
    if ($relative -like ".\crates\susun-engine-bollard\src\*") {
        continue
    }

    $publicLines = Select-String -Path $source.FullName -Pattern '^\s*pub([({\s]|$)' -ErrorAction SilentlyContinue
    if (-not $publicLines) {
        continue
    }

    foreach ($line in $publicLines) {
        $text = $line.Line
        if ($text -match 'bollard::|Bollard[A-Za-z0-9_]*') {
            $failures.Add("$relative public API must not mention Bollard adapter/backend types")
        }
        if ($text -match 'tokio::sync|tokio::task|JoinHandle') {
            $failures.Add("$relative public API must not mention Tokio channel/task handle types")
        }
        if ($text -match '(?i)buildkit.*(client|proto|transport)|buildx.*(client|proto)|tonic::') {
            $failures.Add("$relative public API must not mention raw BuildKit transport types")
        }
        if ($text -match '(?i)registry.*(client|token|credential)|oci_distribution|reqwest::') {
            $failures.Add("$relative public API must not mention raw registry client types")
        }
    }
    }
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        Write-Error "architecture violation: $failure"
    }
    exit 1
}

Write-Host "architecture dependency checks passed"

$ErrorActionPreference = "Stop"

$metadata = cargo metadata --no-deps --format-version 1 | ConvertFrom-Json
$workspaceRoot = (Resolve-Path -LiteralPath $metadata.workspace_root).Path
$cratesRoot = (Join-Path $workspaceRoot "crates")
$errors = New-Object System.Collections.Generic.List[string]

foreach ($licenseFile in @("LICENSE-MIT", "LICENSE-APACHE")) {
    if (-not (Test-Path -LiteralPath (Join-Path $workspaceRoot $licenseFile))) {
        $errors.Add("$licenseFile is required for MIT OR Apache-2.0 publication")
    }
}

$rootManifest = Get-Content -LiteralPath (Join-Path $workspaceRoot "Cargo.toml") -Raw
foreach ($field in @("description", "repository", "homepage", "readme", "keywords", "categories")) {
    if ($rootManifest -notmatch "(?m)^$field\s*=") {
        $errors.Add("workspace.package.$field is required")
    }
}

$publishable = New-Object System.Collections.Generic.List[string]

foreach ($package in $metadata.packages) {
    $manifestPath = [string]$package.manifest_path
    if (-not $manifestPath.StartsWith($cratesRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
        continue
    }

    $publishDisabled = ($null -ne $package.publish -and @($package.publish).Count -eq 0)
    $contents = Get-Content -LiteralPath $manifestPath -Raw

    foreach ($field in @("description", "repository", "homepage", "readme", "keywords", "categories")) {
        if ($contents -notmatch "(?m)^$field\.workspace\s*=\s*true") {
            $errors.Add("$($package.name) must inherit $field.workspace")
        }
    }

    if (-not $publishDisabled) {
        $publishable.Add($package.name)
        foreach ($dependency in $package.dependencies) {
            # Path-only dev-dependencies are exempt: cargo strips them from
            # the packaged crate entirely, so they never need a registry
            # version. Requiring one here would force circular internal
            # dependencies (e.g. a crate's tests depending on a crate that
            # depends on it normally) to be permanently unpublishable.
            if ($dependency.kind -eq "dev") {
                continue
            }
            if ($null -eq $dependency.source -and $dependency.path) {
                $dependencyPath = [string]$dependency.path
                if ($dependencyPath.StartsWith($cratesRoot, [System.StringComparison]::OrdinalIgnoreCase) -and $dependency.req -eq "*") {
                    $errors.Add("$($package.name) depends on $($dependency.name) without a publishable version requirement")
                }
            }
        }
    }
}

if ($publishable.Count -eq 0) {
    $errors.Add("no publishable crates found under crates/")
}

if ($errors.Count -gt 0) {
    foreach ($errorItem in $errors) {
        Write-Error "package readiness error: $errorItem"
    }
    exit 1
}

if ($env:SUSUN_SKIP_PACKAGE_DRY_RUN -eq "1") {
    Write-Host "package readiness metadata checks passed; package assembly skipped by SUSUN_SKIP_PACKAGE_DRY_RUN=1"
    exit 0
}

foreach ($packageName in ($publishable | Sort-Object -Unique)) {
    Write-Host "checking package assembly for $packageName"
    cargo package -p $packageName --allow-dirty --no-verify --list | Out-Null
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
}

Write-Host "package readiness checks passed"

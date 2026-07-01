$ErrorActionPreference = "Stop"
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
$GitSafeDirectory = $RepoRoot.Replace("\", "/")
Set-Location $RepoRoot

powershell -ExecutionPolicy Bypass -File scripts\check-architecture.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-diagnostics.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-schemas.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-release-policy.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-real-world-catalog.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-release-readiness.ps1
powershell -ExecutionPolicy Bypass -File scripts\check-sdk-readiness.ps1
powershell -ExecutionPolicy Bypass -File scripts\generate-release-docs.ps1
Push-Location $RepoRoot
try {
    git -c "safe.directory=$GitSafeDirectory" rev-parse --show-toplevel | Out-Null
    if ($LASTEXITCODE -ne 0) {
        Write-Error "phase8 gate must run inside the Susun git repository"
        exit $LASTEXITCODE
    }
    git -c "safe.directory=$GitSafeDirectory" diff --exit-code -- docs/generated/capability-and-compatibility.md
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}

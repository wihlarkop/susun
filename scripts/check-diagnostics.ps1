$ErrorActionPreference = "Stop"

$catalog = "crates/susun-diagnostics/catalog.toml"
$failures = New-Object System.Collections.Generic.List[string]

if (-not (Test-Path -LiteralPath $catalog)) {
    Write-Error "diagnostic catalog missing: $catalog"
    exit 1
}

$codePattern = '^SUS-[A-Z0-9]+(?:-[A-Z0-9]+)+$'
$catalogCodes = New-Object System.Collections.Generic.List[string]

foreach ($match in Select-String -Path $catalog -Pattern '^\s*code\s*=\s*"([^"]+)"') {
    $code = $match.Matches[0].Groups[1].Value
    if ($code -notmatch $codePattern) {
        $failures.Add("$catalog contains invalid diagnostic code format: $code")
    }
    $catalogCodes.Add($code)
}

$seen = @{}
foreach ($code in $catalogCodes) {
    if ($seen.ContainsKey($code)) {
        $failures.Add("$catalog contains duplicate diagnostic code: $code")
    } else {
        $seen[$code] = $true
    }
}

$used = @{}
foreach ($sourceDir in Get-ChildItem -Path crates -Directory | ForEach-Object { Join-Path $_.FullName "src" }) {
    if (-not (Test-Path -LiteralPath $sourceDir)) {
        continue
    }

    foreach ($source in Get-ChildItem -Path $sourceDir -Recurse -Filter *.rs) {
        $relative = Resolve-Path -LiteralPath $source.FullName -Relative
        $contents = Get-Content -LiteralPath $source.FullName -Raw
        foreach ($match in [regex]::Matches($contents, 'SUS-[A-Z0-9]+(?:-[A-Z0-9]+)+')) {
            $used[$match.Value] = $relative
        }
    }
}

foreach ($code in ($used.Keys | Sort-Object)) {
    if (-not $seen.ContainsKey($code)) {
        $failures.Add("$($used[$code]) emits undocumented diagnostic code: $code")
    }
}

if ($failures.Count -gt 0) {
    foreach ($failure in $failures) {
        Write-Error "diagnostic catalog violation: $failure"
    }
    exit 1
}

Write-Host "diagnostic catalog checks passed"

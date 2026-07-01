$ErrorActionPreference = "Stop"

$catalogPath = "fixtures/compatibility/real-world-catalog.json"
$corpusPath = "fixtures/compatibility/corpus.json"

$catalog = Get-Content -LiteralPath $catalogPath -Raw | ConvertFrom-Json
$corpus = Get-Content -LiteralPath $corpusPath -Raw | ConvertFrom-Json
$errors = New-Object System.Collections.Generic.List[string]

if ($catalog.schema_version.major -ne 1) {
    $errors.Add("catalog schema_version.major must be 1")
}

$patterns = @($catalog.patterns)
if ($patterns.Count -eq 0) {
    $errors.Add("catalog must contain at least one pattern")
}

$fixtureIds = @{}
foreach ($fixture in $corpus.fixtures) {
    $fixtureIds[$fixture.id] = $true
}

$validSupport = @("supported", "supported_subset", "experimental", "unsupported")
$validOperations = @("config", "analyze", "plan", "build", "run", "exec", "events", "wait", "cp", "port", "watch")
$seen = @{}

for ($i = 0; $i -lt $patterns.Count; $i++) {
    $pattern = $patterns[$i]
    $prefix = "patterns[$i]"

    if (-not $pattern.id) {
        $errors.Add("$prefix.id is required")
    } elseif ($seen.ContainsKey($pattern.id)) {
        $errors.Add("duplicate pattern id: $($pattern.id)")
    } else {
        $seen[$pattern.id] = $true
    }

    if ($validSupport -notcontains $pattern.support) {
        $errors.Add("$prefix.support has invalid value")
    }

    $fixtures = @($pattern.fixtures)
    if ($fixtures.Count -eq 0) {
        $errors.Add("$prefix.fixtures must not be empty")
    }
    foreach ($fixture in $fixtures) {
        if (-not $fixtureIds.ContainsKey($fixture)) {
            $errors.Add("$prefix.fixtures references unknown fixture '$fixture'")
        }
    }

    $operations = @($pattern.operations)
    if ($operations.Count -eq 0) {
        $errors.Add("$prefix.operations must not be empty")
    }
    foreach ($operation in $operations) {
        if ($validOperations -notcontains $operation) {
            $errors.Add("$prefix.operations contains unknown operation '$operation'")
        }
    }

    $deferred = @($pattern.deferred)
    if (($pattern.support -eq "supported_subset" -or $pattern.support -eq "experimental" -or $pattern.support -eq "unsupported") -and $deferred.Count -eq 0) {
        $errors.Add("$prefix.deferred must explain gaps for $($pattern.support)")
    }

    if (-not $pattern.evidence) {
        $errors.Add("$prefix.evidence is required")
    }
}

if ($errors.Count -gt 0) {
    foreach ($errorItem in $errors) {
        Write-Error "real-world catalog error: $errorItem"
    }
    exit 1
}

Write-Host "validated $($patterns.Count) real-world compatibility patterns"

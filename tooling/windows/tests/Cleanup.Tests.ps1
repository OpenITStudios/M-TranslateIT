$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
Import-Module (Join-Path $PSScriptRoot '../modules/Cleanup.psm1') -Force
$repository = (Resolve-Path (Join-Path $PSScriptRoot '../../..')).Path
$testParent = Join-Path $repository '.workspace/temp'
New-Item -ItemType Directory -Path $testParent -Force | Out-Null
$testRoot = Join-Path $testParent ('cleanup-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$git = (Get-Command git -ErrorAction Stop).Source
$link = Join-Path $testRoot 'EngineData/Frontend/RustApp/src-tauri/target/redirect'
function Assert-Blocked([scriptblock]$Action, [string]$ExpectedReason) {
    $blocked = $false
    try { & $Action } catch {
        if ($_.Exception.Message -notmatch $ExpectedReason) { throw }
        $blocked = $true
    }
    if (-not $blocked) { throw "Cleanup did not block: $ExpectedReason" }
}
try {
    & $git -C $testRoot init -q
    if ($LASTEXITCODE -ne 0) { throw 'Could not initialize cleanup fixture.' }
    Set-Content -LiteralPath (Join-Path $testRoot 'manage.cmd') -Value 'fixture'
    Set-Content -LiteralPath (Join-Path $testRoot '.gitignore') -Value @('target/', 'dist/', '.workspace/', '.pytest_cache/', '.ruff_cache/')
    $target = Join-Path $testRoot 'EngineData/Frontend/RustApp/src-tauri/target'
    $protected = Join-Path $testRoot 'UserData/SavedProject'
    $assets = Join-Path $testRoot 'EngineData/Backend/RuntimeAssets'
    $reports = Join-Path $testRoot '.workspace/reports'
    $staging = Join-Path $testRoot '.workspace/staging'
    foreach ($path in @($target, $protected, $assets, $reports, $staging)) {
        New-Item -ItemType Directory -Path $path -Force | Out-Null
        Set-Content -LiteralPath (Join-Path $path 'keep.txt') -Value 'sentinel'
    }
    # Isolate process gating from unrelated developer activity during filesystem tests.
    & (Get-Module Cleanup) { function script:Get-CimInstance { @() } }
    $before = @(Get-ChildItem -LiteralPath $testRoot -Recurse -Force).Count
    Invoke-TranslateITCleanup -RepositoryRoot $testRoot
    if (@(Get-ChildItem -LiteralPath $testRoot -Recurse -Force).Count -ne $before) { throw 'Dry run mutated fixture.' }
    & $git -C $testRoot add -f -- EngineData/Frontend/RustApp/src-tauri/target/keep.txt
    if ($LASTEXITCODE -ne 0) { throw 'Could not stage tracked-content fixture.' }
    Assert-Blocked { Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Apply } 'tracked or unverifiable content'
    & $git -C $testRoot rm --cached -q -- EngineData/Frontend/RustApp/src-tauri/target/keep.txt
    if ($LASTEXITCODE -ne 0) { throw 'Could not unstage fixture.' }
    New-Item -ItemType Junction -Path $link -Target $protected | Out-Null
    Assert-Blocked { Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Apply } 'redirected content'
    [IO.Directory]::Delete($link)
    & (Get-Module Cleanup) { function script:Get-CimInstance { [pscustomobject]@{ ProcessId=0; Name='cargo.exe'; ExecutablePath=''; CommandLine='' } } }
    Assert-Blocked { Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Apply } 'active development/runtime processes'
    if (-not (Test-Path -LiteralPath (Join-Path $target 'keep.txt'))) { throw 'Blocked cleanup deleted content.' }
    & (Get-Module Cleanup) { function script:Get-CimInstance { @() } }
    # Editor tooling can mention the repository as cwd without using app/build paths.
    & (Get-Module Cleanup) { param($fixtureRoot)
        $script:fixtureRoot = $fixtureRoot
        function script:Get-CimInstance { [pscustomobject]@{ ProcessId=0; Name='node.exe'; ExecutablePath='C:\editor\node.exe'; CommandLine=('editor --cwd "' + $script:fixtureRoot + '"') } }
    } $testRoot
    Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Apply
    if (Test-Path -LiteralPath $target) { throw 'Build output survived cleanup.' }
    foreach ($path in @($protected, $assets, $reports, $staging)) {
        if ((Get-Content -LiteralPath (Join-Path $path 'keep.txt') -Raw).Trim() -ne 'sentinel') { throw 'Protected content changed.' }
    }
    Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Apply
    $cache = Join-Path $testRoot '.workspace/cache/npm'
    New-Item -ItemType Directory -Path $cache -Force | Out-Null
    Set-Content -LiteralPath (Join-Path $cache 'cache.bin') -Value 'renewable'
    # A non-ignored later candidate must block the entire plan before npm is deleted.
    Set-Content -LiteralPath (Join-Path $testRoot '.gitignore') -Value '.workspace/cache/npm/'
    Assert-Blocked { Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Profile cache -Apply } 'requires an ignored output'
    if (-not (Test-Path -LiteralPath $cache)) { throw 'Partial deletion before full preflight.' }
    Set-Content -LiteralPath (Join-Path $testRoot '.gitignore') -Value @('.workspace/', '.pytest_cache/', '.ruff_cache/')
    Invoke-TranslateITCleanup -RepositoryRoot $testRoot -Profile cache -Apply
    if (Test-Path -LiteralPath $cache) { throw 'Renewable cache survived cleanup.' }
    Write-Host 'Cleanup dry-run, tracked/ignored checks, junction protection, active-process block, protected data and idempotence: PASS'
} finally {
    if (Test-Path -LiteralPath $link) { [IO.Directory]::Delete($link) }
    $resolved = (Get-Item -LiteralPath $testRoot -Force).FullName
    if (-not $resolved.StartsWith($testParent + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe fixture cleanup path.' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
    Remove-Module Cleanup
}

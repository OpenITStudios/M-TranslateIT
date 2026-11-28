$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

throw 'Native My Voice packaging is held pending its exact artifact closure and integrated target acceptance.'

$Implementation = Join-Path $PSScriptRoot 'stage_release_inputs_impl.ps1'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
Import-Module (Join-Path $PSScriptRoot 'ReleaseStaging.psm1') -Force
Assert-TranslateITReleaseStagingAvailable -RepositoryRoot $RepoRoot
$stagingBase = Join-Path $RepoRoot '.workspace\staging'
New-Item -ItemType Directory -Force -Path $stagingBase | Out-Null
$lock = Join-Path $stagingBase 'release-active'
New-Item -ItemType Directory -Path $lock -ErrorAction Stop | Out-Null
try {
    & $Implementation -RepositoryRoot $RepoRoot
} finally {
    Remove-Item -LiteralPath $lock -ErrorAction Stop
}

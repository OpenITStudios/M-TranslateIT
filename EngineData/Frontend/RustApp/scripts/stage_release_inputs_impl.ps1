param([Parameter(Mandatory=$true)][string]$RepositoryRoot)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

throw 'Native packaging is held pending exact artifact closure and integrated target acceptance.'

$RepoRoot = (Resolve-Path -LiteralPath $RepositoryRoot).Path
$expectedRepo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..\..')).Path
if ($RepoRoot -cne $expectedRepo) { throw 'Staging requires the owning repository.' }
Import-Module (Join-Path $PSScriptRoot 'ReleaseStaging.psm1') -Force
$workspace = Join-Path $RepoRoot '.workspace\staging'
$lock = Join-Path $workspace 'release-active'
if (-not (Test-Path -LiteralPath $lock -PathType Container)) { throw 'Release input staging must start from stage_release_inputs.ps1.' }
$builder = Join-Path $PSScriptRoot 'build_r3_external_payload.py'
$hostPython = (Get-Command python -ErrorAction Stop).Source

function Assert-ProviderSignature([string]$Root) {
    $provider = Join-Path $Root 'EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package'
    foreach ($exeName in @('VBCABLE_Setup.exe','VBCABLE_Setup_x64.exe')) {
        $sig = Get-AuthenticodeSignature -LiteralPath (Join-Path $provider $exeName)
        if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notmatch 'BUREL VINCENT') {
            throw "VB-CABLE Authenticode invalid: $exeName"
        }
    }
}

# No downloads or dependency installs here. The existing developer model
# acquisition and reviewed offline artifacts are separate inputs.
& $hostPython -B -s $builder --repo-root $RepoRoot --verify-inputs
if ($LASTEXITCODE -eq 0) {
    Assert-ProviderSignature $RepoRoot
    Write-Host '[release-stage] Exact native inputs already present; no staging or replacement needed.'
    return
}
if (-not $env:TRANSLATEIT_NATIVE_RELEASE_INPUT_ROOT) {
    throw 'A complete reviewed native candidate below .workspace/staging is required.'
}
$PayloadRoot = (Resolve-Path -LiteralPath $env:TRANSLATEIT_NATIVE_RELEASE_INPUT_ROOT).Path
Assert-TranslateITPrivateStage -RepositoryRoot $RepoRoot -StageRoot $PayloadRoot
& $hostPython -B -s $builder --repo-root $RepoRoot --verify-inputs --asset-root $PayloadRoot
if ($LASTEXITCODE -ne 0) { throw 'Native candidate verification failed; no promotion attempted.' }

# Verify repository-owned built-in references without rewriting tracked files.
# Candidate built-ins must be an exact copy of the tracked source closure;
# promotion never moves the tracked BuiltInVoices directory.
$builtin = 'EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices'
$tracked = Join-Path $RepoRoot $builtin
$candidate = Join-Path $PayloadRoot $builtin
foreach ($relative in @('SOURCES.json','MaleVoice/reference.wav','MaleVoice/REFERENCE_SOURCE.txt','FemaleVoice/reference.wav','FemaleVoice/REFERENCE_SOURCE.txt')) {
    $source = Join-Path $tracked $relative
    $target = Join-Path $candidate $relative
    if ((Get-FileHash -LiteralPath $source -Algorithm SHA256).Hash -cne (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash) {
        throw "Built-in voice source or attribution mismatch: $relative"
    }
}

Assert-ProviderSignature $PayloadRoot

# Move the already verified candidate on the same volume; no second GB-scale copy.
Publish-TranslateITReleaseStage -RepositoryRoot $RepoRoot -StageRoot $PayloadRoot
Write-Host '[release-stage] Native inputs promoted; artifact build and installed-runtime proof remain separate.'

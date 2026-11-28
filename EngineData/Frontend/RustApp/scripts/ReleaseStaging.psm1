# Controlled release input promotion; staged payload and rollback stay on one volume.
# Only the canonical runtime subtrees below can be replaced. UserData is forbidden.
Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

function Get-TranslateITReleasePayloadRoots([string]$RepositoryRoot) {
    $manifestPath = Join-Path $RepositoryRoot 'EngineData/Backend/LocalWorker/WorkerRuntime/model_manifest.json'
    $manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
    if ($manifest.native_payload.schema -cne 'translateit.r3.external_payload.v3') { throw 'Native staging schema mismatch.' }
    $tracked = 'EngineData/Backend/RuntimeAssets/Voice/BuiltInVoices'
    # Explicit safety allowlist for the destructive promotion boundary.
    $allowed = @(
        'EngineData/Backend/RuntimeAssets/ASR/ModelData',
        'EngineData/Backend/RuntimeAssets/Translation/ModelData',
        'EngineData/Backend/RuntimeAssets/NativeCuda',
        'EngineData/Backend/RuntimeAssets/NativeOrt',
        $tracked,
        'EngineData/Backend/RuntimeAssets/Voice/ModelData',
        'EngineData/Backend/RuntimeAssets/AudioProvider/VBCABLE/Package'
    )
    $roots = @($manifest.native_payload.roots)
    if (($roots -join '|') -cne ($allowed -join '|')) { throw 'Native staging root closure mismatch.' }
    return @($roots | Where-Object { $_ -cne $tracked })
}

function Assert-TranslateITPrivateStage {
    [CmdletBinding()]
    param([Parameter(Mandatory=$true)][string]$RepositoryRoot,
          [Parameter(Mandatory=$true)][string]$StageRoot)
    $repo = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $stage = (Resolve-Path -LiteralPath $StageRoot).Path
    $workspace = Join-Path $repo '.workspace'
    $private = Join-Path $workspace 'staging'
    if (-not $stage.StartsWith($private + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
        throw "Untrusted release stage: $stage"
    }
    $relative = $stage.Substring($private.Length + 1)
    $first = $relative.Split([IO.Path]::DirectorySeparatorChar)[0]
    if ($first -in @('release-active','release-rollback')) { throw 'Reserved staging directory cannot supply payload.' }
    $dir = [IO.DirectoryInfo]::new($stage)
    while ($null -ne $dir) {
        if (-not $dir.Exists -or ($dir.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
            throw "Release promotion forbids redirected staging paths: $($dir.FullName)"
        }
        if ($dir.FullName -ieq $repo) { break }
        $dir = $dir.Parent
    }
    if ($null -eq $dir) { throw 'Stage is not under repository root.' }
}

function Assert-TranslateITReleaseStagingAvailable {
    [CmdletBinding()]
    param([Parameter(Mandatory = $true)][string]$RepositoryRoot)

    $workspaceRoot = Join-Path $RepositoryRoot ".workspace"
    $workspace = Join-Path $workspaceRoot "staging"
    $backup = Join-Path $workspace "release-rollback"
    $lock = Join-Path $workspace "release-active"
    foreach ($p in @($workspaceRoot, $workspace, $backup, $lock)) {
        if (Test-Path -LiteralPath $p) {
            $item = Get-Item -LiteralPath $p -Force
            if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                throw "Release staging refuses reparse paths: $p"
            }
        }
    }
    if (Test-Path -LiteralPath $backup) {
        throw "Previous release promotion backup remains; inspect and recover before retry: $backup"
    }
    if (Test-Path -LiteralPath $lock) {
        throw "Another release staging operation or interrupted run owns: $lock"
    }
}

function Publish-TranslateITReleaseStage {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [Parameter(Mandatory = $true)][string]$StageRoot
    )

    $repo = (Resolve-Path -LiteralPath $RepositoryRoot).Path
    $stage = (Resolve-Path -LiteralPath $StageRoot).Path
    Assert-TranslateITPrivateStage -RepositoryRoot $repo -StageRoot $stage
    $releasePayloadRoots = @(Get-TranslateITReleasePayloadRoots $repo)
    $privateStaging = Join-Path $repo '.workspace/staging'
    $backup = Join-Path $privateStaging "release-rollback"
    $lock = Join-Path $privateStaging "release-active"
    if (-not (Test-Path -LiteralPath $lock -PathType Container)) {
        throw "Release promotion needs the exclusive staging reservation."
    }
    if (Test-Path -LiteralPath $backup) {
        throw "Unfinished release rollback state is present: $backup"
    }
    foreach ($relative in $releasePayloadRoots) {
        $from = Join-Path $stage $relative
        if (-not (Test-Path -LiteralPath $from -PathType Container)) {
            throw "Missing verified staging subtree: $relative"
        }
        $source = Get-Item -LiteralPath $from -Force
        if ($source.Attributes -band [IO.FileAttributes]::ReparsePoint) {
            throw "Staged subtree must not be a reparse point: $relative"
        }
        $dest = Join-Path $repo $relative
        $parent = [IO.DirectoryInfo]::new((Split-Path -Parent $dest))
        while ($null -ne $parent -and $parent.FullName -ine $repo) {
            if ($parent.Exists -and ($parent.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Runtime destination parent is redirected: $relative"
            }
            $parent = $parent.Parent
        }
        if (Test-Path -LiteralPath $dest) {
            $existing = Get-Item -LiteralPath $dest -Force
            if (-not $existing.PSIsContainer -or
                ($existing.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                throw "Runtime destination is unsafe: $relative"
            }
        }
    }

    # Creating this directory is the atomic promotion reservation. If power is
    # interrupted, preserve it as a recovery signal; never silently erase it.
    New-Item -ItemType Directory -Path $backup -ErrorAction Stop | Out-Null
    $moved = New-Object System.Collections.Generic.List[object]
    try {
        foreach ($relative in $releasePayloadRoots) {
            $from = Join-Path $stage $relative
            $to = Join-Path $repo $relative
            $saved = Join-Path $backup $relative
            $item = [pscustomobject]@{
                from = $from; to = $to; saved = $saved;
                oldMoved = $false; newMoved = $false
            }
            $moved.Add($item)
            New-Item -ItemType Directory -Force -Path (Split-Path -Parent $to),
                (Split-Path -Parent $saved) | Out-Null
            if (Test-Path -LiteralPath $to) {
                Move-Item -LiteralPath $to -Destination $saved -ErrorAction Stop
                $item.oldMoved = $true
            }
            Move-Item -LiteralPath $from -Destination $to -ErrorAction Stop
            $item.newMoved = $true
        }
        foreach ($relative in $releasePayloadRoots) {
            if (-not (Test-Path -LiteralPath (Join-Path $repo $relative) -PathType Container)) {
                throw "Promoted release subtree went missing: $relative"
            }
        }
    } catch {
        $original = $_
        $rollbackFailed = $false
        for ($i = $moved.Count - 1; $i -ge 0; $i--) {
            $item = $moved[$i]
            try {
                if ($item.newMoved) {
                    if (-not (Test-Path -LiteralPath $item.to) -or (Test-Path -LiteralPath $item.from)) {
                        throw "Cannot restore staged candidate safely: $($item.from)"
                    }
                    # Preserve the verified candidate for retry rather than
                    # discard multi-GB files on a later promotion failure.
                    Move-Item -LiteralPath $item.to -Destination $item.from -ErrorAction Stop
                }
                if ($item.oldMoved) {
                    if (-not (Test-Path -LiteralPath $item.saved)) {
                        throw "Original runtime backup is missing: $($item.saved)"
                    }
                    Move-Item -LiteralPath $item.saved -Destination $item.to -ErrorAction Stop
                }
            } catch {
                $rollbackFailed = $true
                Write-Error -Message "Release staging rollback needs manual recovery: $($_.Exception.Message)" -ErrorAction Continue
            }
        }
        if (-not $rollbackFailed) {
            Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction Stop
        }
        throw "Release promotion failed. Rolled back: $(-not $rollbackFailed). Original error: $original"
    }
    Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction Stop
    Write-Host "[release-stage] Verified runtime subtrees committed; previous versions retired."
}

Export-ModuleMember -Function Assert-TranslateITReleaseStagingAvailable, Assert-TranslateITPrivateStage, Publish-TranslateITReleaseStage

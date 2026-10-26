# Only renewable development outputs. Runtime assets, staging and UserData are excluded.
Set-StrictMode -Version Latest

function Get-TranslateITCleanupPlan {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [ValidateSet('build', 'cache')][string]$Profile = 'build'
    )
    $root = (Get-Item -LiteralPath $RepositoryRoot -Force -ErrorAction Stop).FullName
    if (-not (Test-Path -LiteralPath (Join-Path $root 'manage.cmd') -PathType Leaf)) {
        throw 'Cleanup requires a TranslateIT repository root.'
    }
    # Inspect every ancestor before enumeration; Resolve-Path alone does not reject junctions.
    $ancestor = Get-Item -LiteralPath $root -Force -ErrorAction Stop
    while ($null -ne $ancestor) {
        if ($ancestor.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw "Redirected cleanup ancestor: $($ancestor.FullName)" }
        $ancestor = $ancestor.Parent
    }
    $paths = if ($Profile -eq 'build') {
        @('EngineData/Frontend/RustApp/src-tauri/target', 'EngineData/Frontend/RustApp/dist')
    } else {
        @('.workspace/cache/npm', '.workspace/cache/uv', '.workspace/cache/pip', '.pytest_cache', '.ruff_cache')
    }
    foreach ($relative in $paths) {
        $path = [IO.Path]::GetFullPath((Join-Path $root $relative))
        if (-not $path.StartsWith($root.TrimEnd('\') + '\', [StringComparison]::OrdinalIgnoreCase)) {
            throw "Cleanup path escaped repository: $path"
        }
        $current = $root
        foreach ($part in $relative.Split('/')) {
            $current = Join-Path $current $part
            if (Test-Path -LiteralPath $current) {
                $item = Get-Item -LiteralPath $current -Force -ErrorAction Stop
                if (-not $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
                    throw "Unsafe cleanup directory: $current"
                }
            }
        }
        [long]$bytes = 0
        [long]$files = 0
        $exists = Test-Path -LiteralPath $path
        if ($exists) {
            $pending = [Collections.Generic.Stack[string]]::new()
            $pending.Push($path)
            while ($pending.Count -gt 0) {
                foreach ($item in Get-ChildItem -LiteralPath $pending.Pop() -Force -ErrorAction Stop) {
                    if ($item.Attributes -band [IO.FileAttributes]::ReparsePoint) {
                        throw "Cleanup refuses redirected content: $($item.FullName)"
                    }
                    if ($item.PSIsContainer) { $pending.Push($item.FullName) }
                    else { $bytes += $item.Length; $files++ }
                }
            }
        }
        [pscustomobject]@{ RelativePath=$relative; Path=$path; Exists=$exists; Bytes=$bytes; Files=$files }
    }
}

function Invoke-TranslateITCleanup {
    [CmdletBinding()]
    param(
        [Parameter(Mandatory = $true)][string]$RepositoryRoot,
        [ValidateSet('build', 'cache')][string]$Profile = 'build',
        [switch]$Apply
    )
    $root = (Get-Item -LiteralPath $RepositoryRoot -Force -ErrorAction Stop).FullName
    $plan = @(Get-TranslateITCleanupPlan -RepositoryRoot $root -Profile $Profile)
    $plan | Select-Object RelativePath, Exists, Files, @{Name='LogicalGiB'; Expression={[math]::Round($_.Bytes / 1GB, 3)}} | Format-Table -AutoSize | Out-Host
    if (-not $Apply) {
        Write-Host 'DRY RUN: nothing deleted. Use -Apply with the same -Profile to execute.'
        return
    }
    # A running tool can recreate outputs while they are being removed. Do not stop processes automatically.
    $busy = @(Get-CimInstance Win32_Process -ErrorAction Stop | Where-Object {
        $_.ProcessId -ne $PID -and (
            $_.Name -match '^(cargo|rustc|rustup|sccache|uv|translateit)(\.exe)?$' -or
            ($_.ExecutablePath -and $_.ExecutablePath.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) -or
            ($_.Name -match '^(node|python|pythonw|npm)(\.exe)?$' -and
                (-not $_.CommandLine -or
                    $_.CommandLine.IndexOf($root + '\EngineData\', [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
                    $_.CommandLine.IndexOf($root + '/EngineData/', [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
                    $_.CommandLine.IndexOf($root + '\.workspace\', [StringComparison]::OrdinalIgnoreCase) -ge 0 -or
                    $_.CommandLine.IndexOf($root + '/.workspace/', [StringComparison]::OrdinalIgnoreCase) -ge 0))
        )
    })
    if ($busy.Count -gt 0) { throw 'Cleanup blocked by active development/runtime processes. Close them before applying.' }
    $localGit = Join-Path $root '.workspace/tools/git/cmd/git.exe'
    $git = if (Test-Path -LiteralPath $localGit -PathType Leaf) { $localGit } else { (Get-Command git -ErrorAction Stop).Source }
    # Validate the complete plan before deleting the first directory.
    foreach ($entry in $plan) {
        $tracked = @(& $git -C $root ls-files -- $entry.RelativePath)
        if ($LASTEXITCODE -ne 0 -or $tracked.Count -gt 0) { throw "Cleanup refuses tracked or unverifiable content: $($entry.RelativePath)" }
        & $git -C $root check-ignore -q -- ($entry.RelativePath + '/')
        if ($LASTEXITCODE -ne 0) { throw "Cleanup requires an ignored output: $($entry.RelativePath)" }
    }
    $fresh = @(Get-TranslateITCleanupPlan -RepositoryRoot $root -Profile $Profile)
    for ($index = 0; $index -lt $plan.Count; $index++) {
        if ($fresh[$index].Bytes -ne $plan[$index].Bytes -or $fresh[$index].Files -ne $plan[$index].Files -or $fresh[$index].Exists -ne $plan[$index].Exists) {
            throw 'Cleanup contents changed during preflight; nothing deleted.'
        }
    }
    foreach ($entry in $fresh) {
        if ($entry.Exists) {
            Remove-Item -LiteralPath $entry.Path -Recurse -Force -ErrorAction Stop
            if (Test-Path -LiteralPath $entry.Path) { throw "Cleanup incomplete: $($entry.Path)" }
            Write-Host "Removed: $($entry.RelativePath)"
        }
    }
    Write-Host 'Cleanup complete. Logical file sizes are not a physical free-space guarantee (hardlinks/compression).'
}

Export-ModuleMember -Function Get-TranslateITCleanupPlan, Invoke-TranslateITCleanup

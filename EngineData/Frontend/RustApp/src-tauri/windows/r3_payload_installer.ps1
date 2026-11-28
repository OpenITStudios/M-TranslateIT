param(
    [Parameter(Mandatory=$true)]
    [ValidateSet('Verify','Install','VerifyInstalled')]
    [string]$Mode,
    [string]$PayloadPath,
    [string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$ExpectedPayloadSchema,
    [Parameter(Mandatory=$true)][string]$ExpectedInstalledRuntimeSchema,
    [Parameter(Mandatory=$true)][string]$ExpectedAppVersion,
    [Parameter(Mandatory=$true)][string]$ExpectedRuntimeContractSha256,
    [Int64]$ExpectedExpandedBytes=0,
    [string]$InstallRoot
)

$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest

$NativePayloadSchema='translateit.r3.external_payload.v3'
$NativeInstalledSchema='translateit.installed_runtime.v3'
$PayloadContract='EngineData\Backend\RuntimeAssets\NativeOrt\TRANSLATEIT_PAYLOAD_CONTRACT.json'
$InstalledManifest='EngineData\Backend\TRANSLATEIT_INSTALLED_RUNTIME.json'
$PayloadRoots=@(
    'EngineData\Backend\RuntimeAssets\ASR\ModelData',
    'EngineData\Backend\RuntimeAssets\Translation\ModelData',
    'EngineData\Backend\RuntimeAssets\NativeCuda',
    'EngineData\Backend\RuntimeAssets\NativeOrt',
    'EngineData\Backend\RuntimeAssets\Voice\BuiltInVoices',
    'EngineData\Backend\RuntimeAssets\Voice\ModelData',
    'EngineData\Backend\RuntimeAssets\AudioProvider\VBCABLE\Package'
)
$RetiredRoots=@(
    'EngineData\Backend\LocalWorker\PythonRuntime',
    'EngineData\Backend\RuntimeAssets\Voice\GPTSoVITS'
)

# Never exit inside a validation helper: failures after promotion must
# unwind through the owning transaction and restore the previous runtime.
function Fail([int]$Code,[string]$Message){
    $failure=[System.InvalidOperationException]::new($Message)
    $failure.Data['TranslateITExitCode']=$Code
    throw $failure
}
function NeedFile([string]$Path,[int]$Code,[string]$Label){if(-not(Test-Path -LiteralPath $Path -PathType Leaf)){Fail $Code "$Label missing: $Path"}}

function Get-TarProgram {
    $systemTar=Join-Path $env:SystemRoot 'System32\tar.exe'
    if(Test-Path -LiteralPath $systemTar -PathType Leaf){return $systemTar}
    try{return(Get-Command tar.exe -ErrorAction Stop).Source}catch{Fail 40 'Windows tar.exe/bsdtar is required.'}
}

# The Setup embeds a digest of the complete runtime contract, not just the
# CUDA pins. Never trust an installed receipt or package-supplied file list alone.
function Assert-InstallRoot([string]$Path){
    $full=[IO.Path]::GetFullPath($Path).TrimEnd([char]92)
    if($full-ceq[IO.Path]::GetPathRoot($full).TrimEnd([char]92)){Fail 44 'Install root cannot be a volume root.'}
    $directory=[IO.DirectoryInfo]::new($full)
    while($null-ne$directory){
        if($directory.Exists-and($directory.Attributes-band[IO.FileAttributes]::ReparsePoint)){Fail 44 'Install root is redirected.'}
        $directory=$directory.Parent
    }
    return $full
}

function Assert-PlainPath([string]$Root,[string]$Relative){
    $current=$Root
    foreach($part in $Relative.Replace('/','\').Split('\')){
        if($part-notmatch'^[A-Za-z0-9_.-]+$'-or$part-in@('.','..')-or$part.EndsWith('.')){Fail 44 'Unsafe native payload path.'}
        $current=Join-Path $current $part
        if(Test-Path -LiteralPath $current){
            if((Get-Item -LiteralPath $current -Force).Attributes-band[IO.FileAttributes]::ReparsePoint){Fail 44 'Native payload path is redirected.'}
        }
    }
    return $current
}

function Assert-Contract($Contract){
    if($ExpectedPayloadSchema-cne$NativePayloadSchema-or$ExpectedInstalledRuntimeSchema-cne$NativeInstalledSchema-or
        [string]$Contract.schema-cne$NativePayloadSchema-or[string]$Contract.installed_schema-cne$NativeInstalledSchema){Fail 44 'Native payload schema mismatch.'}
    $actual=@($Contract.roots|ForEach-Object{([string]$_).Replace('/','\')})
    if(($actual-join'|')-cne($PayloadRoots-join'|')){Fail 44 'Native payload root closure mismatch.'}
    if([string]$Contract.user_data_policy-cne'preserve_app_local_user_data'-or[string]$Contract.driver_policy-cne'preserve_system_driver'){Fail 44 'Native payload preservation policy mismatch.'}
}

function Assert-Runtime([string]$Root){
    $Root=Assert-InstallRoot $Root
    $contractPath=Assert-PlainPath $Root $PayloadContract
    NeedFile $contractPath 50 'Native runtime contract'
    if((Get-Item -LiteralPath $contractPath).Length-gt 1MB-or
        (Get-FileHash -LiteralPath $contractPath -Algorithm SHA256).Hash.ToLowerInvariant()-cne$ExpectedRuntimeContractSha256){Fail 50 'Native runtime contract differs from signed Setup.'}
    $contract=Get-Content -LiteralPath $contractPath -Raw|ConvertFrom-Json
    Assert-Contract $contract
    $expected=[Collections.Generic.Dictionary[string,object]]::new([StringComparer]::Ordinal)
    foreach($entry in $contract.files.PSObject.Properties){
        $relative=$entry.Name.Replace('/','\')
        $inside=$false
        foreach($prefix in $PayloadRoots){if($relative.StartsWith($prefix+'\',[StringComparison]::Ordinal)){$inside=$true;break}}
        if(-not$inside-or$relative-ceq$PayloadContract-or[string]$entry.Value.sha256-cnotmatch'^[a-f0-9]{64}$'-or[Int64]$entry.Value.bytes-le 0){Fail 50 'Invalid native runtime file identity.'}
        $path=Assert-PlainPath $Root $relative
        NeedFile $path 50 'Native runtime file'
        if((Get-Item -LiteralPath $path).Length-ne[Int64]$entry.Value.bytes-or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()-cne[string]$entry.Value.sha256){Fail 50 "Native runtime file identity mismatch: $relative"}
        $expected.Add($relative,$entry.Value)
    }
    if($expected.Count-eq 0){Fail 50 'Empty native runtime file closure.'}
    $seen=0
    foreach($relative in $PayloadRoots){
        $directory=Assert-PlainPath $Root $relative
        $pending=[Collections.Generic.Stack[string]]::new()
        $pending.Push($directory)
        while($pending.Count-gt 0){
            foreach($item in Get-ChildItem -LiteralPath ($pending.Pop()) -Force -ErrorAction Stop){
                if($item.Attributes-band[IO.FileAttributes]::ReparsePoint){Fail 50 'Native runtime contains a redirected entry.'}
                if($item.PSIsContainer){$pending.Push($item.FullName);continue}
                $name=$item.FullName.Substring($Root.TrimEnd([char]92).Length+1)
                if($name-ceq$PayloadContract){continue}
                if(-not$expected.ContainsKey($name)){Fail 50 "Unexpected native runtime file: $name"}
                $seen++
            }
        }
    }
    if($seen-ne$expected.Count){Fail 50 'Native runtime file count mismatch.'}
    return $contract
}

function Assert-InstalledRuntimeManifest([string]$Root){
    $manifestPath=Assert-PlainPath $Root $InstalledManifest
    NeedFile $manifestPath 57 'Installed runtime manifest'
    if((Get-Item -LiteralPath $manifestPath).Length-gt 64KB){Fail 57 'Installed runtime manifest oversized.'}
    try{$manifest=Get-Content -LiteralPath $manifestPath -Raw|ConvertFrom-Json}catch{Fail 57 'Installed runtime manifest invalid.'}
    if([string]$manifest.schema-cne$NativeInstalledSchema-or$manifest.installed_complete-ne$true-or
        [string]$manifest.payload_schema-cne$NativePayloadSchema-or[string]$manifest.runtime_contract_sha256-cne$ExpectedRuntimeContractSha256){Fail 57 'Installed runtime identity is incompatible with this app update.'}
}

function Assert-NoRetiredRuntime([string]$Root){
    foreach($relative in $RetiredRoots){
        if(Test-Path -LiteralPath (Assert-PlainPath $Root $relative)){Fail 58 'Legacy production runtime remains; full native payload required.'}
    }
}

function Verify-InstalledRuntime([string]$Root){
    if([string]::IsNullOrWhiteSpace($Root)){Fail 58 'InstallRoot required for installed-runtime verification.'}
    $full=Assert-InstallRoot $Root
    Assert-InstalledRuntimeManifest $full
    [void](Assert-Runtime $full)
    Assert-NoRetiredRuntime $full
    if(-not(Test-VBCablePresent)){Fail 58 'Required VB-CABLE provider is not present.'}
    exit 0
}

function Test-VBCablePresent {
    $pnputil=Join-Path $env:SystemRoot 'System32\pnputil.exe'
    if(Test-Path -LiteralPath $pnputil -PathType Leaf){
        try{if((&$pnputil /enum-drivers 2>&1|Out-String)-match'(?i)vbmmecable|vb-audio.+virtual cable'){return $true}}catch{}
    }
    try{
        $devices=@(Get-CimInstance -ClassName Win32_SoundDevice -ErrorAction Stop)
        if(@($devices|Where-Object{[string]$_.Name-match'(?i)VB-Audio.*(?:CABLE|Virtual Cable)|^CABLE (?:Input|Output)'}).Count-gt 0){return $true}
    }catch{}
    return $false
}

function Install-VBCableIfNeeded([string]$StageRoot){
    if(Test-VBCablePresent){return $false}
    $package=Join-Path $StageRoot 'EngineData\Backend\RuntimeAssets\AudioProvider\VBCABLE\Package'
    $setup=if([Environment]::Is64BitOperatingSystem){Join-Path $package 'VBCABLE_Setup_x64.exe'}else{Join-Path $package 'VBCABLE_Setup.exe'}
    NeedFile $setup 60 'VB-CABLE vendor installer'
    Write-Host '[translateit-r3-installer] Installing required VB-CABLE provider; Windows driver consent may appear.'
    $process=Start-Process -FilePath $setup -ArgumentList @('-i','-h') -Wait -PassThru
    if($process.ExitCode-ne 0){Fail 61 "VB-CABLE installer failed or was declined (exit $($process.ExitCode))."}
    if(-not(Test-VBCablePresent)){Fail 61 'VB-CABLE installer returned success but the driver package is not present.'}
    return $true
}

function Ensure-FreeSpace([string]$Root){
    if($ExpectedExpandedBytes-le 0){return}
    $driveRoot=[IO.Path]::GetPathRoot([IO.Path]::GetFullPath($Root))
    $free=(New-Object IO.DriveInfo($driveRoot)).AvailableFreeSpace
    $required=[Int64]($ExpectedExpandedBytes*1.15)+536870912
    if($free-lt$required){Fail 62 "Insufficient disk space. Need at least $required bytes free."}
}

function Rollback-Payload([string]$Root,[string]$Backup,[System.Collections.IList]$States){
    for($i=$States.Count-1;$i-ge 0;$i--){
        $state=$States[$i]
        $target=Join-Path $Root $state.relative
        $saved=Join-Path $Backup $state.relative
        if($state.new_moved-and(Test-Path -LiteralPath $target)){
            Remove-Item -LiteralPath $target -Recurse -Force -ErrorAction Stop
        }
        if($state.backup_moved){
            if(-not(Test-Path -LiteralPath $saved)){throw "Rollback backup missing: $($state.relative)"}
            if(Test-Path -LiteralPath $target){throw "Rollback destination still occupied: $($state.relative)"}
            New-Item -ItemType Directory -Force -Path(Split-Path $target -Parent)|Out-Null
            Move-Item -LiteralPath $saved -Destination $target -ErrorAction Stop
        }
    }
}

function Install-Payload([string]$Tar){
    if([string]::IsNullOrWhiteSpace($InstallRoot)){Fail 70 'InstallRoot required.'}
    $root=Assert-InstallRoot $InstallRoot
    New-Item -ItemType Directory -Force -Path $root|Out-Null
    Ensure-FreeSpace $root
    $stage=Join-Path $root '.translateit-r3-stage'
    $backup=Join-Path $root '.translateit-r3-backup'
    # A prior failed rollback may own the only copy of the old runtime.
    # Do not delete or reuse either transaction directory automatically.
    if((Test-Path -LiteralPath $stage)-or(Test-Path -LiteralPath $backup)){
        Fail 72 'An earlier payload transaction remains. Preserve its stage/backup for recovery.'
    }
    $states=New-Object Collections.Generic.List[object]
    $cleanupVerified=$false
    $stageCreated=$false
    $backupCreated=$false
    try{
        New-Item -ItemType Directory -Path $stage -ErrorAction Stop|Out-Null
        $stageCreated=$true
        New-Item -ItemType Directory -Path $backup -ErrorAction Stop|Out-Null
        $backupCreated=$true
        &$Tar -xf $script:PayloadPath -C $stage
        if($LASTEXITCODE-ne 0){Fail 71 'Payload extraction failed.'}
        [void](Assert-Runtime $stage)

        # Vendor/system flow precedes runtime replacement; consent stays with Windows.
        $restartRequired=Install-VBCableIfNeeded $stage
        foreach($relative in ($PayloadRoots+$RetiredRoots)){
            $source=Join-Path $stage $relative
            $target=Assert-PlainPath $root $relative
            $saved=Join-Path $backup $relative
            $state=[pscustomobject]@{relative=$relative;backup_moved=$false;new_moved=$false}
            $states.Add($state)
            New-Item -ItemType Directory -Force -Path(Split-Path $target -Parent),(Split-Path $saved -Parent)|Out-Null
            if(Test-Path -LiteralPath $target){Move-Item -LiteralPath $target -Destination $saved -Force;$state.backup_moved=$true}
            if($relative-in$PayloadRoots){
                Move-Item -LiteralPath $source -Destination $target -Force
                $state.new_moved=$true
            }
        }
        [void](Assert-Runtime $root)
        Assert-NoRetiredRuntime $root
        $manifest=[ordered]@{
            schema=$ExpectedInstalledRuntimeSchema
            installed_complete=$true
            app_version=$ExpectedAppVersion
            payload_schema=$ExpectedPayloadSchema
            payload_sha256=$ExpectedSha256.ToLowerInvariant()
            runtime_contract_sha256=$ExpectedRuntimeContractSha256
            vb_cable_restart_required=$restartRequired
            vb_cable_uninstall_policy='preserve_system_driver'
            user_data_policy='preserve_app_local_user_data'
            installed_utc=[DateTime]::UtcNow.ToString('o')
        }
        # The installed receipt is part of the same transaction, not an
        # in-place write that could leave the old runtime with a new receipt.
        $manifestPath=Assert-PlainPath $root $InstalledManifest
        $newManifest=Join-Path $stage $InstalledManifest
        $savedManifest=Join-Path $backup $InstalledManifest
        New-Item -ItemType Directory -Force -Path(Split-Path $newManifest -Parent),(Split-Path $savedManifest -Parent),(Split-Path $manifestPath -Parent)|Out-Null
        $manifest|ConvertTo-Json -Depth 5|Set-Content -LiteralPath $newManifest -Encoding UTF8
        $state=[pscustomobject]@{relative=$InstalledManifest;backup_moved=$false;new_moved=$false}
        $states.Add($state)
        if(Test-Path -LiteralPath $manifestPath){
            Move-Item -LiteralPath $manifestPath -Destination $savedManifest -ErrorAction Stop
            $state.backup_moved=$true
        }
        Move-Item -LiteralPath $newManifest -Destination $manifestPath -ErrorAction Stop
        $state.new_moved=$true
        $cleanupVerified=$true
    }catch{
        $originalFailure=$_
        [Console]::Error.WriteLine("[translateit-r3-installer] install failed; rolling back application runtime: $($_.Exception.Message)")
        try{
            Rollback-Payload $root $backup $states
            $cleanupVerified=$true
        }catch{
            [Console]::Error.WriteLine("[translateit-r3-installer] rollback incomplete; stage and backup retained: $($_.Exception.Message)")
            Fail 81 'Payload rollback incomplete; recovery files retained.'
        }
        if($states.Count-eq 0){throw $originalFailure}
        Fail 80 'Payload replacement failed; previous runtime restored.'
    }finally{
        if($cleanupVerified){
            if($stageCreated){Remove-Item -LiteralPath $stage -Recurse -Force -ErrorAction SilentlyContinue}
            if($backupCreated){Remove-Item -LiteralPath $backup -Recurse -Force -ErrorAction SilentlyContinue}
        }
    }
    if($restartRequired){exit 3010}
    exit 0
}

try{
    # Native package source is not release-admitted before reviewed closure
    # and integrated target acceptance. Keep all installer modes held.
    Fail 44 'Native packaging is held: native installer and app-only update are disabled.'
    if($Mode-eq'VerifyInstalled'){Verify-InstalledRuntime $InstallRoot}
    $script:PayloadPath=[IO.Path]::GetFullPath($PayloadPath)
    NeedFile $script:PayloadPath 42 'External payload'
    $actualHash=(Get-FileHash -Algorithm SHA256 -LiteralPath $script:PayloadPath).Hash.ToLowerInvariant()
    if($actualHash-ne$ExpectedSha256.ToLowerInvariant()){Fail 44 'External payload SHA-256 mismatch.'}
    $tar=Get-TarProgram
    $listing=@(&$tar -tf $script:PayloadPath 2>&1)
    if($LASTEXITCODE-ne 0-or$listing.Count-eq 0){Fail 45 'External payload is not a readable 7z archive.'}
    $seenEntries=[Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach($entry in $listing){
        $relative=([string]$entry).Replace('/','\').TrimEnd([char]92)
        if($relative-notmatch'^[A-Za-z0-9_.-]+(\\[A-Za-z0-9_.-]+)*$'-or
            @($relative.Split('\')|Where-Object{$_-in@('.','..')-or$_.EndsWith('.')}).Count-gt 0){Fail 45 'Unsafe archive path.'}
        $allowed=$false
        foreach($prefix in $PayloadRoots){if($relative-ceq$prefix-or$relative.StartsWith($prefix+'\',[StringComparison]::Ordinal)){$allowed=$true;break}}
        if(-not$allowed-or-not$seenEntries.Add($relative)){Fail 45 'Unexpected or duplicate archive entry.'}
    }
    if(-not$seenEntries.Contains($PayloadContract)){Fail 45 'Native runtime contract missing from archive.'}
    # The signed Setup already verified the exact archive digest. Full file
    # and runtime-contract verification is repeated after extraction.
    if($Mode-eq'Verify'){exit 0}
    Install-Payload $tar
}catch{
    [Console]::Error.WriteLine("[translateit-r3-installer] $($_.Exception.Message)")
    if($_.Exception.Data.Contains('TranslateITExitCode')){
        exit ([int]$_.Exception.Data['TranslateITExitCode'])
    }
    exit 90
}

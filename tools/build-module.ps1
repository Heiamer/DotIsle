param(
 [Parameter(Mandatory=$true)][ValidateSet('Core','ControlCenter','Updater','Focus','Todos','Music','Lyrics','Notifications','History','Automation','Devices','Osd','Sound','Widgets','Themes','Glow')][string]$Module,
 [ValidateSet('release','fast')][string]$Profile='fast',
 [string]$CacheRoot
)
$ErrorActionPreference='Stop'
$root=Split-Path $PSScriptRoot -Parent
if(-not $CacheRoot){$CacheRoot=Join-Path ([Environment]::GetFolderPath('MyDocuments')) 'Codex/BuildCache/DotIsle'}
$env:DOTISLE_TARGET_DIR=Join-Path $CacheRoot 'target'
$support=Join-Path $CacheRoot 'support'
foreach($library in @('shlwapi','ktmw32')){
 $destination=Join-Path $support "build/$library"
 New-Item -ItemType Directory -Force $destination | Out-Null
 $inputLibrary=Join-Path $root "build/$library/lib$library.a"
 $outputLibrary=Join-Path $destination "lib$library.a"
 if(-not(Test-Path -LiteralPath $outputLibrary) -or (Get-FileHash -LiteralPath $inputLibrary).Hash -ne (Get-FileHash -LiteralPath $outputLibrary).Hash){Copy-Item -LiteralPath $inputLibrary -Destination $outputLibrary -Force}
}
if(-not $env:DOTISLE_BUILD_SUPPORT){$env:DOTISLE_BUILD_SUPPORT=$support}
if(-not $env:ZIG_GLOBAL_CACHE_DIR){$env:ZIG_GLOBAL_CACHE_DIR=Join-Path $CacheRoot 'zig'}
$package=@{Core='lyric-native';ControlCenter='dotisle-app';Updater='lyric-bootstrap'}[$Module]
if(-not $package){$package='dotisle-'+$Module.ToLowerInvariant()}
$arguments=@('build','--locked','--package',$package,'--profile',$Profile)
if($Module -ne 'Updater'){$arguments+='--lib'}else{$arguments+=@('--bin','lyric-updater')}
& "$PSScriptRoot/li-cargo.ps1" @arguments
if($LASTEXITCODE -ne 0){exit $LASTEXITCODE}
$file=@{Core='dotisle_core.dll';ControlCenter='dotisle_settings.dll';Updater='lyric-updater.exe'}[$Module]
if(-not $file){$file='dotisle_'+$Module.ToLowerInvariant()+'.dll'}
Write-Output (Join-Path $env:DOTISLE_TARGET_DIR "$Profile/$file")

param(
  [Parameter(Mandatory = $true)]
  [string]$InstallerPath,

  [string]$ObsRoot = "C:\Program Files\obs-studio",
  [string]$ReportPath = "artifacts/obs-smoke-report.json",
  [string]$LogCopyPath = "artifacts/obs-last-log.txt",
  [int]$ObsRunSeconds = 20
)

$ErrorActionPreference = "Stop"

function Get-PluginLayoutState {
  param(
    [Parameter(Mandatory = $true)]
    [string]$StandardBase,
    [Parameter(Mandatory = $true)]
    [string]$ObsRootDir
  )

  $state = [ordered]@{
    standard_plugin_dll = Test-Path (Join-Path $StandardBase "bin/64bit/overlay_plugin.dll")
    standard_helper_exe = Test-Path (Join-Path $StandardBase "bin/64bit/overlay-helper.exe")
    portable_plugin_dll = Test-Path (Join-Path $ObsRootDir "obs-plugins/64bit/overlay_plugin.dll")
    portable_helper_exe = Test-Path (Join-Path $ObsRootDir "obs-plugins/64bit/overlay-helper.exe")
  }

  return [pscustomobject]$state
}

function Remove-PathIfExists {
  param(
    [Parameter(Mandatory = $true)]
    [string]$Path
  )

  if (Test-Path $Path) {
    Remove-Item $Path -Force
  }
}

$installerFullPath = Resolve-Path $InstallerPath -ErrorAction Stop
$obsExe = Join-Path $ObsRoot "bin/64bit/obs64.exe"
if (-not (Test-Path $obsExe)) {
  throw "OBS executable not found: $obsExe"
}

$programData = $env:ProgramData
if ([string]::IsNullOrWhiteSpace($programData)) {
  $programData = "C:\ProgramData"
}

$standardBase = Join-Path $programData "obs-studio/plugins/soul-memory-obs-overlay"

Remove-PathIfExists -Path (Join-Path $standardBase "bin/64bit/overlay_plugin.dll")
Remove-PathIfExists -Path (Join-Path $standardBase "bin/64bit/overlay-helper.exe")
Remove-PathIfExists -Path (Join-Path $ObsRoot "obs-plugins/64bit/overlay_plugin.dll")
Remove-PathIfExists -Path (Join-Path $ObsRoot "obs-plugins/64bit/overlay-helper.exe")

$standardInstall = Start-Process -FilePath $installerFullPath.Path -ArgumentList "/S" -PassThru -Wait
$postStandardState = Get-PluginLayoutState -StandardBase $standardBase -ObsRootDir $ObsRoot

$obsRootAttempt = Start-Process -FilePath $installerFullPath.Path -ArgumentList @("/S", "/D=$ObsRoot") -PassThru -Wait
$obsRootRejected = $obsRootAttempt.ExitCode -ne 0

$logDir = Join-Path $env:APPDATA "obs-studio/logs"
if (-not (Test-Path $logDir)) {
  New-Item -ItemType Directory -Path $logDir -Force | Out-Null
}

$beforeLogs = @()
if (Test-Path $logDir) {
  $beforeLogs = @(Get-ChildItem $logDir -File -Filter "*.txt" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty FullName)
}

$obsProcess = Start-Process -FilePath $obsExe -PassThru
Start-Sleep -Seconds $ObsRunSeconds
if (-not $obsProcess.HasExited) {
  Stop-Process -Id $obsProcess.Id -Force
}

Start-Sleep -Seconds 2

$afterLogs = @(Get-ChildItem $logDir -File -Filter "*.txt" -ErrorAction SilentlyContinue | Sort-Object LastWriteTime -Descending)
$newestLog = $afterLogs | Where-Object { $beforeLogs -notcontains $_.FullName } | Select-Object -First 1
if (-not $newestLog) {
  $newestLog = $afterLogs | Select-Object -First 1
}

if (-not $newestLog) {
  throw "OBS did not produce a log file in $logDir"
}

$logText = Get-Content -Path $newestLog.FullName -Raw
$containsPluginDll = $logText -match "overlay_plugin\\.dll"
$containsModuleName = $logText -match "soul-memory-obs-overlay|Soul Memory Overlay|soul_memory_overlay_source"
$overlayLoadFailure = $logText -match "Failed to load module.*overlay_plugin\\.dll|Module .*overlay_plugin\\.dll.*not loaded|LoadLibrary failed.*overlay_plugin\\.dll"
$sourceVisibleInferred = ($containsPluginDll -or $containsModuleName) -and (-not $overlayLoadFailure)

$report = [ordered]@{
  installer_path = $installerFullPath.Path
  obs_root = $ObsRoot
  standard_install_exit_code = $standardInstall.ExitCode
  obs_root_attempt_exit_code = $obsRootAttempt.ExitCode
  obs_root_rejected_in_standard_mode = $obsRootRejected
  post_standard_layout = $postStandardState
  obs_log_path = $newestLog.FullName
  obs_log_markers = [ordered]@{
    contains_overlay_plugin_dll = $containsPluginDll
    contains_source_markers = $containsModuleName
    has_overlay_plugin_load_failure = $overlayLoadFailure
  }
  source_visibility_inferred = $sourceVisibleInferred
}

$reportDir = Split-Path -Parent $ReportPath
if ($reportDir -and -not (Test-Path $reportDir)) {
  New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
}

$logCopyDir = Split-Path -Parent $LogCopyPath
if ($logCopyDir -and -not (Test-Path $logCopyDir)) {
  New-Item -ItemType Directory -Path $logCopyDir -Force | Out-Null
}

$report | ConvertTo-Json -Depth 5 | Out-File -FilePath $ReportPath -Encoding utf8
Copy-Item -Path $newestLog.FullName -Destination $LogCopyPath -Force

$standardLayoutValid = $postStandardState.standard_plugin_dll -and $postStandardState.standard_helper_exe
$portableNotWritten = (-not $postStandardState.portable_plugin_dll) -and (-not $postStandardState.portable_helper_exe)

if (-not $standardLayoutValid) {
  throw "Standard mode install did not place plugin/helper files in ProgramData layout. See $ReportPath"
}

if (-not $portableNotWritten) {
  throw "Standard mode install unexpectedly wrote portable layout files under OBS root. See $ReportPath"
}

if (-not $obsRootRejected) {
  throw "Standard mode OBS-root path attempt was not rejected. See $ReportPath"
}

if (-not $sourceVisibleInferred) {
  throw "Could not infer source registration visibility from OBS log markers. See $ReportPath and $LogCopyPath"
}

Write-Host "OBS smoke verification passed"
Write-Host "Report: $ReportPath"
Write-Host "Log copy: $LogCopyPath"

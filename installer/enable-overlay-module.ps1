param(
  [string]$ModuleName = "overlay_plugin",
  [string]$ObsConfigRoot = ""
)

if ([string]::IsNullOrWhiteSpace($ObsConfigRoot)) {
  $ObsConfigRoot = Join-Path $env:APPDATA "obs-studio"
}

$modulesPath = Join-Path $ObsConfigRoot "plugin_manager/modules.json"
if (-not (Test-Path $modulesPath)) {
  Write-Host "No modules.json found at $modulesPath"
  exit 0
}

try {
  $raw = Get-Content -Path $modulesPath -Raw -ErrorAction Stop
  if ([string]::IsNullOrWhiteSpace($raw)) {
    Write-Host "modules.json is empty; skipping"
    exit 0
  }

  $json = $raw | ConvertFrom-Json -ErrorAction Stop
  $modules = @()
  if ($json -is [System.Array]) {
    $modules = $json
  }
  elseif ($json -is [PSCustomObject]) {
    $modules = @($json)
  }
  else {
    Write-Host "modules.json has unexpected structure; skipping"
    exit 0
  }

  $changed = $false
  foreach ($module in $modules) {
    if (($module.PSObject.Properties.Name -contains "module_name") -and
        ($module.module_name -eq $ModuleName) -and
        ($module.PSObject.Properties.Name -contains "enabled") -and
        ($module.enabled -eq $false)) {
      $module.enabled = $true
      $changed = $true
    }
  }

  if (-not $changed) {
    Write-Host "Module '$ModuleName' was not disabled; no update needed"
    exit 0
  }

  $outJson = $modules | ConvertTo-Json -Depth 32
  $utf8NoBom = [System.Text.UTF8Encoding]::new($false)
  [System.IO.File]::WriteAllText($modulesPath, $outJson, $utf8NoBom)
  Write-Host "Re-enabled OBS module '$ModuleName' in $modulesPath"
}
catch {
  Write-Warning "Failed to update OBS modules.json: $($_.Exception.Message)"
}

exit 0

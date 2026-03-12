param(
  [switch]$WhatIf
)

$ErrorActionPreference = "Stop"

function Step {
  param(
    [string]$Title,
    [scriptblock]$Action
  )

  Write-Host ""
  Write-Host "== $Title =="
  & $Action
}

Write-Host "Soul Memory OBS Overlay - Release QA Checklist"
if ($WhatIf) {
  Write-Host "Mode: WhatIf (checklist preview only)"
}

Step "Install plugin (Standard mode)" {
  Write-Host "Run SoulMemoryOverlay-<version>-setup.exe and choose Standard (recommended)."
  Write-Host "Confirm files are installed under C:\\ProgramData\\obs-studio\\plugins\\soul-memory-obs-overlay\\..."
}

Step "Validate legacy-to-Standard migration cleanup" {
  Write-Host "Precondition: place prior-version plugin files in OBS-root layout (obs-plugins\\64bit and data\\obs-plugins\\soul-memory-obs-overlay)."
  Write-Host "Run Standard mode install and confirm ProgramData files are present."
  Write-Host "Confirm legacy OBS-root files for this plugin are removed (no duplicate side-by-side plugin files)."
}

Step "Validate Portable/custom mode path checks" {
  Write-Host "Run installer again and choose Portable/custom OBS mode."
  Write-Host "Use a machine/config where OBS is in a non-default folder, or where auto-detection does not resolve OBS."
  Write-Host "Confirm installer prompts for manual OBS folder selection in Portable/custom mode."
  Write-Host "Select an invalid folder first and confirm installer blocks progress with a message requiring bin\\64bit\\obs64.exe."
  Write-Host "Select the correct OBS folder and confirm installation proceeds with OBS-root layout output."
}

Step "Launch OBS and add Soul Memory source" {
  Write-Host "In OBS: Sources -> + -> Soul Memory Overlay"
}

Step "Validate fresh/stale transitions" {
  Write-Host "Start DS2: source should move from [STALE] to value."
  Write-Host "Close DS2/helper: source should return to [STALE]."
}

Step "Validate update status surface" {
  Write-Host "Open source properties and verify status/update text is present."
}

Step "Uninstall and cleanup" {
  Write-Host "Run obs-overlay-uninstall.exe from Standard mode install folder and confirm ProgramData plugin files are removed."
  Write-Host "After Standard-mode uninstall, confirm legacy OBS-root files for this plugin are also absent."
  Write-Host "Run obs-overlay-uninstall.exe from Portable/custom install folder and confirm OBS-root plugin files are removed."
}

if ($WhatIf) {
  Write-Host ""
  Write-Host "Checklist generated successfully."
}

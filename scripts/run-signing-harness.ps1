param(
  [string[]]$Files,
  [string]$OutputDir = "artifacts/signing-harness",
  [switch]$UseSystemNotepadSample,
  [switch]$KeepTempPfx
)

$ErrorActionPreference = "Stop"

if (-not ($env:OS -eq "Windows_NT")) {
  throw "run-signing-harness.ps1 must run on Windows (pwsh.exe or powershell.exe)."
}

if (-not (Test-Path $OutputDir)) {
  New-Item -ItemType Directory -Path $OutputDir -Force | Out-Null
}

$targetFiles = @()
if ($Files -and $Files.Count -gt 0) {
  $targetFiles = @($Files)
}

if ($targetFiles.Count -eq 0 -or $UseSystemNotepadSample) {
  $sampleSource = Join-Path $env:WINDIR "System32/notepad.exe"
  if (-not (Test-Path $sampleSource)) {
    throw "Sample source EXE not found: $sampleSource"
  }

  $sampleDest = Join-Path $OutputDir "harness-notepad.exe"
  Copy-Item $sampleSource $sampleDest -Force
  $targetFiles = @($sampleDest)
  Write-Host "Using local sample target: $sampleDest"
}

foreach ($file in $targetFiles) {
  if (-not (Test-Path $file)) {
    throw "Harness input file not found: $file"
  }
}

$randPassword = -join ((33..126 | Get-Random -Count 28) | ForEach-Object { [char]$_ })
$securePassword = ConvertTo-SecureString $randPassword -AsPlainText -Force

$subject = "CN=SoulMemoryOverlay Harness Test"
$cert = New-SelfSignedCertificate `
  -Type CodeSigningCert `
  -Subject $subject `
  -CertStoreLocation "Cert:\CurrentUser\My" `
  -NotAfter (Get-Date).AddDays(7)

$pfxPath = Join-Path $OutputDir "harness-test-signing.pfx"
Export-PfxCertificate -Cert $cert -FilePath $pfxPath -Password $securePassword | Out-Null

$reportPath = Join-Path $OutputDir "signing-report.json"
$scriptPath = Join-Path $PSScriptRoot "sign-windows-artifacts.ps1"

try {
  & $scriptPath `
    -Files $targetFiles `
    -PfxPath $pfxPath `
    -PfxPassword $randPassword `
    -ReportPath $reportPath `
    -AllowSelfSignedUntrusted:$true
}
finally {
  if ($cert -and $cert.Thumbprint) {
    $storePath = "Cert:\CurrentUser\My\$($cert.Thumbprint)"
    if (Test-Path $storePath) {
      Remove-Item $storePath -Force
    }
  }

  if (-not $KeepTempPfx -and (Test-Path $pfxPath)) {
    Remove-Item $pfxPath -Force
  }
}

Write-Host "Harness complete. Report: $reportPath"

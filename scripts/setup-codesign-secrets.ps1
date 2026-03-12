param(
  [string]$PfxPath,
  [string]$Repo,
  [string]$TimestampUrl = "http://timestamp.digicert.com",
  [switch]$SkipTimestamp,
  [switch]$Check,
  [switch]$CreateTestCert,
  [string]$TestCertSubject = "CN=SoulMemoryOverlay Test Signing",
  [int]$TestCertDays = 90
)

$ErrorActionPreference = "Stop"

function Test-CommandAvailable {
  param([string]$Name)

  if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
    throw "Missing required command: $Name"
  }
}

function Resolve-RepoName {
  return (gh repo view --json nameWithOwner --jq .nameWithOwner).Trim()
}

function Convert-SecureStringToPlain {
  param([Security.SecureString]$SecureValue)

  $bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($SecureValue)
  try {
    return [Runtime.InteropServices.Marshal]::PtrToStringBSTR($bstr)
  }
  finally {
    [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr)
  }
}

function Show-Status {
  param([string]$TargetRepo)

  $secretList = gh secret list -R $TargetRepo
  $variableList = gh variable list -R $TargetRepo

  if ($secretList -match '(?m)^WINDOWS_CODESIGN_PFX_BASE64\b') {
    Write-Host "OK secret WINDOWS_CODESIGN_PFX_BASE64"
  }
  else {
    Write-Host "MISSING secret WINDOWS_CODESIGN_PFX_BASE64"
  }

  if ($secretList -match '(?m)^WINDOWS_CODESIGN_PFX_PASSWORD\b') {
    Write-Host "OK secret WINDOWS_CODESIGN_PFX_PASSWORD"
  }
  else {
    Write-Host "MISSING secret WINDOWS_CODESIGN_PFX_PASSWORD"
  }

  if ($variableList -match '(?m)^WINDOWS_CODESIGN_TIMESTAMP_URL\b') {
    Write-Host "OK variable WINDOWS_CODESIGN_TIMESTAMP_URL"
  }
  else {
    Write-Host "MISSING variable WINDOWS_CODESIGN_TIMESTAMP_URL"
  }
}

function Test-IsWindowsPlatform {
  return $env:OS -eq "Windows_NT"
}

function New-TestPfx {
  param(
    [string]$OutputPath,
    [string]$Subject,
    [int]$Days,
    [Security.SecureString]$Password
  )

  if (-not (Test-IsWindowsPlatform)) {
    throw "-CreateTestCert requires Windows PowerShell/PowerShell on Windows."
  }

  $cert = New-SelfSignedCertificate `
    -Type CodeSigningCert `
    -Subject $Subject `
    -CertStoreLocation "Cert:\CurrentUser\My" `
    -NotAfter (Get-Date).AddDays($Days)

  $outputDir = Split-Path -Parent $OutputPath
  if (-not (Test-Path $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
  }

  Export-PfxCertificate -Cert $cert -FilePath $OutputPath -Password $Password | Out-Null
}

Test-CommandAvailable gh
gh auth status | Out-Null

if ([string]::IsNullOrWhiteSpace($Repo)) {
  $Repo = Resolve-RepoName
}

Write-Host "Target repository: $Repo"

if ($Check) {
  Show-Status -TargetRepo $Repo
  exit 0
}

$passwordSecure = $null

if ($CreateTestCert) {
  if ([string]::IsNullOrWhiteSpace($PfxPath)) {
    $PfxPath = Join-Path $env:TEMP "soulmemory-test-signing.pfx"
  }

  $passwordSecure = Read-Host "Password for new test .pfx and CI secret" -AsSecureString
  $passwordPlain = Convert-SecureStringToPlain -SecureValue $passwordSecure
  if ([string]::IsNullOrWhiteSpace($passwordPlain)) {
    throw "Password cannot be empty."
  }
  $passwordPlain = $null

  New-TestPfx -OutputPath $PfxPath -Subject $TestCertSubject -Days $TestCertDays -Password $passwordSecure
  Write-Host "Created test cert PFX at: $PfxPath"
}

if ([string]::IsNullOrWhiteSpace($PfxPath)) {
  $PfxPath = Read-Host "Path to .pfx file"
}

if (-not (Test-Path $PfxPath)) {
  throw "PFX file not found: $PfxPath"
}

Write-Host "Setting WINDOWS_CODESIGN_PFX_BASE64 from $PfxPath"
$pfxBytes = [IO.File]::ReadAllBytes($PfxPath)
$pfxBase64 = [Convert]::ToBase64String($pfxBytes)
$pfxBase64 | gh secret set WINDOWS_CODESIGN_PFX_BASE64 -R $Repo | Out-Null

if (-not $passwordSecure) {
  $passwordSecure = Read-Host "Enter PFX password" -AsSecureString
}

$password = Convert-SecureStringToPlain -SecureValue $passwordSecure
if ([string]::IsNullOrWhiteSpace($password)) {
  throw "Password cannot be empty."
}

Write-Host "Setting WINDOWS_CODESIGN_PFX_PASSWORD"
$password | gh secret set WINDOWS_CODESIGN_PFX_PASSWORD -R $Repo | Out-Null
$password = $null
$passwordSecure = $null

if (-not $SkipTimestamp) {
  Write-Host "Setting WINDOWS_CODESIGN_TIMESTAMP_URL to $TimestampUrl"
  gh variable set WINDOWS_CODESIGN_TIMESTAMP_URL -R $Repo --body $TimestampUrl | Out-Null
}

Write-Host ""
Write-Host "Configuration status:"
Show-Status -TargetRepo $Repo

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

  $secretList = (gh secret list -R $TargetRepo | Out-String)
  $variableList = (gh variable list -R $TargetRepo | Out-String)

  function Show-NameStatus {
    param(
      [string]$Type,
      [string]$Name,
      [string]$Source
    )

    if ($Source -match "(?m)^$([regex]::Escape($Name))\b") {
      Write-Host "OK $Type $Name"
    }
    else {
      Write-Host "MISSING $Type $Name"
    }
  }

  Write-Host "Managed trusted signing (preferred):"
  Show-NameStatus -Type "secret" -Name "AZURE_CLIENT_ID" -Source $secretList
  Show-NameStatus -Type "secret" -Name "AZURE_TENANT_ID" -Source $secretList
  Show-NameStatus -Type "secret" -Name "AZURE_SUBSCRIPTION_ID" -Source $secretList
  Show-NameStatus -Type "variable" -Name "WINDOWS_TRUSTED_SIGNING_ENDPOINT" -Source $variableList
  Show-NameStatus -Type "variable" -Name "WINDOWS_TRUSTED_SIGNING_ACCOUNT_NAME" -Source $variableList
  Show-NameStatus -Type "variable" -Name "WINDOWS_TRUSTED_SIGNING_CERT_PROFILE_NAME" -Source $variableList

  Write-Host ""
  Write-Host "PFX signing (fallback/testing):"
  Show-NameStatus -Type "secret" -Name "WINDOWS_CODESIGN_PFX_BASE64" -Source $secretList
  Show-NameStatus -Type "secret" -Name "WINDOWS_CODESIGN_PFX_PASSWORD" -Source $secretList
  Show-NameStatus -Type "variable" -Name "WINDOWS_CODESIGN_TIMESTAMP_URL" -Source $variableList
}

function Test-IsWindowsPlatform {
  return $env:OS -eq "Windows_NT"
}

function Get-DefaultTestPfxPath {
  if (Test-IsWindowsPlatform) {
    $userProfile = $env:USERPROFILE
    if (-not [string]::IsNullOrWhiteSpace($userProfile)) {
      return (Join-Path $userProfile ".codesign/soulmemory-test-signing.pfx")
    }
  }

  return (Join-Path $env:TEMP "soulmemory-test-signing.pfx")
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
    $PfxPath = Get-DefaultTestPfxPath
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

param(
  [Parameter(Mandatory = $true)]
  [string[]]$Files,

  [string]$ReportPath = "release-assets/signing-report.json",
  [string]$TimestampUrl = $env:WINDOWS_CODESIGN_TIMESTAMP_URL,
  [string]$PfxPath,
  [string]$PfxBase64 = $env:WINDOWS_CODESIGN_PFX_BASE64,
  [string]$PfxPassword = $env:WINDOWS_CODESIGN_PFX_PASSWORD,
  [switch]$ReleaseContext,
  [int]$SignTimeoutSeconds = 180,
  [bool]$AllowSelfSignedUntrusted = $true
)

$ErrorActionPreference = "Stop"

function Resolve-SignTool {
  $command = Get-Command signtool.exe -ErrorAction SilentlyContinue
  if ($command) {
    return $command.Source
  }

  $roots = @(
    (Join-Path ${env:ProgramFiles(x86)} "Windows Kits/10/bin"),
    (Join-Path $env:ProgramFiles "Windows Kits/10/bin")
  )

  foreach ($root in $roots) {
    if (-not $root -or -not (Test-Path $root)) {
      continue
    }

    $versions = Get-ChildItem -Path $root -Directory -ErrorAction SilentlyContinue |
      Sort-Object Name -Descending
    foreach ($versionDir in $versions) {
      $candidate = Join-Path $versionDir.FullName "x64/signtool.exe"
      if (Test-Path $candidate) {
        return $candidate
      }
    }
  }

  return $null
}

function Invoke-SignTool {
  param(
    [Parameter(Mandatory = $true)]
    [string]$SignToolPath,

    [Parameter(Mandatory = $true)]
    [string[]]$Arguments,

    [int]$TimeoutSeconds = 180
  )

  $psi = [System.Diagnostics.ProcessStartInfo]::new()
  $psi.FileName = $SignToolPath
  $psi.UseShellExecute = $false
  $psi.CreateNoWindow = $true
  $psi.RedirectStandardOutput = $true
  $psi.RedirectStandardError = $true

  foreach ($arg in $Arguments) {
    [void]$psi.ArgumentList.Add($arg)
  }

  $process = [System.Diagnostics.Process]::new()
  $process.StartInfo = $psi

  try {
    if (-not $process.Start()) {
      throw "Failed to start signtool process"
    }

    $waitMs = $TimeoutSeconds * 1000
    if (-not $process.WaitForExit($waitMs)) {
      try {
        $process.Kill($true)
      }
      catch {
        Write-Warning "Failed to terminate timed-out signtool process: $($_.Exception.Message)"
      }

      $timedOutStdout = $process.StandardOutput.ReadToEnd().Trim()
      $timedOutStderr = $process.StandardError.ReadToEnd().Trim()
      return [pscustomobject]@{
        TimedOut = $true
        ExitCode = $null
        StdOut = $timedOutStdout
        StdErr = $timedOutStderr
      }
    }

    $stdOut = $process.StandardOutput.ReadToEnd().Trim()
    $stdErr = $process.StandardError.ReadToEnd().Trim()

    return [pscustomobject]@{
      TimedOut = $false
      ExitCode = $process.ExitCode
      StdOut = $stdOut
      StdErr = $stdErr
    }
  }
  finally {
    $process.Dispose()
  }
}

if ([string]::IsNullOrWhiteSpace($TimestampUrl)) {
  $TimestampUrl = "http://timestamp.digicert.com"
}

if ($Files.Count -eq 0) {
  throw "No files were provided for signing."
}

Write-Host "Resolving signtool path..."
$signTool = Resolve-SignTool
if (-not $signTool) {
  throw "signtool.exe not found on this machine"
}
Write-Host "Using signtool: $signTool"

$report = @()
$cleanupPfx = $false

if ([string]::IsNullOrWhiteSpace($PfxPath)) {
  $hasPfx = -not [string]::IsNullOrWhiteSpace($PfxBase64)
  $hasPassword = -not [string]::IsNullOrWhiteSpace($PfxPassword)

  if (-not ($hasPfx -and $hasPassword)) {
    $reason = "Signing secrets are not configured. Set WINDOWS_CODESIGN_PFX_BASE64 and WINDOWS_CODESIGN_PFX_PASSWORD."
    foreach ($file in $Files) {
      $report += [pscustomobject]@{
        file = $file
        status = "NotSigned"
        signed = $false
        signer = $null
        timestamp = $null
        reason = "missing-signing-secrets"
      }
    }

    $reportDir = Split-Path -Parent $ReportPath
    if ($reportDir -and -not (Test-Path $reportDir)) {
      New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
    }
    $report | ConvertTo-Json -Depth 4 | Out-File -FilePath $ReportPath -Encoding utf8

    if ($ReleaseContext) {
      throw $reason
    }

    Write-Warning $reason
    exit 0
  }

  Write-Host "Materializing PFX from base64 secret..."
  $PfxPath = Join-Path $env:TEMP "windows-codesign.pfx"
  [IO.File]::WriteAllBytes($PfxPath, [Convert]::FromBase64String($PfxBase64))
  $cleanupPfx = $true
}

if ([string]::IsNullOrWhiteSpace($PfxPassword)) {
  throw "PFX password is required."
}

if (-not (Test-Path $PfxPath)) {
  throw "PFX file not found: $PfxPath"
}

try {
  Write-Host "Starting signing pass for $($Files.Count) file(s)..."

  foreach ($file in $Files) {
    if (-not (Test-Path $file)) {
      throw "File to sign not found: $file"
    }

    Write-Host "Signing file: $file"

    $baseArgs = @("sign", "/fd", "SHA256", "/f", $PfxPath, "/p", $PfxPassword)
    $argsWithTimestamp = $baseArgs + @("/tr", $TimestampUrl, "/td", "SHA256", $file)
    $argsWithoutTimestamp = $baseArgs + @($file)

    $result = Invoke-SignTool -SignToolPath $signTool -Arguments $argsWithTimestamp -TimeoutSeconds $SignTimeoutSeconds
    if ($result.TimedOut) {
      throw "signtool timed out after $SignTimeoutSeconds seconds for ${file}. stdout: $($result.StdOut) stderr: $($result.StdErr)"
    }

    if ($result.ExitCode -ne 0) {
      Write-Warning "Timestamped signing failed for ${file}. Retrying without timestamp."
      if ($result.StdOut) {
        Write-Host $result.StdOut
      }
      if ($result.StdErr) {
        Write-Host $result.StdErr
      }

      $retry = Invoke-SignTool -SignToolPath $signTool -Arguments $argsWithoutTimestamp -TimeoutSeconds $SignTimeoutSeconds
      if ($retry.TimedOut) {
        throw "signtool retry timed out after $SignTimeoutSeconds seconds for ${file}. stdout: $($retry.StdOut) stderr: $($retry.StdErr)"
      }
      if ($retry.ExitCode -ne 0) {
        throw "signtool sign failed for ${file} (exit code $($retry.ExitCode)). stdout: $($retry.StdOut) stderr: $($retry.StdErr)"
      }
      $result = $retry
    }

    if ($result.StdOut) {
      Write-Host $result.StdOut
    }
    if ($result.StdErr) {
      Write-Host $result.StdErr
    }

    $signature = Get-AuthenticodeSignature -FilePath $file
    $signerCert = $signature.SignerCertificate
    $hasSigner = $null -ne $signerCert
    $isSelfSignedSigner = $hasSigner -and ($signerCert.Subject -eq $signerCert.Issuer)

    if (-not $hasSigner) {
      throw "Signature verification failed for ${file}: signer certificate missing"
    }

    if ($signature.Status -ne "Valid") {
      $allowSelfSigned = $AllowSelfSignedUntrusted -and $isSelfSignedSigner -and @("UnknownError", "NotTrusted", "UntrustedRoot") -contains [string]$signature.Status
      if (-not $allowSelfSigned) {
        throw "Signature verification failed for ${file}: $($signature.Status) $($signature.StatusMessage)"
      }
      Write-Warning "Self-signed certificate produced non-Valid status for ${file}: $($signature.Status). Continuing."
    }

    $report += [pscustomobject]@{
      file = $file
      status = [string]$signature.Status
      signed = $true
      signer = $signerCert.Subject
      timestamp = if ($signature.TimeStamperCertificate) { $signature.TimeStamperCertificate.Subject } else { $null }
      reason = if ($signature.Status -eq "Valid") { $null } elseif ($isSelfSignedSigner) { "self-signed-untrusted-chain" } else { "signature-status-$([string]$signature.Status)" }
    }
  }
}
finally {
  if ($cleanupPfx -and (Test-Path $PfxPath)) {
    Remove-Item $PfxPath -Force
  }
}

$reportDir = Split-Path -Parent $ReportPath
if ($reportDir -and -not (Test-Path $reportDir)) {
  New-Item -ItemType Directory -Path $reportDir -Force | Out-Null
}

$report | ConvertTo-Json -Depth 4 | Out-File -FilePath $ReportPath -Encoding utf8
Write-Host "Wrote signing report: $ReportPath"

param(
    [string]$HelperExe = "$PSScriptRoot\..\target\x86_64-pc-windows-msvc\debug\overlay-helper.exe",
    [string]$Config = "$PSScriptRoot\..\config\overlay.toml",
    [string]$OutputFile = "$PSScriptRoot\..\overlay\soul-memory.txt",
    [string]$HtmlOutputFile = "$PSScriptRoot\..\overlay\soul-memory.html",
    [int]$IntervalMs = 500,
    [switch]$Once
)

$ErrorActionPreference = "Stop"

$outputDir = Split-Path -Parent $OutputFile
if (-not (Test-Path -LiteralPath $outputDir)) {
    New-Item -ItemType Directory -Path $outputDir -Force | Out-Null
}

$htmlDir = Split-Path -Parent $HtmlOutputFile
if (-not (Test-Path -LiteralPath $htmlDir)) {
    New-Item -ItemType Directory -Path $htmlDir -Force | Out-Null
}

$lastGood = $null

while ($true) {
    $isStale = $false
    $displayValue = "N/A"

    try {
        $raw = & $HelperExe --config $Config --once
        if ($LASTEXITCODE -eq 0 -and $raw) {
            $obj = $raw | ConvertFrom-Json
            if ($obj.status -eq "ok" -and $null -ne $obj.value) {
                $lastGood = [string]$obj.value
            } else {
                $isStale = $true
            }
        } else {
            $isStale = $true
        }
    } catch {
        $isStale = $true
    }

    if ($null -ne $lastGood) {
        $displayValue = $lastGood
    }

    if ($isStale) {
        Set-Content -LiteralPath $OutputFile -Value ("[STALE] " + $displayValue) -NoNewline
    } else {
        Set-Content -LiteralPath $OutputFile -Value $displayValue -NoNewline
    }

    $color = if ($isStale) { "#ffd24a" } else { "#ffffff" }
    $refreshMs = [Math]::Max($IntervalMs, 200)
    $html = @"
<!doctype html>
<html>
<head>
  <meta charset="utf-8" />
  <meta http-equiv="Cache-Control" content="no-cache, no-store, must-revalidate" />
  <meta http-equiv="Pragma" content="no-cache" />
  <meta http-equiv="Expires" content="0" />
  <style>
    html, body {
      margin: 0;
      padding: 0;
      background: transparent;
      overflow: hidden;
      font-family: Segoe UI, sans-serif;
      font-size: 48px;
      font-weight: 700;
      color: $color;
      text-shadow: 0 0 8px rgba(0,0,0,0.75);
    }
  </style>
  <script>
    setTimeout(function () {
      var u = new URL(window.location.href);
      u.searchParams.set('t', String(Date.now()));
      window.location.replace(u.toString());
    }, $refreshMs);
  </script>
</head>
<body>$displayValue</body>
</html>
"@
    Set-Content -LiteralPath $HtmlOutputFile -Value $html -NoNewline

    if ($Once) {
        break
    }

    Start-Sleep -Milliseconds $IntervalMs
}

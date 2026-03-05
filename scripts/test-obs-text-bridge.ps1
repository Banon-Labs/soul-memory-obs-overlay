param(
    [string]$BridgeScript = "$PSScriptRoot\obs-text-bridge.ps1",
    [string]$HelperExe = "$PSScriptRoot\..\target\x86_64-pc-windows-msvc\debug\overlay-helper.exe",
    [string]$Config = "$PSScriptRoot\..\config\overlay.toml"
)

$ErrorActionPreference = "Stop"

function Assert-True {
    param(
        [Parameter(Mandatory = $true)]
        [bool]$Condition,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Condition) {
        throw $Message
    }
}

function Assert-Contains {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Text,
        [Parameter(Mandatory = $true)]
        [string]$Needle,
        [Parameter(Mandatory = $true)]
        [string]$Message
    )

    if (-not $Text.Contains($Needle)) {
        throw $Message
    }
}

$testDir = Join-Path $env:TEMP "obs-bridge-test-$PID"
New-Item -ItemType Directory -Path $testDir -Force | Out-Null

$txt = Join-Path $testDir "soul-memory.txt"
$html = Join-Path $testDir "soul-memory.html"

try {
    # Fresh run
    & $BridgeScript -HelperExe $HelperExe -Config $Config -OutputFile $txt -HtmlOutputFile $html -Once

    Assert-True (Test-Path -LiteralPath $txt) "fresh run did not create text output"
    Assert-True (Test-Path -LiteralPath $html) "fresh run did not create html output"

    $freshText = Get-Content -LiteralPath $txt -Raw
    $freshHtml = Get-Content -LiteralPath $html -Raw

    Assert-Contains $freshHtml "<html>" "fresh html missing <html>"
    Assert-Contains $freshHtml "</html>" "fresh html missing </html>"
    Assert-Contains $freshHtml "<body>" "fresh html missing <body>"
    Assert-Contains $freshHtml "#ffffff" "fresh html should be white"
    Assert-True (-not $freshText.StartsWith("[STALE]")) "fresh text unexpectedly marked stale"

    # Stale run
    & $BridgeScript -HelperExe "C:\nope\overlay-helper.exe" -Config $Config -OutputFile $txt -HtmlOutputFile $html -Once

    $staleText = Get-Content -LiteralPath $txt -Raw
    $staleHtml = Get-Content -LiteralPath $html -Raw

    Assert-True ($staleText.StartsWith("[STALE]")) "stale text missing [STALE] marker"
    Assert-Contains $staleHtml "#ffd24a" "stale html should be yellow"
    Assert-Contains $staleHtml "</html>" "stale html malformed"

    # Repeated writes should never produce malformed html
    for ($i = 0; $i -lt 20; $i++) {
        if (($i % 2) -eq 0) {
            & $BridgeScript -HelperExe $HelperExe -Config $Config -OutputFile $txt -HtmlOutputFile $html -Once
        } else {
            & $BridgeScript -HelperExe "C:\nope\overlay-helper.exe" -Config $Config -OutputFile $txt -HtmlOutputFile $html -Once
        }

        $htmlText = Get-Content -LiteralPath $html -Raw
        Assert-Contains $htmlText "<html>" "loop html missing <html>"
        Assert-Contains $htmlText "</html>" "loop html missing </html>"
        Assert-Contains $htmlText "<body>" "loop html missing <body>"
    }

    Write-Output "PASS: bridge stale/fresh and html integrity checks"
}
finally {
    Remove-Item -LiteralPath $testDir -Recurse -Force -ErrorAction SilentlyContinue
}

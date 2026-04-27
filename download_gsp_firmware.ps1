$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot
$FwDir = Join-Path $Root "firmware"

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS - Download GSP Firmware Blobs (GA102/GA106)" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

if (!(Test-Path $FwDir)) {
    New-Item -Path $FwDir -ItemType Directory -Force | Out-Null
}

$files = @(
    "bootloader-535.113.01.bin",
    "booter_load-535.113.01.bin",
    "booter_unload-535.113.01.bin"
)

$urls = @(
    "https://gitlab.com/kernel-firmware/linux-firmware/-/raw/main/nvidia/ga102/gsp",
    "https://git.kernel.org/pub/scm/linux/kernel/git/firmware/linux-firmware.git/plain/nvidia/ga102/gsp"
)

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

$downloaded = 0

foreach ($fname in $files) {
    $dest = Join-Path $FwDir $fname

    if (Test-Path $dest) {
        $sz = (Get-Item $dest).Length
        Write-Host "  [SKIP] $fname - already exists, $sz bytes" -ForegroundColor Yellow
        $downloaded++
        continue
    }

    Write-Host "  [DOWN] $fname ..." -ForegroundColor Cyan
    $ok = $false

    foreach ($base in $urls) {
        $url = "$base/$fname"
        Write-Host "         $url" -ForegroundColor DarkGray

        try {
            $wc = New-Object System.Net.WebClient
            $wc.Headers.Add("User-Agent", "FastOS/1.0")
            $wc.DownloadFile($url, $dest)

            $sz = (Get-Item $dest).Length
            if ($sz -gt 1000) {
                Write-Host "         OK - $sz bytes" -ForegroundColor Green
                $ok = $true
                $downloaded++
                break
            } else {
                Remove-Item $dest -ErrorAction SilentlyContinue
            }
        } catch {
            Write-Host "         Failed" -ForegroundColor DarkGray
        }
    }

    if (!$ok) {
        Write-Host "  [FAIL] Could not download $fname" -ForegroundColor Red
    }
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  Downloaded $downloaded / $($files.Count) files" -ForegroundColor Green
Write-Host "  Directory: $FwDir" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

Get-ChildItem (Join-Path $FwDir "*.bin") -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host "    $($_.Name) - $($_.Length) bytes" -ForegroundColor White
}

$gspMain = Join-Path $Root "gsp_ga10x.bin"
if (Test-Path $gspMain) {
    $gspSz = (Get-Item $gspMain).Length
    $gspMB = [math]::Round($gspSz / 1MB, 1)
    Write-Host "    gsp_ga10x.bin - ${gspMB}MB (GSP-RM payload, in root)" -ForegroundColor White
}

Write-Host ""
if ($downloaded -lt 2) {
    Write-Host "  MANUAL DOWNLOAD:" -ForegroundColor Red
    Write-Host "    https://gitlab.com/kernel-firmware/linux-firmware/-/tree/main/nvidia/ga102/gsp" -ForegroundColor Yellow
    Write-Host "    Download bootloader-535.113.01.bin and booter_load-535.113.01.bin" -ForegroundColor Yellow
    Write-Host "    Place them in: $FwDir" -ForegroundColor Yellow
}
Write-Host ""

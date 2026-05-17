# write_gsp.ps1 — Write GSP firmware + Boot binary to SATA
# GSP firmware (36MB) at LBA 4096
# Boot binary (24KB) at LBA 78436 (right after GSP)

param(
    [string]$DiskNumber = "1",
    [string]$GspPath = "c:\Users\andre\OneDrive\Documentos\SigDead\firmware\linux-firmware\nvidia\ga102\gsp\gsp-535.113.01.bin",
    [string]$BootPath = "c:\Users\andre\OneDrive\Documentos\FastOS\USB_boot\firmware\gsp_rm_boot_ga102.bin"
)

# ── Write GSP firmware at LBA 4096 ──
if (-not (Test-Path $GspPath)) { Write-Error "GSP not found: $GspPath"; exit 1 }
$gspData = [System.IO.File]::ReadAllBytes($GspPath)
$gspSectors = [math]::Ceiling($gspData.Length / 512)
$paddedSize = $gspSectors * 512
if ($gspData.Length -lt $paddedSize) {
    $padded = New-Object byte[] $paddedSize
    [Array]::Copy($gspData, $padded, $gspData.Length)
    $gspData = $padded
}

Write-Host "GSP firmware: $([math]::Round($gspData.Length/1MB, 2)) MB ($gspSectors sectors) -> LBA 4096"

$diskPath = "\\.\PhysicalDrive$DiskNumber"
$handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)

$handle.Seek(4096 * 512, [System.IO.SeekOrigin]::Begin) | Out-Null
$chunkSize = 1024 * 1024
$written = 0
while ($written -lt $gspData.Length) {
    $remaining = $gspData.Length - $written
    $thisChunk = [math]::Min($remaining, $chunkSize)
    $handle.Write($gspData, $written, $thisChunk)
    $written += $thisChunk
    Write-Host "`r  GSP: $([math]::Round($written*100/$gspData.Length))%" -NoNewline
}
Write-Host ""

# ── Write Boot binary at LBA 78436 ──
if (Test-Path $BootPath) {
    $bootData = [System.IO.File]::ReadAllBytes($BootPath)
    $bootSectors = [math]::Ceiling($bootData.Length / 512)
    $bootPadded = $bootSectors * 512
    if ($bootData.Length -lt $bootPadded) {
        $p = New-Object byte[] $bootPadded
        [Array]::Copy($bootData, $p, $bootData.Length)
        $bootData = $p
    }
    
    $bootLBA = 4096 + $gspSectors  # Right after GSP
    Write-Host "Boot binary: $($bootData.Length) bytes ($bootSectors sectors) -> LBA $bootLBA"
    
    $handle.Seek($bootLBA * 512, [System.IO.SeekOrigin]::Begin) | Out-Null
    $handle.Write($bootData, 0, $bootData.Length)
    Write-Host "  Boot binary written OK"
} else {
    Write-Host "[WARN] Boot binary not found: $BootPath"
}

$handle.Flush()
$handle.Close()

Write-Host "`n[OK] All firmware written to disk $DiskNumber"
Write-Host "     GSP firmware: LBA 4096 ($gspSectors sectors)"
if (Test-Path $BootPath) {
    Write-Host "     Boot binary:  LBA $(4096 + $gspSectors) ($bootSectors sectors)"
}

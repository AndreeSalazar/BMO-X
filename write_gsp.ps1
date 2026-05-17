# write_gsp.ps1 — Write GSP firmware to SATA at LBA 4096
# This writes the 36MB gsp-535.113.01.bin to the raw partition
# so the FastOS kernel can load it via AHCI

param(
    [string]$DiskNumber = "1",  # USB drive disk number
    [string]$GspPath = "c:\Users\andre\OneDrive\Documentos\SigDead\firmware\linux-firmware\nvidia\ga102\gsp\gsp-535.113.01.bin"
)

if (-not (Test-Path $GspPath)) {
    Write-Error "GSP firmware not found: $GspPath"
    exit 1
}

$gspSize = (Get-Item $GspPath).Length
$sectors = [math]::Ceiling($gspSize / 512)
Write-Host "GSP firmware: $([math]::Round($gspSize/1MB, 2)) MB ($gspSize bytes, $sectors sectors)"
Write-Host "Target: Disk $DiskNumber, LBA 4096"

# Read GSP firmware
$gspData = [System.IO.File]::ReadAllBytes($GspPath)

# Pad to sector boundary
$paddedSize = $sectors * 512
if ($gspData.Length -lt $paddedSize) {
    $padded = New-Object byte[] $paddedSize
    [Array]::Copy($gspData, $padded, $gspData.Length)
    $gspData = $padded
}

Write-Host "Writing $sectors sectors to LBA 4096..."

# Open disk for raw write
$diskPath = "\\.\PhysicalDrive$DiskNumber"
$handle = [System.IO.File]::Open($diskPath, 
    [System.IO.FileMode]::Open, 
    [System.IO.FileAccess]::Write, 
    [System.IO.FileShare]::ReadWrite)

# Seek to LBA 4096
$offset = 4096 * 512  # = 2MB offset
$handle.Seek($offset, [System.IO.SeekOrigin]::Begin) | Out-Null

# Write in 1MB chunks
$chunkSize = 1024 * 1024
$written = 0
while ($written -lt $gspData.Length) {
    $remaining = $gspData.Length - $written
    $thisChunk = [math]::Min($remaining, $chunkSize)
    $handle.Write($gspData, $written, $thisChunk)
    $written += $thisChunk
    $pct = [math]::Round($written * 100 / $gspData.Length)
    Write-Host "  $pct% ($([math]::Round($written/1MB, 1)) MB)" -NoNewline
    Write-Host "`r" -NoNewline
}
$handle.Flush()
$handle.Close()

Write-Host ""
Write-Host "[OK] GSP firmware written to disk $DiskNumber at LBA 4096 ($sectors sectors)"
Write-Host "     The FastOS kernel will load it at boot via AHCI"

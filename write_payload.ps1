# write_payload.ps1 — Write fastos_boot.bin to raw SATA disk at LBA 2048
# Usage: Run as Administrator: .\write_payload.ps1 -DiskNumber 1
#
# Layout on disk:
#   LBA 2048 (byte offset 1048576): FASTPAY header (512 bytes)
#   LBA 2049+: raw fastos_boot.bin content
#
# The header format:
#   [8 bytes] "FASTPAY\0" magic
#   [4 bytes] payload size (little-endian u32)
#   [500 bytes] padding (zeros)

param(
    [Parameter(Mandatory=$false)]
    [int]$DiskNumber = -1,
    
    [Parameter(Mandatory=$false)]
    [string]$PayloadPath = ".\fastos_boot.bin"
)

# Must run as admin
if (-not ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    Write-Host "[ERROR] Run this script as Administrator!" -ForegroundColor Red
    exit 1
}

# Check payload exists
if (-not (Test-Path $PayloadPath)) {
    Write-Host "[ERROR] Payload not found: $PayloadPath" -ForegroundColor Red
    Write-Host "Place fastos_boot.bin in the FastOS directory or specify -PayloadPath" -ForegroundColor Yellow
    exit 1
}

$payloadBytes = [System.IO.File]::ReadAllBytes((Resolve-Path $PayloadPath).Path)
Write-Host "[INFO] Payload: $PayloadPath ($($payloadBytes.Length) bytes)" -ForegroundColor Cyan

# Auto-detect SATA disk if not specified
if ($DiskNumber -eq -1) {
    Write-Host ""
    Write-Host "Available disks:" -ForegroundColor Yellow
    Get-Disk | Format-Table Number, FriendlyName, @{L="Size (GB)";E={[math]::Round($_.Size/1GB,1)}}, BusType -AutoSize
    Write-Host ""
    
    # Find SATA disk (BusType = SATA or ATA)
    $sataDisk = Get-Disk | Where-Object { $_.BusType -eq "SATA" -or $_.BusType -eq "ATA" } | Select-Object -First 1
    if ($sataDisk) {
        $DiskNumber = $sataDisk.Number
        Write-Host "[AUTO] Detected SATA disk: #$DiskNumber ($($sataDisk.FriendlyName), $([math]::Round($sataDisk.Size/1GB,1)) GB)" -ForegroundColor Green
    } else {
        Write-Host "[ERROR] No SATA disk found. Specify -DiskNumber manually." -ForegroundColor Red
        exit 1
    }
}

# Confirm
$disk = Get-Disk -Number $DiskNumber
Write-Host ""
Write-Host "Target: Disk #$DiskNumber - $($disk.FriendlyName) ($([math]::Round($disk.Size/1GB,1)) GB)" -ForegroundColor Yellow
Write-Host "Action: Write $($payloadBytes.Length) bytes to LBA 2048 (byte offset 1048576)" -ForegroundColor Yellow
Write-Host ""
$confirm = Read-Host "Type YES to proceed"
if ($confirm -ne "YES") {
    Write-Host "Aborted." -ForegroundColor Red
    exit 0
}

# Build header (512 bytes)
$header = New-Object byte[] 512
# Magic: "FASTPAY\0"
$magic = [System.Text.Encoding]::ASCII.GetBytes("FASTPAY")
[Array]::Copy($magic, 0, $header, 0, 7)
$header[7] = 0
# Payload size (u32 LE)
$sizeBytes = [BitConverter]::GetBytes([uint32]$payloadBytes.Length)
[Array]::Copy($sizeBytes, 0, $header, 8, 4)

# Open physical disk
$diskPath = "\\.\PhysicalDrive$DiskNumber"
Write-Host "[WRITE] Opening $diskPath..." -ForegroundColor Cyan

try {
    $fs = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)
    
    # Seek to LBA 2048 = byte 1048576
    $offset = 2048 * 512
    $fs.Seek($offset, [System.IO.SeekOrigin]::Begin) | Out-Null
    
    # Write header (512 bytes)
    $fs.Write($header, 0, 512)
    Write-Host "[WRITE] Header written at LBA 2048 (FASTPAY magic + size=$($payloadBytes.Length))" -ForegroundColor Green
    
    # Write payload (aligned to 512 bytes)
    $alignedSize = [math]::Ceiling($payloadBytes.Length / 512) * 512
    $alignedPayload = New-Object byte[] $alignedSize
    [Array]::Copy($payloadBytes, 0, $alignedPayload, 0, $payloadBytes.Length)
    
    $fs.Write($alignedPayload, 0, $alignedSize)
    Write-Host "[WRITE] Payload written at LBA 2049 ($alignedSize bytes, $($alignedSize/512) sectors)" -ForegroundColor Green
    
    $fs.Flush()
    $fs.Close()
    
    Write-Host ""
    Write-Host "SUCCESS! Payload written to disk." -ForegroundColor Green
    Write-Host "FastOS will read it from LBA 2048 on next boot." -ForegroundColor Cyan
    
} catch {
    Write-Host "[ERROR] Failed to write to disk: $_" -ForegroundColor Red
    Write-Host ""
    Write-Host "Common fixes:" -ForegroundColor Yellow
    Write-Host "  1. Run as Administrator" -ForegroundColor Yellow
    Write-Host "  2. Close any apps using the SATA disk" -ForegroundColor Yellow
    Write-Host "  3. Make sure the disk is not write-protected" -ForegroundColor Yellow
    exit 1
}

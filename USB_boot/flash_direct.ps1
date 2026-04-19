# ============================================================================
# FastOS — Flash directly to USB disk (LBA 0, no partitions needed)
# ============================================================================
# MUST RUN AS ADMINISTRATOR.
#
# Usage:
#   .\flash_direct.ps1 -DiskNumber 3
#   .\flash_direct.ps1 -DiskNumber 3 -Verify
# ============================================================================

param(
    [int]$DiskNumber = -1,
    [switch]$Verify
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# ── Check admin ──────────────────────────────────────────────────────────────
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (!$isAdmin) {
    Write-Host "ERROR: Must run as Administrator" -ForegroundColor Red
    Write-Host "  Right-click PowerShell -> Run as Administrator" -ForegroundColor Yellow
    exit 1
}

if ($DiskNumber -lt 0) {
    Write-Host "Usage: .\flash_direct.ps1 -DiskNumber <N>" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "Available disks:" -ForegroundColor Cyan
    Get-Disk | Format-Table Number, FriendlyName, @{L="Size GB";E={[int]($_.Size/1GB)}}, BusType -AutoSize
    exit 1
}

# ── Find image ───────────────────────────────────────────────────────────────
$ImagePath = "$Root\fastos.img"
if (!(Test-Path $ImagePath)) {
    Write-Host "ERROR: fastos.img not found in $Root. Run build.ps1 first." -ForegroundColor Red
    exit 1
}
$imgData = [System.IO.File]::ReadAllBytes($ImagePath)
$imgSize = $imgData.Length

# Verify MBR signature
if ($imgData.Length -ge 512) {
    $sig = [BitConverter]::ToUInt16($imgData, 510)
    if ($sig -ne 0xAA55) {
        Write-Host "ERROR: No valid MBR signature (0xAA55)" -ForegroundColor Red
        exit 1
    }
}

# ── Validate target ─────────────────────────────────────────────────────────
$disk = Get-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
if (!$disk) {
    Write-Host "ERROR: Disk $DiskNumber not found" -ForegroundColor Red
    exit 1
}

# Safety: refuse system disk
$sysDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
if ($DiskNumber -eq $sysDisk) {
    Write-Host "ERROR: Disk $DiskNumber is the SYSTEM DISK. Refusing." -ForegroundColor Red
    exit 1
}

$diskSizeGB = [int]($disk.Size / 1GB)

Write-Host ""
Write-Host "========================================" -ForegroundColor Cyan
Write-Host "  FastOS USB Flash (Direct LBA 0)" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Image  : fastos.img ($([int]($imgSize/1024)) KB)" -ForegroundColor White
Write-Host "  Target : Disk $DiskNumber - $($disk.FriendlyName)" -ForegroundColor White
Write-Host "  Size   : $diskSizeGB GB ($($disk.BusType))" -ForegroundColor White
Write-Host "  Write  : LBA 0 (offset 0x0, raw MBR boot)" -ForegroundColor White
Write-Host ""
Write-Host "WARNING: This will OVERWRITE the beginning of Disk $DiskNumber!" -ForegroundColor Red
Write-Host ""
$confirm = Read-Host "Type 'FLASH' to proceed"
if ($confirm -ne "FLASH") {
    Write-Host "Aborted." -ForegroundColor Yellow
    exit 0
}

# ── Dismount all volumes on this disk ────────────────────────────────────────
Write-Host ""
Write-Host "[1/3] Preparing disk..." -ForegroundColor Cyan

try {
    # Set disk offline then online to release locks
    Set-Disk -Number $DiskNumber -IsOffline $true -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
    Set-Disk -Number $DiskNumber -IsOffline $false -ErrorAction SilentlyContinue
    Start-Sleep -Milliseconds 500
} catch {
    Write-Host "      (disk prep warning: $($_.Exception.Message))" -ForegroundColor DarkGray
}

Write-Host "[1/3] Ready" -ForegroundColor Green

# ── Write image at LBA 0 ────────────────────────────────────────────────────
Write-Host "[2/3] Writing fastos.img -> Disk $DiskNumber (LBA 0)..." -ForegroundColor Cyan

$diskPath = "\\.\PhysicalDrive$DiskNumber"
$handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)

try {
    $handle.Seek(0, [System.IO.SeekOrigin]::Begin) | Out-Null

    $chunkSize = 64 * 1024
    $written = 0
    while ($written -lt $imgData.Length) {
        $remaining = $imgData.Length - $written
        $toWrite = [math]::Min($chunkSize, $remaining)
        $handle.Write($imgData, $written, $toWrite)
        $written += $toWrite
        $pct = [math]::Round(($written / $imgData.Length) * 100)
        Write-Host "`r      Writing: $pct% ($([int]($written/1024)) KB / $([int]($imgSize/1024)) KB)" -NoNewline -ForegroundColor Gray
    }
    $handle.Flush()
    Write-Host ""
    Write-Host "      Written: $written bytes" -ForegroundColor DarkGray
}
finally {
    $handle.Close()
}

Write-Host "[2/3] Write OK" -ForegroundColor Green

# ── Verify ───────────────────────────────────────────────────────────────────
if ($Verify) {
    Write-Host "[3/3] Verifying (read-back)..." -ForegroundColor Cyan

    $handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
        $handle.Seek(0, [System.IO.SeekOrigin]::Begin) | Out-Null
        $readBack = New-Object byte[] $imgData.Length
        $totalRead = 0
        while ($totalRead -lt $imgData.Length) {
            $bytesRead = $handle.Read($readBack, $totalRead, [math]::Min(65536, $imgData.Length - $totalRead))
            if ($bytesRead -eq 0) { break }
            $totalRead += $bytesRead
        }

        $ok = $true
        for ($i = 0; $i -lt $imgData.Length; $i++) {
            if ($imgData[$i] -ne $readBack[$i]) {
                Write-Host "      MISMATCH at byte $i" -ForegroundColor Red
                $ok = $false
                break
            }
        }
        if ($ok) {
            Write-Host "      Verify OK: $totalRead bytes match" -ForegroundColor Green
        }
    }
    finally {
        $handle.Close()
    }
    Write-Host "[3/3] Verification complete" -ForegroundColor Green
} else {
    Write-Host "[3/3] Skipped verify (use -Verify to enable)" -ForegroundColor DarkGray
}

# ── Done ─────────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host "  FLASH COMPLETE!" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "  Next steps:" -ForegroundColor Yellow
Write-Host "    1. Reboot PC" -ForegroundColor White
Write-Host "    2. Enter BIOS (DEL or F2)" -ForegroundColor White
Write-Host "    3. Set Boot -> Legacy/CSM -> USB first" -ForegroundColor White
Write-Host "    4. Save & reboot" -ForegroundColor White
Write-Host ""
Write-Host "  Expected on screen:" -ForegroundColor Yellow
Write-Host "    - FastOS boot screen (green border, gradient)" -ForegroundColor White
Write-Host "    - Shell prompt: fastos>" -ForegroundColor White
Write-Host "    - Type: gputest  (GPU hardware tests)" -ForegroundColor White
Write-Host "    - Type: gpucmd   (GPU command engine Level 2)" -ForegroundColor White
Write-Host ""

# ============================================================================
# FastOS â€" Flash to USB Partition 3 (Bare Metal)
# ============================================================================
# Writes fastos.img raw to partition 3 of a USB disk.
# MUST RUN AS ADMINISTRATOR.
#
# Usage:
#   .\flash_usb.ps1 -DiskNumber 2 -Partition 3
#   .\flash_usb.ps1 -ListDisks              # Show available disks
#   .\flash_usb.ps1 -DiskNumber 2 -Verify   # Write + read-back verify
# ============================================================================

param(
    [int]$DiskNumber = -1,
    [int]$Partition = 3,
    [switch]$ListDisks,
    [switch]$Verify,
    [string]$ImagePath = ""
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

# â"€â"€ Check admin â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
$isAdmin = ([Security.Principal.WindowsPrincipal] [Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
if (!$isAdmin) {
    Write-Host "ERROR: Must run as Administrator" -ForegroundColor Red
    Write-Host "  Right-click PowerShell â†’ Run as Administrator" -ForegroundColor Yellow
    exit 1
}

# â"€â"€ List disks â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
if ($ListDisks) {
    Write-Host ""
    Write-Host "Available disks:" -ForegroundColor Cyan
    Get-Disk | Format-Table Number, FriendlyName, @{L="Size";E={"{0:N1} GB" -f ($_.Size/1GB)}}, BusType, PartitionStyle -AutoSize
    Write-Host ""
    Write-Host "Partitions:" -ForegroundColor Cyan
    Get-Partition | Where-Object { $_.DiskNumber -ne 0 } | Format-Table DiskNumber, PartitionNumber, @{L="Size";E={"{0:N1} MB" -f ($_.Size/1MB)}}, Type, DriveLetter -AutoSize
    exit 0
}

if ($DiskNumber -lt 0) {
    Write-Host "Usage: .\flash_usb.ps1 -DiskNumber <N> -Partition $Partition" -ForegroundColor Yellow
    Write-Host "       .\flash_usb.ps1 -ListDisks" -ForegroundColor Yellow
    exit 1
}

# â"€â"€ Find image â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
if ($ImagePath -eq "") { $ImagePath = "$Root\fastos.img" }
if (!(Test-Path $ImagePath)) {
    Write-Host "ERROR: $ImagePath not found. Run .\build.ps1 first." -ForegroundColor Red
    exit 1
}
$imgSize = (Get-Item $ImagePath).Length
$imgData = [System.IO.File]::ReadAllBytes($ImagePath)

# Verify MBR
$sig = [BitConverter]::ToUInt16($imgData, 510)
if ($sig -ne 0xAA55) {
    Write-Host "ERROR: Image has no valid MBR signature (0x$($sig.ToString('X4')))" -ForegroundColor Red
    exit 1
}

# â"€â"€ Validate target â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
$disk = Get-Disk -Number $DiskNumber -ErrorAction SilentlyContinue
if (!$disk) {
    Write-Host "ERROR: Disk $DiskNumber not found" -ForegroundColor Red
    exit 1
}

# Safety: refuse to write to the system disk
$systemDisk = (Get-Partition -DriveLetter C -ErrorAction SilentlyContinue).DiskNumber
if ($DiskNumber -eq $systemDisk) {
    Write-Host "ERROR: Disk $DiskNumber is the SYSTEM DISK. Refusing." -ForegroundColor Red
    exit 1
}

$part = Get-Partition -DiskNumber $DiskNumber -PartitionNumber $Partition -ErrorAction SilentlyContinue
if (!$part) {
    Write-Host "ERROR: Partition $Partition not found on disk $DiskNumber" -ForegroundColor Red
    Write-Host ""
    Write-Host "Available partitions on disk $DiskNumber`:" -ForegroundColor Yellow
    Get-Partition -DiskNumber $DiskNumber | Format-Table PartitionNumber, @{L="Size";E={"{0:N1} MB" -f ($_.Size/1MB)}}, Type, DriveLetter -AutoSize
    exit 1
}

$partSize = $part.Size
$partOffset = $part.Offset

Write-Host ""
Write-Host "â•"â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—" -ForegroundColor Cyan
Write-Host "â•‘  FastOS USB Flash                                            â•‘" -ForegroundColor Cyan
Write-Host "â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•" -ForegroundColor Cyan
Write-Host ""
Write-Host "  Image     : $ImagePath ($([math]::Round($imgSize/1024))KB)" -ForegroundColor White
Write-Host "  Target    : Disk $DiskNumber, Partition $Partition" -ForegroundColor White
Write-Host "  Disk      : $($disk.FriendlyName)" -ForegroundColor White
Write-Host "  Bus       : $($disk.BusType)" -ForegroundColor White
Write-Host "  Part size : $([math]::Round($partSize/1MB, 1)) MB" -ForegroundColor White
Write-Host "  Part off  : 0x$($partOffset.ToString('X')) ($partOffset bytes)" -ForegroundColor White
Write-Host ""

if ($imgSize -gt $partSize) {
    Write-Host "ERROR: Image ($imgSize B) larger than partition ($partSize B)" -ForegroundColor Red
    exit 1
}

# â"€â"€ Confirmation â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
Write-Host "WARNING: This will OVERWRITE partition $Partition on disk $DiskNumber!" -ForegroundColor Red
Write-Host "         All data on this partition will be DESTROYED." -ForegroundColor Red
Write-Host ""
$confirm = Read-Host "Type 'FLASH' to proceed"
if ($confirm -ne "FLASH") {
    Write-Host "Aborted." -ForegroundColor Yellow
    exit 0
}

# â"€â"€ Dismount partition â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
Write-Host ""
Write-Host "[1/3] Preparing partition..." -ForegroundColor Cyan

# Dismount if mounted
if ($part.DriveLetter) {
    Write-Host "      Dismounting $($part.DriveLetter):..." -ForegroundColor Gray
    $vol = Get-Volume -DriveLetter $part.DriveLetter -ErrorAction SilentlyContinue
    if ($vol) {
        # Use diskpart to offline the volume
        $dpScript = @"
select disk $DiskNumber
select partition $Partition
remove all dismount
"@
        $dpScript | diskpart | Out-Null
    }
}

# â"€â"€ Write image â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
Write-Host "[2/3] Writing fastos.img â†’ Disk $DiskNumber Partition $Partition..." -ForegroundColor Cyan

# Open physical disk for raw write
$diskPath = "\\.\PhysicalDrive$DiskNumber"
$handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Write, [System.IO.FileShare]::ReadWrite)

try {
    # Seek to partition offset
    $handle.Seek($partOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
    Write-Host "      Seeking to offset $partOffset (0x$($partOffset.ToString('X')))" -ForegroundColor DarkGray

    # Write in 64KB chunks with progress
    $chunkSize = 64 * 1024
    $written = 0
    while ($written -lt $imgData.Length) {
        $remaining = $imgData.Length - $written
        $toWrite = [math]::Min($chunkSize, $remaining)
        $handle.Write($imgData, $written, $toWrite)
        $written += $toWrite
        $pct = [math]::Round(($written / $imgData.Length) * 100)
        Write-Host "`r      Writing: $pct% ($([math]::Round($written/1024))KB / $([math]::Round($imgData.Length/1024))KB)" -NoNewline -ForegroundColor Gray
    }
    $handle.Flush()
    Write-Host ""
    Write-Host "      Write complete: $written bytes" -ForegroundColor DarkGray
}
finally {
    $handle.Close()
}

Write-Host "[2/3] Write OK" -ForegroundColor Green

# â"€â"€ Verify â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
if ($Verify) {
    Write-Host "[3/3] Verifying (read-back)..." -ForegroundColor Cyan

    $handle = [System.IO.File]::Open($diskPath, [System.IO.FileMode]::Open, [System.IO.FileAccess]::Read, [System.IO.FileShare]::ReadWrite)
    try {
        $handle.Seek($partOffset, [System.IO.SeekOrigin]::Begin) | Out-Null
        $readBack = New-Object byte[] $imgData.Length
        $totalRead = 0
        while ($totalRead -lt $imgData.Length) {
            $bytesRead = $handle.Read($readBack, $totalRead, [math]::Min(65536, $imgData.Length - $totalRead))
            if ($bytesRead -eq 0) { break }
            $totalRead += $bytesRead
        }

        # Compare
        $mismatch = $false
        for ($i = 0; $i -lt $imgData.Length; $i++) {
            if ($imgData[$i] -ne $readBack[$i]) {
                Write-Host "      MISMATCH at byte $i: wrote 0x$($imgData[$i].ToString('X2')), read 0x$($readBack[$i].ToString('X2'))" -ForegroundColor Red
                $mismatch = $true
                break
            }
        }
        if (!$mismatch) {
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

# â"€â"€ Done â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€â"€
Write-Host ""
Write-Host "â•"â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•—" -ForegroundColor Green
Write-Host "â•‘  FLASH COMPLETE                                              â•‘" -ForegroundColor Green
Write-Host "â•‘                                                              â•‘" -ForegroundColor Green
Write-Host "â•‘  Boot: Set BIOS to boot from USB (Legacy/CSM mode)           â•‘" -ForegroundColor Green
Write-Host "â•‘  Expected VGA output:                                        â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] Stage1: MBR loaded                               â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] Stage2: starting                                 â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] CPUID: OK                                        â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] Long Mode: OK                                    â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] Protected Mode OK                                â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] 64-bit Long Mode: ACTIVE                         â•‘" -ForegroundColor Green
Write-Host "â•‘    [FastOS] SSE4.2+AVX2+FMA3: READY                          â•‘" -ForegroundColor Green
Write-Host "â•‘    FastOS v0.1.0 â€" Ryzen 5 5600X + RTX 3060 12G              â•‘" -ForegroundColor Green
Write-Host "â•‘    [OK] Boot info valid                                      â•‘" -ForegroundColor Green
Write-Host "â•‘    [PCI] Scanning bus...                                     â•‘" -ForegroundColor Green
Write-Host "â•‘    [GPU] Vendor:Device = 0x10DE:0x2504                       â•‘" -ForegroundColor Green
Write-Host "â•‘    [GPU] GA106 confirmed â€" initializing driver...            â•‘" -ForegroundColor Green
Write-Host "â•‘    [GPU] READY  VRAM: 12288 MB  Chip: 0x...                  â•‘" -ForegroundColor Green
Write-Host "â•šâ•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•" -ForegroundColor Green
Write-Host ""


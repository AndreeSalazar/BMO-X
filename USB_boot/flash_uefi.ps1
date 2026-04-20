# ============================================================================
# FastOS — UEFI USB Flash Script
# ============================================================================
# Automatically formats USB as GPT + FAT32 (ESP) and copies EFI files
# Target: Ryzen 5 5600X + RTX 3060 12G | UEFI Native
# ============================================================================

param(
    [Parameter(Mandatory=$true)]
    [int]$DiskNumber
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS UEFI USB Flash" -ForegroundColor Cyan
Write-Host "  Target: Disk $DiskNumber" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# ── Verify files exist ─────────────────────────────────────────────────────
Write-Host "[1/5] Verifying build files..." -ForegroundColor Cyan

$efiPath = "$Root\BOOTX64.EFI"
$kernelPath = "$Root\kernel.bin"

if (!(Test-Path $efiPath)) {
    Write-Host "      ERROR: BOOTX64.EFI not found. Run build_uefi.ps1 first." -ForegroundColor Red
    exit 1
}

if (!(Test-Path $kernelPath)) {
    Write-Host "      ERROR: kernel.bin not found. Run build_uefi.ps1 first." -ForegroundColor Red
    exit 1
}

$efiSize = (Get-Item $efiPath).Length
$kernelSize = (Get-Item $kernelPath).Length

Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024))KB" -ForegroundColor DarkGray
Write-Host "      kernel.bin: $([math]::Round($kernelSize/1024))KB" -ForegroundColor DarkGray
Write-Host "[1/5] Files verified" -ForegroundColor Green

# ── Show disk info for confirmation ─────────────────────────────────────────
Write-Host "[2/5] Disk $DiskNumber information:" -ForegroundColor Cyan

$disk = Get-Disk -Number $DiskNumber
Write-Host "      Size: $([math]::Round($disk.Size/1GB, 1))GB" -ForegroundColor DarkGray
Write-Host "      BusType: $($disk.BusType)" -ForegroundColor DarkGray
Write-Host "      FriendlyName: $($disk.FriendlyName)" -ForegroundColor DarkGray
Write-Host "      Partitions: $($disk.NumberOfPartitions)" -ForegroundColor DarkGray

Write-Host ""
Write-Host "      WARNING: This will ERASE ALL DATA on Disk $DiskNumber!" -ForegroundColor Red
$confirm = Read-Host "      Continue? (yes/no)"

if ($confirm -ne "yes") {
    Write-Host "      Aborted." -ForegroundColor Yellow
    exit 0
}

# ── Clear disk and create GPT ───────────────────────────────────────────────
Write-Host "[3/5] Formatting USB as GPT + FAT32 (ESP)..." -ForegroundColor Cyan

# Clear partition table
Write-Host "      Clearing disk..." -ForegroundColor DarkGray
$disk | Clear-Disk -RemoveData -RemoveOEM -Confirm:$false

# Convert to GPT
Write-Host "      Converting to GPT..." -ForegroundColor DarkGray
$disk | Set-Disk -PartitionStyle GPT

# Create EFI System Partition (100MB minimum, FAT32)
Write-Host "      Creating EFI System Partition..." -ForegroundColor DarkGray
$partition = $disk | New-Partition -Size 100MB -GptType '{C12A7328-F81F-11D2-BA4B-00A0C93EC93B}' -AssignDriveLetter

# Format as FAT32
Write-Host "      Formatting as FAT32..." -ForegroundColor DarkGray
$partition | Format-Volume -FileSystem FAT32 -NewFileSystemLabel "FastOS-ESP" -Confirm:$false

$driveLetter = $partition.DriveLetter
Write-Host "      Drive: ${driveLetter}:" -ForegroundColor DarkGray

Write-Host "[3/5] USB formatted" -ForegroundColor Green

# ── Copy EFI files ──────────────────────────────────────────────────────────
Write-Host "[4/5] Copying EFI files..." -ForegroundColor Cyan

# Create EFI\BOOT\ directory
$efiBootPath = "${driveLetter}:\EFI\BOOT"
if (!(Test-Path $efiBootPath)) {
    New-Item -Path $efiBootPath -ItemType Directory -Force | Out-Null
}

# Copy BOOTX64.EFI
Write-Host "      Copying BOOTX64.EFI to EFI\BOOT\BOOTX64.EFI..." -ForegroundColor DarkGray
Copy-Item $efiPath "$efiBootPath\BOOTX64.EFI" -Force

# Copy kernel.bin
Write-Host "      Copying kernel.bin to root..." -ForegroundColor DarkGray
Copy-Item $kernelPath "${driveLetter}:\kernel.bin" -Force

Write-Host "[4/5] Files copied" -ForegroundColor Green

# ── Verify ──────────────────────────────────────────────────────────────────
Write-Host "[5/5] Verifying..." -ForegroundColor Cyan

if (!(Test-Path "$efiBootPath\BOOTX64.EFI")) {
    Write-Host "      ERROR: BOOTX64.EFI not copied!" -ForegroundColor Red
    exit 1
}

if (!(Test-Path "${driveLetter}:\kernel.bin")) {
    Write-Host "      ERROR: kernel.bin not copied!" -ForegroundColor Red
    exit 1
}

$copiedEfiSize = (Get-Item "$efiBootPath\BOOTX64.EFI").Length
$copiedKernelSize = (Get-Item "${driveLetter}:\kernel.bin").Length

Write-Host "      BOOTX64.EFI: $copiedEfiSize bytes" -ForegroundColor DarkGray
Write-Host "      kernel.bin: $copiedKernelSize bytes" -ForegroundColor DarkGray

Write-Host "[5/5] Verification passed" -ForegroundColor Green

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  FLASH COMPLETE - UEFI NATIVE" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  USB Drive: Disk $DiskNumber (${driveLetter}:)" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  To boot:" -ForegroundColor Green
Write-Host "    1. Reboot your computer" -ForegroundColor Green
Write-Host "    2. Enter BIOS/UEFI setup (F2 or Del)" -ForegroundColor Green
Write-Host "    3. Disable CSM/Legacy Boot (set to UEFI Only)" -ForegroundColor Green
Write-Host "    4. Add USB to boot order" -ForegroundColor Green
Write-Host "    5. Save and boot from USB" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Your BIOS should detect 'FastOS-ESP' as bootable." -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

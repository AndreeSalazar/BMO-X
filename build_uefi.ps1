# ============================================================================
# FastOS — UEFI Native Build Pipeline
# ============================================================================
# UEFI Bootloader (Rust) → Rust kernel (release) → EFI Application
# Target: bare metal, Ryzen 5 5600X + RTX 3060 12G
# Output: BOOTX64.EFI + kernel.bin (ready to flash to ESP)
# ============================================================================

param(
    [switch]$Clean
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS UEFI Native Build - Rust → EFI Application" -ForegroundColor Cyan
Write-Host "  Target: Ryzen 5 5600X + RTX 3060 12G | UEFI Native" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# ── Clean ────────────────────────────────────────────────────────────────────
if ($Clean) {
    Write-Host "[CLEAN] Removing build artifacts..." -ForegroundColor Yellow
    Remove-Item "$Root\bootloader\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel\target" -Recurse -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel.bin" -ErrorAction SilentlyContinue
    Remove-Item "$Root\BOOTX64.EFI" -ErrorAction SilentlyContinue
    Remove-Item "$Root\USB_boot" -Recurse -ErrorAction SilentlyContinue
    Write-Host "[CLEAN] Done." -ForegroundColor Green
    return
}

# ── Step 1: Build UEFI Bootloader ────────────────────────────────────────────
Write-Host "[1/3] Building UEFI Bootloader..." -ForegroundColor Cyan

Push-Location "$Root\bootloader"

# Ensure nightly toolchain
$savedEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$cargoOutput = & rustup run nightly cargo build --release 2>&1
$cargoExit = $LASTEXITCODE
$ErrorActionPreference = $savedEAP

$cargoOutput | ForEach-Object {
    $line = $_.ToString()
    if ($line -match "error\[") {
        Write-Host "      $line" -ForegroundColor Red
    } elseif ($line -match "Compiling|Finished") {
        Write-Host "      $line" -ForegroundColor DarkGray
    }
}

if ($cargoExit -ne 0) {
    $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
    Pop-Location
    throw "UEFI Bootloader build failed"
}

# Find the EFI file
$efiPath = Get-ChildItem "$Root\bootloader\target\x86_64-unknown-uefi\release\fastos-bootloader*.efi" -File | Select-Object -First 1 -ExpandProperty FullName
if (!$efiPath) {
    Pop-Location
    throw "Cannot find BOOTX64.EFI at target\x86_64-unknown-uefi\release\"
}

$efiSize = (Get-Item $efiPath).Length
Write-Host "      BOOTX64.EFI: $([math]::Round($efiSize/1024, 1))KB" -ForegroundColor DarkGray

# Copy to root as BOOTX64.EFI for convenience
Copy-Item $efiPath "$Root\BOOTX64.EFI" -Force

Pop-Location
Write-Host "[1/3] UEFI Bootloader OK" -ForegroundColor Green

# ── Step 2: Build Rust Kernel ──────────────────────────────────────────────
Write-Host "[2/3] Building Rust Kernel..." -ForegroundColor Cyan

Push-Location "$Root\kernel"

$savedEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$cargoOutput = & rustup run nightly cargo build --release 2>&1
$cargoExit = $LASTEXITCODE
$ErrorActionPreference = $savedEAP

$cargoOutput | ForEach-Object {
    $line = $_.ToString()
    if ($line -match "error\[") {
        Write-Host "      $line" -ForegroundColor Red
    } elseif ($line -match "Compiling|Finished") {
        Write-Host "      $line" -ForegroundColor DarkGray
    }
}

if ($cargoExit -ne 0) {
    $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
    Pop-Location
    throw "Kernel build failed"
}

# Extract kernel binary
$sysroot = (& rustup run nightly rustc --print sysroot).Trim()
$objcopy = "$sysroot\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-objcopy.exe"
if (!(Test-Path $objcopy)) {
    $objcopy = "$sysroot\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-objcopy.exe"
}

$elfPath = "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel"
if (!(Test-Path $elfPath)) {
    $elfPath = Get-ChildItem "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel*" -File | Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } | Select-Object -First 1 -ExpandProperty FullName
}

& $objcopy $elfPath --output-target binary -O binary "$Root\kernel.bin"
if ($LASTEXITCODE -ne 0) {
    Pop-Location
    throw "objcopy failed"
}

$kernelSize = (Get-Item "$Root\kernel.bin").Length
Write-Host "      kernel.bin: $([math]::Round($kernelSize/1024, 1))KB" -ForegroundColor DarkGray

Pop-Location
Write-Host "[2/3] Kernel OK" -ForegroundColor Green

# ── Step 3: Export to USB_boot folder ──────────────────────────────────────
Write-Host "[3/3] Exporting to USB_boot folder..." -ForegroundColor Cyan

$usbDir = "$Root\USB_boot"
if (!(Test-Path $usbDir)) {
    New-Item -Path $usbDir -ItemType Directory -Force | Out-Null
}

Copy-Item "$Root\BOOTX64.EFI" "$usbDir\BOOTX64.EFI" -Force
Copy-Item "$Root\kernel.bin" "$usbDir\kernel.bin" -Force

# Create README with UEFI instructions
$buildDate = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
$readme = @"
======================================================
  FastOS - UEFI Native Boot Image
======================================================

  Bootloader: BOOTX64.EFI ($([math]::Round($efiSize/1024))KB)
  Kernel:    kernel.bin ($([math]::Round($kernelSize/1024))KB)
  Built:     $buildDate
  Target:    Ryzen 5 5600X + RTX 3060 12G
  Mode:      UEFI Native (No CSM/Legacy)

------------------------------------------------------
  HOW TO FLASH TO USB (UEFI)
------------------------------------------------------

  Step 1: Format USB as GPT + FAT32 (ESP)
    - In Windows: Disk Management → Delete partitions → New → GPT → FAT32
    - Or: diskpart → clean → convert gpt → create partition efi → format fs=fat32 quick

  Step 2: Copy EFI files to ESP
    - Create folder: EFI\BOOT\ on the USB
    - Copy BOOTX64.EFI to: EFI\BOOT\BOOTX64.EFI
    - Copy kernel.bin to: kernel.bin (root of ESP)

  Step 3: Enable UEFI boot in BIOS
    - Disable CSM/Legacy Boot in BIOS
    - Set boot mode to UEFI Only
    - Add USB to boot order

  Step 4: Boot from USB
    - Select USB from UEFI boot menu
    - FastOS bootloader will load

------------------------------------------------------
  MEMORY MAP (UEFI Native)
------------------------------------------------------
  Bootloader: Loaded by UEFI firmware
  Kernel:     Loaded at 0x100000 (1MB) by bootloader
  DMA:        0x400000 (4MB) buffer pool
  Stack:      0x800000 (8MB) grows down

------------------------------------------------------
  ADVANTAGES OF UEFI NATIVE
------------------------------------------------------
  - No legacy BIOS limitations (INT 15h, MBR, etc.)
  - GPT partition support (> 2TB)
  - Secure Boot support
  - Faster boot times
  - Modern firmware interface
  - Better driver support

======================================================
"@

Set-Content "$usbDir\README.txt" -Value $readme -Encoding UTF8

Write-Host "[3/3] USB_boot folder ready!" -ForegroundColor Green
Write-Host "      Path: $usbDir" -ForegroundColor DarkGray

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETE - UEFI NATIVE" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Output:" -ForegroundColor Green
Write-Host "    BOOTX64.EFI ($([math]::Round($efiSize/1024))KB)" -ForegroundColor Green
Write-Host "    kernel.bin ($([math]::Round($kernelSize/1024))KB)" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Location: USB_boot\" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  To boot:" -ForegroundColor Green
Write-Host "    1. Format USB as GPT + FAT32 (ESP)" -ForegroundColor Green
Write-Host "    2. Copy BOOTX64.EFI to EFI\BOOT\BOOTX64.EFI" -ForegroundColor Green
Write-Host "    3. Copy kernel.bin to root of ESP" -ForegroundColor Green
Write-Host "    4. Boot from USB in UEFI mode" -ForegroundColor Green
Write-Host "    5. Disable CSM in BIOS for best results" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

# ============================================================================
# FastOS — UEFI Native Build Pipeline
# ============================================================================
# UEFI Bootloader (Rust) → Rust kernel ELF (release) → EFI Application
# Target: bare metal, Ryzen 5 5600X + RTX 3060 12G
# Output: BOOTX64.EFI + kernel.elf (ready to flash to ESP)
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
    Remove-Item "$Root\kernel.elf" -ErrorAction SilentlyContinue
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

# ── Step 2: Build Rust Kernel (ELF) ──────────────────────────────────────────
Write-Host "[2/3] Building Rust Kernel (ELF)..." -ForegroundColor Cyan

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

# Copy the ELF directly (no objcopy needed — bootloader loads ELF natively)
$elfPath = "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel"
if (!(Test-Path $elfPath)) {
    $elfPath = Get-ChildItem "$Root\kernel\target\x86_64-unknown-none\release\fastos-kernel*" -File | Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } | Select-Object -First 1 -ExpandProperty FullName
}

if (!$elfPath -or !(Test-Path $elfPath)) {
    Pop-Location
    throw "Cannot find kernel ELF at target\x86_64-unknown-none\release\"
}

Copy-Item $elfPath "$Root\kernel.elf" -Force

$kernelSize = (Get-Item "$Root\kernel.elf").Length
Write-Host "      kernel.elf: $([math]::Round($kernelSize/1024, 1))KB" -ForegroundColor DarkGray

Pop-Location
Write-Host "[2/3] Kernel OK" -ForegroundColor Green

# ── Step 3: Export to USB_boot folder ──────────────────────────────────────
Write-Host "[3/3] Exporting to USB_boot folder..." -ForegroundColor Cyan

$usbDir = "$Root\USB_boot"
if (!(Test-Path $usbDir)) {
    New-Item -Path $usbDir -ItemType Directory -Force | Out-Null
}

Copy-Item "$Root\BOOTX64.EFI" "$usbDir\BOOTX64.EFI" -Force
Copy-Item "$Root\kernel.elf" "$usbDir\kernel.elf" -Force

# Copy GSP firmware if available
$gspPath = "$Root\gsp_ga10x.bin"
if (Test-Path $gspPath) {
    Copy-Item $gspPath "$usbDir\gsp_ga10x.bin" -Force
    $gspSize = (Get-Item $gspPath).Length
    Write-Host "      gsp_ga10x.bin: $([math]::Round($gspSize/1MB, 1))MB (GPU firmware)" -ForegroundColor DarkGray
} else {
    Write-Host "      WARNING: gsp_ga10x.bin not found - GPU GSP will not be available" -ForegroundColor Yellow
}

Copy-Item "$Root\flash_uefi.ps1" "$usbDir\flash_uefi.ps1" -Force
Copy-Item "$Root\flash_uefi.ps1" "$usbDir\flash_direct.ps1" -Force

# Create README with UEFI instructions
$buildDate = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
$readme = @"
======================================================
  FastOS - UEFI Native Boot Image
======================================================

  Bootloader: BOOTX64.EFI ($([math]::Round($efiSize/1024))KB)
  Kernel:    kernel.elf ($([math]::Round($kernelSize/1024))KB)
  Built:     $buildDate
  Target:    Ryzen 5 5600X + RTX 3060 12G
  Mode:      UEFI Native (No CSM/Legacy)

------------------------------------------------------
  HOW TO FLASH TO USB (UEFI) - AUTOMATED
------------------------------------------------------

  Option 1 - flash_uefi.ps1 (RECOMMENDED, simple):
    .\flash_uefi.ps1 -DiskNumber <N>
    
    Example: If your USB is Disk 3:
    .\flash_uefi.ps1 -DiskNumber 3

  Option 2 - flash_direct.ps1 (auto-detect USB):
    .\flash_direct.ps1
    # Or: .\flash_direct.ps1 -DiskNumber 3

  Both scripts will:
    - Format USB as GPT + FAT32 (full size ESP)
    - Create EFI\BOOT\ directory
    - Copy BOOTX64.EFI to EFI\BOOT\BOOTX64.EFI
    - Copy kernel.elf to root
    - Make USB bootable

  Option 3 - Manual:
    1. Format USB as GPT + FAT32 (full size)
       - Disk Management -> Delete partitions -> New -> GPT -> FAT32
    2. Copy EFI files to ESP
       - Create: EFI\BOOT\ on USB
       - Copy: BOOTX64.EFI -> EFI\BOOT\BOOTX64.EFI
       - Copy: kernel.elf -> root of ESP

  BIOS Setup:
    - Disable CSM/Legacy Boot (set to UEFI Only)
    - Disable Secure Boot
    - Add USB to boot order
    - Select USB from UEFI boot menu

------------------------------------------------------
  BOOT SEQUENCE
------------------------------------------------------
  1. UEFI firmware loads BOOTX64.EFI
  2. Bootloader queries GOP (framebuffer)
  3. Bootloader loads kernel.elf (ELF64)
  4. Bootloader finds RSDP (ACPI)
  5. Bootloader builds BootInfo struct
  6. Bootloader exits boot services
  7. Bootloader jumps to kernel _start
  8. Kernel validates BootInfo, inits serial
  9. Kernel inits PIC/IDT/PIT, enables IRQs
  10. Kernel runs interactive shell on GOP FB

======================================================
"@

Set-Content "$usbDir\README.txt" -Value $readme -Encoding UTF8

Write-Host "[3/3] USB_boot folder ready!" -ForegroundColor Green
Write-Host "      Path: $usbDir" -ForegroundColor DarkGray

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETE - UEFI NATIVE (ELF)" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Output:" -ForegroundColor Green
Write-Host "    BOOTX64.EFI ($([math]::Round($efiSize/1024))KB)" -ForegroundColor Green
Write-Host "    kernel.elf ($([math]::Round($kernelSize/1024))KB)" -ForegroundColor Green
if (Test-Path "$Root\gsp_ga10x.bin") {
    Write-Host "    gsp_ga10x.bin ($([math]::Round((Get-Item "$Root\gsp_ga10x.bin").Length/1MB, 1))MB) (GPU firmware)" -ForegroundColor Green
}
Write-Host "" -ForegroundColor Green
Write-Host "  Location: USB_boot\" -ForegroundColor Green
Write-Host ""
Write-Host "  To flash USB:" -ForegroundColor Green
Write-Host "    cd USB_boot" -ForegroundColor Green
Write-Host "    .\flash_uefi.ps1 -DiskNumber <N>" -ForegroundColor Green
Write-Host "    # Or (auto-detect USB):" -ForegroundColor Green
Write-Host "    .\flash_direct.ps1" -ForegroundColor Green
Write-Host ""
Write-Host "  IMPORTANT BIOS settings:" -ForegroundColor Yellow
Write-Host "    - CSM: DISABLED (UEFI Only)" -ForegroundColor Yellow
Write-Host "    - Secure Boot: DISABLED" -ForegroundColor Yellow
Write-Host ""
Write-Host "  Example (if USB is Disk 3):" -ForegroundColor Green
Write-Host "    .\flash_uefi.ps1 -DiskNumber 3" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

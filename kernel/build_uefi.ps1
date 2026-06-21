#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Build FastOS UEFI and prepare for USB boot
.DESCRIPTION
    1. Builds bootloader (nightly, x86_64-unknown-uefi)
    2. Builds kernel (stable, x86_64-unknown-none)
    3. Stages files in EFI/BOOT/ structure
    4. Provides Rufus instructions to flash USB
.PARAMETER BuildOnly
    Just build, don't stage or show flash instructions.
.PARAMETER Run
    After building, launch in QEMU with OVMF.
.PARAMETER Force
    Force clean rebuild of both bootloader and kernel.
.EXAMPLE
    .\build_uefi.ps1              # Build + stage + instructions
    .\build_uefi.ps1 -BuildOnly   # Build only
    .\build_uefi.ps1 -Run         # Build + QEMU test
#>
param(
    [switch]$BuildOnly,
    [switch]$Run,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

# ── Paths ───────────────────────────────────────────────────────────────
$rootDir       = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$bootloaderDir = Join-Path $rootDir "bootloader"
$kernelDir     = Join-Path $rootDir "kernel"
$targetDir     = Join-Path $rootDir "target_build"
$stagingDir    = Join-Path $targetDir "staging"
$efiBootDir    = Join-Path $stagingDir "EFI\BOOT"

Write-Host ""
Write-Host "  ============================================"
Write-Host "         FastOS UEFI Build System"
Write-Host "  ============================================"
Write-Host ""

# ── Phase 0: Clean ──────────────────────────────────────────────────────
if ($Force) {
    Write-Host "[clean] Removing build artifacts..." -ForegroundColor Yellow
    if (Test-Path $targetDir) { Remove-Item $targetDir -Recurse -Force }
}

# ── Phase 1: Build Bootloader (nightly) ─────────────────────────────────
Write-Host "[bootloader] Building UEFI bootloader (nightly)..." -ForegroundColor Green
Push-Location $bootloaderDir
try {
    cargo build --release
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[bootloader] BUILD FAILED" -ForegroundColor Red
        exit 1
    }
} finally {
    Pop-Location
}

$bootloaderEfi = Join-Path $bootloaderDir "target\x86_64-unknown-uefi\release\fastos-bootloader.efi"
if (-not (Test-Path $bootloaderEfi)) {
    Write-Host "[bootloader] EFI binary not found" -ForegroundColor Red
    exit 1
}
$bootloaderSize = (Get-Item $bootloaderEfi).Length
Write-Host ("[bootloader] OK: {0} bytes" -f $bootloaderSize) -ForegroundColor Green

# ── Phase 2: Build Kernel (stable) ──────────────────────────────────────
Write-Host "[kernel] Building kernel (stable)..." -ForegroundColor Green
$kernelTargetDir = Join-Path $targetDir "kernel"
Push-Location $kernelDir
try {
    cargo build --release --target x86_64-unknown-none --target-dir $kernelTargetDir
    if ($LASTEXITCODE -ne 0) {
        Write-Host "[kernel] BUILD FAILED" -ForegroundColor Red
        exit 1
    }
} finally {
    Pop-Location
}

$kernelElf = Join-Path $kernelTargetDir "x86_64-unknown-none\release\fastos-kernel"
if (-not (Test-Path $kernelElf)) {
    Write-Host "[kernel] ELF not found" -ForegroundColor Red
    exit 1
}
$kernelSize = (Get-Item $kernelElf).Length
Write-Host ("[kernel] OK: {0} bytes" -f $kernelSize) -ForegroundColor Green

if ($BuildOnly) {
    Write-Host ""
    Write-Host "Build-only mode."
    Write-Host ("  Bootloader: {0}" -f $bootloaderEfi)
    Write-Host ("  Kernel:     {0}" -f $kernelElf)
    exit 0
}

# ── Phase 3: Stage EFI boot files ───────────────────────────────────────
Write-Host "[stage] Preparing EFI boot structure..." -ForegroundColor Green
if (Test-Path $stagingDir) { Remove-Item $stagingDir -Recurse -Force }
New-Item -ItemType Directory -Path $efiBootDir -Force | Out-Null

Copy-Item $bootloaderEfi -Destination (Join-Path $efiBootDir "BOOTX64.EFI")
Copy-Item $kernelElf     -Destination (Join-Path $efiBootDir "kernel.elf")

Write-Host ("  {0}\EFI\BOOT\BOOTX64.EFI  ({1} bytes)" -f $stagingDir, $bootloaderSize)
Write-Host ("  {0}\EFI\BOOT\kernel.elf   ({1} bytes)" -f $stagingDir, $kernelSize)

# ── Phase 4: Summary + Flash instructions ───────────────────────────────
Write-Host ""
Write-Host "  ============================================"
Write-Host "         BUILD COMPLETE"
Write-Host "  ============================================"
Write-Host ""
Write-Host "  Files staged in: $stagingDir"
Write-Host ""
Write-Host "  To flash to USB (Windows, no admin needed):" -ForegroundColor Cyan
Write-Host "    1. Insert USB drive (4GB+)" -ForegroundColor White
Write-Host "    2. Note the drive letter (e.g. E:)" -ForegroundColor White
Write-Host "    3. Open CMD as administrator and run:" -ForegroundColor White
Write-Host ""
Write-Host "       format E: /FS:FAT32 /Q /V:FASTOS" -ForegroundColor Yellow
Write-Host "       xcopy /s /e $stagingDir\EFI\* E:\EFI\" -ForegroundColor Yellow
Write-Host ""
Write-Host "    4. Boot from USB in your BIOS/UEFI" -ForegroundColor White
Write-Host ""
Write-Host "  Alternative: use Rufus" -ForegroundColor Cyan
Write-Host "    1. Open Rufus" -ForegroundColor White
Write-Host "    2. Device: select your USB" -ForegroundColor White
Write-Host "    3. Click SELECT → choose any .iso or .img" -ForegroundColor White
Write-Host "    4. Partition scheme: GPT" -ForegroundColor White
Write-Host "    5. Target system: UEFI (non CSM)" -ForegroundColor White
Write-Host "    6. File system: Large FAT32" -ForegroundColor White
Write-Host "    7. After format, manually copy EFI folder to USB" -ForegroundColor White
Write-Host ""

# ── Phase 5: QEMU test ──────────────────────────────────────────────────
if ($Run) {
    Write-Host "[qemu] Launching QEMU..." -ForegroundColor Green
    $qemu = "qemu-system-x86_64"
    if (Get-Command $qemu -ErrorAction SilentlyContinue) {
        $imgDir = $stagingDir
        & $qemu -bios OVMF.fd -drive file=fat:rw:$imgDir,format=raw -m 2048 -serial stdio
    } else {
        Write-Host "  QEMU not found. Install qemu-system-x86_64." -ForegroundColor Yellow
    }
}

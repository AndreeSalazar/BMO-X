#!/usr/bin/env pwsh
<#
.SYNOPSIS
    Build FastOS UEFI kernel
.DESCRIPTION
    Builds the kernel for UEFI boot on x86_64 with release optimizations.
.PARAMETER BuildOnly
    Skip UEFI disk image creation, just build the ELF.
.PARAMETER Run
    After building, launch in QEMU.
.PARAMETER Force
    Force clean rebuild.
#>
param(
    [switch]$BuildOnly,
    [switch]$Run,
    [switch]$Force
)

$ErrorActionPreference = "Stop"

$kernelDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$targetDir = Join-Path (Split-Path -Parent $kernelDir) "target_build\kernel"
$kernelElf = Join-Path $targetDir "x86_64-unknown-none\release\fastos-kernel"

Write-Host "=== FastOS UEFI Build ===" -ForegroundColor Cyan
Write-Host "Kernel dir: $kernelDir"
Write-Host "Target dir: $targetDir"

# Clean if requested
if ($Force) {
    Write-Host "Cleaning..." -ForegroundColor Yellow
    cargo clean --target-dir $targetDir 2>&1 | Out-Null
}

# Build
Write-Host "Building kernel (release)..." -ForegroundColor Green
Push-Location $kernelDir
try {
    cargo build --release --target x86_64-unknown-none --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) {
        Write-Host "BUILD FAILED" -ForegroundColor Red
        exit 1
    }
} finally {
    Pop-Location
}

if (Test-Path $kernelElf) {
    $size = (Get-Item $kernelElf).Length
    Write-Host "Build OK: $kernelElf ($size bytes)" -ForegroundColor Green
} else {
    Write-Host "ELF not found at $kernelElf" -ForegroundColor Red
    exit 1
}

if ($BuildOnly) {
    Write-Host "Build-only mode. Done." -ForegroundColor Cyan
    exit 0
}

# Create UEFI bootable disk image
Write-Host "Creating UEFI disk image..." -ForegroundColor Green
# TODO: create UEFI disk image with EDK2 or similar
Write-Host "UEFI disk image creation not yet implemented." -ForegroundColor Yellow
Write-Host "Use QEMU directly: qemu-system-x86_64 -bios OVMF.fd -kernel $kernelElf" -ForegroundColor Cyan

# ============================================================================
# FastOS — UEFI Target Setup
# ============================================================================
# Adds x86_64-unknown-uefi target to Rust toolchain
# ============================================================================

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS UEFI Target Setup" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

Write-Host "[1/2] Checking if x86_64-unknown-uefi target is installed..." -ForegroundColor Cyan

$targetInstalled = rustup target list --installed | Select-String "x86_64-unknown-uefi"

if ($targetInstalled) {
    Write-Host "      Target already installed." -ForegroundColor Green
} else {
    Write-Host "      Installing x86_64-unknown-uefi target..." -ForegroundColor Yellow
    rustup target add x86_64-unknown-uefi
    if ($LASTEXITCODE -ne 0) {
        Write-Host "      Failed to install target." -ForegroundColor Red
        exit 1
    }
    Write-Host "      Target installed successfully." -ForegroundColor Green
}

Write-Host "[2/2] Verifying Rust nightly toolchain..." -ForegroundColor Cyan

$nightlyInstalled = rustup toolchain list | Select-String "nightly"

if ($nightlyInstalled) {
    Write-Host "      Rust nightly is installed." -ForegroundColor Green
} else {
    Write-Host "      Installing Rust nightly..." -ForegroundColor Yellow
    rustup toolchain install nightly
    if ($LASTEXITCODE -ne 0) {
        Write-Host "      Failed to install nightly." -ForegroundColor Red
        exit 1
    }
    Write-Host "      Rust nightly installed successfully." -ForegroundColor Green
}

Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  SETUP COMPLETE" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  You can now build the UEFI bootloader:" -ForegroundColor Green
Write-Host "    cd bootloader" -ForegroundColor Green
Write-Host "    cargo build --release" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Or build the complete system:" -ForegroundColor Green
Write-Host "    powershell -File build_uefi.ps1" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""

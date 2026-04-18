# FastOS Boot — Windows Build Script
# Requires: NASM in PATH

$ErrorActionPreference = "Stop"

Write-Host "[FastOS] Building bootloader..." -ForegroundColor Cyan

Write-Host "  Assembling stage1..." -ForegroundColor Gray
nasm -f bin -I .\ -o stage1.bin stage1.asm
if ($LASTEXITCODE -ne 0) { throw "stage1 failed" }

Write-Host "  Assembling stage2..." -ForegroundColor Gray
nasm -f bin -I .\ -o stage2.bin stage2.asm
if ($LASTEXITCODE -ne 0) { throw "stage2 failed" }

Write-Host "  Creating disk image..." -ForegroundColor Gray
$stage1 = [System.IO.File]::ReadAllBytes("stage1.bin")
$stage2 = [System.IO.File]::ReadAllBytes("stage2.bin")

$img = New-Object byte[] (1MB)
[Array]::Copy($stage1, 0, $img, 0, $stage1.Length)
[Array]::Copy($stage2, 0, $img, 512, $stage2.Length)
[System.IO.File]::WriteAllBytes("fastos.img", $img)

Write-Host "[FastOS] Done: fastos.img ($($img.Length) bytes)" -ForegroundColor Green
Write-Host "  stage1: $($stage1.Length)B | stage2: $($stage2.Length)B" -ForegroundColor Gray

# ============================================================================
# FastOS — Complete Build Pipeline
# ============================================================================
# NASM bootloader → Rust kernel (release) → Raw binary → Disk image
# Target: bare metal, Ryzen 5 5600X + RTX 3060 12G
# Output: fastos.img (ready to flash to USB partition 3)
# ============================================================================

param(
    [switch]$Clean,
    [switch]$Release
)

$ErrorActionPreference = "Stop"
$Root = $PSScriptRoot

Write-Host ""
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host "  FastOS Build - NASM + Rust -> Bare Metal Image" -ForegroundColor Cyan
Write-Host "  Target: Ryzen 5 5600X + RTX 3060 12G | Ring 0" -ForegroundColor Cyan
Write-Host "================================================================" -ForegroundColor Cyan
Write-Host ""

# ── Clean ────────────────────────────────────────────────────────────────────
if ($Clean) {
    Write-Host "[CLEAN] Removing build artifacts..." -ForegroundColor Yellow
    Remove-Item "$Root\boot\stage1.bin" -ErrorAction SilentlyContinue
    Remove-Item "$Root\boot\stage2.bin" -ErrorAction SilentlyContinue
    Remove-Item "$Root\boot\fastos.img" -ErrorAction SilentlyContinue
    Remove-Item "$Root\fastos.img" -ErrorAction SilentlyContinue
    Remove-Item "$Root\kernel.bin" -ErrorAction SilentlyContinue
    Push-Location "$Root\kernel"
    $null = & rustup run nightly cargo clean 2>&1
    Pop-Location
    Write-Host "[CLEAN] Done." -ForegroundColor Green
    if (!$Release -and !$PSBoundParameters.ContainsKey('Release')) { return }
}

$BuildProfile = if ($Release) { "release" } else { "release" } # Always release for bare metal
$CargoFlag = "--release"

# ── Step 1: Assemble NASM bootloader ─────────────────────────────────────────
Write-Host "[1/5] Assembling NASM bootloader..." -ForegroundColor Cyan

Push-Location "$Root\boot"

Write-Host "      stage1.asm → stage1.bin" -ForegroundColor Gray
& nasm -f bin -I .\ -o stage1.bin stage1.asm
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "NASM: stage1.asm failed" }
$s1Size = (Get-Item stage1.bin).Length
if ($s1Size -ne 512) { Pop-Location; throw "stage1.bin must be 512 bytes, got $s1Size" }

Write-Host "      stage2.asm → stage2.bin" -ForegroundColor Gray
& nasm -f bin -I .\ -o stage2.bin stage2.asm
if ($LASTEXITCODE -ne 0) { Pop-Location; throw "NASM: stage2.asm failed" }
$s2Size = (Get-Item stage2.bin).Length
Write-Host "      stage1: ${s1Size}B | stage2: ${s2Size}B" -ForegroundColor DarkGray

Pop-Location
Write-Host "[1/5] Bootloader OK" -ForegroundColor Green

# ── Step 2: Build Rust kernel ────────────────────────────────────────────────
Write-Host "[2/5] Building Rust kernel ($BuildProfile)..." -ForegroundColor Cyan

Push-Location "$Root\kernel"

# Ensure nightly toolchain with correct target
# Temporarily allow stderr output (Rust warnings) without PowerShell treating them as errors
$savedEAP = $ErrorActionPreference
$ErrorActionPreference = "Continue"
$cargoOutput = & rustup run nightly cargo build $CargoFlag 2>&1
$cargoExit = $LASTEXITCODE
$ErrorActionPreference = $savedEAP
$cargoOutput | ForEach-Object {
    $line = $_.ToString()
    if ($line -match "error\[") {
        Write-Host "      $line" -ForegroundColor Red
    } elseif ($line -match "warning:") {
        # Suppress noisy warnings, just count them
    } elseif ($line -match "Compiling|Finished") {
        Write-Host "      $line" -ForegroundColor DarkGray
    }
}
if ($cargoExit -ne 0) {
    # Show all output on failure
    $cargoOutput | ForEach-Object { Write-Host "      $_" -ForegroundColor Red }
    Pop-Location; throw "Cargo build failed"
}

$elfPath = "$Root\kernel\target\x86_64-unknown-none\$BuildProfile\fastos-kernel"
if (!(Test-Path $elfPath)) {
    # Try without extension
    $elfPath = Get-ChildItem "$Root\kernel\target\x86_64-unknown-none\$BuildProfile\fastos-kernel*" -File | Where-Object { $_.Extension -eq "" -or $_.Extension -eq ".exe" } | Select-Object -First 1 -ExpandProperty FullName
    if (!$elfPath) { Pop-Location; throw "Cannot find kernel ELF at target\x86_64-unknown-none\$BuildProfile\" }
}
$elfSize = (Get-Item $elfPath).Length
Write-Host "      ELF: $([math]::Round($elfSize/1024, 1))KB" -ForegroundColor DarkGray

Pop-Location
Write-Host "[2/5] Kernel compiled OK" -ForegroundColor Green

# ── Step 3: Extract raw binary from ELF ──────────────────────────────────────
Write-Host "[3/5] Extracting raw binary (objcopy)..." -ForegroundColor Cyan

$sysroot = (& rustup run nightly rustc --print sysroot).Trim()
$objcopy = "$sysroot\lib\rustlib\x86_64-pc-windows-msvc\bin\rust-objcopy.exe"
if (!(Test-Path $objcopy)) {
    $objcopy = "$sysroot\lib\rustlib\x86_64-pc-windows-msvc\bin\llvm-objcopy.exe"
}
if (!(Test-Path $objcopy)) { throw "rust-objcopy/llvm-objcopy not found in $sysroot" }

$kernelBin = "$Root\kernel.bin"
& $objcopy $elfPath --output-target binary -O binary $kernelBin
if ($LASTEXITCODE -ne 0) { throw "objcopy failed" }

$binSize = (Get-Item $kernelBin).Length
Write-Host "      kernel.bin: $([math]::Round($binSize/1024, 1))KB ($binSize bytes)" -ForegroundColor DarkGray

# Verify kernel isn't too large for the 128KB load window
$maxKernel = 128 * 1024 # 256 sectors × 512 bytes
if ($binSize -gt $maxKernel) {
    Write-Host "      WARNING: kernel ($binSize B) exceeds 128KB load window!" -ForegroundColor Red
    Write-Host "      Update dap_kernel sectors in stage2.asm" -ForegroundColor Red
    # Calculate needed sectors
    $neededSectors = [math]::Ceiling($binSize / 512)
    Write-Host "      Need $neededSectors sectors (currently 256)" -ForegroundColor Yellow
}

Write-Host "[3/5] Binary extracted OK" -ForegroundColor Green

# ── Step 4: Build disk image ─────────────────────────────────────────────────
Write-Host "[4/5] Building disk image..." -ForegroundColor Cyan

# Layout:
#   LBA 0       : stage1.bin (512B MBR)
#   LBA 1..32   : stage2.bin (16KB, 32 sectors)
#   LBA 33..N   : kernel.bin (up to 128KB, 256 sectors)
#   Pad to 1MB total image

$stage1 = [System.IO.File]::ReadAllBytes("$Root\boot\stage1.bin")
$stage2 = [System.IO.File]::ReadAllBytes("$Root\boot\stage2.bin")
$kernel = [System.IO.File]::ReadAllBytes($kernelBin)

# Image size: at least 1MB, or larger if kernel needs it
$kernelEnd = 33 * 512 + $kernel.Length
$imgSize = [math]::Max(1MB, (([math]::Ceiling($kernelEnd / 512) + 1) * 512))
# Round up to next 512 boundary
$imgSize = [math]::Ceiling($imgSize / 512) * 512

$img = New-Object byte[] $imgSize

# MBR (sector 0)
[Array]::Copy($stage1, 0, $img, 0, $stage1.Length)

# Stage2 (sector 1, offset 512)
[Array]::Copy($stage2, 0, $img, 512, $stage2.Length)

# Kernel (sector 33, offset 33*512 = 16896)
$kernelOffset = 33 * 512
[Array]::Copy($kernel, 0, $img, $kernelOffset, $kernel.Length)

$imgPath = "$Root\fastos.img"
[System.IO.File]::WriteAllBytes($imgPath, $img)

Write-Host "      Image layout:" -ForegroundColor DarkGray
Write-Host "        LBA  0      : MBR         (512B)" -ForegroundColor DarkGray
Write-Host "        LBA  1-32   : Stage2      ($($stage2.Length)B)" -ForegroundColor DarkGray
Write-Host "        LBA 33-$([math]::Ceiling($kernel.Length/512)+32)   : Kernel      ($($kernel.Length)B)" -ForegroundColor DarkGray
Write-Host "        Total image : $([math]::Round($imgSize/1024))KB" -ForegroundColor DarkGray

Write-Host "[4/5] fastos.img ready" -ForegroundColor Green

# ── Step 5: Verify image ─────────────────────────────────────────────────────
Write-Host "[5/5] Verifying image..." -ForegroundColor Cyan

# Check MBR signature
$sig = [BitConverter]::ToUInt16($img, 510)
if ($sig -ne 0xAA55) { throw "MBR signature invalid: 0x$($sig.ToString('X4'))" }
Write-Host "      MBR signature: 0xAA55 OK" -ForegroundColor DarkGray

# Check stage2 starts at expected offset
$s2First = $img[512]
Write-Host "      Stage2 first byte: 0x$($s2First.ToString('X2'))" -ForegroundColor DarkGray

# Check kernel is present
if ($img[$kernelOffset] -eq 0 -and $img[$kernelOffset+1] -eq 0 -and $img[$kernelOffset+2] -eq 0 -and $img[$kernelOffset+3] -eq 0) {
    Write-Host "      WARNING: Kernel region appears empty!" -ForegroundColor Red
} else {
    Write-Host "      Kernel present at LBA 33" -ForegroundColor DarkGray
}

Write-Host "[5/5] Verification passed" -ForegroundColor Green

# ── Step 6: Export USB_boot folder ────────────────────────────────────────────
Write-Host "[6/6] Exporting to USB_boot folder..." -ForegroundColor Cyan

$usbDir = "$Root\USB_boot"
if (!(Test-Path $usbDir)) {
    New-Item -Path $usbDir -ItemType Directory -Force | Out-Null
}

Copy-Item $imgPath "$usbDir\fastos.img" -Force

# Copy flash script for convenience
if (Test-Path "$Root\flash_usb.ps1") {
    Copy-Item "$Root\flash_usb.ps1" "$usbDir\flash_usb.ps1" -Force
}

# Create README with instructions
$buildDate = Get-Date -Format 'yyyy-MM-dd HH:mm:ss'
$readme = @"
======================================================
  FastOS - USB Boot Image
======================================================

  Image:   fastos.img ($([math]::Round($imgSize/1024))KB)
  Built:   $buildDate
  Target:  Ryzen 5 5600X + RTX 3060 12G

------------------------------------------------------
  HOW TO FLASH TO USB
------------------------------------------------------

  Option 1 - PowerShell (included script):
    .\flash_usb.ps1 -DiskNumber <N> -Partition 3

  Option 2 - dd (Linux/WSL):
    sudo dd if=fastos.img of=/dev/sdX bs=512

  Option 3 - Rufus:
    Select fastos.img, write in "DD Image" mode

  IMPORTANT: Use a USB 2.0 port if possible.
  Some UEFI CSM implementations have issues with
  legacy boot from USB 3.0 ports.

------------------------------------------------------
  MEMORY MAP (bare metal)
------------------------------------------------------
  0x007C00          MBR (Stage1)
  0x007E00          Stage2
  0x010000          Kernel load buffer (128KB)
  0x100000 (1MB)    Kernel final location
  0x400000 (4MB)    DMA buffer pool
  0x800000 (8MB)    Stack (grows down)

------------------------------------------------------
  TROUBLESHOOTING
------------------------------------------------------
  Q: Only see "Stage1: MBR loaded" repeated?
  A: Stage2 may not be loading. Check:
     - USB is formatted correctly (raw image, not partition)
     - Try a different USB port (prefer USB 2.0)
     - Check BIOS: enable CSM/Legacy Boot

  Q: See "S2" at bottom-left of screen?
  A: Stage2 code IS reached but crashes before printing.
     This is a CPU/memory init issue - report the exact
     screen contents.

  Q: See "NO LBA EXTENSIONS"?
  A: Your BIOS doesn't support INT 13h extended reads.
     This is very rare on modern hardware.

  Q: See "STAGE2 DATA INVALID"?
  A: INT 13h claimed success but loaded wrong data.
     Try re-flashing the USB drive.

======================================================
"@

Set-Content "$usbDir\README.txt" -Value $readme -Encoding UTF8

Write-Host "[6/6] USB_boot folder ready!" -ForegroundColor Green
Write-Host "      Path: $usbDir" -ForegroundColor DarkGray

# ── Summary ──────────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "================================================================" -ForegroundColor Green
Write-Host "  BUILD COMPLETE" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Output: fastos.img ($([math]::Round($imgSize/1024))KB)" -ForegroundColor Green
Write-Host "  USB:    USB_boot\fastos.img (ready to flash)" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Flash to USB:" -ForegroundColor Green
Write-Host "    cd USB_boot" -ForegroundColor Green
Write-Host "    .\flash_usb.ps1 -DiskNumber <N> -Partition 3" -ForegroundColor Green
Write-Host "" -ForegroundColor Green
Write-Host "  Memory map (bare metal):" -ForegroundColor Green
Write-Host "    0x007C00          MBR (stage1)" -ForegroundColor Green
Write-Host "    0x007E00          Stage2" -ForegroundColor Green
Write-Host "    0x010000          Kernel load buffer" -ForegroundColor Green
Write-Host "    0x100000 (1MB)    Kernel final location" -ForegroundColor Green
Write-Host "    0x400000 (4MB)    DMA buffer pool" -ForegroundColor Green
Write-Host "    0x800000 (8MB)    Stack (grows down)" -ForegroundColor Green
Write-Host "================================================================" -ForegroundColor Green
Write-Host ""


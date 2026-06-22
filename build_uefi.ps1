#!/usr/bin/env pwsh
<#
.SYNOPSIS
    FastOS UEFI Build System — Build, Flash, Verify (Real Hardware)
.DESCRIPTION
    Professional build pipeline for FastOS on real hardware:
      1. Validates environment (toolchains, disk space, admin)
      2. Builds bootloader (nightly) + kernel (stable) with timing
      3. Stages EFI/BOOT structure with SHA256 hashes
      4. Flashes directly to USB with automatic detection
      5. Verifies flash by reading back and comparing hashes
.PARAMETER Flash
    Flash to USB after building. Auto-detects drives or use -Drive.
.PARAMETER Drive
    USB drive letter to flash (e.g. "E"). Used with -Flash.
.PARAMETER Verify
    Verify a previously flashed USB by comparing hashes.
.PARAMETER Clean
    Force clean rebuild of all artifacts.
.PARAMETER BuildOnly
    Build + stage only. No flash.
.PARAMETER Silent
    Minimal output. Only show errors and final summary.
.EXAMPLE
    .\build_uefi.ps1                        # Build + stage
    .\build_uefi.ps1 -Flash                 # Build + flash (auto-detect USB)
    .\build_uefi.ps1 -Flash -Drive E        # Build + flash to E:
    .\build_uefi.ps1 -Verify -Drive E       # Verify USB contents
    .\build_uefi.ps1 -Clean                 # Clean + rebuild
#>
param(
    [switch]$Flash,
    [switch]$Verify,
    [switch]$Clean,
    [switch]$BuildOnly,
    [switch]$Silent,
    [string]$Drive
)

$ErrorActionPreference = "Stop"
$scriptVersion = "2.0.0"

# ── Colors ─────────────────────────────────────────────────────────────
function Write-Step    { param($msg) Write-Host "  [>>] " -NoNewline -ForegroundColor Cyan;    Write-Host $msg }
function Write-OK      { param($msg) Write-Host "  [OK] " -NoNewline -ForegroundColor Green;   Write-Host $msg }
function Write-Warn    { param($msg) Write-Host "  [!!] " -NoNewline -ForegroundColor Yellow;  Write-Host $msg }
function Write-Fail    { param($msg) Write-Host "  [XX] " -NoNewline -ForegroundColor Red;     Write-Host $msg }
function Write-Info    { param($msg) if (-not $Silent) { Write-Host "  [..] " -NoNewline -ForegroundColor DarkGray; Write-Host $msg } }

# ── Timing helpers ─────────────────────────────────────────────────────
$script:totalTimer = [System.Diagnostics.Stopwatch]::StartNew()
function Start-PhaseTimer { param($name) $t = [System.Diagnostics.Stopwatch]::StartNew(); Write-Step "$name..."; return @{ Name=$name; Timer=$t } }
function Stop-PhaseTimer  { param($phase) $phase.Timer.Stop(); Write-OK ("{0} completed in {1:N1}s" -f $phase.Name, $phase.Timer.Elapsed.TotalSeconds) }

# ── Banner ─────────────────────────────────────────────────────────────
function Show-Banner {
    Write-Host ""
    Write-Host "  +========================================================+" -ForegroundColor DarkCyan
    Write-Host "  |         FastOS UEFI Build System  v$scriptVersion              |" -ForegroundColor Cyan
    Write-Host "  |         Ryzen 5 5600X  ·  GOP Framebuffer              |" -ForegroundColor DarkCyan
    Write-Host "  +========================================================+" -ForegroundColor DarkCyan
    Write-Host ""
}

# ── Admin check ────────────────────────────────────────────────────────
function Test-Admin {
    $identity  = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]$identity
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

# ── Get version from Cargo.toml ────────────────────────────────────────
function Get-KernelVersion {
    param($tomlPath)
    $content = Get-Content $tomlPath -Raw
    if ($content -match 'version\s*=\s*"([^"]+)"') {
        return $Matches[1]
    }
    return "unknown"
}

# ── Get available USB drives ───────────────────────────────────────────
function Get-USBDrives {
    $drives = Get-CimInstance Win32_DiskDrive | Where-Object {
        $_.InterfaceType -eq "USB" -or $_.MediaType -eq "External hard disk media"
    }
    $result = @()
    foreach ($disk in $drives) {
        $partitions = Get-CimInstance Win32_DiskPartition | Where-Object { $_.DiskIndex -eq $disk.Index }
        foreach ($part in $partitions) {
            $logical = Get-CimInstance Win32_LogicalDisk | Where-Object { $_.DiskIndex -eq $part.Index -and $_.DriveType -eq 2 }
            if ($logical) {
                $result += [PSCustomObject]@{
                    Letter     = $logical.DeviceID
                    Label      = $logical.VolumeName
                    SizeGB     = [math]::Round($logical.Size / 1GB, 1)
                    FreeGB     = [math]::Round($logical.FreeSpace / 1GB, 1)
                    FileSystem = $logical.FileSystem
                    DiskIndex  = $disk.Index
                }
            }
        }
    }
    return $result
}

# ── SHA256 helper ──────────────────────────────────────────────────────
function Get-FileHash256 { param($path) return (Get-FileHash -Path $path -Algorithm SHA256).Hash.ToLower() }

# ══════════════════════════════════════════════════════════════════════
# MAIN
# ══════════════════════════════════════════════════════════════════════

Show-Banner

# ── Paths (use $PSScriptRoot — always resolves to script location) ────
$rootDir       = $PSScriptRoot
if (-not $rootDir) { $rootDir = Split-Path -Parent $MyInvocation.MyCommand.Path }
$bootloaderDir = Join-Path $rootDir "bootloader"
$kernelDir     = Join-Path $rootDir "kernel"
$targetDir     = Join-Path $rootDir "target_build"
$stagingDir    = Join-Path $targetDir "staging"
$efiBootDir    = Join-Path $stagingDir "EFI\BOOT"

# Verify project structure
if (-not (Test-Path (Join-Path $bootloaderDir "Cargo.toml"))) {
    Write-Fail "Bootloader not found at: $bootloaderDir"
    Write-Info "Make sure you run this script from the FastOS project root."
    exit 1
}
if (-not (Test-Path (Join-Path $kernelDir "Cargo.toml"))) {
    Write-Fail "Kernel not found at: $kernelDir"
    Write-Info "Make sure you run this script from the FastOS project root."
    exit 1
}

$kernelVersion = Get-KernelVersion (Join-Path $kernelDir "Cargo.toml")

Write-Host "  Kernel version:  $kernelVersion" -ForegroundColor White
Write-Host "  Target:          x86_64-unknown-none (kernel) / x86_64-unknown-uefi (bootloader)" -ForegroundColor White
Write-Host "  Profile:         release (opt-level=3, LTO, strip=symbols)" -ForegroundColor White
Write-Host "  Build dir:       $targetDir" -ForegroundColor White
Write-Host ""

# ── Phase 0: Environment validation ────────────────────────────────────
$envPhase = Start-PhaseTimer "Environment validation"

# Check Rust toolchains
$hasNightly = rustup toolchain list 2>$null | Select-String "nightly" | Measure-Object | Select-Object -ExpandProperty Count
$hasStable  = rustup toolchain list 2>$null | Select-String "stable"  | Measure-Object | Select-Object -ExpandProperty Count

if ($hasNightly -eq 0) {
    Write-Fail "Nightly toolchain not found. Install: rustup toolchain install nightly"
    exit 1
}
Write-Info "Nightly toolchain: installed"

if ($hasStable -eq 0) {
    Write-Fail "Stable toolchain not found. Install: rustup toolchain install stable"
    exit 1
}
Write-Info "Stable toolchain: installed"

# Check UEFI target
$uefiTarget = rustup target list --installed --toolchain nightly 2>$null | Select-String "x86_64-unknown-uefi"
if (-not $uefiTarget) {
    Write-Warn "UEFI target not installed for nightly. Installing..."
    rustup target add x86_64-unknown-uefi --toolchain nightly
}

# Check disk space
$systemDrive = $env:SystemDrive
$freeSpace = (Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$systemDrive'").FreeSpace
if ($freeSpace -lt 50MB) {
    Write-Fail "Low disk space on $systemDrive ($([math]::Round($freeSpace/1MB,0))MB free). Need at least 50MB."
    exit 1
}
Write-Info "Disk space: $([math]::Round($freeSpace/1GB,1))GB free on $systemDrive"

# Check admin
$isAdmin = Test-Admin
if ($isAdmin) { Write-Info "Running as Administrator" }
else          { Write-Info "Running as User" }

Stop-PhaseTimer $envPhase

# ── Phase 1: Clean (optional) ──────────────────────────────────────────
if ($Clean) {
    $cleanPhase = Start-PhaseTimer "Clean build artifacts"
    if (Test-Path $targetDir) {
        Remove-Item $targetDir -Recurse -Force
        Write-Info "Removed $targetDir"
    }
    Stop-PhaseTimer $cleanPhase
}

# ── Phase 2: Build Bootloader ──────────────────────────────────────────
$bootPhase = Start-PhaseTimer "Build bootloader (nightly)"
Push-Location $bootloaderDir
try {
    $bootOut = & { cargo build --release } 2>$null
    if ($LASTEXITCODE -ne 0) {
        Write-Fail "Bootloader build FAILED"
        exit 1
    }
    $bootOut | ForEach-Object {
        if (-not $Silent -and $_ -match "Compiling|Finished") { Write-Info $_ }
    }
} finally {
    Pop-Location
}

$bootloaderEfi = Join-Path $bootloaderDir "target\x86_64-unknown-uefi\release\fastos-bootloader.efi"
if (-not (Test-Path $bootloaderEfi)) {
    Write-Fail "Bootloader EFI binary not found at: $bootloaderEfi"
    exit 1
}
$bootloaderSize = (Get-Item $bootloaderEfi).Length
$bootloaderHash = Get-FileHash256 $bootloaderEfi
Write-OK ("BOOTX64.EFI: {0:N0} bytes ({1:N1} KB)  sha256:{2}" -f $bootloaderSize, ($bootloaderSize/1024), $bootloaderHash.Substring(0,16))
Stop-PhaseTimer $bootPhase

# ── Phase 3: Build Kernel ──────────────────────────────────────────────
$kernelPhase = Start-PhaseTimer "Build kernel (stable)"
$kernelTargetDir = Join-Path $targetDir "kernel"
Push-Location $kernelDir
try {
    $kernOut = cargo build --release --target x86_64-unknown-none --target-dir $kernelTargetDir 2>&1
    $kernErr = $kernOut | Where-Object { $_ -is [System.Management.Automation.ErrorRecord] }
    $kernOut | Where-Object { $_ -isnot [System.Management.Automation.ErrorRecord] } | ForEach-Object {
        if (-not $Silent -and $_ -match "Compiling|Finished") { Write-Info $_ }
    }
    foreach ($e in $kernErr) {
        $msg = "$e"
        if ($msg -match "error") { Write-Fail $msg }
        elseif (-not $Silent) { Write-Info "  warning: $($msg -replace '.*warning:\s*','')" }
    }
    if ($LASTEXITCODE -ne 0) {
        Write-Fail "Kernel build FAILED"
        exit 1
    }
} finally {
    Pop-Location
}

$kernelElf = Join-Path $kernelTargetDir "x86_64-unknown-none\release\fastos-kernel"
if (-not (Test-Path $kernelElf)) {
    Write-Fail "Kernel ELF not found at: $kernelElf"
    exit 1
}
$kernelSize = (Get-Item $kernelElf).Length
$kernelHash = Get-FileHash256 $kernelElf
Write-OK ("kernel.elf: {0:N0} bytes ({1:N1} KB)  sha256:{2}" -f $kernelSize, ($kernelSize/1024), $kernelHash.Substring(0,16))
Stop-PhaseTimer $kernelPhase

# ── Phase 4: Stage EFI boot files ──────────────────────────────────────
$stagePhase = Start-PhaseTimer "Stage EFI boot structure"
if (Test-Path $stagingDir) { Remove-Item $stagingDir -Recurse -Force }
New-Item -ItemType Directory -Path $efiBootDir -Force | Out-Null

Copy-Item $bootloaderEfi -Destination (Join-Path $efiBootDir "BOOTX64.EFI")
Copy-Item $kernelElf     -Destination (Join-Path $efiBootDir "kernel.elf")

# Write manifest with hashes
$manifest = @"
# FastOS EFI Boot Manifest
# Generated: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
# Version: $kernelVersion
#
# File           Size        SHA256
# ----           ----        ------
BOOTX64.EFI     $bootloaderSize   $bootloaderHash
kernel.elf      $kernelSize       $kernelHash
"@
$manifest | Set-Content -Path (Join-Path $efiBootDir "MANIFEST.TXT") -Encoding UTF8

Write-OK "EFI/BOOT/ staged:"
Write-Info "  BOOTX64.EFI  ($bootloaderSize bytes)"
Write-Info "  kernel.elf   ($kernelSize bytes)"
Write-Info "  MANIFEST.TXT (hash manifest)"
Stop-PhaseTimer $stagePhase

$totalBuildTime = $script:totalTimer.Elapsed.TotalSeconds

# ── Build summary ──────────────────────────────────────────────────────
Write-Host ""
Write-Host "  +========================================================+" -ForegroundColor Green
Write-Host "  |                 BUILD COMPLETE                         |" -ForegroundColor Green
Write-Host "  +========================================================+" -ForegroundColor Green
Write-Host ""
Write-Host "  Total build time:  $([math]::Round($totalBuildTime, 1))s" -ForegroundColor White
Write-Host "  Staged files:      $stagingDir" -ForegroundColor White
Write-Host "  Total image size:  $([math]::Round(($bootloaderSize + $kernelSize) / 1024, 1)) KB" -ForegroundColor White
Write-Host ""

if ($BuildOnly) {
    Write-Host "  Build-only mode. Files at: $stagingDir" -ForegroundColor DarkGray
    Write-Host ""
    exit 0
}

# ── Phase 5: Flash to USB ──────────────────────────────────────────────
if ($Flash) {
    Write-Host "  -- USB Flash ------------------------------------------------" -ForegroundColor Cyan
    Write-Host ""

    # Detect USB drives
    $usbDrives = Get-USBDrives

    if ($usbDrives.Count -eq 0) {
        Write-Warn "No USB drives detected. Insert a USB drive and re-run with -Flash."
        Write-Host ""
    } else {
        Write-Host "  Available USB drives:" -ForegroundColor White
        Write-Host ""
        for ($i = 0; $i -lt $usbDrives.Count; $i++) {
            $d = $usbDrives[$i]
            $label = if ($d.Label) { $d.Label } else { "(no label)" }
            Write-Host ("    [{0}] {1} {2} - {3} GB free of {4} GB ({5})" -f ($i+1), $d.Letter, $label, $d.FreeGB, $d.SizeGB, $d.FileSystem) -ForegroundColor White
        }
        Write-Host ""

        # Select drive
        $targetLetter = $Drive
        if (-not $targetLetter) {
            $selection = Read-Host "  Select drive number (1-$($usbDrives.Count)) or press Enter to skip"
            if ($selection -and $selection -match '^\d+$' -and [int]$selection -ge 1 -and [int]$selection -le $usbDrives.Count) {
                $targetLetter = $usbDrives[[int]$selection - 1].Letter
            }
        }

        if ($targetLetter) {
            $targetLetter = $targetLetter.TrimEnd(':').TrimEnd('\').ToUpper()
            $targetRoot = "${targetLetter}:\"

            if (-not (Test-Path $targetRoot)) {
                Write-Fail "Drive $targetLetter does not exist"
                exit 1
            }

            # Safety check
            $targetInfo = $usbDrives | Where-Object { $_.Letter -eq $targetLetter }
            if (-not $targetInfo) {
                Write-Warn "Drive $targetLetter is not detected as a USB drive."
                $confirm = Read-Host "  Flash anyway? (type YES to confirm)"
                if ($confirm -ne "YES") { Write-Info "Aborted."; exit 0 }
            } else {
                Write-Host ""
                Write-Host "  Target: $($targetLetter):\ ($($targetInfo.Label)) - $($targetInfo.FreeGB)GB free" -ForegroundColor Yellow
                $confirm = Read-Host "  Flash FastOS to this drive? (type YES to confirm)"
                if ($confirm -ne "YES") { Write-Info "Aborted."; exit 0 }
            }

            # Flash
            $flashPhase = Start-PhaseTimer "Flash to $targetLetter`:"
            $efiDest = Join-Path $targetRoot "EFI\BOOT"
            New-Item -ItemType Directory -Path $efiDest -Force | Out-Null

            Copy-Item (Join-Path $efiBootDir "BOOTX64.EFI") -Destination (Join-Path $efiDest "BOOTX64.EFI") -Force
            Copy-Item (Join-Path $efiBootDir "kernel.elf")   -Destination (Join-Path $efiDest "kernel.elf")   -Force
            Copy-Item (Join-Path $efiBootDir "MANIFEST.TXT") -Destination (Join-Path $efiDest "MANIFEST.TXT") -Force

            # Flush to physical device
            Write-Info "Flushing to physical device..."
            & cmd /c "echo. > \\.\$targetLetter`:" 2>$null
            Stop-PhaseTimer $flashPhase

            # Verify
            $verifyPhase = Start-PhaseTimer "Verify flash"
            $verifyOk = $true

            $checkFiles = @(
                @{ Name="BOOTX64.EFI"; Hash=$bootloaderHash; Size=$bootloaderSize },
                @{ Name="kernel.elf";  Hash=$kernelHash;     Size=$kernelSize }
            )

            foreach ($f in $checkFiles) {
                $destPath = Join-Path $efiDest $f.Name
                if (-not (Test-Path $destPath)) {
                    Write-Fail "MISSING: $($f.Name)"
                    $verifyOk = $false
                    continue
                }
                $destSize = (Get-Item $destPath).Length
                $destHash = Get-FileHash256 $destPath

                if ($destHash -ne $f.Hash) {
                    Write-Fail "HASH MISMATCH: $($f.Name) (expected $($f.Hash.Substring(0,16))... got $($destHash.Substring(0,16))...)"
                    $verifyOk = $false
                } elseif ($destSize -ne $f.Size) {
                    Write-Fail "SIZE MISMATCH: $($f.Name) (expected $($f.Size) got $destSize)"
                    $verifyOk = $false
                } else {
                    Write-OK ("$($f.Name): verified ({0:N0} bytes, sha256 match)" -f $destSize)
                }
            }

            Stop-PhaseTimer $verifyPhase

            if ($verifyOk) {
                Write-Host ""
                Write-Host "  +========================================================+" -ForegroundColor Green
                Write-Host "  |         FLASH SUCCESSFUL -- BOOT READY                 |" -ForegroundColor Green
                Write-Host "  +========================================================+" -ForegroundColor Green
                Write-Host ""
                Write-Host "  Drive $targetLetter`:\ is ready to boot." -ForegroundColor White
                Write-Host "  Reboot and select USB from BIOS/UEFI boot menu." -ForegroundColor White
            } else {
                Write-Host ""
                Write-Fail "FLASH VERIFICATION FAILED -- re-flash or check USB drive"
            }
            Write-Host ""
        }
    }
}

# ── Phase 6: Verify existing USB (standalone) ──────────────────────────
if ($Verify -and -not $Flash) {
    $targetLetter = $Drive
    if (-not $targetLetter) {
        $usbDrives = Get-USBDrives
        if ($usbDrives.Count -eq 0) {
            Write-Fail "No USB drives detected"
            exit 1
        }
        Write-Host "  USB drives:" -ForegroundColor White
        for ($i = 0; $i -lt $usbDrives.Count; $i++) {
            $d = $usbDrives[$i]
            Write-Host ("    [{0}] {1} {2}" -f ($i+1), $d.Letter, $d.Label)
        }
        $selection = Read-Host "  Select drive number"
        $targetLetter = $usbDrives[[int]$selection - 1].Letter
    }

    $targetLetter = $targetLetter.TrimEnd(':').TrimEnd('\').ToUpper()
    $efiDest = "${targetLetter}:\EFI\BOOT"

    Write-Host ""
    Write-Host "  Verifying $targetLetter`:\EFI\BOOT\..." -ForegroundColor Cyan

    if (-not (Test-Path $efiDest)) {
        Write-Fail "EFI\BOOT not found on $targetLetter`:"
        exit 1
    }

    $ok = $true
    foreach ($name in @("BOOTX64.EFI", "kernel.elf")) {
        $path = Join-Path $efiDest $name
        if (Test-Path $path) {
            $sz = (Get-Item $path).Length
            $hash = Get-FileHash256 $path
            Write-OK ("${name}: {0:N0} bytes  sha256:{1}" -f $sz, $hash.Substring(0,16))
        } else {
            Write-Fail "MISSING: $name"
            $ok = $false
        }
    }

    if ($ok) { Write-Host "  USB verification PASSED" -ForegroundColor Green }
    else     { Write-Host "  USB verification FAILED" -ForegroundColor Red }
    Write-Host ""
}

# ── Final summary ──────────────────────────────────────────────────────
$totalTime = $script:totalTimer.Elapsed.TotalSeconds
Write-Host "  Total elapsed: $([math]::Round($totalTime, 1))s" -ForegroundColor DarkGray
Write-Host ""

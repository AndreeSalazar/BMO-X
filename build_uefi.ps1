#!/usr/bin/env pwsh
<#
.SYNOPSIS
    FastOS Build System v3.1 — Maximum Performance Build Pipeline
.DESCRIPTION
    Parallel-optimized build pipeline for FastOS:
      1. Parallel builds: bootloader (nightly) + kernel (stable) simultaneously
      2. Smart rebuild: skip if no source files changed
      3. SHA256 verification on flash
      4. Auto-flash to SSD (S:) or use -Drive
.PARAMETER Flash
    Flash to SSD after building. Defaults to S: (FastOS SSD).
.PARAMETER Drive
    Drive letter to flash (e.g. "S", "E").
.PARAMETER Verify
    Verify a previously flashed drive.
.PARAMETER Clean
    Force clean rebuild.
.PARAMETER BuildOnly
    Build + stage only. No flash.
.PARAMETER Jobs
    Max cargo parallel jobs (default: all CPU cores).
.EXAMPLE
    .\build_uefi.ps1                     # Smart build (skip unchanged)
    .\build_uefi.ps1 -Flash              # Build + flash to SSD (S:)
    .\build_uefi.ps1 -Flash -Drive E     # Build + flash to E:
    .\build_uefi.ps1 -Clean              # Clean + rebuild
    .\build_uefi.ps1 -Jobs 4             # Limit to 4 parallel cargo jobs
.NOTES
    v3.1: SSD-first approach, simplified flash, parallel builds.
#>
param(
    [switch]$Flash,
    [switch]$Verify,
    [switch]$Clean,
    [switch]$BuildOnly,
    [switch]$Yes,
    [string]$Drive,
    [int]$Jobs = 0
)

$ErrorActionPreference = "Stop"
$scriptVersion = "3.1.0"

# ── Colors ─────────────────────────────────────────────────────────────
function Step  { param($m) Write-Host "  >> " -NoNewline -ForegroundColor Cyan;    Write-Host $m }
function OK    { param($m) Write-Host "  OK " -NoNewline -ForegroundColor Green;   Write-Host $m }
function Warn  { param($m) Write-Host "  !! " -NoNewline -ForegroundColor Yellow;  Write-Host $m }
function Fail  { param($m) Write-Host "  XX " -NoNewline -ForegroundColor Red;     Write-Host $m; exit 1 }

# ── Timing ─────────────────────────────────────────────────────────────
$script:timer = [System.Diagnostics.Stopwatch]::StartNew()
function PhaseStart { param($n) $t = [System.Diagnostics.Stopwatch]::StartNew(); Write-Host "  >> $n" -ForegroundColor Cyan; return $t }
function PhaseDone  { param($t, $n) $t.Stop(); Write-Host "  OK $n ($([math]::Round($t.Elapsed.TotalSeconds,1))s)" -ForegroundColor Green }

# ── Banner ─────────────────────────────────────────────────────────────
Write-Host ""
Write-Host "  +============================================+" -ForegroundColor DarkCyan
Write-Host "  |   FastOS Build System  v$scriptVersion                |" -ForegroundColor Cyan
Write-Host "  |   Ryzen 5600X · GOP · SSD Boot                   |" -ForegroundColor DarkCyan
Write-Host "  +============================================+" -ForegroundColor DarkCyan
Write-Host ""

# ── Admin elevation ────────────────────────────────────────────────────
function Test-Admin {
    $id = [Security.Principal.WindowsIdentity]::GetCurrent()
    return ([Security.Principal.WindowsPrincipal]$id).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if ($Flash -or $Verify) {
    if (-not (Test-Admin)) {
        $args = @()
        if ($Flash)     { $args += "-Flash" }
        if ($Verify)    { $args += "-Verify" }
        if ($Clean)     { $args += "-Clean" }
        if ($BuildOnly) { $args += "-BuildOnly" }
        if ($Yes)       { $args += "-Yes" }
        if ($Drive)     { $args += "-Drive", "`"$Drive`"" }
        $argStr = $args -join " "
        Write-Host "  !! Requesting Administrator elevation..." -ForegroundColor Yellow
        try {
            $proc = Start-Process "powershell.exe" -ArgumentList "-NoProfile -ExecutionPolicy Bypass -File `"$PSCommandPath`" $argStr" -Verb RunAs -PassThru
            if ($proc) { $proc.WaitForExit(); exit $proc.ExitCode }
        } catch { Fail "UAC elevation failed: $_" }
        exit 1
    }
}

# ── Paths ──────────────────────────────────────────────────────────────
$root     = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }
$bootDir  = Join-Path $root "crates_Personal\bootloader"
$kernDir  = Join-Path $root "kernel"
$target   = Join-Path $root "target_build"
$stage    = Join-Path $target "staging\EFI\BOOT"

if (-not (Test-Path (Join-Path $bootDir "Cargo.toml"))) { Fail "Bootloader not found at $bootDir" }
if (-not (Test-Path (Join-Path $kernDir "Cargo.toml"))) { Fail "Kernel not found at $kernDir" }

# ── Version ────────────────────────────────────────────────────────────
$kv = "unknown"
$toml = Get-Content (Join-Path $kernDir "Cargo.toml") -Raw
if ($toml -match 'version\s*=\s*"(.+?)"') { $kv = $Matches[1] }

# ── Cargo jobs flag ────────────────────────────────────────────────────
$jobsFlag = if ($Jobs -gt 0) { "-j$Jobs" } else { "" }

# ── SHA256 ─────────────────────────────────────────────────────────────
function Hash256 { param($p) return (Get-FileHash -Path $p -Algorithm SHA256).Hash.ToLower() }

# ══════════════════════════════════════════════════════════════════════
# ENVIRONMENT CHECK
# ══════════════════════════════════════════════════════════════════════
$t0 = PhaseStart "Environment check"

$hasNightly = (rustup toolchain list 2>$null | Select-String "nightly" | Measure-Object).Count -gt 0
$hasStable  = (rustup toolchain list 2>$null | Select-String "stable"  | Measure-Object).Count -gt 0
if (-not $hasNightly) { Fail "Need nightly toolchain: rustup toolchain install nightly" }
if (-not $hasStable)  { Fail "Need stable toolchain: rustup toolchain install stable" }

$uefiTarget = rustup target list --installed --toolchain nightly 2>$null | Select-String "x86_64-unknown-uefi"
if (-not $uefiTarget) {
    Warn "Installing UEFI target for nightly..."
    rustup target add x86_64-unknown-uefi --toolchain nightly 2>$null | Out-Null
}

$freeGB = [math]::Round((Get-CimInstance Win32_LogicalDisk -Filter "DeviceID='$env:SystemDrive'").FreeSpace / 1GB, 1)
if ($freeGB -lt 0.05) { Fail "Low disk space: ${freeGB}GB free" }

PhaseDone $t0 "Rust nightly+stable OK, UEFI target OK, ${freeGB}GB free"

# ══════════════════════════════════════════════════════════════════════
# CLEAN
# ══════════════════════════════════════════════════════════════════════
if ($Clean) {
    $tc = PhaseStart "Cleaning"
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    PhaseDone $tc "Clean"
}

# ══════════════════════════════════════════════════════════════════════
# SMART REBUILD DETECTION
# ══════════════════════════════════════════════════════════════════════
function Get-NewestSourceTime {
    param($dir, $exts = @("rs","toml","ld"))
    $newest = [DateTime]::MinValue
    foreach ($ext in $exts) {
        Get-ChildItem -Path $dir -Recurse -Filter "*.$ext" -ErrorAction SilentlyContinue | ForEach-Object {
            if ($_.LastWriteTime -gt $newest) { $newest = $_.LastWriteTime }
        }
    }
    return $newest
}

function Needs-Rebuild {
    param($sourceDir, $outputFile)
    if (-not (Test-Path $outputFile)) { return $true }
    $srcTime = Get-NewestSourceTime $sourceDir
    $outTime = (Get-Item $outputFile).LastWriteTime
    return $srcTime -gt $outTime
}

$bootEfi = Join-Path $target "bootloader\x86_64-unknown-uefi\release\fastos-bootloader.efi"
$kernelElf = Join-Path $target "kernel\x86_64-unknown-none\release\fastos-kernel"

$needBoot = Needs-Rebuild $bootDir $bootEfi
$needKern = Needs-Rebuild $kernDir $kernelElf

if (-not $needBoot -and -not $needKern -and -not $Clean) {
    Write-Host "  OK All up to date. Nothing to rebuild." -ForegroundColor Green
    if (-not $Flash -and -not $Verify) {
        Write-Host "  Use -Clean to force rebuild, or -Flash to flash existing build." -ForegroundColor DarkGray
        Write-Host ""
        exit 0
    }
}

# ══════════════════════════════════════════════════════════════════════
# PARALLEL BUILD: BOOTLOADER + KERNEL
# ══════════════════════════════════════════════════════════════════════
$buildTimer = PhaseStart "Building bootloader + kernel (parallel)"

# Define build jobs
$bootJob = $null
$kernJob = $null

if ($needBoot) {
    $bootTargetDir = Join-Path $target "bootloader"
    $bootScript = {
        param($bootDir, $bootTargetDir, $jobsFlag)
        Push-Location $bootDir
        try {
            $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $bootTargetDir $jobsFlag 2>&1
            $err = $out | Where-Object { $_ -match "^error" }
            if ($err) { throw "Bootloader build error: $err" }
            return @{ Ok=$true; Output=$out }
        } catch {
            return @{ Ok=$false; Error=$_.Exception.Message }
        } finally {
            Pop-Location
        }
    }
    $bootJob = Start-Job -ScriptBlock $bootScript -ArgumentList $bootDir, $bootTargetDir, $jobsFlag
    Step "Bootloader build started (PID $($bootJob.Id))"
}

if ($needKern) {
    $kernTargetDir = Join-Path $target "kernel"
    $kernScript = {
        param($kernDir, $kernTargetDir, $jobsFlag)
        Push-Location $kernDir
        try {
            $out = cargo build --release --target x86_64-unknown-none --target-dir $kernTargetDir $jobsFlag 2>&1
            $err = $out | Where-Object { $_ -match "^error" }
            if ($err) { throw "Kernel build error: $err" }
            return @{ Ok=$true; Output=$out }
        } catch {
            return @{ Ok=$false; Error=$_.Exception.Message }
        } finally {
            Pop-Location
        }
    }
    $kernJob = Start-Job -ScriptBlock $kernScript -ArgumentList $kernDir, $kernTargetDir, $jobsFlag
    Step "Kernel build started (PID $($kernJob.Id))"
}

# Wait for both jobs
$bootResult = $null
$kernResult = $null

if ($bootJob) {
    Step "Waiting for bootloader..."
    $bootResult = Receive-Job -Job $bootJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $bootJob -Force
    if (-not $bootResult.Ok) { Fail "BOOTLOADER FAILED: $($bootResult.Error)" }
    $bootOutput = $bootResult.Output
    foreach ($line in $bootOutput) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [boot] $l" -ForegroundColor DarkGray }
    }
}

if ($kernJob) {
    Step "Waiting for kernel..."
    $kernResult = Receive-Job -Job $kernJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $kernJob -Force
    if (-not $kernResult.Ok) { Fail "KERNEL FAILED: $($kernResult.Error)" }
    $kernOutput = $kernResult.Output
    foreach ($line in $kernOutput) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [kern] $l" -ForegroundColor DarkGray }
    }
}

PhaseDone $buildTimer "Parallel build"

# ══════════════════════════════════════════════════════════════════════
# VALIDATE OUTPUTS
# ══════════════════════════════════════════════════════════════════════
$tval = PhaseStart "Validate outputs"

if (-not (Test-Path $bootEfi))  { Fail "Bootloader EFI not found: $bootEfi" }
if (-not (Test-Path $kernelElf)) { Fail "Kernel ELF not found: $kernelElf" }

$bootSize = (Get-Item $bootEfi).Length
$kernSize = (Get-Item $kernelElf).Length
$bootHash = Hash256 $bootEfi
$kernHash = Hash256 $kernelElf

Write-Host "    BOOTX64.EFI  $([math]::Round($bootSize/1024,1)) KB  sha256:$($bootHash.Substring(0,16))" -ForegroundColor White
Write-Host "    kernel.elf   $([math]::Round($kernSize/1024,1)) KB  sha256:$($kernHash.Substring(0,16))" -ForegroundColor White

PhaseDone $tval "Outputs validated"

# ══════════════════════════════════════════════════════════════════════
# STAGE EFI BOOT STRUCTURE
# ══════════════════════════════════════════════════════════════════════
$tstage = PhaseStart "Stage EFI/BOOT"

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Copy-Item $bootEfi  -Destination (Join-Path $stage "BOOTX64.EFI")
Copy-Item $kernelElf -Destination (Join-Path $stage "kernel.elf")

$manifest = @"
# FastOS EFI Boot Manifest
# Generated: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
# Version: $kv
#
# File           Size        SHA256
# ----           ----        ------
BOOTX64.EFI     $bootSize   $bootHash
kernel.elf      $kernSize   $kernHash
"@
$manifest | Set-Content -Path (Join-Path $stage "MANIFEST.TXT") -Encoding UTF8

PhaseDone $tstage "Staged $([math]::Round(($bootSize+$kernSize)/1024,1)) KB total"

# ══════════════════════════════════════════════════════════════════════
# BUILD SUMMARY
# ══════════════════════════════════════════════════════════════════════
Write-Host ""
Write-Host "  +============================================+" -ForegroundColor Green
Write-Host "  |            BUILD COMPLETE                   |" -ForegroundColor Green
Write-Host "  +============================================+" -ForegroundColor Green
Write-Host "  Total: $([math]::Round($script:timer.Elapsed.TotalSeconds,1))s | Image: $([math]::Round(($bootSize+$kernSize)/1024,1)) KB" -ForegroundColor White
Write-Host ""

if ($BuildOnly) { exit 0 }

# ══════════════════════════════════════════════════════════════════════
# SSD FLASH
# ══════════════════════════════════════════════════════════════════════
if ($Flash) {
    Write-Host "  ── Flash to SSD ──────────────────────────────" -ForegroundColor Cyan
    Write-Host ""

    $targetLetter = $Drive
    if (-not $targetLetter) {
        $targetLetter = "S"
        Warn "No -Drive specified. Defaulting to S: (FastOS SSD)"
    }

    $targetLetter = $targetLetter.TrimEnd([char]':',[char]'\').ToUpper()
    $targetRoot = "${targetLetter}:\"

    if (-not (Test-Path $targetRoot)) { Fail "Drive $targetLetter not found" }
    if (-not $Yes) {
        $c = Read-Host "  Flash to ${targetLetter}:? (YES)"
        if ($c -ne "YES") { Write-Host "  Aborted."; exit 0 }
    }

    $tf = PhaseStart "Flash → $targetLetter`:"
    $efiDest = Join-Path $targetRoot "EFI\BOOT"
    New-Item -ItemType Directory -Path $efiDest -Force | Out-Null

    Copy-Item (Join-Path $stage "BOOTX64.EFI") -Destination (Join-Path $efiDest "BOOTX64.EFI") -Force
    Copy-Item (Join-Path $stage "kernel.elf")   -Destination (Join-Path $efiDest "kernel.elf")   -Force
    Copy-Item (Join-Path $stage "MANIFEST.TXT") -Destination (Join-Path $efiDest "MANIFEST.TXT") -Force

    try {
        $fs = [System.IO.File]::Open("${targetRoot}EFI\BOOT\kernel.elf", 'Open', 'Write')
        $fs.Flush(1)
        $fs.Close()
    } catch { Warn "Flush failed: $_" }
    PhaseDone $tf "Flash"

    $tv = PhaseStart "Verify"
    foreach ($f in @(@{Name="BOOTX64.EFI";Hash=$bootHash;Size=$bootSize}, @{Name="kernel.elf";Hash=$kernHash;Size=$kernSize})) {
        $dp = Join-Path $efiDest $f.Name
        if (-not (Test-Path $dp)) { Fail "MISSING: $($f.Name)" }
        $dh = Hash256 $dp
        $ds = (Get-Item $dp).Length
        if ($dh -ne $f.Hash) {
            Fail "HASH MISMATCH: $($f.Name)"
        } else {
            Write-Host "    $($f.Name): $ds bytes, SHA256 OK" -ForegroundColor Green
        }
    }
    PhaseDone $tv "Verify"

    Write-Host ""
    Write-Host "  +============================================+" -ForegroundColor Green
    Write-Host "  |   FLASH OK -- Reboot from SSD              |" -ForegroundColor Green
    Write-Host "  +============================================+" -ForegroundColor Green
    Write-Host ""
}

# ══════════════════════════════════════════════════════════════════════
# STANDALONE VERIFY
# ══════════════════════════════════════════════════════════════════════
if ($Verify -and -not $Flash) {
    $tl = $Drive
    if (-not $tl) { $tl = "S" }
    $tl = $tl.TrimEnd([char]':',[char]'\').ToUpper()
    $efiCheck = "${tl}:\EFI\BOOT"
    if (-not (Test-Path $efiCheck)) { Fail "EFI\BOOT not found on ${tl}:" }

    foreach ($n in @("BOOTX64.EFI","kernel.elf")) {
        $p2 = Join-Path $efiCheck $n
        if (Test-Path $p2) {
            $sz2 = (Get-Item $p2).Length
            $h2 = Hash256 $p2
            Write-Host "    $n  $sz2 bytes  sha256:$($h2.Substring(0,16))" -ForegroundColor Green
        } else {
            Fail "MISSING: $n"
        }
    }
    Write-Host "  SSD verified OK" -ForegroundColor Green
    Write-Host ""
}

# ── Final ──────────────────────────────────────────────────────────────
Write-Host "  Total: $([math]::Round($script:timer.Elapsed.TotalSeconds,1))s" -ForegroundColor DarkGray
Write-Host ""

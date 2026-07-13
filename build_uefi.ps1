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
.PARAMETER LLFree
    Build kernel with the LLFree physical backing allocator.
.PARAMETER Jobs
    Max cargo parallel jobs (default: all CPU cores).
.EXAMPLE
    .\build_uefi.ps1                     # Smart build (skip unchanged)
    .\build_uefi.ps1 -Flash              # Build + flash to SSD (S:)
    .\build_uefi.ps1 -Flash -LLFree      # Build + flash with LLFree allocator
    .\build_uefi.ps1 -Flash -Drive E     # Build + flash to E:
    .\build_uefi.ps1 -Clean              # Clean + rebuild
    .\build_uefi.ps1 -Jobs 4             # Limit to 4 parallel cargo jobs
.NOTES
    v3.1: SSD-first approach, simplified flash, parallel builds.

    SSD Partition Layout (3-partition plan):
      S: FASTOS-EFI    (10 GB) — UEFI boot partition. BOOTX64.EFI + kernel.elf.
      T: FastOS-Data   (~50 GB) — Apps, user data, /home.
      X: Commit-Real   (~60 GB) — TimeBack git repo (commits, trees, blobs).
    Run with -Drive S to flash the kernel.efi to S: (FASTOS-EFI).
    Type `layout` in the CABINA to display this layout.
#>
param(
    [switch]$Flash,
    [switch]$Verify,
    [switch]$Clean,
    [switch]$BuildOnly,
    [switch]$LLFree,
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
        if ($LLFree)    { $args += "-LLFree" }
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
$moduleDir = Join-Path $root "crates_Personal\modules\bmo_core"
$timebackDir = Join-Path $root "crates_Personal\modules\timeback"
$cabinaDir = Join-Path $root "crates_Personal\modules\cabina"
$linuxDevourDir = Join-Path $root "crates_Personal\modules\linux_devour"
$wineDevourDir = Join-Path $root "crates_Personal\modules\wine_devour"
$terminalDir = Join-Path $root "crates_Personal\modules\terminal"
$target   = Join-Path $root "target_build"
$stage    = Join-Path $target "staging\EFI\BOOT"

if (-not (Test-Path (Join-Path $bootDir "Cargo.toml")))      { Fail "Bootloader not found at $bootDir" }
if (-not (Test-Path (Join-Path $kernDir "Cargo.toml")))       { Fail "Kernel not found at $kernDir" }
if (-not (Test-Path (Join-Path $moduleDir "Cargo.toml")))     { Fail "mod_bmo_core not found at $moduleDir" }
if (-not (Test-Path (Join-Path $timebackDir "Cargo.toml")))   { Fail "mod_timeback not found at $timebackDir" }
if (-not (Test-Path (Join-Path $cabinaDir "Cargo.toml")))     { Fail "mod_cabina not found at $cabinaDir" }
if (-not (Test-Path (Join-Path $linuxDevourDir "Cargo.toml"))) { Fail "mod_linux_devour not found at $linuxDevourDir" }
if (-not (Test-Path (Join-Path $wineDevourDir "Cargo.toml")))  { Fail "mod_wine_devour not found at $wineDevourDir" }
if (-not (Test-Path (Join-Path $terminalDir "Cargo.toml")))    { Fail "mod_terminal not found at $terminalDir" }

# ── Version ────────────────────────────────────────────────────────────
$kv = "unknown"
$toml = Get-Content (Join-Path $kernDir "Cargo.toml") -Raw
if ($toml -match 'version\s*=\s*"(.+?)"') { $kv = $Matches[1] }

# ── Cargo jobs flag ────────────────────────────────────────────────────
$jobsFlag = if ($Jobs -gt 0) { @("-j$Jobs") } else { @() }
$kernelFeatureKey = if ($LLFree) { "llfree" } else { "buddy" }
$kernelFeatureFlag = if ($LLFree) { @("--no-default-features", "--features", "alloc-llfree") } else { @() }

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
# SMART REBUILD DETECTION (content-hash based)
# ══════════════════════════════════════════════════════════════════════
# Uses content hashing instead of timestamps. Timestamps are unreliable
# when source files are edited by external tools that don't update mtime.
$script:srcHashCache = @{}

function Get-SourceContentHash {
    param($dir, $exts = @("rs","toml","ld"))
    $hasher = [System.Security.Cryptography.SHA256]::Create()
    $files = @()
    foreach ($ext in $exts) {
        Get-ChildItem -Path $dir -Recurse -Filter "*.$ext" -ErrorAction SilentlyContinue | ForEach-Object {
            $files += $_.FullName
        }
    }
    $files = $files | Sort-Object
    $combined = New-Object System.IO.MemoryStream
    foreach ($f in $files) {
        $bytes = [System.IO.File]::ReadAllBytes($f)
        $combined.Write($bytes, 0, $bytes.Length)
    }
    $hash = $hasher.ComputeHash($combined.ToArray())
    $combined.Dispose()
    return [BitConverter]::ToString($hash).Replace("-","").ToLower()
}

function Needs-Rebuild {
    param($sourceDir, $outputFile, $variant = "")
    if (-not (Test-Path $outputFile)) { return $true }
    $srcHash = Get-SourceContentHash $sourceDir
    $hashFile = if ($variant) { "$outputFile.$variant.srcsha256" } else { "$outputFile.srcsha256" }
    if (Test-Path $hashFile) {
        $savedHash = Get-Content $hashFile -ErrorAction SilentlyContinue
        if ($srcHash -eq $savedHash) { return $false }
    }
    return $true
}

function Save-SourceHash {
    param($sourceDir, $outputFile, $variant = "")
    $srcHash = Get-SourceContentHash $sourceDir
    $hashFile = if ($variant) { "$outputFile.$variant.srcsha256" } else { "$outputFile.srcsha256" }
    $srcHash | Set-Content $hashFile -NoNewline
}

$bootEfi = Join-Path $target "bootloader\x86_64-unknown-uefi\release\bmo-bootloader.efi"
$nanoWakeElf = Join-Path $target "kernel\x86_64-unknown-none\release\layer-nano-wake"
$kernelElf = Join-Path $target "kernel\x86_64-unknown-none\release\bmo-kernel"
$moduleTargetDir = Join-Path $target "modules"
$moduleElf = Join-Path $moduleTargetDir "bmo_core\x86_64-unknown-none\release\mod-bmo-core"
$timebackElf = Join-Path $moduleTargetDir "timeback\x86_64-unknown-none\release\mod-timeback"
$cabinaElf = Join-Path $moduleTargetDir "cabina\x86_64-unknown-none\release\mod-cabina"
$linuxDevourElf = Join-Path $moduleTargetDir "linux_devour\x86_64-unknown-none\release\mod-linux-devour"
$wineDevourElf = Join-Path $moduleTargetDir "wine_devour\x86_64-unknown-none\release\mod-wine-devour"
$terminalElf = Join-Path $moduleTargetDir "terminal\x86_64-unknown-none\release\mod-terminal"

$needBoot      = Needs-Rebuild $bootDir $bootEfi
$needKern      = Needs-Rebuild $kernDir $kernelElf $kernelFeatureKey
$needModule    = Needs-Rebuild $moduleDir $moduleElf
$needTimeback  = Needs-Rebuild $timebackDir $timebackElf
$needCabina    = Needs-Rebuild $cabinaDir $cabinaElf
$needLinuxDevour = Needs-Rebuild $linuxDevourDir $linuxDevourElf
$needWineDevour = Needs-Rebuild $wineDevourDir $wineDevourElf
$needTerminal  = Needs-Rebuild $terminalDir $terminalElf

if (-not $needBoot -and -not $needKern -and -not $needModule -and -not $needTimeback -and -not $needCabina -and -not $needLinuxDevour -and -not $needTerminal -and -not $Clean) {
    Write-Host "  OK All up to date. Nothing to rebuild." -ForegroundColor Green
    if (-not $Flash -and -not $Verify) {
        Write-Host "  Use -Clean to force rebuild, or -Flash to flash existing build." -ForegroundColor DarkGray
        Write-Host ""
        exit 0
    }
}

# ══════════════════════════════════════════════════════════════════════
# PARALLEL BUILD: BOOTLOADER + KERNEL + MODULE
# ══════════════════════════════════════════════════════════════════════
$buildTimer = PhaseStart "Building bootloader + kernel + modules (parallel)"

# Define build jobs
$bootJob = $null
$kernJob = $null
$moduleJob = $null
$timebackJob = $null
$cabinaJob = $null
$terminalJob = $null

if ($needBoot) {
    $bootTargetDir = Join-Path $target "bootloader"
    $bootScript = {
        param($bootDir, $bootTargetDir, $jobsFlag)
        Push-Location $bootDir
        try {
            $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
            $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $bootTargetDir $(if ($jobsFlag) { $jobsFlag }) 2>&1
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
        param($kernDir, $kernTargetDir, $jobsFlag, $kernelFeatureFlag)
        Push-Location $kernDir
        try {
            $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
            $out1 = cargo build --release --target x86_64-unknown-none --target-dir $kernTargetDir -p layer-nano-wake @jobsFlag 2>&1
            $err1 = $out1 | Where-Object { $_ -match "^error" }
            if ($err1) { throw "Nano-Wake build error: $err1" }

            $out2 = cargo build --release --target x86_64-unknown-none --target-dir $kernTargetDir -p bmo-kernel @jobsFlag @kernelFeatureFlag 2>&1
            $err2 = $out2 | Where-Object { $_ -match "^error" }
            if ($err2) { throw "Kernel build error: $err2" }

            return @{ Ok=$true; Output=(@($out1) + @($out2)) }
        } catch {
            return @{ Ok=$false; Error=$_.Exception.Message }
        } finally {
            Pop-Location
        }
    }
    $kernJob = Start-Job -ScriptBlock $kernScript -ArgumentList $kernDir, $kernTargetDir, $jobsFlag, $kernelFeatureFlag
    Step "Kernel build started (PID $($kernJob.Id), allocator=$kernelFeatureKey)"
}

# Build all 3 modules (Ring 3)
    $moduleScript = {
        param($mdir, $mtargetDir, $jobsFlag)
        Set-Location -LiteralPath $mdir
        try {
            $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
            $out = cargo build --release --target x86_64-unknown-none --target-dir $mtargetDir @jobsFlag 2>&1
        $err = $out | Where-Object { $_ -match "^error" }
        if ($err) { throw "Module build error: $err" }
        return @{ Ok=$true; Output=$out }
    } catch {
        return @{ Ok=$false; Error=$_.Exception.Message }
    }
}

if ($needModule) {
    $moduleJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $moduleDir, (Join-Path $moduleTargetDir "bmo_core"), $jobsFlag
    Step "mod_bmo_core build started (PID $($moduleJob.Id))"
}

if ($needTimeback) {
    $timebackJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $timebackDir, (Join-Path $moduleTargetDir "timeback"), $jobsFlag
    Step "mod_timeback build started (PID $($timebackJob.Id))"
}

if ($needCabina) {
    $cabinaJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $cabinaDir, (Join-Path $moduleTargetDir "cabina"), $jobsFlag
    Step "mod_cabina build started (PID $($cabinaJob.Id))"
}

if ($needLinuxDevour) {
    $linuxDevourJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $linuxDevourDir, (Join-Path $moduleTargetDir "linux_devour"), $jobsFlag
    Step "mod_linux_devour build started (PID $($linuxDevourJob.Id))"
}

if ($needWineDevour) {
    $wineDevourJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $wineDevourDir, (Join-Path $moduleTargetDir "wine_devour"), $jobsFlag
    Step "mod_wine_devour build started (PID $($wineDevourJob.Id))"
}

if ($needTerminal) {
    $terminalJob = Start-Job -ScriptBlock $moduleScript -ArgumentList $terminalDir, (Join-Path $moduleTargetDir "terminal"), $jobsFlag
    Step "mod_terminal build started (PID $($terminalJob.Id))"
}

# Wait for all jobs
$bootResult = $null
$kernResult = $null
$moduleResult = $null
$timebackResult = $null
$cabinaResult = $null
$terminalResult = $null

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

if ($moduleJob) {
    Step "Waiting for mod_bmo_core..."
    $moduleResult = Receive-Job -Job $moduleJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $moduleJob -Force
    if (-not $moduleResult.Ok) { Fail "BMO_CORE FAILED: $($moduleResult.Error)" }
    $moduleOutput = $moduleResult.Output
    foreach ($line in $moduleOutput) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [core]  $l" -ForegroundColor DarkGray }
    }
}

if ($timebackJob) {
    Step "Waiting for mod_timeback..."
    $timebackResult = Receive-Job -Job $timebackJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $timebackJob -Force
    if (-not $timebackResult.Ok) { Fail "TIMEBACK FAILED: $($timebackResult.Error)" }
    foreach ($line in $timebackResult.Output) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [tb]   $l" -ForegroundColor DarkGray }
    }
}

if ($linuxDevourJob) {
    Step "Waiting for mod_linux_devour..."
    $linuxDevourResult = Receive-Job -Job $linuxDevourJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $linuxDevourJob -Force
    if (-not $linuxDevourResult.Ok) { Fail "LINUX DEVOUR FAILED: $($linuxDevourResult.Error)" }
    foreach ($line in $linuxDevourResult.Output) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [linux] $l" -ForegroundColor DarkGray }
    }
}

if ($wineDevourJob) {
    Step "Waiting for mod_wine_devour..."
    $wineDevourResult = Receive-Job -Job $wineDevourJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $wineDevourJob -Force
    if (-not $wineDevourResult.Ok) { Fail "WINE DEVOUR FAILED: $($wineDevourResult.Error)" }
    foreach ($line in $wineDevourResult.Output) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [wine]  $l" -ForegroundColor DarkGray }
    }
}

if ($cabinaJob) {
    Step "Waiting for mod_cabina..."
    $cabinaResult = Receive-Job -Job $cabinaJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $cabinaJob -Force
    if (-not $cabinaResult.Ok) { Fail "CABINA FAILED: $($cabinaResult.Error)" }
    foreach ($line in $cabinaResult.Output) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [cab]  $l" -ForegroundColor DarkGray }
    }
}

if ($terminalJob) {
    Step "Waiting for mod_terminal..."
    $terminalResult = Receive-Job -Job $terminalJob -Wait -ErrorAction SilentlyContinue
    Remove-Job -Job $terminalJob -Force
    if (-not $terminalResult.Ok) { Fail "TERMINAL FAILED: $($terminalResult.Error)" }
    foreach ($line in $terminalResult.Output) {
        $l = "$line"
        if ($l -match "Compiling|Finished|warning") { Write-Host "    [term] $l" -ForegroundColor DarkGray }
    }
}

PhaseDone $buildTimer "Parallel build"

# Save source hashes for next build's change detection
if ($needBoot)      { Save-SourceHash $bootDir $bootEfi }
if ($needKern)      { Save-SourceHash $kernDir $kernelElf $kernelFeatureKey }
if ($needModule)    { Save-SourceHash $moduleDir $moduleElf }
if ($needTimeback)  { Save-SourceHash $timebackDir $timebackElf }
if ($needCabina)    { Save-SourceHash $cabinaDir $cabinaElf }
if ($needLinuxDevour) { Save-SourceHash $linuxDevourDir $linuxDevourElf }
if ($needWineDevour) { Save-SourceHash $wineDevourDir $wineDevourElf }
if ($needTerminal)  { Save-SourceHash $terminalDir $terminalElf }

# ══════════════════════════════════════════════════════════════════════
# VALIDATE OUTPUTS
# ══════════════════════════════════════════════════════════════════════
$tval = PhaseStart "Validate outputs"

if (-not (Test-Path $bootEfi))      { Fail "Bootloader EFI not found: $bootEfi" }
if (-not (Test-Path $nanoWakeElf))  { Fail "Nano-Wake ELF not found: $nanoWakeElf" }
if (-not (Test-Path $kernelElf))     { Fail "Kernel ELF not found: $kernelElf" }
if (-not (Test-Path $moduleElf))     { Fail "Module ELF not found: $moduleElf" }
if (-not (Test-Path $timebackElf))   { Fail "Timeback ELF not found: $timebackElf" }
if (-not (Test-Path $cabinaElf))     { Fail "Cabina ELF not found: $cabinaElf" }
if (-not (Test-Path $linuxDevourElf)) { Fail "Linux Devour ELF not found: $linuxDevourElf" }
if (-not (Test-Path $wineDevourElf))  { Fail "Wine Devour ELF not found: $wineDevourElf" }
if (-not (Test-Path $terminalElf))    { Fail "Terminal ELF not found: $terminalElf" }

$bootSize      = (Get-Item $bootEfi).Length
$nanoWakeSize  = (Get-Item $nanoWakeElf).Length
$kernSize      = (Get-Item $kernelElf).Length
$moduleSize    = (Get-Item $moduleElf).Length
$timebackSize  = (Get-Item $timebackElf).Length
$cabinaSize    = (Get-Item $cabinaElf).Length
$bootHash      = Hash256 $bootEfi
$nanoWakeHash  = Hash256 $nanoWakeElf
$kernHash      = Hash256 $kernelElf
$moduleHash    = Hash256 $moduleElf
$timebackHash  = Hash256 $timebackElf
$cabinaHash    = Hash256 $cabinaElf
$linuxDevourSize = (Get-Item $linuxDevourElf).Length
$linuxDevourHash = Hash256 $linuxDevourElf
$wineDevourSize  = (Get-Item $wineDevourElf).Length
$wineDevourHash  = Hash256 $wineDevourElf
$terminalSize    = (Get-Item $terminalElf).Length
$terminalHash    = Hash256 $terminalElf

Write-Host "    BOOTX64.EFI         $([math]::Round($bootSize/1024,1)) KB  sha256:$($bootHash.Substring(0,16))" -ForegroundColor White
Write-Host "    layer-nano-wake     $([math]::Round($nanoWakeSize/1024,1)) KB  sha256:$($nanoWakeHash.Substring(0,16))" -ForegroundColor White
Write-Host "    kernel_services.elf $([math]::Round($kernSize/1024,1)) KB  sha256:$($kernHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_bmo_core.elf    $([math]::Round($moduleSize/1024,1)) KB  sha256:$($moduleHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_timeback.elf    $([math]::Round($timebackSize/1024,1)) KB  sha256:$($timebackHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_cabina.elf      $([math]::Round($cabinaSize/1024,1)) KB  sha256:$($cabinaHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_linux_devour.elf $([math]::Round($linuxDevourSize/1024,1)) KB  sha256:$($linuxDevourHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_wine_devour.elf $([math]::Round($wineDevourSize/1024,1)) KB  sha256:$($wineDevourHash.Substring(0,16))" -ForegroundColor White
Write-Host "    mod_terminal.elf    $([math]::Round($terminalSize/1024,1)) KB  sha256:$($terminalHash.Substring(0,16))" -ForegroundColor White

PhaseDone $tval "Outputs validated"

# ══════════════════════════════════════════════════════════════════════
# STAGE EFI BOOT STRUCTURE
# ══════════════════════════════════════════════════════════════════════
$tstage = PhaseStart "Stage EFI/BOOT"

if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

# Locate llvm-objcopy from rustup
$llvmObjcopy = Get-ChildItem -Path "C:\Users\andre\.rustup" -Filter "llvm-objcopy.exe" -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopy) { Fail "llvm-objcopy.exe not found in rustup directory" }

Copy-Item $bootEfi  -Destination (Join-Path $stage "BOOTX64.EFI")
# Convert ELF to flat raw binary
& $llvmObjcopy -O binary $nanoWakeElf (Join-Path $stage "kernel.bin")
Copy-Item $kernelElf -Destination (Join-Path $stage "kernel_services.elf")

$modulesStageDir = Join-Path $stage "modules"
if (-not (Test-Path $modulesStageDir)) { New-Item -ItemType Directory -Path $modulesStageDir -Force | Out-Null }
Copy-Item $moduleElf   -Destination (Join-Path $modulesStageDir "mod_bmo_core.elf")
Copy-Item $timebackElf -Destination (Join-Path $modulesStageDir "mod_timeback.elf")
Copy-Item $cabinaElf   -Destination (Join-Path $modulesStageDir "mod_cabina.elf")
Copy-Item $linuxDevourElf -Destination (Join-Path $modulesStageDir "mod_linux_devour.elf")
Copy-Item $wineDevourElf  -Destination (Join-Path $modulesStageDir "mod_wine_devour.elf")
Copy-Item $terminalElf   -Destination (Join-Path $modulesStageDir "mod_terminal.elf")

$nanoWakeBin = Join-Path $stage "kernel.bin"
$nanoWakeBinSize = (Get-Item $nanoWakeBin).Length
$nanoWakeBinHash = Hash256 $nanoWakeBin

$totalSize = $bootSize + $nanoWakeBinSize + $kernSize + $moduleSize + $timebackSize + $cabinaSize + $linuxDevourSize + $wineDevourSize + $terminalSize
$manifest = @"
# FastOS EFI Boot Manifest
# Generated: $(Get-Date -Format "yyyy-MM-dd HH:mm:ss")
# Version: $kv
# Allocator: $kernelFeatureKey
#
# File           Size        SHA256
# ----           ----        ------
BOOTX64.EFI               $bootSize   $bootHash
kernel.bin                $nanoWakeBinSize $nanoWakeBinHash
kernel_services.elf       $kernSize   $kernHash
modules/mod_bmo_core.elf  $moduleSize $moduleHash
modules/mod_timeback.elf  $timebackSize $timebackHash
modules/mod_cabina.elf    $cabinaSize $cabinaHash
modules/mod_linux_devour.elf $linuxDevourSize $linuxDevourHash
modules/mod_wine_devour.elf  $wineDevourSize $wineDevourHash
modules/mod_terminal.elf     $terminalSize $terminalHash
"@
$manifest | Set-Content -Path (Join-Path $stage "MANIFEST.TXT") -Encoding UTF8

PhaseDone $tstage "Staged $([math]::Round($totalSize/1024,1)) KB total"

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

    Copy-Item (Join-Path $stage "BOOTX64.EFI")       -Destination (Join-Path $efiDest "BOOTX64.EFI")       -Force
    Copy-Item (Join-Path $stage "kernel.bin")        -Destination (Join-Path $efiDest "kernel.bin")        -Force
    Remove-Item (Join-Path $efiDest "kernel.elf")    -Force -ErrorAction SilentlyContinue
    Copy-Item (Join-Path $stage "kernel_services.elf") -Destination (Join-Path $efiDest "kernel_services.elf") -Force
    $modulesDestDir = Join-Path $efiDest "modules"
    if (-not (Test-Path $modulesDestDir)) { New-Item -ItemType Directory -Path $modulesDestDir -Force | Out-Null }
    Copy-Item (Join-Path $stage "modules\mod_bmo_core.elf")  -Destination "${modulesDestDir}\mod_bmo_core.elf" -Force
    Copy-Item (Join-Path $stage "modules\mod_timeback.elf")  -Destination "${modulesDestDir}\mod_timeback.elf" -Force
    Copy-Item (Join-Path $stage "modules\mod_cabina.elf")    -Destination "${modulesDestDir}\mod_cabina.elf" -Force
    Copy-Item (Join-Path $stage "modules\mod_linux_devour.elf") -Destination "${modulesDestDir}\mod_linux_devour.elf" -Force
    Copy-Item (Join-Path $stage "modules\mod_wine_devour.elf")  -Destination "${modulesDestDir}\mod_wine_devour.elf" -Force
    Copy-Item (Join-Path $stage "modules\mod_terminal.elf")    -Destination "${modulesDestDir}\mod_terminal.elf" -Force
    Copy-Item (Join-Path $stage "MANIFEST.TXT")      -Destination (Join-Path $efiDest "MANIFEST.TXT")      -Force

    try {
        $fs = [System.IO.File]::Open("${targetRoot}EFI\BOOT\kernel.bin", 'Open', 'Write')
        $fs.Flush(1)
        $fs.Close()
        $fss = [System.IO.File]::Open("${targetRoot}EFI\BOOT\kernel_services.elf", 'Open', 'Write')
        $fss.Flush(1)
        $fss.Close()
        $fs1 = [System.IO.File]::Open("${modulesDestDir}\mod_bmo_core.elf", 'Open', 'Write')
        $fs1.Flush(1)
        $fs1.Close()
        $fs2 = [System.IO.File]::Open("${modulesDestDir}\mod_terminal.elf", 'Open', 'Write')
        $fs2.Flush(1)
        $fs2.Close()
    } catch { Warn "Flush failed: $_" }
    PhaseDone $tf "Flash"

    $tv = PhaseStart "Verify"
    foreach ($f in @(
        @{Name="BOOTX64.EFI";Hash=$bootHash;Size=$bootSize;IsModule=$false},
        @{Name="kernel.bin";Hash=$nanoWakeBinHash;Size=$nanoWakeBinSize;IsModule=$false},
        @{Name="kernel_services.elf";Hash=$kernHash;Size=$kernSize;IsModule=$false},
        @{Name="mod_bmo_core.elf";Hash=$moduleHash;Size=$moduleSize;IsModule=$true},
        @{Name="mod_timeback.elf";Hash=$timebackHash;Size=$timebackSize;IsModule=$true},
        @{Name="mod_cabina.elf";Hash=$cabinaHash;Size=$cabinaSize;IsModule=$true},
        @{Name="mod_linux_devour.elf";Hash=$linuxDevourHash;Size=$linuxDevourSize;IsModule=$true},
        @{Name="mod_wine_devour.elf";Hash=$wineDevourHash;Size=$wineDevourSize;IsModule=$true},
        @{Name="mod_terminal.elf";Hash=$terminalHash;Size=$terminalSize;IsModule=$true}
    )) {
        $dp = if ($f.IsModule) { Join-Path (Join-Path $efiDest "modules") $f.Name } else { Join-Path $efiDest $f.Name }
        if (-not (Test-Path $dp)) { Fail "MISSING: $($f.Name) at $dp" }
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

    foreach ($n in @("BOOTX64.EFI","kernel.elf","modules\mod_bmo_core.elf","modules\mod_timeback.elf","modules\mod_cabina.elf")) {
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

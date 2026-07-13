param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$BuildOnly,
    [string]$Drive = "S",
    [switch]$Yes
)

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }

function Step { param($m) Write-Host "  => $m" -ForegroundColor Cyan }
function Fail { param($m) Write-Host "  [X] $m" -ForegroundColor Red; exit 1 }

$target = Join-Path $root "target"
$stage  = Join-Path $root "staging\EFI\BOOT"

if ($Clean) {
    Step "Cleaning"
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    if (Test-Path $stage)  { Remove-Item $stage  -Recurse -Force }
}

Write-Host ""
Write-Host "  ═══ BMO Ultra Kernel v2 Build (UEFI 5 layers + 12 faggin stages + kernel) ═══" -ForegroundColor Magenta
Write-Host ""

# ── Build uefi_chain (5 UEFI layers) ──────────────────────────────
# Uses cargo +nightly for UEFI target. boot_context is a workspace
# dep; uefi_chain statically links it.
Step "Building uefi_chain (5 UEFI layers)..."
Push-Location $root
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $target 2>&1
    $out | ForEach-Object {
        if ($_ -match "Compiling|Finished|error") { Write-Host "    [uefi_chain] $_" -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail "uefi_chain build failed" }
} finally { Pop-Location }

# ── Build the 12 faggin stages in order ────────────────────────────
# Each stage is a separate crate with its own target/x86_64-unknown-none
# build. They depend on serial_shared and boot_context (rlibs from
# the Ultra_kernel workspace) statically linked.
#
# Per-stage opt-level = "z" is set in faggin/Cargo.toml to keep
# each binary as small as possible (target: 1-3 KB flat binary).
$stages = @("s1_serial", "s2_gdt", "s3_idt", "s4_cpuid", "s5_control", "s6_fpu",
            "s7_tsc", "s8_syscall", "s9_paging", "s10_heap", "s11_acpi", "s12_devices")
$idx = 0
foreach ($s in $stages) {
    Step ("[{0,2}/12] Building faggin " -f ($idx + 1)) $s..."
    $stageDir = Join-Path (Join-Path $root "faggin") $s
    Push-Location $stageDir
    try {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        $td = Join-Path $target "faggin\$s"
        $out = cargo +nightly build --release --target x86_64-unknown-none --target-dir $td 2>&1
        $out | ForEach-Object {
            if ($_ -match "Compiling|Finished|error") { Write-Host "    [$s] $_" -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail "$s build failed" }
    } finally { Pop-Location }
    $idx++
}

# ── Build kernel (Ring 0 base) ────────────────────────────────────
Step "Building kernel (Ring 0 base)..."
$stageDir = Join-Path $root "kernel"
Push-Location $stageDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $out = cargo +nightly build --release --target x86_64-unknown-none --target-dir (Join-Path $target "kernel") 2>&1
    $out | ForEach-Object {
        if ($_ -match "Compiling|Finished|error") { Write-Host "    [kernel] $_" -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail "kernel build failed" }
} finally { Pop-Location }

# ── Validate outputs ──────────────────────────────────────────────
Step "Validating outputs"
$uefi_chain = Join-Path $target "x86_64-unknown-uefi\release\uefi_chain.efi"

$all_binaries = @($uefi_chain)
$idx = 0
foreach ($s in $stages) {
    $td = Join-Path $target "faggin\$s"
    $bin = Join-Path $td "x86_64-unknown-none\release\$s.exe"
    if (-not (Test-Path $bin)) { $bin = Join-Path $td "x86_64-unknown-none\release\$s" }
    $all_binaries += $bin
    $idx++
}
$kernel = Join-Path $target "kernel\x86_64-unknown-none\release\bmo-kernel.exe"
if (-not (Test-Path $kernel)) { $kernel = Join-Path $target "kernel\x86_64-unknown-none\release\bmo-kernel" }
$all_binaries += $kernel

Write-Host ""
Write-Host "  bin                        raw       flat     address" -ForegroundColor White
Write-Host "  ──────────────────────    ──────   ──────   ─────────" -ForegroundColor DarkGray
$total = 0
$base = 0x100000
foreach ($f in $all_binaries) {
    if (-not (Test-Path $f)) { Fail "Not found: $f" }
    $raw = (Get-Item $f).Length
    $name = Split-Path $f -Leaf
    if ($name -eq "uefi_chain.efi") {
        $line = "  {0,-25} {1,6} B              0xFE000000 (UEFI load)" -f $name, $raw
    } else {
        $flat = Join-Path (Join-Path $target "faggin\$(Split-Path (Split-Path $f -Parent) -Leaf)") (Split-Path $f -Leaf)
        $flat = $flat -replace '\.exe$', ''
        $flat = $flat + ".bin"
        if (Test-Path $flat) {
            $fsz = (Get-Item $flat).Length
        } else {
            $fsz = 0
        }
        $line = "  {0,-25} {1,6} B   {2,5} B   0x{3:X6}" -f $name, $raw, $fsz, $base
        $base += 0x10000
        $total += $fsz
    }
    Write-Host $line
}
Write-Host "  ──────────────────────    ──────   ──────" -ForegroundColor DarkGray
Write-Host ("  TOTAL flat size:              {0,6} B" -f $total) -ForegroundColor Yellow
Write-Host ""

# ── Stage to ESP layout ───────────────────────────────────────────
Step "Staging to $stage"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage "modules") -Force | Out-Null

Copy-Item $uefi_chain (Join-Path $stage "BOOTX64.EFI")

$llvmObjcopy = Get-ChildItem -Path "C:\Users\andre\.rustup" -Filter "llvm-objcopy.exe" -Recurse | Select-Object -First 1 -ExpandProperty FullName
if ($llvmObjcopy) {
    $idx = 0
    foreach ($s in $stages) {
        $td = Join-Path $target "faggin\$s"
        $bin = Join-Path $td "x86_64-unknown-none\release\$s.exe"
        if (-not (Test-Path $bin)) { $bin = Join-Path $td "x86_64-unknown-none\release\$s" }
        $out = Join-Path $stage "$s.bin"
        & $llvmObjcopy -O binary $bin $out
        if ($LASTEXITCODE -ne 0) { Fail "objcopy failed for $s" }
        $idx++
    }
    $bin = $kernel
    & $llvmObjcopy -O binary $bin (Join-Path $stage "kernel.bin")
    if ($LASTEXITCODE -ne 0) { Fail "objcopy failed for kernel" }
} else {
    Write-Host "  [WARN] llvm-objcopy not found — copying ELF files as-is" -ForegroundColor Yellow
    $idx = 0
    foreach ($s in $stages) {
        $td = Join-Path $target "faggin\$s"
        $bin = Join-Path $td "x86_64-unknown-none\release\$s.exe"
        if (-not (Test-Path $bin)) { $bin = Join-Path $td "x86_64-unknown-none\release\$s" }
        Copy-Item $bin (Join-Path $stage "$s.bin")
        $idx++
    }
    Copy-Item $kernel (Join-Path $stage "kernel.bin")
}

Write-Host ""
Write-Host "  ═══ BUILD COMPLETE ═══" -ForegroundColor Green
Write-Host "  Staged to: $stage" -ForegroundColor White
Write-Host ""
Write-Host "  12 faggin stages + kernel (Ring 0)" -ForegroundColor Cyan
Write-Host "  Each stage loads at 0x100000 + (i-1)*0x10000" -ForegroundColor Cyan
Write-Host ""

if ($BuildOnly) { exit 0 }

# ── Flash ────────────────────────────────────────────────────────
if ($Flash) {
    $targetLetter = $Drive.TrimEnd([char]':',[char]'\').ToUpper()
    $targetRoot = "${targetLetter}:\"
    if (-not (Test-Path $targetRoot)) { Fail "Drive $targetLetter not found" }

    if (-not $Yes) {
        $c = Read-Host "  Flash to ${targetLetter}:? (YES)"
        if ($c -ne "YES") { Write-Host "  Aborted."; exit 0 }
    }

    Step "Flashing to $targetLetter`:\EFI\BOOT"
    $efiDest = Join-Path $targetRoot "EFI\BOOT"
    New-Item -ItemType Directory -Path $efiDest -Force | Out-Null
    Copy-Item (Join-Path $stage "BOOTX64.EFI") -Destination (Join-Path $efiDest "BOOTX64.EFI") -Force
    foreach ($s in $stages) {
        Copy-Item (Join-Path $stage "$s.bin") -Destination (Join-Path $efiDest "$s.bin") -Force
    }
    Copy-Item (Join-Path $stage "kernel.bin") -Destination (Join-Path $efiDest "kernel.bin") -Force

    try {
        foreach ($f in @("BOOTX64.EFI", "kernel.bin") + ($stages | ForEach-Object { "$_.bin" })) {
            $fs = [System.IO.File]::Open("$targetRoot\EFI\BOOT\$f", 'Open', 'Write')
            $fs.Flush(1); $fs.Close()
        }
    } catch { Write-Host "  [WARN] Flush failed: $_" -ForegroundColor Yellow }

    Write-Host ""
    Write-Host "  ═══ FLASH OK ═══" -ForegroundColor Green
    Write-Host "  Reboot from SSD to test." -ForegroundColor White
    Write-Host ""
}

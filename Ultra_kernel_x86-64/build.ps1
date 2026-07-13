param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$BuildOnly,
    [string]$Drive = 'S',
    [switch]$Yes
)

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }

function Step { param($m) Write-Host ('  => ' + $m) -ForegroundColor Cyan }
function Fail { param($m) Write-Host ('  [X] ' + $m) -ForegroundColor Red; exit 1 }

$target = Join-Path $root 'target'
$stage  = Join-Path $root (Join-Path 'staging' 'EFI\BOOT')

if ($Clean) {
    Step 'Cleaning'
    if (Test-Path $target) { Remove-Item $target -Recurse -Force }
    if (Test-Path $stage)  { Remove-Item $stage  -Recurse -Force }
}

Write-Host ''
Write-Host '  === BMO Ultra Kernel v2 Build (UEFI 5 layers + 12 faggin stages + kernel) ===' -ForegroundColor Magenta
Write-Host ''

# ── Build uefi_chain (5 UEFI layers) ──────────────────────────────
# uefi_chain is its own single-crate workspace at Ultra_kernel/uefi_chain/.
# It targets x86_64-unknown-uefi and depends on boot-context (path).
Step 'Building uefi_chain (5 UEFI layers)...'
$uefiDir = Join-Path $root 'uefi_chain'
Push-Location $uefiDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    # Use a separate target dir so uefi_chain's build artifacts
    # don't pollute the bare-metal target dir.
    $uefiTarget = Join-Path $target 'uefi_chain'
    $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $uefiTarget 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [uefi_chain] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'uefi_chain build failed' }
} finally { Pop-Location }

# ── Build the 12 faggin stages in order ────────────────────────────
$stages = @(
    's1_serial', 's2_gdt', 's3_idt', 's4_cpuid',
    's5_control', 's6_fpu', 's7_tsc', 's8_syscall',
    's9_paging', 's10_heap', 's11_acpi', 's12_devices'
)
$idx = 0
foreach ($s in $stages) {
    Step (('[{0,2}/12] Building faggin ' -f ($idx + 1)) + $s + '...')
    $stageDir = Join-Path (Join-Path $root 'faggin') $s
    Push-Location $stageDir
    try {
        $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
        $td = Join-Path $target (Join-Path 'faggin' $s)
        # Each faggin stage is a standalone crate, NOT part of any workspace.
        # We pass opt-level=z via --config so the resulting binary is
        # as small as possible. Each stage also has its own
        # .cargo/config.toml that points to its linker.ld and adds
        # -C relocation-model=static -C target-feature=+crt-static.
        $out = cargo +nightly build --release --target x86_64-unknown-none --target-dir $td 2>&1
        $out | ForEach-Object {
            if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [' + $s + '] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ($s + ' build failed') }
    } finally { Pop-Location }
    $idx++
}

# ── Build kernel (Ring 0 base) ────────────────────────────────────
Step 'Building kernel (Ring 0 base)...'
$stageDir = Join-Path $root 'kernel'
Push-Location $stageDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $kd = Join-Path $target 'kernel'
    $out = cargo +nightly build --release --target x86_64-unknown-none --target-dir $kd 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [kernel] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'kernel build failed' }
} finally { Pop-Location }

# ── Validate outputs ──────────────────────────────────────────────
Step 'Validating outputs'
$uefi_chain = Join-Path $target (Join-Path 'uefi_chain' (Join-Path 'x86_64-unknown-uefi' (Join-Path 'release' 'uefi_chain.efi')))

$all_binaries = @($uefi_chain)
foreach ($s in $stages) {
    $binName = $s -replace '_', '-'
    $td = Join-Path $target (Join-Path 'faggin' $s)
    $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($binName + '.exe')))
    if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $binName)) }
    $all_binaries += $bin
}
$kernel = Join-Path $target (Join-Path 'kernel' (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe')))
if (-not (Test-Path $kernel)) { $kernel = Join-Path $target (Join-Path 'kernel' (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel'))) }
$all_binaries += $kernel

Write-Host ''
Write-Host '  bin                         raw      flat     address' -ForegroundColor White
Write-Host '  --------------------------  -------  -------  ----------' -ForegroundColor DarkGray
$total = 0
$base = 0x100000
foreach ($f in $all_binaries) {
    if (-not (Test-Path $f)) { Fail ('Not found: ' + $f) }
    $raw = (Get-Item $f).Length
    $name = Split-Path $f -Leaf
    if ($name -eq 'uefi_chain.efi') {
        $line = ('  {0,-25} {1,6} B             0xFE000000 (UEFI load)' -f $name, $raw)
    } else {
        # Find the matching .bin in staging/ (if already there) or in target/
        $flat_name = ($name -replace '\.exe$', '') + '.bin'
        $flat = Join-Path $stage $flat_name
        if (-not (Test-Path $flat)) { $flat = $null }
        $fsz = if ($flat) { (Get-Item $flat).Length } else { 0 }
        $line = ('  {0,-25} {1,6} B   {2,5} B   0x{3:X6}' -f $name, $raw, $fsz, $base)
        $base += 0x10000
        $total += $fsz
    }
    Write-Host $line
}
Write-Host '  --------------------------  -------  -------' -ForegroundColor DarkGray
Write-Host ('  TOTAL flat size:           {0,6} B' -f $total) -ForegroundColor Yellow
Write-Host ''

# ── Stage to ESP layout ───────────────────────────────────────────
Step ('Staging to ' + $stage)
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'modules') -Force | Out-Null

Copy-Item $uefi_chain (Join-Path $stage 'BOOTX64.EFI')

$llvmObjcopy = Get-ChildItem -Path 'C:\Users\andre\.rustup' -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if ($llvmObjcopy) {
    foreach ($s in $stages) {
        $bn = $s -replace '_', '-'
        $td = Join-Path $target (Join-Path 'faggin' $s)
        $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($bn + '.exe')))
        if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $bn)) }
        $out = Join-Path $stage ($s + '.bin')
        & $llvmObjcopy -O binary $bin $out
        if ($LASTEXITCODE -ne 0) { Fail ('objcopy failed for ' + $s) }
    }
    & $llvmObjcopy -O binary $kernel (Join-Path $stage 'kernel.bin')
    if ($LASTEXITCODE -ne 0) { Fail 'objcopy failed for kernel' }
} else {
    Write-Host '  [WARN] llvm-objcopy not found - copying ELF files as-is' -ForegroundColor Yellow
    foreach ($s in $stages) {
        $bn = $s -replace '_', '-'
        $td = Join-Path $target (Join-Path 'faggin' $s)
        $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($bn + '.exe')))
        if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $bn)) }
        Copy-Item $bin (Join-Path $stage ($s + '.bin'))
    }
    Copy-Item $kernel (Join-Path $stage 'kernel.bin')
}

Write-Host ''
Write-Host '  === BUILD COMPLETE ===' -ForegroundColor Green
Write-Host ('  Staged to: ' + $stage) -ForegroundColor White
Write-Host ''
Write-Host '  12 faggin stages + kernel (Ring 0)' -ForegroundColor Cyan
Write-Host '  Each stage loads at 0x100000 + (i-1)*0x10000' -ForegroundColor Cyan
Write-Host ''

if ($BuildOnly) { exit 0 }

# ── Flash ────────────────────────────────────────────────────────
if ($Flash) {
    $targetLetter = $Drive.TrimEnd([char]':',[char]'\').ToUpper()
    $targetRoot = ($targetLetter + ':\')
    if (-not (Test-Path $targetRoot)) { Fail ('Drive ' + $targetLetter + ' not found') }

    if (-not $Yes) {
        $c = Read-Host ('  Flash to ' + $targetLetter + ':? (YES)')
        if ($c -ne 'YES') { Write-Host '  Aborted.'; exit 0 }
    }

    Step ('Flashing to ' + $targetLetter + ':\EFI\BOOT')
    $efiDest = Join-Path $targetRoot (Join-Path 'EFI' 'BOOT')
    New-Item -ItemType Directory -Path $efiDest -Force | Out-Null
    Copy-Item (Join-Path $stage 'BOOTX64.EFI') -Destination (Join-Path $efiDest 'BOOTX64.EFI') -Force
    foreach ($s in $stages) {
        Copy-Item (Join-Path $stage ($s + '.bin')) -Destination (Join-Path $efiDest ($s + '.bin')) -Force
    }
    Copy-Item (Join-Path $stage 'kernel.bin') -Destination (Join-Path $efiDest 'kernel.bin') -Force

    try {
        $all_files = @('BOOTX64.EFI', 'kernel.bin')
        foreach ($s in $stages) { $all_files += ($s + '.bin') }
        foreach ($f in $all_files) {
            $fs = [System.IO.File]::Open(($targetRoot + 'EFI\BOOT\' + $f), 'Open', 'Write')
            $fs.Flush(1); $fs.Close()
        }
    } catch { Write-Host ('  [WARN] Flush failed: ' + $_) -ForegroundColor Yellow }

    Write-Host ''
    Write-Host '  === FLASH OK ===' -ForegroundColor Green
    Write-Host '  Reboot from SSD to test.' -ForegroundColor White
    Write-Host ''
}

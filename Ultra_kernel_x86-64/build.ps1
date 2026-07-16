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
Write-Host '  === BMO Ultra Kernel x86-64 Build (UEFI 5 layers + 2 stages + kernel) ===' -ForegroundColor Magenta
Write-Host ''

# ── Build uefi_chain (5 UEFI layers) ──────────────────────────────
# uefi_chain is its own single-crate workspace at Ultra_kernel_x86-64/uefi_chain/.
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

# ── Build the 2 consolidated stages ───────────────────────────────
$stages = @('s1_cpu', 's2_mem')
$idx = 0
foreach ($s in $stages) {
    Step (('[{0,2}/2] Building stage ' -f ($idx + 1)) + $s + '...')
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
        # Cargo does not track linker.ld. Include its content hash in a harmless
        # linker symbol so script-only changes invalidate just the final crate.
        $linkerHash = (Get-FileHash (Join-Path $stageDir 'linker.ld') -Algorithm SHA256).Hash.Substring(0, 16)
        $out = cargo +nightly rustc --release --target x86_64-unknown-none --target-dir $td -- `
            -C ("link-arg=--defsym=BMO_LINKER_REV_" + $linkerHash + '=0') 2>&1
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
    $linkerHash = (Get-FileHash (Join-Path $stageDir 'linker.ld') -Algorithm SHA256).Hash.Substring(0, 16)
    $out = cargo +nightly rustc --release --target x86_64-unknown-none --target-dir $kd -- `
        -C ("link-arg=--defsym=BMO_LINKER_REV_" + $linkerHash + '=0') 2>&1
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
# ESP layout:
#   $stage\BOOTX64.EFI              (UEFI spec, must be here)
#   $stage\ring0\faggin\s*.bin      (12 pre-kernel stages)
#   $stage\ring0\kernel.bin         (Ring 0 base)
#   $stage\ring3\                   (Ring 3 userland — empty for now)
Step ('Staging to ' + $stage)
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'ring0\faggin') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'ring0') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'ring3\services') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'ring3\drivers') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $stage 'ring3\apps') -Force | Out-Null
# Marker files so empty dirs survive git/flash
'# Reserved for future BEF modules. See Ultra_userspace/ for source.' `
    | Set-Content -LiteralPath (Join-Path $stage 'ring3\README.txt')

Copy-Item $uefi_chain (Join-Path $stage 'BOOTX64.EFI')

$llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopy) { $llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup\toolchains" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName }
if ($llvmObjcopy) {
    foreach ($s in $stages) {
        $bn = $s -replace '_', '-'
        $td = Join-Path $target (Join-Path 'faggin' $s)
        $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($bn + '.exe')))
        if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $bn)) }
        $out = Join-Path $stage (Join-Path 'ring0\faggin' ($s + '.bin'))
        & $llvmObjcopy -O binary $bin $out
        if ($LASTEXITCODE -ne 0) { Fail ('objcopy failed for ' + $s) }
    }
    & $llvmObjcopy -O binary $kernel (Join-Path $stage 'ring0\kernel.bin')
    if ($LASTEXITCODE -ne 0) { Fail 'objcopy failed for kernel' }

    # Flat binaries have no ELF entry metadata: the entry symbol itself must
    # be the first byte at the fixed load address.
    $llvmNm = Join-Path (Split-Path $llvmObjcopy -Parent) 'llvm-nm.exe'
    if (-not (Test-Path $llvmNm)) { Fail 'llvm-nm not found; cannot validate flat-binary entry points' }
    $entryChecks = @(
        @{ File = $all_binaries[1]; Symbol = 's1_entry'; Expected = [uint64]0x100000 },
        @{ File = $all_binaries[2]; Symbol = '_start';   Expected = [uint64]0x200000 },
        @{ File = $kernel;          Symbol = '_start';   Expected = [uint64]0x400000 }
    )
    foreach ($check in $entryChecks) {
        $pattern = '^[0-9A-Fa-f]+\s+\S\s+' + [regex]::Escape($check.Symbol) + '$'
        $line = & $llvmNm -n $check.File 2>&1 | Where-Object { $_ -match $pattern } | Select-Object -First 1
        if (-not $line) { Fail ('entry symbol not found: ' + $check.Symbol + ' in ' + $check.File) }
        $actual = [Convert]::ToUInt64(($line -split '\s+')[0], 16)
        if ($actual -ne $check.Expected) {
            Fail ('flat entry mismatch for ' + $check.Symbol + ': expected 0x{0:X}, got 0x{1:X}' -f $check.Expected, $actual)
        }
    }
    $stackChecks = @(
        @{ File = $all_binaries[2]; Symbol = 'S2_STACK_END' },
        @{ File = $kernel;          Symbol = 'KERNEL_STACK_END_MARKER' }
    )
    foreach ($check in $stackChecks) {
        $pattern = '^[0-9A-Fa-f]+\s+\S\s+' + [regex]::Escape($check.Symbol) + '$'
        $line = & $llvmNm -n $check.File 2>&1 | Where-Object { $_ -match $pattern } | Select-Object -First 1
        if (-not $line) { Fail ('stack symbol not found: ' + $check.Symbol) }
        $address = [Convert]::ToUInt64(($line -split '\s+')[0], 16)
        if (($address -band 0xF) -ne 0) { Fail ('stack is not 16-byte aligned: ' + $check.Symbol) }
    }
} else {
    Fail 'llvm-objcopy not found; the boot chain requires flat binaries and cannot load ELF files'
}

Write-Host ''
Write-Host '  === BUILD COMPLETE ===' -ForegroundColor Green
Write-Host ('  Staged to: ' + $stage) -ForegroundColor White
Write-Host ''
Write-Host '  2 stages + kernel (Ring 0)' -ForegroundColor Cyan
Write-Host '  s1_cpu@0x100000 s2_mem@0x200000 kernel@0x400000' -ForegroundColor Cyan
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
    # Wipe the destination's old files (we are changing layout).
    if (Test-Path $efiDest) {
        Get-ChildItem -LiteralPath $efiDest -Force | Where-Object { -not $_.PSIsContainer } | Remove-Item -Force
    }
    # Mirror the staging tree's directory structure onto the target.
    $ring0Dest = Join-Path $efiDest 'ring0'
    $fagginDest = Join-Path $ring0Dest 'faggin'
    $ring3Dest = Join-Path $efiDest 'ring3'
    New-Item -ItemType Directory -Path (Join-Path $ring3Dest 'services') -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $ring3Dest 'drivers')  -Force | Out-Null
    New-Item -ItemType Directory -Path (Join-Path $ring3Dest 'apps')     -Force | Out-Null
    New-Item -ItemType Directory -Path $fagginDest -Force | Out-Null
    # Mark the ring3 dir as reserved (in case it's empty after wipe).
    if (-not (Test-Path (Join-Path $ring3Dest 'README.txt'))) {
        '# Reserved for future BEF modules. See Ultra_userspace/ for source.' `
            | Set-Content -LiteralPath (Join-Path $ring3Dest 'README.txt')
    }

    Copy-Item (Join-Path $stage 'BOOTX64.EFI') -Destination (Join-Path $efiDest 'BOOTX64.EFI') -Force
    foreach ($s in $stages) {
        Copy-Item (Join-Path $stage (Join-Path 'ring0\faggin' ($s + '.bin'))) `
                   -Destination (Join-Path $fagginDest ($s + '.bin')) -Force
    }
    Copy-Item (Join-Path $stage 'ring0\kernel.bin') -Destination (Join-Path $ring0Dest 'kernel.bin') -Force

    try {
        $all_files = @('BOOTX64.EFI', 'ring0\kernel.bin')
        foreach ($s in $stages) { $all_files += ('ring0\faggin\' + $s + '.bin') }
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

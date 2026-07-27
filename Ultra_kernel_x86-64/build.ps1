param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$Verify,
    [switch]$BuildOnly,
    [string]$Drive = 'D',
    [switch]$Yes
)

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }

function Step { param($m) Write-Host ('  => ' + $m) -ForegroundColor Cyan }
function Fail { param($m) Write-Host ('  [X] ' + $m) -ForegroundColor Red; exit 1 }
function Hash256 { param($p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }

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

# Keep the no-alloc Ring 0 syscall view synchronized with canonical bmo-abi.
Step 'Validating Ring 0 syscall contract'
$kernelSyscalls = Get-Content (Join-Path $root 'kernel\src\ring0\syscall.rs') -Raw
$abiSurface = Get-Content (Join-Path $root '..\platform\abi\bmo-abi\src\syscalls\surface.rs') -Raw
foreach ($name in @('NR_INVOKE', 'NR_CHANNEL_KICK', 'NR_WAIT')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface syscall contract mismatch: ' + $name)
    }
}
foreach ($name in @('CURRENT_TASK', 'TASK_OP_GET_PID', 'TASK_OP_GET_TID', 'TASK_OP_YIELD', 'TASK_OP_EXIT', 'TASK_OP_CHANNEL_OPEN', 'TASK_OP_CONSOLE_WRITE', 'CHANNEL_OP_GET_SEQ', 'CHANNEL_OP_GET_INDEX')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface operation contract mismatch: ' + $name)
    }
}
# The kernel's capability-engine mirror (cap.rs) must match bmo-abi too:
# handle-kind codes and rights bits are part of the frozen contract.
$kernelCap = Get-Content (Join-Path $root 'kernel\src\ring0\cap.rs') -Raw
if (-not ($kernelCap -match 'KIND_CHANNEL:\s*u8\s*=\s*0x60')) {
    Fail 'capability contract mismatch: KIND_CHANNEL must be 0x60 (bmo-abi HandleKind::Channel)'
}
$abiKind = Get-Content (Join-Path $root '..\platform\abi\bmo-abi\src\fundamentals\handle\kind.rs') -Raw
if (-not ($abiKind -match 'Channel\s*=\s*0x60')) {
    Fail 'capability contract mismatch: bmo-abi HandleKind::Channel must be 0x60'
}

# NOTE: uefi_chain is now the UNIFIED shim — it embeds the flat binaries
# of s1_cpu, s2_mem and the kernel via include_bytes!, so it is built
# AFTER them (see 'Building unified uefi_chain' below). Rationale: some
# firmwares (MSI A320M AMI fast path) never bind SimpleFileSystem, so
# the boot chain cannot read the ESP through UEFI protocols at all.

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

# ── Build Ring 3 userspace (Ultra_userspace/) ─────────────────────
#
# VA ANTES DEL KERNEL a proposito: el kernel embebe el .bex resultante con
# `include_bytes!`, asi que el archivo tiene que existir y estar al dia cuando
# el kernel compile. Si esto fallara en silencio, el kernel arrancaria con el
# compositor de la vez anterior y nadie se enteraria.
#
# Ultra_userspace/ es su PROPIO workspace: se compila a x86_64-unknown-none con
# su guion de enlazado, que fija la base en USER_IMAGE_BASE. `bex-link` traduce
# el ELF a un contenedor BEF y comprueba, seccion por seccion, que las
# direcciones que escribio el enlazador son las que el kernel va a mapear.
Step 'Building Ring 3 userspace (compositor)...'
$usDir = Join-Path (Split-Path -Parent $root) 'Ultra_userspace'
if (-not (Test-Path $usDir)) { Fail 'Ultra_userspace/ no existe' }
Push-Location $usDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $out = cargo +nightly build -p bmo-service-gui --release --target x86_64-unknown-none 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [userspace] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'userspace build failed' }
} finally { Pop-Location }

$compositorElf = Join-Path $usDir 'target\x86_64-unknown-none\release\compositor'
if (-not (Test-Path $compositorElf)) { Fail 'no salio el ELF del compositor' }
$compositorBex = Join-Path $root 'kernel\src\ring0\compositor.bex'
Push-Location (Split-Path -Parent $root)
try {
    if (Test-Path $compositorBex) { Remove-Item $compositorBex -Force }
    $out = cargo run -p bmo-bex-link --quiet -- $compositorElf $compositorBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|\.rodata|\.data|\.bss|entrada|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link failed' }
    # Se borro antes a proposito: si `bex-link` no lo ha vuelto a escribir, el
    # kernel embeberia el compositor de la vez anterior y el build mentiria.
    if (-not (Test-Path $compositorBex)) { Fail 'bex-link no produjo compositor.bex' }
} finally { Pop-Location }

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

# ── Embed payloads + build unified uefi_chain ─────────────────────
Step 'Preparing embedded payloads'
$embedDir = Join-Path $target 'embed'
New-Item -ItemType Directory -Path $embedDir -Force | Out-Null
$llvmObjcopyEmbed = Get-ChildItem -Path "$env:USERPROFILE\.rustup" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopyEmbed) { Fail 'llvm-objcopy not found for embedded payloads' }
$embedMap = @{}
foreach ($s in $stages) {
    $bn = $s -replace '_', '-'
    $td = Join-Path $target (Join-Path 'faggin' $s)
    $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' ($bn + '.exe')))
    if (-not (Test-Path $bin)) { $bin = Join-Path $td (Join-Path 'x86_64-unknown-none' (Join-Path 'release' $bn)) }
    $outBin = Join-Path $embedDir ($s + '.bin')
    & $llvmObjcopyEmbed -O binary $bin $outBin
    if ($LASTEXITCODE -ne 0) { Fail ('objcopy (embed) failed for ' + $s) }
    $embedMap[$s] = $outBin
}
$kernelElf = Join-Path $target (Join-Path 'kernel' (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe')))
if (-not (Test-Path $kernelElf)) { $kernelElf = Join-Path $target (Join-Path 'kernel' (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel'))) }
$kernelEmbed = Join-Path $embedDir 'kernel.bin'
& $llvmObjcopyEmbed -O binary $kernelElf $kernelEmbed
if ($LASTEXITCODE -ne 0) { Fail 'objcopy (embed) failed for kernel' }

Step 'Building unified uefi_chain (embedded stages)...'
$uefiDir = Join-Path $root 'uefi_chain'
Push-Location $uefiDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $env:BMO_S1_BIN = $embedMap['s1_cpu']
    $env:BMO_S2_BIN = $embedMap['s2_mem']
    $env:BMO_KERNEL_BIN = $kernelEmbed
    $uefiTarget = Join-Path $target 'uefi_chain'
    $out = cargo +nightly build --release --target x86_64-unknown-uefi --target-dir $uefiTarget 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [uefi_chain] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'uefi_chain build failed' }
} finally {
    Pop-Location
    Remove-Item Env:\BMO_S1_BIN, Env:\BMO_S2_BIN, Env:\BMO_KERNEL_BIN -ErrorAction SilentlyContinue
}

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
Write-Host '  bin                         linked size  load address' -ForegroundColor White
Write-Host '  --------------------------  -----------  ------------' -ForegroundColor DarkGray
$loadAddresses = @($null, [uint64]0x100000, [uint64]0x200000, [uint64]0x400000)
for ($binaryIndex = 0; $binaryIndex -lt $all_binaries.Count; $binaryIndex++) {
    $f = $all_binaries[$binaryIndex]
    if (-not (Test-Path $f)) { Fail ('Not found: ' + $f) }
    $raw = (Get-Item $f).Length
    $name = Split-Path $f -Leaf
    if ($name -eq 'uefi_chain.efi') {
        $line = ('  {0,-25} {1,9} B  firmware managed' -f $name, $raw)
    } else {
        $line = ('  {0,-25} {1,9} B  0x{2:X6}' -f $name, $raw, $loadAddresses[$binaryIndex])
    }
    Write-Host $line
}
Write-Host '  --------------------------  -----------  ------------' -ForegroundColor DarkGray
Write-Host ''

# ── Stage to ESP layout ───────────────────────────────────────────
# ESP layout — UNA SOLA COSA:
#   $stage\BOOTX64.EFI        el shim unificado; lleva s1_cpu, s2_mem y el
#                             kernel EMBEBIDOS (include_bytes!), asi que es
#                             todo el sistema en un archivo
#   $stage\BMO-MANIFEST.TXT   revision + hash de lo desplegado
#
# Antes tambien se copiaban ring0\kernel.bin y ring0\faggin\s*.bin: los
# MISMOS binarios que el shim ya lleva dentro, ~575 KB de duplicado exacto.
# Eran el camino de respaldo por sistema de ficheros, que dejo de existir el
# dia del "EFI unificado" (esta placa no tiene driver FAT UEFI que conectar,
# por eso se embebio todo). Nadie los leia. Se fueron.
Step ('Staging to ' + $stage)
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Path $stage -Force | Out-Null

Copy-Item $uefi_chain (Join-Path $stage 'BOOTX64.EFI')

$llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName
if (-not $llvmObjcopy) { $llvmObjcopy = Get-ChildItem -Path "$env:USERPROFILE\.rustup\toolchains" -Filter 'llvm-objcopy.exe' -Recurse | Select-Object -First 1 -ExpandProperty FullName }
if ($llvmObjcopy) {
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

# Un solo archivo desplegado, un solo hash que verificar.
$deployFiles = @('BOOTX64.EFI')
$gitRevision = (& git -C $root rev-parse --short=12 HEAD 2>$null)
if (-not $gitRevision) { $gitRevision = 'unknown' }
$manifestLines = @(
    'BMO UEFI deployment manifest v1',
    ('revision=' + $gitRevision),
    'architecture=x86_64',
    'boot_chain=uefi_chain,s1_cpu,s2_mem,kernel'
)
foreach ($relativePath in $deployFiles) {
    $sourcePath = Join-Path $stage $relativePath
    $manifestLines += ('file={0}|bytes={1}|sha256={2}' -f `
        $relativePath, (Get-Item -LiteralPath $sourcePath).Length, (Hash256 $sourcePath))
}
$manifestPath = Join-Path $stage 'BMO-MANIFEST.TXT'
$manifestLines | Set-Content -LiteralPath $manifestPath -Encoding Ascii
$deployFiles += 'BMO-MANIFEST.TXT'

Write-Host ''
Write-Host '  === BUILD COMPLETE ===' -ForegroundColor Green
Write-Host ('  Staged to: ' + $stage) -ForegroundColor White
Write-Host ''
Write-Host '  2 stages + kernel (Ring 0)' -ForegroundColor Cyan
Write-Host '  s1_cpu@0x100000 s2_mem@0x200000 kernel@0x400000' -ForegroundColor Cyan
Write-Host ''

if ($BuildOnly) { exit 0 }

# ── Flash ────────────────────────────────────────────────────────
if ($Flash -or $Verify) {
    $targetLetter = $Drive.TrimEnd([char]':',[char]'\').ToUpper()
    if ($targetLetter -notmatch '^[A-Z]$') { Fail ('Invalid drive letter: ' + $Drive) }
    $systemLetter = $env:SystemDrive.TrimEnd([char]':',[char]'\').ToUpper()
    if ($targetLetter -eq $systemLetter) { Fail 'Refusing to deploy BMO onto the Windows system volume' }
    $targetRoot = ($targetLetter + ':\')
    if (-not (Test-Path $targetRoot)) { Fail ('Drive ' + $targetLetter + ' not found') }

    $volume = Get-Volume -DriveLetter $targetLetter -ErrorAction SilentlyContinue
    if ($volume) {
        $sizeGiB = [math]::Round($volume.Size / 1GB, 1)
        Write-Host ('  Target: {0}: label="{1}" filesystem={2} size={3} GiB' -f `
            $targetLetter, $volume.FileSystemLabel, $volume.FileSystem, $sizeGiB) -ForegroundColor Yellow
        if ($volume.FileSystem -notin @('FAT', 'FAT32')) {
            Fail ('UEFI boot requires a FAT/FAT32 ESP; ' + $targetLetter + ': is ' + $volume.FileSystem)
        }
    }

    $efiDest = Join-Path $targetRoot (Join-Path 'EFI' 'BOOT')
    if ($Flash) {
        if (-not $Yes) {
            $expected = 'FLASH ' + $targetLetter + ' BMO'
            $confirmation = Read-Host ('  Type "' + $expected + '" to update Ring 0')
            if ($confirmation -ne $expected) { Write-Host '  Aborted.'; exit 0 }
        }

        Step ('Deploying Ring 0 to ' + $targetLetter + ':\EFI\BOOT')
        New-Item -ItemType Directory -Path $efiDest -Force | Out-Null
        $nextDest = Join-Path $efiDest ('.bmo-next-' + $PID)
        if (Test-Path $nextDest) { Remove-Item -LiteralPath $nextDest -Recurse -Force }
        New-Item -ItemType Directory -Path $nextDest -Force | Out-Null
        Copy-Item (Join-Path $stage '*') -Destination $nextDest -Recurse -Force

        foreach ($relativePath in $deployFiles) {
            $expectedHash = Hash256 (Join-Path $stage $relativePath)
            $nextPath = Join-Path $nextDest $relativePath
            if (-not (Test-Path -LiteralPath $nextPath) -or (Hash256 $nextPath) -ne $expectedHash) {
                Remove-Item -LiteralPath $nextDest -Recurse -Force -ErrorAction SilentlyContinue
                Fail ('Staged SSD copy failed verification: ' + $relativePath)
            }
        }

        # LIMPIEZA del destino: los subarboles ring0\ y ring3\ que dejaban las
        # versiones anteriores ya no se generan (eran el duplicado de lo que
        # BOOTX64.EFI lleva embebido). Si siguen en el disco, se borran: un
        # deploy tiene que dejar el destino como el build, no acumular restos
        # de builds pasados que nadie lee pero que confunden al mirarlos.
        foreach ($stale in @('ring0', 'ring3')) {
            $stalePath = Join-Path $efiDest $stale
            if (Test-Path $stalePath) {
                Write-Host ('    limpiando resto de deploys viejos: ' + $stale + '\') -ForegroundColor DarkYellow
                Remove-Item -LiteralPath $stalePath -Recurse -Force
            }
        }
        Copy-Item -LiteralPath (Join-Path $nextDest 'BOOTX64.EFI') -Destination (Join-Path $efiDest 'BOOTX64.EFI') -Force
        Copy-Item -LiteralPath (Join-Path $nextDest 'BMO-MANIFEST.TXT') -Destination (Join-Path $efiDest 'BMO-MANIFEST.TXT') -Force
        Remove-Item -LiteralPath $nextDest -Recurse -Force
    }

    Step ('Verifying ' + $targetLetter + ':\EFI\BOOT against the current build')
    foreach ($relativePath in $deployFiles) {
        $sourcePath = Join-Path $stage $relativePath
        $destinationPath = Join-Path $efiDest $relativePath
        if (-not (Test-Path -LiteralPath $destinationPath)) { Fail ('Missing on SSD: ' + $relativePath) }
        if ((Get-Item -LiteralPath $destinationPath).Length -ne (Get-Item -LiteralPath $sourcePath).Length) {
            Fail ('Size mismatch on SSD: ' + $relativePath)
        }
        if ((Hash256 $destinationPath) -ne (Hash256 $sourcePath)) { Fail ('SHA-256 mismatch on SSD: ' + $relativePath) }
        Write-Host ('    SHA-256 OK  ' + $relativePath) -ForegroundColor Green
    }

    Write-Host ''
    Write-Host '  === BMO RING 0 SSD VERIFIED ===' -ForegroundColor Green
    if ($Flash) { Write-Host '  Reboot from the SSD to test the new kernel.' -ForegroundColor White }
    Write-Host ''
}

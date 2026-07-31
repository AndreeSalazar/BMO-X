param(
    [switch]$Clean,
    [switch]$Flash,
    [switch]$Verify,
    [switch]$BuildOnly,
    [string]$Drive = 'D',
    # Letra del volumen de DATOS (BMO-DATA) al que copiar los programas de
    # Ring 3. Vacio = no se toca ningun disco, que es el valor por defecto y la
    # postura de este build: escribir en discos esta cerrado salvo que se pida.
    [string]$Data = '',
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
# Las operaciones del kernel no viven todas en `syscall.rs`: las de un objeto
# estan con su objeto (`ARCH_OP_*` en `obj\archivo.rs`), que es donde deben
# estar. Se leen los dos y se comparan contra el MISMO surface.
$kernelSyscalls = (Get-Content (Join-Path $root 'kernel\src\ring0\syscall.rs') -Raw) + "`n" +
                  (Get-Content (Join-Path $root 'kernel\src\ring0\obj\archivo.rs') -Raw)
$abiSurface = Get-Content (Join-Path $root '..\platform\abi\bmo-abi\src\syscalls\surface.rs') -Raw
foreach ($name in @('NR_INVOKE', 'NR_CHANNEL_KICK', 'NR_WAIT')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface syscall contract mismatch: ' + $name)
    }
}
# La lista lleva TODAS las operaciones, no solo las seis primeras. Se habia
# quedado congelada en las del principio, asi que las que se anadieron despues
# —EJECUTAR, CONSOLA_CREAR, DIR_ABRIR, las de archivo, REINICIAR e INFO— se
# escribian en el kernel y nadie comprobaba que coincidieran con el ABI. Un
# guardian que solo mira la mitad da una tranquilidad que no ha ganado.
foreach ($name in @('CURRENT_TASK', 'TASK_OP_GET_PID', 'TASK_OP_GET_TID', 'TASK_OP_YIELD', 'TASK_OP_EXIT', 'TASK_OP_CHANNEL_OPEN', 'TASK_OP_CONSOLE_WRITE', 'TASK_OP_ENDPOINT_CREATE', 'TASK_OP_RUTA', 'TASK_OP_EJECUTAR', 'TASK_OP_CONSOLA_CREAR', 'TASK_OP_DIR_ABRIR', 'TASK_OP_CONSOLE_READ', 'TASK_OP_ARCHIVO_ABRIR', 'TASK_OP_ARCHIVO_CREAR', 'TASK_OP_REINICIAR', 'TASK_OP_INFO', 'TASK_OP_INFO_TEXTO', 'ARCH_OP_LEER', 'ARCH_OP_ESCRIBIR', 'ARCH_OP_TAMANO', 'ARCH_OP_CERRAR', 'ARCH_OP_LEER_LINEA', 'CHANNEL_OP_GET_SEQ', 'CHANNEL_OP_GET_INDEX')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface operation contract mismatch: ' + $name)
    }
}
# The kernel's capability-engine mirror (cap.rs) must match bmo-abi too:
# handle-kind codes and rights bits are part of the frozen contract.
$kernelCap = Get-Content (Join-Path $root 'kernel\src\ring0\obj\cap.rs') -Raw
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
# ★ El kernel YA NO EMBEBE el compositor. Antes lo metia con `include_bytes!` y
# este paso tenia que ir antes del kernel para que el blob estuviera al dia;
# ademas dejaba un binario de 24 KiB dentro de kernel/src/ que se reescribia en
# cada build y ensuciaba el repositorio en cada commit.
#
# Ahora sale a staging\BMO-DATA\apps\ y se COPIA al volumen de datos, igual que
# cualquier otro programa. El kernel lo arranca con `lanzar::ruta` despues de
# montar el disco (ver `phase::arrancar_escritorio`). Cambiar el escritorio ya
# no obliga a recompilar Ring 0.
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
# El .bex sale a staging\BMO-DATA\apps\, que es el espejo de lo que hay que
# copiar al volumen de datos. La ruta de dentro (apps\gui.bex) tiene que cuadrar
# con `RUTA_COMPOSITOR` de phase.rs: es el contrato entre el build y el arranque.
#
# ★ `gui.bex` y no `compositor.bex`: el driver FAT32 del kernel es 8.3 y se
# NIEGA a recortar nombres (un nombre recortado abre otro archivo, y en un
# cargador de programas eso es ejecutar otro binario). `compositor` son diez
# caracteres y no cabe en los ocho del campo.
# ── El volumen de datos, POR CATEGORIAS ───────────────────────────
#
# Antes todo caia en un solo `apps\`: los siete .bex de COBOL, los de C, el de
# Ada, el compositor y los .txt de entrada, revueltos. Un `ls` daba diecisiete
# lineas sin orden, y para lanzar algo habia que acordarse del nombre exacto.
#
# La primera division es **programa o dato**; dentro de los programas, por quien
# los compila:
#
#     sys\     el sistema: lo que arranca solo (gui.bex)
#     cobol\  c\  ada\      los ejemplos, por lenguaje
#     datos\   lo que los programas LEEN y ESCRIBEN
#
# ★ Y se teclea MENOS que antes: `cobol/banco.bex` es mas corto que
#   `apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.
#
# Los nombres de carpeta tambien son 8.3: el driver FAT32 del kernel se NIEGA a
# recortar, y una carpeta recortada manda a otro sitio igual que un fichero.
$dataBase = Join-Path $root 'staging\BMO-DATA'
foreach ($d in @('sys', 'cobol', 'c', 'ada', 'datos')) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase $d) -Force | Out-Null
}
$compositorBex = Join-Path $dataBase 'sys\gui.bex'
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
    # Se borro antes a proposito: si `bex-link` no lo ha vuelto a escribir, se
    # copiaria al disco el compositor de la vez anterior y el build mentiria.
    if (-not (Test-Path $compositorBex)) { Fail 'bex-link no produjo gui.bex' }
} finally { Pop-Location }

# ── Programas COBOL de ejemplo ───────────────────────────────────
#
# Se compilan AQUI y salen al mismo staging que el compositor. Antes se
# generaban a mano dentro de toolchain/lang/cobol/examples/ y nunca llegaban al
# disco: `run apps/extracto.bex` contestaba "no esta: revisa la ruta" y parecia
# un fallo del cargador cuando el archivo sencillamente no se habia copiado.
#
# El nombre destino se recorta a 8.3 a proposito y de forma explicita: el
# driver FAT32 del kernel se NIEGA a recortar (un nombre recortado abre otro
# archivo), asi que el que no quepa se dice aqui y no en el arranque.
Step 'Building COBOL example programs...'
# Las rutas llevan el NIVEL delante: los ejemplos estan en escalera (1-basico,
# 2-decimal, 3-presentacion, 4-ficheros, 5-tablas), ordenados por cuanto COBOL
# hace falta que el compilador sepa. Ver examples\README.md.
$cobolEjemplos = @(
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\banco.cob';        out = 'banco.bex'    ; dir = 'cobol' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calc.cob';         out = 'calc.bex'     ; dir = 'cobol' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calcgui.cob';      out = 'calcgui.bex'  ; dir = 'cobol' },
    @{ src = 'toolchain\lang\cobol\examples\3-presentacion\extracto.cob'; out = 'extracto.bex' ; dir = 'cobol' },
    @{ src = 'toolchain\lang\cobol\examples\4-ficheros\batch.cob';       out = 'batch.bex'    ; dir = 'cobol' },
    # `conceptos` son nueve letras y el driver FAT32 se NIEGA a recortar, asi
    # que el destino es `concep`. La comprobacion de 8.3 de abajo lo cazaria
    # igual, pero mejor no llegar a que la cace.
    @{ src = 'toolchain\lang\cobol\examples\5-tablas\conceptos.cob';     out = 'concep.bex'   ; dir = 'cobol' },
    @{ src = 'toolchain\lang\cobol\examples\6-condiciones\cartera.cob';  out = 'carter.bex'   ; dir = 'cobol' }
)
# ── Programas ADA de ejemplo ─────────────────────────────────────
#
# Crate PROPIO (`bmo-ada-front`), sin dependencia de los otros frontends. El
# decimal exacto es el mismo que el de COBOL y no por copia: el Annex F de Ada
# copio las reglas de COBOL, asi que dos lenguajes que dicen lo mismo acaban en
# la misma aritmetica de enteros escalados.
$adaEjemplos = @(
    @{ src = 'toolchain\lang\ada\examples\1-basico\cierre.adb'; out = 'cierre.bex' ; dir = 'ada' }
)
# ── Programas C de ejemplo ───────────────────────────────────────
#
# ★ Este paso NO EXISTIA. COBOL y Ada llegaban al disco y C no, asi que los
# ejemplos de C solo se podian ejecutar si alguien los embebia a mano en el
# kernel — y `scroll_C.bex` no llegaba al Kingston por eso, no por un fallo del
# compilador. Un lenguaje que compila y cuyo binario no se despliega esta a
# medias.
#
# `hola_C.c` prueba lo basico (bucles, %d, resta con signo, switch, %s).
# `scroll_C.c` usa las cabeceras `<bmo/...>`: la puerta de syscalls desde C, la
# capability de entrada y el modelo de scroll.
$cEjemplos = @(
    @{ src = 'toolchain\lang\c\examples\hola_C.c';   out = 'holac.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\scroll_C.c'; out = 'scrollc.bex' ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\pregunta_C.c'; out = 'pregc.bex'  ; dir = 'c' }
)

$repo = Split-Path -Parent $root
Push-Location $repo
try {
    foreach ($e in $cobolEjemplos) {
        $tallo = [System.IO.Path]::GetFileNameWithoutExtension($e.out)
        if ($tallo.Length -gt 8) { Fail ($e.out + ': el tallo no cabe en 8.3') }
        $dst = Join-Path (Join-Path $dataBase $e.dir) $e.out
        $out = cargo run -p bmo-cobol-front --quiet -- (Join-Path $repo $e.src) -o $dst 2>&1
        $out | ForEach-Object {
            if ($_ -match 'ok:|error') { Write-Host ('    [cobol] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
        if (-not (Test-Path $dst)) { Fail ('no salio ' + $e.out) }
    }

    Step 'Building ADA example programs...'
    foreach ($e in $adaEjemplos) {
        $tallo = [System.IO.Path]::GetFileNameWithoutExtension($e.out)
        if ($tallo.Length -gt 8) { Fail ($e.out + ': el tallo no cabe en 8.3') }
        $dst = Join-Path (Join-Path $dataBase $e.dir) $e.out
        $out = cargo run -p bmo-ada-front --quiet -- (Join-Path $repo $e.src) -o $dst 2>&1
        $out | ForEach-Object {
            if ($_ -match 'ok:|error|linea') { Write-Host ('    [ada] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
        if (-not (Test-Path $dst)) { Fail ('no salio ' + $e.out) }
    }

    Step 'Building C example programs...'
    foreach ($e in $cEjemplos) {
        $tallo = [System.IO.Path]::GetFileNameWithoutExtension($e.out)
        if ($tallo.Length -gt 8) { Fail ($e.out + ': el tallo no cabe en 8.3') }
        $dst = Join-Path (Join-Path $dataBase $e.dir) $e.out
        # Sin --base ni --asm-path: ese camino usa el PREPROCESADOR, que es lo
        # que resuelve `#include <bmo/...>`. Con ellos se toma el camino de
        # modulos, que no lo llama.
        $out = cargo run -p bmo-c-front --quiet -- (Join-Path $repo $e.src) -o $dst 2>&1
        $out | ForEach-Object {
            if ($_ -match 'ok:|error') { Write-Host ('    [c] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
        if (-not (Test-Path $dst)) { Fail ('no salio ' + $e.out) }
    }

    # ── Los DATOS de los ejemplos ─────────────────────────────────
    #
    # ★ Este paso tampoco existia, y era peor que el de C: los .txt que leen
    # `batch`, `conceptos` y `cartera` vivian SOLO en staging\, que esta en el
    # .gitignore. O sea, no eran del repositorio. Un `-Clean` o un disco nuevo
    # los borraba y **no habia forma de regenerarlos**: los ejemplos de ficheros
    # quedaban sin entrada y sin nadie que supiera que debian contener.
    #
    # Ahora viven en toolchain\lang\cobol\examples\datos\ y se despliegan como
    # se despliega un .bex.
    Step 'Staging example data...'
    $datosSrc = Join-Path $repo 'toolchain\lang\cobol\examples\datos'
    $datosDst = Join-Path $dataBase 'datos'
    foreach ($d in (Get-ChildItem -LiteralPath $datosSrc -Filter '*.txt')) {
        Copy-Item -LiteralPath $d.FullName -Destination (Join-Path $datosDst $d.Name) -Force
        Write-Host ('    [datos] ' + $d.Name + ' (' + $d.Length + ' B)') -ForegroundColor DarkGray
    }
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

# ── Data: los programas de Ring 3 al volumen de datos ─────────────
#
# Separado de -Flash a proposito. Son dos discos distintos con dos riesgos
# distintos: -Flash toca la ESP de arranque, esto toca BMO-DATA. Que compartan
# bandera invitaria a escribir en uno cuando se queria el otro.
#
# ★ Este es el UNICO sitio del build que escribe fuera del arbol del proyecto.
# Por eso lleva tres cierres antes de copiar un byte: no puede ser el disco del
# sistema, tiene que ser FAT/FAT32, y hay que teclear la frase entera. El NVMe
# de esta maquina lleva un Windows que no es de BMO.
if ($Data) {
    $dataLetter = $Data.TrimEnd([char]':',[char]'\').ToUpper()
    if ($dataLetter.Length -ne 1) { Fail ('-Data espera UNA letra de unidad, no: ' + $Data) }
    $dataRoot = $dataLetter + ':\'

    # Cierre 1: nunca el disco del sistema. Es el que tiene el Windows del
    # dueno de la maquina, y una copia ahi no es un error recuperable.
    $sistema = ($env:SystemDrive).TrimEnd([char]':').ToUpper()
    if ($dataLetter -eq $sistema) {
        Fail ('NO: ' + $dataLetter + ': es el disco del sistema (' + $env:SystemDrive + '). BMO no escribe ahi.')
    }
    if (-not (Test-Path $dataRoot)) { Fail ('no existe la unidad ' + $dataRoot) }

    # Cierre 2: tiene que ser el tipo de volumen correcto, y se ENSENA cual es
    # antes de preguntar. Una confirmacion a ciegas no es una confirmacion.
    $dataVol = Get-Volume -DriveLetter $dataLetter -ErrorAction SilentlyContinue
    if ($dataVol) {
        $dSize = [math]::Round($dataVol.Size / 1GB, 1)
        Write-Host ''
        Write-Host ('  Destino de datos: {0}: label="{1}" filesystem={2} size={3} GiB' -f `
            $dataLetter, $dataVol.FileSystemLabel, $dataVol.FileSystem, $dSize) -ForegroundColor Yellow
        if ($dataVol.FileSystem -notin @('FAT', 'FAT32')) {
            Fail ('el volumen de datos de BMO es FAT32; ' + $dataLetter + ': es ' + $dataVol.FileSystem)
        }
    }

    # Cierre 3: la frase, igual que -Flash. Con la letra dentro, para que
    # copiar-pegar la de otra sesion no valga.
    if (-not $Yes) {
        $esperado = 'DATA ' + $dataLetter + ' BMO'
        $conf = Read-Host ('  Escribe "' + $esperado + '" para copiar los programas de Ring 3')
        if ($conf -ne $esperado) { Write-Host '  Abortado.'; exit 0 }
    }

    # ★ Ya no es una sola carpeta: van a sys\ cobol\ c\ ada\ datos\. El bucle de
    # abajo copia recursivo y crea los directorios que falten, asi que no hay
    # nada que cambiar aqui salvo el mensaje — pero **la vieja `apps\` del disco
    # NO se borra**: este deploy no borra nada que no haya puesto el. Hay que
    # quitarla a mano una vez, o quedan dos copias de cada programa.
    Step ('Copiando programas de Ring 3 a ' + $dataLetter + ':\  (sys, cobol, c, ada, datos)')
    $dataSrc = Join-Path $root 'staging\BMO-DATA'
    if (-not (Test-Path $dataSrc)) { Fail 'no hay staging\BMO-DATA (ejecuta el build primero)' }

    # Se copia SOLO lo que este build produjo, archivo a archivo. Nada de
    # borrar el destino: en BMO-DATA puede haber cosas que no salen de aqui, y
    # un deploy no tiene derecho a decidir sobre ellas.
    $copiados = 0
    foreach ($f in Get-ChildItem -Path $dataSrc -Recurse -File) {
        $rel = $f.FullName.Substring($dataSrc.Length).TrimStart([char]'\')
        $dst = Join-Path $dataRoot $rel
        New-Item -ItemType Directory -Path (Split-Path -Parent $dst) -Force | Out-Null
        Copy-Item -LiteralPath $f.FullName -Destination $dst -Force
        # Verificado por hash, como Ring 0. Un .bex a medio copiar no falla al
        # arrancar: falla en la admision BEX, y ese mensaje manda a buscar el
        # bug al compilador en vez de al cable.
        if ((Hash256 $dst) -ne (Hash256 $f.FullName)) { Fail ('copia corrupta: ' + $rel) }
        Write-Host ('    SHA-256 OK  ' + $rel) -ForegroundColor Green
        $copiados++
    }
    if ($copiados -eq 0) { Fail 'staging\BMO-DATA esta vacio' }

    Write-Host ''
    Write-Host ('  === BMO-DATA VERIFICADO (' + $copiados + ' archivos) ===') -ForegroundColor Green
    Write-Host ''
}

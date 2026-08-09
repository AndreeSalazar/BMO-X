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
    # * Las DOS mitades del despliegue a la misma unidad, que es el caso normal
    # en esta maquina. Equivale a `-Flash -Data <la misma letra de -Drive>`.
    #
    # Existe porque `-Flash` y `-Data` separados tienen una trampa silenciosa:
    # `-Flash` actualiza el ARRANQUE y `-Data` los PROGRAMAS, y quien olvida el
    # segundo arranca un kernel nuevo con un `sys\gui.bex` viejo. No falla nada:
    # simplemente estas probando el build de antes y no lo sabes. Paso una tarde
    # el 2026-08-04.
    #
    # Las dos banderas SIGUEN existiendo por separado a proposito -son dos
    # discos logicos con dos riesgos distintos-, pero el camino corto es el
    # correcto y por eso tiene nombre.
    [switch]$Todo,
    [switch]$Yes
)

# `-Todo` se resuelve a las dos banderas de siempre ANTES de nada, para que
# todo lo de abajo no tenga que saber que existe.
if ($Todo) {
    $Flash = $true
    if (-not $Data) { $Data = $Drive }
}

$root = $PSScriptRoot
if (-not $root) { $root = Split-Path -Parent $MyInvocation.MyCommand.Path }

function Step { param($m) Write-Host ('  => ' + $m) -ForegroundColor Cyan }
function Fail { param($m) Write-Host ('  [X] ' + $m) -ForegroundColor Red; exit 1 }
function Hash256 { param($p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }

# ===========================================================================
#  EL ESPEJO: lo que hay EN EL DISCO contra lo que acaba de salir del build
# ===========================================================================
#
# ** Esto existe por un fallo concreto y caro: el 2026-08-04 se desplego con
# `-Flash` y sin `-Data`, o sea que se actualizo el ARRANQUE y no los
# PROGRAMAS. La maquina arranco un kernel nuevo con un `sys\gui.bex` de dos
# commits antes. **No fallo nada** -- simplemente se estuvo probando el build de
# ayer, y las conclusiones de esa tarde eran sobre codigo que ya no existia.
#
# Un deploy incompleto que no dice nada es peor que uno que revienta: el que
# revienta se arregla en un minuto; este manda a depurar un fantasma.
#
# Solo LEE el disco. No copia, no borra y no puede fallar el build por lo que
# encuentre: informa. Si la unidad no esta puesta, se calla.
function Espejo {
    param([string]$letra)
    if (-not $letra) { return }
    if ($letra -and (Test-Path ($letra + ':\'))) {
        $espejoSrc = Join-Path $root 'staging\BMO-DATA'
        if (Test-Path $espejoSrc) {
            Write-Host ('  === ESPEJO: ' + $letra + ':\ contra este build ===') -ForegroundColor Cyan
            $viejos = 0
            $faltan = 0
            $aldia  = 0
            foreach ($f in Get-ChildItem -Path $espejoSrc -Recurse -File) {
                $rel = $f.FullName.Substring($espejoSrc.Length).TrimStart([char]'\')
                $enDisco = Join-Path ($letra + ':\') $rel
                if (-not (Test-Path -LiteralPath $enDisco)) {
                    Write-Host ('    FALTA    ' + $rel) -ForegroundColor Red
                    $faltan++
                } elseif ((Hash256 $enDisco) -ne (Hash256 $f.FullName)) {
                    # El tamano se ensena porque es lo que se compara a ojo cuando
                    # uno mira el disco desde fuera.
                    $dl = (Get-Item -LiteralPath $enDisco).Length
                    $sl = $f.Length
                    Write-Host ('    VIEJO    {0}   disco {1} B / build {2} B' -f $rel, $dl, $sl) -ForegroundColor Red
                    $viejos++
                } else {
                    $aldia++
                }
            }
            Write-Host ''
            if ($viejos -eq 0 -and $faltan -eq 0) {
                Write-Host ('  EL DISCO ESTA AL DIA (' + $aldia + ' archivos)') -ForegroundColor Green
            } else {
                # En rojo y con el comando dentro. Un aviso que no dice como
                # arreglarlo obliga a recordar la bandera que uno acaba de olvidar.
                Write-Host '  *********************************************************' -ForegroundColor Red
                Write-Host ('  *  EL DISCO NO TIENE ESTE BUILD: {0} viejos, {1} sin copiar' -f $viejos, $faltan) -ForegroundColor Red
                Write-Host '  *' -ForegroundColor Red
                Write-Host '  *  Lo que arranques NO es lo que acabas de compilar.' -ForegroundColor Red
                Write-Host ('  *  Arreglo:  .\Ultra_kernel_x86-64\build.ps1 -Todo -Drive ' + $letra + ' -Yes') -ForegroundColor Yellow
                Write-Host '  *********************************************************' -ForegroundColor Red
            }
            Write-Host ''
        }
    }
}


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

# ---------------------------------------------------------------------------
# El idioma de las fuentes es un CONTRATO, igual que el de los syscalls, y por
# eso se comprueba en el mismo sitio y de la misma forma.
#
# No es estetica. Son dos fallos que ya se pagaron:
#
#   - El preprocesador de BMO C copiaba byte a byte. Una sola letra acentuada
#     en un literal hacia crecer el .bex de 512 a 492.032 bytes, y con MAX_BEX
#     en 1 MiB, dos palabras con tilde dejan un programa que ya no carga.
#   - La consola del kernel es Latin-1 A PROPOSITO --un byte por caracter, sin
#     decodificador-- y todo el camino de pintado entrega UTF-8 crudo con
#     `s.as_bytes()`. Una raya larga en una cadena del kernel pone TRES bytes
#     en pantalla donde iba un glifo.
#
# Los dos se arreglaron a mano el 2026-08-08. Esto es lo que impide el
# siguiente: sin esta comprobacion, la regla es una limpieza que hicimos una
# vez; con ella, es una propiedad del sistema. Arquitectura, no parche.
#
# Si no hay Python se AVISA y se sigue: un portico que no se puede levantar no
# debe cerrar la puerta. Pero si corre y falla, el build para.
Step 'Validating source encoding (sources are ASCII)'
$sweep = Join-Path (Split-Path -Parent $root) 'toolchain\tools\ascii-sweep\ascii_sweep.py'
$python = (Get-Command python -ErrorAction SilentlyContinue)
if (-not $python) {
    Write-Host '  [!] python no encontrado: no se comprueba la codificacion' -ForegroundColor Yellow
} elseif (-not (Test-Path $sweep)) {
    Write-Host '  [!] falta ascii_sweep.py: no se comprueba la codificacion' -ForegroundColor Yellow
} else {
    $env:PYTHONIOENCODING = 'utf-8'
    $encOut = & $python.Source $sweep --check
    if ($LASTEXITCODE -ne 0) {
        $encOut | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail 'codificacion: hay no-ASCII donde la regla no lo permite (ver arriba)'
    }
    $encOut | Where-Object { $_ -match 'clean:' } | ForEach-Object {
        Write-Host ('    ' + $_.Trim()) -ForegroundColor DarkGray
    }
}

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
# --EJECUTAR, CONSOLA_CREAR, DIR_ABRIR, las de archivo, REINICIAR e INFO-- se
# escribian en el kernel y nadie comprobaba que coincidieran con el ABI. Un
# guardian que solo mira la mitad da una tranquilidad que no ha ganado.
foreach ($name in @('CURRENT_TASK', 'TASK_OP_GET_PID', 'TASK_OP_GET_TID', 'TASK_OP_YIELD', 'TASK_OP_EXIT', 'TASK_OP_CHANNEL_OPEN', 'TASK_OP_CONSOLE_WRITE', 'TASK_OP_ENDPOINT_CREATE', 'TASK_OP_RUTA', 'TASK_OP_EJECUTAR', 'TASK_OP_CONSOLA_CREAR', 'TASK_OP_DIR_ABRIR', 'TASK_OP_CONSOLE_READ', 'TASK_OP_ARCHIVO_ABRIR', 'TASK_OP_ARCHIVO_CREAR', 'TASK_OP_REINICIAR', 'TASK_OP_INFO', 'TASK_OP_INFO_TEXTO', 'TASK_OP_AUDIO_CLAIM', 'TASK_OP_AUDIO_RELEASE', 'TASK_OP_CABINA_INFO', 'TASK_OP_CABINA_TEXTO', 'ARCH_OP_LEER', 'ARCH_OP_ESCRIBIR', 'ARCH_OP_TAMANO', 'ARCH_OP_CERRAR', 'ARCH_OP_LEER_LINEA', 'CHANNEL_OP_GET_SEQ', 'CHANNEL_OP_GET_INDEX')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface operation contract mismatch: ' + $name)
    }
}
# ** DOS OPERACIONES NO PUEDEN LLEVAR EL MISMO NUMERO.
#
# Y esto casi pasa el 2026-08-08: la autopsia se escribio en `0x1D` y `0x1E`,
# que ya eran `PANTALLA_SOLTAR` y `ENTRADA_SOLTAR`. O sea que **leer el informe
# de un fallo habria soltado la pantalla**.
#
# El fichero ya avisaba --el comentario de `PANTALLA_SOLTAR` cuenta que
# `MEMORIA_PEDIR` se puso en `0x12`, ya ocupado por `REINICIAR`, y que pedir
# memoria habria reiniciado la maquina-- y ese aviso es prosa: no para un build.
# Esto si lo para.
#
# No se comprueba contra una lista escrita a mano: se sacan TODOS los opcodes
# del kernel y se busca cualquier numero repetido. Una lista a mano es lo que ya
# se quedo congelada una vez, treinta lineas mas arriba.
$opsKernel = [regex]::Matches($kernelSyscalls, 'const\s+(TASK_OP_\w+)\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f]+)')
$porNumero = @{}
foreach ($m in $opsKernel) {
    $nombre = $m.Groups[1].Value
    $num = $m.Groups[2].Value.ToUpperInvariant()
    if (-not $porNumero.ContainsKey($num)) { $porNumero[$num] = @() }
    $porNumero[$num] += $nombre
}
foreach ($num in $porNumero.Keys) {
    if ($porNumero[$num].Count -gt 1) {
        Fail ('operacion DUPLICADA: ' + $num + ' lo usan ' + ($porNumero[$num] -join ' y '))
    }
}
Write-Host ('    operaciones: ' + $porNumero.Count + ' opcodes, ninguno repetido') -ForegroundColor DarkGray

# Y LA TABLA DE `OP_INFO`, que existe TRES veces: la implementa el kernel
# (`core\informe.rs`), la declara el ABI (`surface.rs`) y la consume el userland
# (`userland\src\lib.rs`). Anadir un dato es una fila -- y una fila escrita en
# dos de los tres sitios es un campo que contesta otra cosa de la que se pidio,
# sin que nada falle al compilar.
#
# No es hipotetico: al escribir esta comprobacion, `INFO_PANTALLA_DUENO` estaba
# en el kernel y en el userland y NO en el ABI. La lista no se escribe a mano
# --se saca de los tres ficheros-- porque una lista a mano es lo que ya se
# quedo congelada una vez, ahi arriba.
$infoFuentes = [ordered]@{
    'kernel'   = Get-Content (Join-Path $root 'kernel\src\ring0\core\informe.rs') -Raw
    'abi'      = $abiSurface
    'userland' = Get-Content (Join-Path $root '..\Ultra_userspace\userland\src\lib.rs') -Raw
}
$infoCampos = @{}
foreach ($fuente in $infoFuentes.GetEnumerator()) {
    $hallados = [regex]::Matches($fuente.Value, '(?m)^\s*(?:pub\s+)?const\s+(INFO_[A-Z0-9_]+)\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+)')
    foreach ($m in $hallados) {
        $campo = $m.Groups[1].Value
        if (-not $infoCampos.ContainsKey($campo)) { $infoCampos[$campo] = [ordered]@{} }
        $infoCampos[$campo][$fuente.Key] = $m.Groups[2].Value.ToUpperInvariant().Replace('_', '')
    }
}
foreach ($campo in ($infoCampos.Keys | Sort-Object)) {
    $vistos = $infoCampos[$campo]
    $faltan = @('kernel', 'abi', 'userland') | Where-Object { -not $vistos.Contains($_) }
    if ($faltan.Count -gt 0) {
        Fail ('OP_INFO field contract: ' + $campo + ' falta en ' + ($faltan -join ', '))
    }
    if ((@($vistos.Values | Sort-Object -Unique)).Count -ne 1) {
        $detalle = ($vistos.GetEnumerator() | ForEach-Object { $_.Key + '=' + $_.Value }) -join ' '
        Fail ('OP_INFO field contract: ' + $campo + ' con ids distintos -- ' + $detalle)
    }
}
Write-Host ('    OP_INFO: ' + $infoCampos.Count + ' campos, el mismo id en kernel, ABI y userland') -ForegroundColor DarkGray

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
# KIND_AUDIO igual: el sonido es `HandleKind::AudioEngine` y los dos lados
# tienen que decir 0x10. Es la ley 20 de BITACORA.md -- lo que no comprueba el
# build no es una regla, es una costumbre, y un kind que se desplaza en un solo
# lado no da error: hace que un handle valido resuelva como otra cosa.
if (-not ($kernelCap -match 'KIND_AUDIO:\s*u8\s*=\s*0x10')) {
    Fail 'capability contract mismatch: KIND_AUDIO must be 0x10 (bmo-abi HandleKind::AudioEngine)'
}
if (-not ($abiKind -match 'AudioEngine\s*=\s*0x10')) {
    Fail 'capability contract mismatch: bmo-abi HandleKind::AudioEngine must be 0x10'
}

# NOTE: uefi_chain is now the UNIFIED shim -- it embeds the flat binaries
# of s1_cpu, s2_mem and the kernel via include_bytes!, so it is built
# AFTER them (see 'Building unified uefi_chain' below). Rationale: some
# firmwares (MSI A320M AMI fast path) never bind SimpleFileSystem, so
# the boot chain cannot read the ESP through UEFI protocols at all.

# -- Build the 2 consolidated stages -------------------------------
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

# -- Build Ring 3 userspace (Ultra_userspace/) ---------------------
#
# * El kernel YA NO EMBEBE el compositor. Antes lo metia con `include_bytes!` y
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
# * `gui.bex` y no `compositor.bex`: el driver FAT32 del kernel es 8.3 y se
# NIEGA a recortar nombres (un nombre recortado abre otro archivo, y en un
# cargador de programas eso es ejecutar otro binario). `compositor` son diez
# caracteres y no cabe en los ocho del campo.
# -- El volumen de datos, POR CATEGORIAS ---------------------------
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
# * Y se teclea MENOS que antes: `cobol/banco.bex` es mas corto que
#   `apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.
#
# Los nombres de carpeta tambien son 8.3: el driver FAT32 del kernel se NIEGA a
# recortar, y una carpeta recortada manda a otro sitio igual que un fichero.
$dataBase = Join-Path $root 'staging\BMO-DATA'
foreach ($d in @('sys', 'cobol', 'c', 'ada', 'datos')) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase $d) -Force | Out-Null
}
# * Y dentro de cobol\, un nivel por carpeta. Ver el bloque de $cobolEjemplos
# para por que el nombre es el numero a secas.
foreach ($n in 1..10) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase ('cobol\' + $n)) -Force | Out-Null
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

# -- Programas COBOL de ejemplo -----------------------------------
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
# * POR NIVELES, y no en un monton. Los ejemplos estan en ESCALERA -cada uno
# pide una cosa mas que el anterior- y esa escalera se pierde si en el disco
# caen todos revueltos. Con niveles se puede VERIFICAR de uno en uno:
#
#     run cobol/1/hola.bex     y si eso va, subir
#     run cobol/2/banco.bex    y si eso va, subir
#     ...
#
# Y cuando algo se rompa, el orden dice por donde empezar a mirar: si falla el
# 10, comprobar primero que el 1 sigue vivo.
#
# [!] La carpeta es el NUMERO a secas y no el nombre largo. No es pereza: el
# driver FAT32 del kernel se NIEGA a recortar, y `3-presentacion` son trece
# letras. Un `n3presen` seria feo y no diria mas que un `3`; el nombre del nivel
# vive en examples\README.md, que es donde se lee.
$cobolEjemplos = @(
    @{ src = 'toolchain\lang\cobol\examples\1-basico\hola.cob';           out = 'hola.bex'     ; dir = 'cobol\1' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\banco.cob';         out = 'banco.bex'    ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calc.cob';          out = 'calc.bex'     ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\2-decimal\calcgui.cob';       out = 'calcgui.bex'  ; dir = 'cobol\2' },
    @{ src = 'toolchain\lang\cobol\examples\3-presentacion\extracto.cob'; out = 'extracto.bex' ; dir = 'cobol\3' },
    @{ src = 'toolchain\lang\cobol\examples\4-ficheros\batch.cob';        out = 'batch.bex'    ; dir = 'cobol\4' },
    # `conceptos` son nueve letras y el driver FAT32 se NIEGA a recortar, asi
    # que el destino es `concep`. La comprobacion de 8.3 de abajo lo cazaria
    # igual, pero mejor no llegar a que la cace.
    @{ src = 'toolchain\lang\cobol\examples\5-tablas\conceptos.cob';      out = 'concep.bex'   ; dir = 'cobol\5' },
    @{ src = 'toolchain\lang\cobol\examples\6-condiciones\cartera.cob';   out = 'carter.bex'   ; dir = 'cobol\6' },
    @{ src = 'toolchain\lang\cobol\examples\7-empaquetado\cuentas.cob';   out = 'cuentas.bex'  ; dir = 'cobol\7' },
    @{ src = 'toolchain\lang\cobol\examples\8-parrafos\cierre.cob';       out = 'cierre.bex'   ; dir = 'cobol\8' },
    @{ src = 'toolchain\lang\cobol\examples\9-decision\comision.cob';     out = 'comisio.bex'  ; dir = 'cobol\9' },
    @{ src = 'toolchain\lang\cobol\examples\10-binario\maestro.cob';      out = 'maestro.bex'  ; dir = 'cobol\10' }
)
# -- Programas ADA de ejemplo -------------------------------------
#
# Crate PROPIO (`bmo-ada-front`), sin dependencia de los otros frontends. El
# decimal exacto es el mismo que el de COBOL y no por copia: el Annex F de Ada
# copio las reglas de COBOL, asi que dos lenguajes que dicen lo mismo acaban en
# la misma aritmetica de enteros escalados.
$adaEjemplos = @(
    @{ src = 'toolchain\lang\ada\examples\1-basico\cierre.adb'; out = 'cierre.bex' ; dir = 'ada' }
)
# -- Programas C de ejemplo ---------------------------------------
#
# * Este paso NO EXISTIA. COBOL y Ada llegaban al disco y C no, asi que los
# ejemplos de C solo se podian ejecutar si alguien los embebia a mano en el
# kernel -- y `scroll_C.bex` no llegaba al Kingston por eso, no por un fallo del
# compilador. Un lenguaje que compila y cuyo binario no se despliega esta a
# medias.
#
# `hola_C.c` prueba lo basico (bucles, %d, resta con signo, switch, %s).
# `scroll_C.c` usa las cabeceras `<bmo/...>`: la puerta de syscalls desde C, la
# capability de entrada y el modelo de scroll.
# `memoria_C.c` ESTRENA `KIND_MEMORIA`: pide, escribe, relee y agota el tope de
# cuatro peticiones. Es el unico programa que ejerce la capability de memoria.
$cEjemplos = @(
    @{ src = 'toolchain\lang\c\examples\hola_C.c';   out = 'holac.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\scroll_C.c'; out = 'scrollc.bex' ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\pregunta_C.c'; out = 'pregc.bex'  ; dir = 'c' },
    @{ src = 'toolchain\lang\c\examples\memoria_C.c'; out = 'memc.bex'   ; dir = 'c' },
    # El ensayo general de DOOM: 2.5D en punto fijo sobre la pantalla real.
    @{ src = 'toolchain\lang\c\examples\raycaster_C.c'; out = 'ray.bex'    ; dir = 'c' },
    # La prueba de fopen/fread/fseek. Lee `datos\salida.txt` DOS veces y
    # compara: si las dos lecturas coinciden, la cadena de ficheros funciona.
    @{ src = 'toolchain\lang\c\examples\leer_C.c';      out = 'leer.bex'   ; dir = 'c' },
    # ESTRENA `KIND_AUDIO`. Comprueba el CONTRATO y no el oido: que hay handle,
    # que el tope de duracion se cumple, que es exclusivo y --la que importa--
    # que el handle soltado ya NO pita. Puede que no se oiga nada y este todo
    # bien: el puerto del altavoz existe en todo x86, el zumbador no.
    @{ src = 'toolchain\lang\c\examples\sonido_C.c';    out = 'sonido.bex' ; dir = 'c' },
    # `<bmo/musica.h>`: notas por nombre, figuras y tempo. Se DIBUJA mientras
    # suena, porque puede que no suene -- si la placa no trae zumbador, la
    # pantalla es la unica prueba de que la cadena entera funciono.
    @{ src = 'toolchain\lang\c\examples\musica_C.c';    out = 'musica.bex' ; dir = 'c' }
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

    # -- Los DATOS de los ejemplos ---------------------------------
    #
    # * Este paso tampoco existia, y era peor que el de C: los .txt que leen
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

# -- Build kernel (Ring 0 base) ------------------------------------
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

# -- Embed payloads + build unified uefi_chain ---------------------
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

# -- Validate outputs ----------------------------------------------
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

# -- Stage to ESP layout -------------------------------------------
# ESP layout -- UNA SOLA COSA:
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


# A que unidad se le hace el espejo. `-Data` manda; luego `-Flash`; y si solo
# se compilo, la que se haya ESCRITO en `-Drive` -no vale el valor por defecto,
# o el espejo saldria contra una unidad que nadie nombro.
$espejoLetra = ''
if ($Data)                                       { $espejoLetra = $Data.TrimEnd([char]':',[char]'\').ToUpper() }
elseif ($Flash -or $Verify)                      { $espejoLetra = $Drive.TrimEnd([char]':',[char]'\').ToUpper() }
elseif ($PSBoundParameters.ContainsKey('Drive')) { $espejoLetra = $Drive.TrimEnd([char]':',[char]'\').ToUpper() }

if ($BuildOnly) {
    Espejo $espejoLetra
    exit 0
}

# -- Flash --------------------------------------------------------
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

# -- Data: los programas de Ring 3 al volumen de datos -------------
#
# Separado de -Flash a proposito. Son dos discos distintos con dos riesgos
# distintos: -Flash toca la ESP de arranque, esto toca BMO-DATA. Que compartan
# bandera invitaria a escribir en uno cuando se queria el otro.
#
# * Este es el UNICO sitio del build que escribe fuera del arbol del proyecto.
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

    # * Ya no es una sola carpeta: van a sys\ cobol\ c\ ada\ datos\. El bucle de
    # abajo copia recursivo y crea los directorios que falten, asi que no hay
    # nada que cambiar aqui salvo el mensaje -- pero **la vieja `apps\` del disco
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

Espejo $espejoLetra

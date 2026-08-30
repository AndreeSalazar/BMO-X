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
    # segundo arranca un kernel nuevo con un `sys\d.bex` viejo. No falla nada:
    # simplemente estas probando el build de antes y no lo sabes. Paso una tarde
    # el 2026-08-04.
    #
    # Las dos banderas SIGUEN existiendo por separado a proposito -son dos
    # discos logicos con dos riesgos distintos-, pero el camino corto es el
    # correcto y por eso tiene nombre.
    [switch]$Todo,
    # No compilar DOOM aunque `BMO-externo\` este puesto.
    #
    # El paso ya se salta solo cuando el port no esta --es GPL y vive fuera del
    # repo-- asi que esta bandera es para el caso contrario: el port esta y no
    # apetece pagar las 56.465 lineas en cada vuelta del build.
    [switch]$SinDoom,
    # ** EL KERNEL DE MEDIDA: `--features metro_puerta`.
    #
    # Devuelve los dos `rdtsc` a `dispatch`, o sea el REPARTO de una puerta
    # entre su mitad Rust y el resto. Es lo unico que puede contestar donde se
    # van los ~945 ciclos, y lo que cuesta esta medido: **112 ciclos en CADA
    # puerta de CADA programa**, un 11%.
    #
    # [!] NO ES EL KERNEL QUE SE DEJA PUESTO. Se flashea, se corre
    # `sys/precio.bex`, se apunta el reparto y se vuelve al build normal. Un
    # instrumento que se queda puesto deja de ser una medida y pasa a ser un
    # peaje -- que es exactamente por lo que se retiro el 16-08.
    [switch]$Metro,
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
# La hora de arranque, para poder distinguir despues lo que este build produjo
# de lo que se quedo de otro. Ver el guardian de fantasmas del final.
$buildStart = Get-Date

function Step { param($m) Write-Host ('  => ' + $m) -ForegroundColor Cyan }
function Fail { param($m) Write-Host ('  [X] ' + $m) -ForegroundColor Red; exit 1 }
function Hash256 { param($p) (Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash.ToLowerInvariant() }

# ** LOS GUARDIANES DE PYTHON, en un sitio: habia DOS bloques identicos y el
# tercero (L6a) seria la tercera. Sin Python se avisa; si falla, el build para.
function Guardian {
    param($paso, $script, $queMide, $siFalla)
    Step $paso
    $ruta = Join-Path (Split-Path -Parent $root) $script
    $py = (Get-Command python -ErrorAction SilentlyContinue)
    # ** FALTA EL SCRIPT y NO HAY PYTHON no son lo mismo, y hasta el 20-08 los
    # dos avisaban y seguian. Un path mal escrito dejo un guardian MUERTO y el
    # build dijo COMPLETE igual -- el fallo que este fichero ya describe:
    # avisar de nada, y con tono tranquilizador. El script es del REPO: para.
    if (-not (Test-Path $ruta)) { Fail ('guardian MUERTO: falta ' + $script) }
    if (-not $py) {
        Write-Host ('  [!] python no encontrado: no se comprueba ' + $queMide) -ForegroundColor Yellow
        return
    }
    $env:PYTHONIOENCODING = 'utf-8'
    $salida = & $py.Source $ruta --check
    if ($LASTEXITCODE -ne 0) {
        $salida | ForEach-Object { Write-Host ('    ' + $_) -ForegroundColor Red }
        Fail $siFalla
    }
    $salida | Where-Object { $_ -match 'clean:' } | ForEach-Object {
        Write-Host ('    ' + $_.Trim()) -ForegroundColor DarkGray
    }
}

# ===========================================================================
#  EL ESPEJO: lo que hay EN EL DISCO contra lo que acaba de salir del build
# ===========================================================================
#
# ** Esto existe por un fallo concreto y caro: el 2026-08-04 se desplego con
# `-Flash` y sin `-Data`, o sea que se actualizo el ARRANQUE y no los
# PROGRAMAS. La maquina arranco un kernel nuevo con un `sys\d.bex` de dos
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
Guardian 'Validating source encoding (sources are ASCII)' `
    'toolchain\tools\ascii-sweep\ascii_sweep.py' 'la codificacion' `
    'codificacion: hay no-ASCII donde la regla no lo permite (ver arriba)'

# ---------------------------------------------------------------------------
# ** EL QUINTO GUARDIAN: LAS CITAS A DOCUMENTOS (2026-08-17).
#
# El arbol cita documentos desde el kernel, desde los `Cargo.toml`, desde este
# mismo fichero y desde los ejemplos de C: casi cuatrocientas veces. Nada lo
# comprobaba, y un puntero roto no falla -- manda al lector a la nada, y el
# lector concluye que el documento nunca se escribio.
#
# El dia que se escribio el guardian encontro catorce. Una de ellas apuntaba a
# AVANCES.md dentro de docs/, cuando ese fichero vive en la raiz, y **no habia
# resuelto nunca**: estaba en un documento cuyo trabajo entero es mandar al
# lector a otro sitio.
#
# ** Y el ejemplo de arriba va SIN backticks a proposito: este guardian no sabe
# distinguir una cita de la CITA DE UNA CITA ROTA, y tiene razon -- si el
# ejemplo tiene forma de ruta, es una ruta. Se cazo a si mismo en este
# comentario el dia que se escribio.
#
# ** Por que va aqui y no en un banco de pruebas: los documentos se mueven con
# `git mv` y las citas no se mueven con ellos. Eso pasa mientras se trabaja, no
# en el despliegue -- y `-BuildOnly` es lo que se corre veinte veces al dia.
# Es la leccion que ya dejo escrita el guardian del `.h`: el que se corre a mano
# no protege igual que el que se corre solo.
#
# El trato lo pone `Guardian`, arriba, y es el mismo para los tres.
Guardian 'Validating document citations resolve' `
    'toolchain\tools\enlaces\enlaces.py' 'las citas' `
    'citas: hay documentos citados que no existen (ver arriba)'
# ** L6a: TRINQUETE, no muro -- juzga el delta contra `LINEA_BASE.txt`. El por
# que esta entero en la cabecera de `censo_modular.py`, y ahi solo hay uno.
Guardian 'Validating L6a: no new module over the line' `
    'toolchain\tools\censo-modular\censo_modular.py' 'L6a' `
    'L6a: un modulo nuevo pasa de las 1.000 lineas, o uno de la linea base crecio'

# ** EL AMBITO de un commit, y SOLO el ambito. Trinquete como el de L6a, y no
# mira la prosa: el por que entero esta en la cabecera de ambitos.py.
# ** LAS CASILLAS DE LOS PLANES, que no las contaba nadie. El 24-08 se
# recontaron a mano y OCHO de cincuenta y cuatro estaban mal -- cinco escalones
# de MAQUETA figuraban sin hacer con su crate hecho y su banco verde. El por que
# entero, y por que el primer intento de este guardian no cazo ninguna, esta en
# la cabecera de casillas.py -- la leccion no es la que se fue a buscar.
Guardian 'Validating plan checkboxes are verifiable' `
    'toolchain\tools\casillas\casillas.py' 'las casillas' `
    'casillas: una casilla no se puede comprobar (ver arriba)'
Guardian 'Validating commit scopes' `
    'toolchain\tools\ambitos\ambitos.py' 'los ambitos de los commits' `
    'ambitos: un commit usa un ambito que no esta en AMBITOS.txt (ver arriba)'

# -- CONTRATO (L6a: `build.ps1` se partio el 2026-08-28) --------
. (Join-Path $PSScriptRoot 'build\contrato.ps1')
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
# cualquier otro programa. El kernel lo arranca con `launch::ruta` despues de
# montar el disco (ver `phase::arrancar_escritorio`). Cambiar el escritorio ya
# no obliga a recompilar Ring 0.
#
# Ultra_userspace/ es su PROPIO workspace: se compila a x86_64-unknown-none con
# su guion de enlazado, que fija la base en USER_IMAGE_BASE. `bex-link` traduce
# el ELF a un contenedor BEF y comprueba, seccion por seccion, que las
# direcciones que escribio el enlazador son las que el kernel va a mapear.
Step 'Building Ring 3 userspace (DIRECTOR)...'
$usDir = Join-Path (Split-Path -Parent $root) 'Ultra_userspace'
if (-not (Test-Path $usDir)) { Fail 'Ultra_userspace/ no existe' }
Push-Location $usDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $out = cargo +nightly build -p bmo-service-director --release --target x86_64-unknown-none 2>&1
    $out | ForEach-Object {
        if ($_ -match 'Compiling|Finished|error') { Write-Host ('    [userspace] ' + $_) -ForegroundColor DarkGray }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'userspace build failed' }
} finally { Pop-Location }

$compositorElf = Join-Path $usDir 'target\x86_64-unknown-none\release\director'
if (-not (Test-Path $compositorElf)) { Fail 'no salio el ELF del DIRECTOR' }
# El .bex sale a staging\BMO-DATA\apps\, que es el espejo de lo que hay que
# copiar al volumen de datos. La ruta de dentro (sys\d.bex) tiene que cuadrar
# con `RUTA_COMPOSITOR` de phase.rs: es el contrato entre el build y el arranque.
#
# ** `d.bex` y no `director.bex`: `director` son ocho exactos y CABRIA. Lo que
# decide es que con el escritorio muerto esto se teclea a mano; el por que
# entero, en `core/desktop.rs`. Y el 8.3 si manda en lo demas: el driver FAT32
# se NIEGA a recortar, porque un nombre recortado abre otro archivo.
# -- EJEMPLOS (L6a: `build.ps1` se partio el 2026-08-28) --------
. (Join-Path $PSScriptRoot 'build\ejemplos.ps1')

# -- Build kernel (Ring 0 base) ------------------------------------
Step 'Building kernel (Ring 0 base)...'
$stageDir = Join-Path $root 'kernel'
Push-Location $stageDir
try {
    $env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"
    $kd = Join-Path $target 'kernel'
    $linkerHash = (Get-FileHash (Join-Path $stageDir 'linker.ld') -Algorithm SHA256).Hash.Substring(0, 16)
    # ** EL KERNEL DE MEDIDA VA A OTRO `--target-dir`, y no es un detalle.
    #
    # Con el mismo directorio, cargo reusa los objetos: se compila con el metro,
    # se vuelve a compilar sin el, y **el que queda es el ultimo que se pidio**
    # sin que nada en pantalla lo diga. Dos directorios significan que los dos
    # binarios existen a la vez y ninguno pisa al otro.
    $rasgos = @()
    if ($Metro) {
        $rasgos = @('--features', 'metro_puerta')
        $kd = Join-Path $target 'kernel-metro'
        Write-Host ''
        Write-Host '  ** KERNEL DE MEDIDA (--features metro_puerta) **' -ForegroundColor Yellow
        Write-Host '     Devuelve los dos rdtsc a `dispatch`: cada puerta cuesta ~112 ciclos MAS.' -ForegroundColor Yellow
        Write-Host '     Correr `sys/precio.bex`, apuntar el reparto, y VOLVER al build normal.' -ForegroundColor Yellow
        Write-Host ''
    }
    $out = cargo +nightly rustc --release @rasgos --target x86_64-unknown-none --target-dir $kd -- `
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
# ** SE LEE DE `$kd` Y NO DE 'kernel' A PELO, y eso es lo que hace utilizable el
# `-Metro`: con la ruta escrita a mano, un build de medida habria compilado el
# kernel instrumentado en `kernel-metro\` y **empaquetado el normal de al lado**,
# sin que una sola linea lo dijera. Se habria medido el build equivocado y el
# reparto habria salido en ceros -- con el disco ya flasheado.
$kernelElf = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe'))
if (-not (Test-Path $kernelElf)) { $kernelElf = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel')) }
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

# -- ** LOS FANTASMAS DE `staging\` -------------------------------
#
# `staging\` NO SE LIMPIA entre builds, a proposito: rehacer el volumen entero
# cada vez cuesta minutos. El precio es que **un fichero que este build dejo de
# producir se queda ahi para siempre**, y no como basura inerte: TAPANDO al
# bueno.
#
# Paso de verdad el 2026-08-18. Los ejemplos de COBOL se reorganizaron de
# `cobol\` a `cobol\<escalon>\`, los planos del 3 de agosto se quedaron, y el
# escritorio --que lanzaba `cobol/calcgui.bex`-- siguio arrancando un motor de
# quince dias atras. Hablaba un protocolo anterior, la calculadora se quedo
# esperando una respuesta que no iba a llegar, y con las teclas suyas el Ryzen
# se quedo sin teclado.
#
# ** UN FANTASMA NO DA ERROR: CONTESTA. Por eso hace falta mirarlo aqui.
#
# La comprobacion es una RESTA, no una lista: cualquier `.bex` mas viejo que el
# arranque de este build es algo que ya no se produce. Un guardian con lista
# tendria el mismo fallo que vigila.
$fantasmas = @(Get-ChildItem -Path $dataBase -Recurse -Filter '*.bex' -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -lt $buildStart })
if ($fantasmas.Count -gt 0) {
    Write-Host ('    [!] {0} .bex en staging que este build NO ha producido:' -f $fantasmas.Count) -ForegroundColor Yellow
    foreach ($f in $fantasmas) {
        Write-Host ('        {0}' -f $f.FullName.Substring($dataBase.Length + 1)) -ForegroundColor Yellow
    }
    Write-Host '        Este build no los produce. Comprobar si alguno TAPA a uno bueno.' -ForegroundColor Yellow
} else {
    Write-Host '    staging: ningun .bex sobrante de un build anterior' -ForegroundColor DarkGray
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
# El mismo `$kd` que se empaqueto: la tabla de tamanos tiene que describir el
# binario que se acaba de meter en el `.efi`, no el que hubiera al lado.
$kernel = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel.exe'))
if (-not (Test-Path $kernel)) { $kernel = Join-Path $kd (Join-Path 'x86_64-unknown-none' (Join-Path 'release' 'bmo-kernel')) }
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

# -- DISCOS (L6a: `build.ps1` se partio el 2026-08-28) --------
. (Join-Path $PSScriptRoot 'build\discos.ps1')

Espejo $espejoLetra

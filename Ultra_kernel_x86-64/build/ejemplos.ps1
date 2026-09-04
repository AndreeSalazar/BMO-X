# LOS PROGRAMAS DE EJEMPLO, Y EL VOLUMEN DE DATOS POR CATEGORIAS.
#
# ** Por que esto es un fichero (2026-08-28)
#
# Porque es lo que MAS CRECE de todo el build: cada lenguaje nuevo trae su
# tabla, su llamada a `Compilar-Ejemplos` y su Step. La lista de subidas de
# techo de L6a tiene una entrada de `build.ps1` por cada uno --INTI, el
# `.ibex`, la sonda-- y todas predijeron la siguiente. Sacarlo aqui es lo que
# hace que anadir un lenguaje deje de engordar el fichero que lo orquesta todo.
#
# Dentro: el reparto del volumen por categorias (`sys cobol c ada inti datos`),
# `Compilar-Ejemplos` --un bucle, cuatro lenguajes--, `Nuevo-Bico` --los iconos
# de 16x16-- y el empaquetado de recursos dentro del `.bex`.
#
# [!] Se carga con punto: corre en el ambito de `build.ps1`, y `$dataBase` se
# define AQUI y lo usan despues el guardian de fantasmas y el despliegue. Es el
# mismo texto en el mismo orden.

# -- El volumen de datos, POR CATEGORIAS ---------------------------
#
# Antes todo caia en un solo `apps\`: los siete .bex de COBOL, los de C, el de
# Ada, el compositor y los .txt de entrada, revueltos. Un `ls` daba diecisiete
# lineas sin orden, y para lanzar algo habia que acordarse del nombre exacto.
#
# La primera division es **programa o dato**; dentro de los programas, por quien
# los compila:
#
#     sys\     el sistema: lo que arranca solo (d.bex, el DIRECTOR)
#     cobol\  c\  ada\      los ejemplos, por lenguaje
#     datos\   lo que los programas LEEN y ESCRIBEN
#
# * Y se teclea MENOS que antes: `cobol/banco.bex` es mas corto que
#   `apps/banco.bex`. Ordenar no ha costado tecleo, lo ha ahorrado.
#
# Los nombres de carpeta tambien son 8.3: el driver FAT32 del kernel se NIEGA a
# recortar, y una carpeta recortada manda a otro sitio igual que un fichero.
$dataBase = Join-Path $root 'staging\BMO-DATA'
# `apps\` es para APLICACIONES, no para ejemplos del toolchain, y la distincion
# no es cosmetica: `c\`, `cobol\` y `ada\` los llena este build desde el repo, y
# `apps\` lo llena lo que alguien traiga de fuera -- hoy DOOM. Hasta ahora no lo
# creaba nadie aunque varios mensajes ya nombraban rutas `apps/...`.
foreach ($d in @('sys', 'cobol', 'c', 'ada', 'inti', 'datos', 'apps')) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase $d) -Force | Out-Null
}
# * Y dentro de cobol\, un nivel por carpeta. Ver el bloque de $cobolEjemplos
# para por que el nombre es el numero a secas.
foreach ($n in 1..10) {
    New-Item -ItemType Directory -Path (Join-Path $dataBase ('cobol\' + $n)) -Force | Out-Null
}
$compositorBex = Join-Path $dataBase 'sys\d.bex'
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
    if (-not (Test-Path $compositorBex)) { Fail 'bex-link no produjo d.bex' }

    # -- La MEDIDA, que no es un servicio -------------------------------
    #
    # `medida/coste` mide la puerta con el bucle escrito en ensamblador y la
    # juzga con `bmo-juicio`, cuyos invariantes se prueban en el anfitrion.
    #
    # ** NO REEMPLAZA a `c/coste.bex`: los dos se quedan a proposito. El fallo
    # del 16-08 lo cazo una DISCREPANCIA entre dos calculos de la misma cosa, y
    # dos implementaciones en dos lenguajes que tienen que coincidir es mas
    # fuerte que una implementacion buena. Si difieren, uno miente y ya se sabe
    # donde mirar.
    $costeElf = Join-Path $usDir 'target\x86_64-unknown-none\release\coste'
    if (-not (Test-Path $costeElf)) { Fail 'no salio el ELF de medida/coste' }
    # ** `precio` y no `coster`: el de C se llama `coste`, y dos palabras para la
    # misma cosa dicen lo que son -- DOS MEDIDAS INDEPENDIENTES DE LA MISMA
    # CANTIDAD, que tienen que coincidir. Si difieren, una miente.
    $costeBex = Join-Path $dataBase 'sys\precio.bex'
    if (Test-Path $costeBex) { Remove-Item $costeBex -Force }
    $out = cargo run -p bmo-bex-link --quiet -- $costeElf $costeBex 2>&1
    $out | ForEach-Object {
        $linea = $_.ToString()
        if ($linea -match '^\s+(\.text|->)|error|!!') {
            Write-Host ('    [bex-link] ' + $linea.Trim()) -ForegroundColor DarkGray
        }
    }
    if ($LASTEXITCODE -ne 0) { Fail 'bex-link fallo con medida/coste' }
    if (-not (Test-Path $costeBex)) { Fail 'bex-link no produjo precio.bex' }
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
    # ** LA SONDA: el unico programa que usa la superficie MAL a proposito.
    # Handles inventados, operaciones que no existen, el renglon de ruta
    # inundado, el tope de memoria forzado, tamanos imposibles. Cada empujon
    # tiene UNA respuesta correcta --el kernel dice que no y sigue vivo-- y que
    # el programa llegue a imprimir su recuento ya es media prueba.
    @{ src = 'toolchain\lang\c\examples\sonda_C.c';  out = 'sonda.bex'  ; dir = 'c' },
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
    @{ src = 'toolchain\lang\c\examples\musica_C.c';    out = 'musica.bex' ; dir = 'c' },
    # ** EL PAQUETE: este `.bex` viaja con datos DENTRO y los lee sin escribir
    # ninguna ruta -- le pide al kernel su propia imagen. Se empaqueta justo
    # despues de compilarlo, ver `$cRecursos`.
    @{ src = 'toolchain\lang\c\examples\caja_C.c';      out = 'caja.bex'   ; dir = 'c' },
    # ** UNA PIEZA DE VERDAD sobre `KIND_AUDIO`: el ritornello de "La primavera"
    # de Vivaldi (1725, dominio publico). `musica.bex` prueba la libreria nota a
    # nota; esta prueba la PIEZA -- que ocho compases seguidos no deriven, y que
    # el eco forte/piano que Vivaldi escribio salga por `BMO_SONIDO_VOLUMEN`.
    @{ src = 'toolchain\lang\c\examples\vivaldi_C.c';   out = 'vivaldi.bex'; dir = 'c' },
    # ** EL NUMERO QUE NO EXISTIA: cuantos ciclos vale una puerta.
    # Compara bucle vacio, llamada normal, `INVOKE` pelado sobre la tarea
    # actual, e `INVOKE` sobre un handle de verdad. Se queda con el MINIMO,
    # porque el temporizador expropia y una media se puede inflar. Decide si
    # algo puede pasar por la superficie o tiene que ser codigo enlazado --
    # empezando por el runtime de Python. Ver `docs/maestro/PYTHON_MAESTRO.md`.
    @{ src = 'toolchain\lang\c\examples\coste_C.c';     out = 'coste.bex'  ; dir = 'c' },
    # LA MEDIDA DEL BLIT: memcpy a RAM contra memcpy al framebuffer (WC), y
    # un bucle de 8 bytes como tercera fila. Ver su cabecera.
    @{ src = 'toolchain\lang\c\examples\blit_C.c';      out = 'blit.bex'   ; dir = 'c' }
)

# * LOS RECURSOS QUE VAN DENTRO DE UN `.bex`.
#
# Se meten DESPUES de compilar, y por eso es un paso aparte y no una opcion del
# compilador: el codigo lo emite el frontend, los datos llegan de quien monta la
# app. Ver `toolchain	oolsmo-pack`.
#
# ** Los bytes van escritos AQUI y no en un fichero suelto a proposito: el
# programa comprueba su contenido (`1..8`), asi que si esta lista y el ejemplo
# se separan, la prueba lo dice en vez de pasar por casualidad.
$cRecursos = @(
    @{ bex = 'c\caja.bex'; recursos = @(
        @{ nombre = 'saludo.txt'; texto = 'hola desde dentro de la caja' },
        @{ nombre = 'cuenta.bin'; bytes  = @(1,2,3,4,5,6,7,8) }
    ) }
)

# * EL FORMATO `BICO`, escrito aqui porque aqui es donde nace un icono.
#
#     0..4   "BICO"      4..6  ancho (u16)     6..8  alto (u16)
#     8..    ancho*alto pixeles BGRA, u32 little-endian
#
# 16x16 y el escritorio lo pinta al doble. Se guarda pequeno a proposito: la
# gracia de meter el icono en el paquete es que **no cueste nada llevarlo**, y
# un icono que engorda la app es un icono que alguien acabara quitando. 16x16
# son 1032 bytes; a 32x32 serian 4104.
#
# Los iconos se escriben como DIBUJO y no como una lista de numeros. Una rejilla
# de dieciseis lineas se lee, se corrige y se ve mal cuando esta mal; un array
# de 256 enteros no. Es el mismo criterio que `ring0\core\gato.rs`.
#
#   .  transparente (alfa 0: el escritorio se ve a traves)
#   o  contorno oscuro   R  rojo   d  rojo oscuro   W  blanco
#
# [!] Los colores van como **cuatro bytes en el orden del fichero (B, G, R, A)**
# y no como un `0xAARRGGBB`, por dos razones y la segunda escuece:
#
#   1. Asi la tabla dice el orden de bytes que se escribe, en vez de obligar a
#      recordar que un `u32` little-endian se guarda al reves de como se lee.
#   2. **PowerShell 5.1 lee `0xFFB4342A` como un `Int32` NEGATIVO** (-4967382) y
#      el cast a `uint32` revienta con "valor demasiado grande o demasiado
#      pequeno". Sin suffijo `u` en esta version, cualquier color con el alfa a
#      `FF` cae en la trampa -- o sea todos los opacos.
$BICO_PALETA = @{
    '.' = @(0x00, 0x00, 0x00, 0x00)
    'o' = @(0x10, 0x10, 0x1B, 0xFF)
    'R' = @(0x2A, 0x34, 0xB4, 0xFF)
    'd' = @(0x16, 0x1C, 0x6B, 0xFF)
    'W' = @(0xE0, 0xE6, 0xF0, 0xFF)
}

function Compilar-Ejemplos {
    # ** UN bucle, tres lenguajes. Estaba escrito TRES VECES -- COBOL, Ada y C--
    # con la misma comprobacion de 8.3, el mismo `Join-Path`, el mismo filtro de
    # salida y los mismos dos `Fail`. Lo unico distinto era el crate, la
    # etiqueta y que el de Ada aceptaba ademas la palabra `linea` en su filtro.
    #
    # Tres copias de una regla es tres sitios donde arreglarla, y el dia que
    # alguien arregle dos se notara en el tercero -- que es exactamente el
    # patron que esta casa lleva pagando todo el dia.
    #
    # [!] El tope de 8 caracteres NO es una convencion nuestra: el driver FAT32
    # del kernel se NIEGA a recortar un nombre, asi que un tallo de nueve letras
    # no es feo, es un fichero que no se puede abrir.
    param($ejemplos, $crate, $etiqueta, $patron, $dataBase, $repo)
    foreach ($e in $ejemplos) {
        $tallo = [System.IO.Path]::GetFileNameWithoutExtension($e.out)
        if ($tallo.Length -gt 8) { Fail ($e.out + ': el tallo no cabe en 8.3') }
        $dst = Join-Path (Join-Path $dataBase $e.dir) $e.out
        $out = cargo run -p $crate --quiet -- (Join-Path $repo $e.src) -o $dst 2>&1
        $out | ForEach-Object {
            if ($_ -match $patron) { Write-Host ('    [' + $etiqueta + '] ' + $_) -ForegroundColor DarkGray }
        }
        if ($LASTEXITCODE -ne 0) { Fail ('no compilo ' + $e.src) }
        if (-not (Test-Path $dst)) { Fail ('no salio ' + $e.out) }
    }
}

function Nuevo-Bico {
    param([string[]]$filas)
    $lado = 16
    if ($filas.Count -ne $lado) { Fail ('un icono son ' + $lado + ' filas, no ' + $filas.Count) }
    $bytes = New-Object System.Collections.Generic.List[byte]
    $bytes.AddRange([byte[]][System.Text.Encoding]::ASCII.GetBytes('BICO'))
    $bytes.AddRange([byte[]][System.BitConverter]::GetBytes([uint16]$lado))
    $bytes.AddRange([byte[]][System.BitConverter]::GetBytes([uint16]$lado))
    foreach ($f in $filas) {
        if ($f.Length -ne $lado) { Fail ('una fila del icono mide ' + $f.Length + ' y no ' + $lado) }
        foreach ($ch in $f.ToCharArray()) {
            $c = [string]$ch
            if (-not $BICO_PALETA.ContainsKey($c)) { Fail ("el icono usa '" + $c + "', que no esta en la paleta") }
            $bytes.AddRange([byte[]]$BICO_PALETA[$c])
        }
    }
    $esperado = 8 + $lado * $lado * 4
    if ($bytes.Count -ne $esperado) { Fail ('el icono salio de ' + $bytes.Count + ' B y son ' + $esperado) }
    return $bytes.ToArray()
}

$repo = Split-Path -Parent $root
Push-Location $repo
try {
    Compilar-Ejemplos $cobolEjemplos 'bmo-cobol-front' 'cobol' 'ok:|error' $dataBase $repo


    Step 'Building ADA example programs...'
    Compilar-Ejemplos $adaEjemplos 'bmo-ada-front' 'ada' 'ok:|error|linea' $dataBase $repo

    Step 'Building C example programs...'
    # Sin --base ni --asm-path: ese camino usa el PREPROCESADOR, que es lo que
    # resuelve `#include <bmo/...>`. Con ellos se toma el de modulos, que no lo
    # llama.
    Compilar-Ejemplos $cEjemplos 'bmo-c-front' 'c' 'ok:|error' $dataBase $repo

    Step 'Building INTI probes...'
    # ** `run inti/cpu.ibex`. Por el MISMO helper que los otros tres: si INTI
    # necesitara un camino propio al disco, seria que no es un frontend mas. Y es
    # el fallo que este bloque ya tenia escrito de C -- *compila y no se
    # despliega* -- repetido con INTI, cuyo binario vivia fuera del espejo.
    #
    # ** `.ibex` desde el 2026-08-22, y no es un cambio de gusto: es el MISMO
    # formato --lo carga el mismo cargador y lo lee el mismo gate-- con un nombre
    # que dice a que se ha comprometido. Un `.ibex` en el disco declara su
    # perfil, sus piezas y su mesa de katanas, y no habria llegado aqui si esa
    # mesa no cuadrara con sus bytes. `.bex` se queda para los otros tres.
    Compilar-Ejemplos @(@{ src = 'toolchain\lang\inti\sondas\cpu.inti'; out = 'cpu.ibex'; dir = 'inti' }) 'bmo-inti-x86-64' 'inti' 'ok:|error|aviso' $dataBase $repo

    # -- Meter los datos DENTRO del .bex ---------------------------
    #
    # Un `.bex` empaquetado sigue siendo un `.bex` que arranca: el cargador
    # mapea Code/RoData/Data/Bss y **salta el resto contandolo**. Lo que cambia
    # es que la app pasa a ser UN fichero.
    if ($cRecursos.Count -gt 0) {
        Step 'Packaging C examples (datos DENTRO del .bex)'
        $tmp = Join-Path $env:TEMP 'bmo-pack-tmp'
        New-Item -ItemType Directory -Force $tmp | Out-Null
        foreach ($paq in $cRecursos) {
            $bex = Join-Path $dataBase $paq.bex
            if (-not (Test-Path $bex)) { Fail ('no esta ' + $paq.bex + ' para empaquetar') }
            $args = @($bex)
            foreach ($r in $paq.recursos) {
                $f = Join-Path $tmp $r.nombre
                if ($r.ContainsKey('texto')) {
                    # Sin salto final y sin BOM: el programa cuenta los bytes.
                    [System.IO.File]::WriteAllText($f, $r.texto, (New-Object System.Text.UTF8Encoding $false))
                } elseif ($r.ContainsKey('icono')) {
                    [System.IO.File]::WriteAllBytes($f, (Nuevo-Bico $r.icono))
                } else {
                    [System.IO.File]::WriteAllBytes($f, [byte[]]$r.bytes)
                }
                $args += @('-r', ($r.nombre + '=' + $f))
            }
            $args += @('-o', $bex)
            $out = cargo run -p bmo-pack --quiet -- @args 2>&1
            # Solo las lineas de ESTE paso. Un `-match '->'` a secas se traga
            # los `-->` de las advertencias de cargo, y entonces el paso que
            # importa queda enterrado en avisos que no son suyos.
            $out | ForEach-Object {
                if ($_ -match 'recurso\(s\)' -or $_ -match '^\s{4}\S+\s+\d+ B$' -or $_ -match '\[X\]') {
                    Write-Host ('    [pack] ' + $_) -ForegroundColor DarkGray
                }
            }
            if ($LASTEXITCODE -ne 0) { Fail ('no se pudo empaquetar ' + $paq.bex) }
        }
        Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
    }

    # -- DOOM: OPCIONAL, y fuera del arbol ------------------------
    #
    # ** POR QUE ESTE PASO SE SALTA SOLO Y NO FALLA.
    #
    # DOOM es GPL-2.0 y su WAD es de id Software; el arbol de BMO tiene licencia
    # Techne. Ni el codigo ni el WAD pueden vivir aqui, asi que el port entero
    # vive en `BMO-externo\`, al lado del repo y fuera de el.
    #
    # Lo que SI puede vivir aqui es una RUTA. Este paso mira si el port esta; si
    # no esta, dice una linea y sigue. Un `build.ps1` que fallara porque a otro
    # no le apetece bajarse DOOM seria un build roto para todo el mundo menos
    # para el dueno.
    #
    # Se puede apagar con `-SinDoom` aunque el port este puesto: compilar 56.465
    # lineas cuesta lo suyo y no hace falta en cada vuelta.
    #
    # [!] El compilador se invoca con `cwd` = RAIZ DEL REPO (lo pone el
    # `Push-Location $repo` de arriba). `Roots::find` sube desde el cwd para
    # encontrar `tables\`, y desde el arbol de DOOM no la encuentra: un dia
    # entero de sintomas raros salio de esto.
    #
    # `BMO_MODS` apunta a las cabeceras de SONDA, que tapan a las del sistema
    # para lo que DOOM incluye y BMO no tiene (`<direct.h>`, `<io.h>`...). Las
    # que el repo si implementa quedaron apartadas como `*.h.sonda-vieja`
    # justamente para que NO tapen: una cabecera que solo declara esconde a la
    # que tiene cuerpo.
    if (-not $SinDoom) {
        $doomRaiz  = Join-Path (Split-Path -Parent $repo) 'BMO-externo'
        $doomFte   = Join-Path $doomRaiz 'doom\doomgeneric\doomgeneric\doomgeneric_bmo.c'
        $doomWad   = Join-Path $doomRaiz 'doom\doom1.wad'
        $doomInc   = Join-Path $doomRaiz 'doom-port\include'
        if (-not (Test-Path $doomFte)) {
            Write-Host '    [doom] BMO-externo no esta: se salta (es GPL, vive fuera del repo)' -ForegroundColor DarkGray
        } elseif (-not (Test-Path $doomWad)) {
            Write-Host '    [doom] falta doom1.wad: se compila igual, pero no habra con que jugar' -ForegroundColor Yellow
        }
        if (Test-Path $doomFte) {
            Step 'Building DOOM (opcional, GPL, fuera del arbol)'
            $doomDst = Join-Path (Join-Path $dataBase 'apps') 'doom.bex'
            $modsPrevio = $env:BMO_MODS
            $env:BMO_MODS = $doomInc
            try {
                $out = cargo run -p bmo-c-front --quiet -- $doomFte -o $doomDst 2>&1
                $out | ForEach-Object {
                    if ($_ -match 'ok:|error') { Write-Host ('    [doom] ' + $_) -ForegroundColor DarkGray }
                }
                if ($LASTEXITCODE -ne 0) { Fail 'DOOM no compilo' }
                if (-not (Test-Path $doomDst)) { Fail 'DOOM compilo y no salio doom.bex' }
            } finally {
                # Se devuelve SIEMPRE. `BMO_MODS` tapa cabeceras del sistema, y
                # dejarlo puesto haria que el paso siguiente compilara contra
                # las de sonda sin que nadie lo pidiera.
                $env:BMO_MODS = $modsPrevio
            }
            # ** LA CARA DE DOOM, DENTRO DE DOOM.
            #
            # El icono es un recurso mas del paquete, y por eso el escritorio no
            # necesita ni un `.lnk` que apunte aqui ni una cache de iconos que
            # reconstruir: copias el `.bex` y va con su cara. Ver
            # `escena\lanzador.rs`.
            #
            # Se dibuja aqui y no se trae de fuera **porque una imagen de DOOM
            # seria de id Software**. Estos 256 pixeles son originales, y por
            # eso pueden vivir en este fichero mientras el resto del port no.
            $doomIco = Join-Path $env:TEMP 'bmo-doom-icono'
            [System.IO.File]::WriteAllBytes($doomIco, (Nuevo-Bico @(
                '................',
                '.....RRRRRR.....',
                '...RRRRRRRRRR...',
                '..RRRRRRRRRRRR..',
                '..RRRRRRRRRRRR..',
                '..RRoooRRoooRR..',
                '..RRoWoRRoWoRR..',
                '..RRoooRRoooRR..',
                '..RRRRRRRRRRRR..',
                '..RRRddddddRRR..',
                '..RRdWdWdWdWdR..',
                '..RRddddddddRR..',
                '...RRRRRRRRRR...',
                '....RRRRRRRR....',
                '................',
                '................'
            )))
            $out = cargo run -p bmo-pack --quiet -- $doomDst '-r' ('icono=' + $doomIco) '-o' $doomDst 2>&1
            $out | ForEach-Object {
                if ($_ -match 'recurso\(s\)' -or $_ -match '\[X\]') {
                    Write-Host ('    [doom] ' + $_) -ForegroundColor DarkGray
                }
            }
            if ($LASTEXITCODE -ne 0) { Fail 'no se pudo meter el icono en doom.bex' }
            Remove-Item -Force $doomIco -ErrorAction SilentlyContinue

            # El WAD va al lado, tal cual. **No se empaqueta dentro del `.bex`**
            # aunque el formato lo permita: `lanzar.rs::con_buffer` se trae el
            # fichero ENTERO a un bufer de 4 MiB, asi que un paquete de 5,5 MB
            # no arrancaria. Ver el escalon 2 de `docs\identidad\LA_RAM.md` -- el dia que
            # el cargador lea solo lo cargable, esto pasa a ser un `bmo-pack`.
            if (Test-Path $doomWad) {
                $wadDst = Join-Path (Join-Path $dataBase 'apps') 'doom1.wad'
                Copy-Item -LiteralPath $doomWad -Destination $wadDst -Force
                Write-Host ('    [doom] doom1.wad (' + (Get-Item $wadDst).Length + ' B) -> apps\') -ForegroundColor DarkGray
            }
            # El consejo de aqui decia "desde el shell de Ring 0", y eso era
            # cierto mientras `lend_screen` tenia un plazo de 500 ms: DOOM tarda
            # ~10 s en reclamar la pantalla, el escritorio se cansaba y se la
            # quedaba. Arreglado -- ahora la espera es por VIDA y no por reloj,
            # asi que el camino bueno es el ICONO: es el unico que devuelve la
            # pantalla al escritorio cuando el programa muere. Por el shell de
            # Ring 0 no hay quien la recupere y se acaba en el panel del kernel.
            Write-Host '    [doom] lanzalo con:  CLIC en su icono del escritorio' -ForegroundColor DarkGray
            Write-Host '    [doom]   (`run apps/doom.bex` desde Ring 0 tambien va, pero al morir' -ForegroundColor DarkGray
            Write-Host '    [doom]    la pantalla NO vuelve al escritorio: se queda el kernel)' -ForegroundColor DarkGray
        }
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

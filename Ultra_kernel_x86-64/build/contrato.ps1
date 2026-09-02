# QUE LAS CUATRO COPIAS DE LA MISMA TABLA DIGAN LO MISMO.
#
# ** Por que esto es un fichero (2026-08-28)
#
# Porque es el trozo mas grande de `build.ps1` y el mas AJENO a construir nada:
# no compila, no copia y no escribe. Solo LEE y compara. Un fichero que solo
# juzga se puede leer entero sin miedo, y mezclado con los pasos que si tocan
# el disco no se distinguia de ellos.
#
# Las cuatro copias que se carean aqui:
#
#     el kernel        syscall/ops.rs y los `*_OP_*` de cada objeto
#     el ABI           platform/abi/bmo-abi/.../surface/
#     el userland      Ultra_userspace/userland/src/
#     bmo.h            la cara en C de la misma tabla
#
# Y ademas: que la cara generada de MAQUETA siga derivando de su `.maqueta`.
#
# [!] Se carga con punto: corre en el ambito de `build.ps1` y usa sus `Step`,
# `Fail` y `$root`. Es el mismo texto en el mismo orden.

# Keep the no-alloc Ring 0 syscall view synchronized with canonical bmo-abi.
Step 'Validating Ring 0 syscall contract'
# Las operaciones del kernel no viven todas en `syscall.rs`: las de un objeto
# estan con su objeto (`ARCH_OP_*` en `obj\file.rs`), que es donde deben
# estar. Se leen los dos y se comparan contra el MISMO surface.
# ** La tabla se mudo a `syscall\ops.rs` el 2026-08-13: los NUMEROS son el
# contrato y estaban mezclados con el despacho que los sirve. Este guardian lo
# cazo en el acto --`NR_INVOKE` desaparecido-- que es exactamente para lo que
# esta, y por eso el reparto no pudo colarse.
# ** Y DESDE EL 2026-08-17 SE LEEN LOS CINCO FICHEROS DE `obj\`, NO UNO.
#
# La lista decia `file.rs` y ya explicaba por que --"las de un objeto estan con
# su objeto"-- pero solo nombraba uno. Los otros cuatro objetos tienen las
# suyas: `memory.rs`, `directory.rs`, `input.rs` y `audio.rs`. Al ensancharlo
# aparecieron **las tres operaciones del directorio, que el ABI no tenia**:
# `DIR_OP_SIGUIENTE`, `DIR_OP_NOMBRE` y `DIR_OP_CERRAR` llevaban desde que
# existe `ls` sirviendose en el kernel y mandandose desde el userland, sin una
# sola linea en el contrato.
#
# Es exactamente lo que ya paso el 2026-08-12 con cuatro `TASK_OP_*`, y la
# leccion es la misma escrita mas abajo: **un guardian que lee menos no avisa de
# menos, avisa de nada**. Se leen por PATRON y no por lista, para que un objeto
# nuevo entre solo.
$kernelObj = @(Get-ChildItem -Path (Join-Path $root 'kernel\src\ring0\obj') -Filter '*.rs' -File -ErrorAction SilentlyContinue)
if ($kernelObj.Count -eq 0) {
    Fail 'no hay ni un .rs en kernel\src\ring0\obj -- las operaciones de los handles no se pueden comprobar'
}
# *** LA CARPETA ENTERA, Y NO DOS FICHEROS POR SU NOMBRE (2026-08-24).
#
# ** Aqui habia esto: `ops.rs` + `mod.rs` + $kernelObj. Dos ficheros nombrados
# a mano.
#
# El 24-08 `syscall/mod.rs` se partio en cinco --op_maquina, op_aparato,
# op_contar, op_consola-- y las operaciones que se fueron a esos cuatro
# **dejaron de existir para este guardian en el acto**. Una que el ABI perdiera
# no la habria cazado nadie.
#
# *** Y la leccion estaba escrita VEINTE LINEAS MAS ABAJO, en este mismo fichero,
# de la vez anterior que paso: *"un guardian que solo mira la mitad da una
# tranquilidad que no ha ganado"*. Volvio a pasar, y esta vez la victima fue el
# reparto que lo provoco.
#
# Ahora se lee la CARPETA. Partir un fichero deja de poder cegar al guardian,
# porque ya no hay ninguna lista de nombres que se pueda quedar corta.
$kernelSyscallDir = Join-Path $root 'kernel\src\ring0\syscall'
$kernelSyscallFiles = @(Get-ChildItem -Path $kernelSyscallDir -Filter '*.rs' -File -Recurse -ErrorAction SilentlyContinue)
if ($kernelSyscallFiles.Count -lt 2) {
    Fail ('apenas hay .rs en ' + $kernelSyscallDir + ' -- el contrato no se puede comprobar, y seguir seria fingir que si')
}
Write-Host ('    contrato: ' + $kernelSyscallFiles.Count + ' ficheros de syscall/ leidos') -ForegroundColor DarkGray
$kernelSyscalls = (($kernelSyscallFiles | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n") + "`n" +
                  (($kernelObj | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n")
# ** EL CONTRATO YA NO ES UN FICHERO: ES UNA CARPETA (2026-08-12).
#
# `surface.rs` llego a 1.166 lineas con 186 constantes en una lista plana y se
# partio en cinco --puertas, tarea, objetos, entrada, informe--. Los TRES
# guardianes de abajo lo leen con `-Raw`, asi que seguir leyendo solo el fichero
# viejo habria hecho que **pasaran creyendo que comprueban**, que es peor que no
# tenerlos: un guardian que lee menos no avisa de menos -- avisa de nada, y con
# tono tranquilizador.
#
# Se concatena la carpeta entera. Y se comprueba que haya algo dentro: si alguien
# la renombra, esto tiene que PARAR en vez de seguir con una cadena vacia, porque
# una cadena vacia hace pasar todas las comprobaciones de golpe.
$abiSurfaceDir = Join-Path $root '..\platform\abi\bmo-abi\src\syscalls\surface'
$abiSurfaceFiles = @(Get-ChildItem -Path $abiSurfaceDir -Filter '*.rs' -File -ErrorAction SilentlyContinue)
if ($abiSurfaceFiles.Count -eq 0) {
    Fail ('no hay ni un .rs en ' + $abiSurfaceDir + ' -- el contrato no se puede comprobar, y seguir seria fingir que si')
}
$abiSurface = ($abiSurfaceFiles | ForEach-Object { Get-Content $_.FullName -Raw }) -join "`n"
Write-Host ('    contrato: ' + $abiSurfaceFiles.Count + ' ficheros de surface/ leidos') -ForegroundColor DarkGray
foreach ($name in @('NR_INVOKE', 'NR_CHANNEL_KICK', 'NR_WAIT')) {
    $kernelMatch = [regex]::Match($kernelSyscalls, ('const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u32\s*=\s*(0x[0-9A-Fa-f]+)'))
    if (-not $kernelMatch.Success -or -not $abiMatch.Success -or $kernelMatch.Groups[1].Value -ne $abiMatch.Groups[1].Value) {
        Fail ('BMO ABI surface syscall contract mismatch: ' + $name)
    }
}
# ** LA LISTA A MANO SE FUE, Y ESTA ES SU LECCION (2026-08-12).
#
# Aqui habia una lista de 34 nombres escrita a mano, con este aviso encima:
# *"se habia quedado congelada en las del principio... un guardian que solo mira
# la mitad da una tranquilidad que no ha ganado"*.
#
# Le habia vuelto a pasar. Al barrer el kernel entero aparecieron **CUATRO
# operaciones que el ABI no tenia**: ENDPOINT_CONNECT, PANTALLA_SOLTAR,
# ENTRADA_SOLTAR y AUDIO_CENSO. Y las dos de SOLTAR son justo las que el
# comentario de mas abajo cita como las que casi chocan con la autopsia: estaban
# en la HISTORIA del fichero y no en el CONTRATO.
#
# Un guardian con lista tiene el mismo fallo que vigila: alguien tiene que
# acordarse de anadir la fila. Ahora se barren TODOS los `TASK_OP_*` y
# `ARCH_OP_*` del kernel y se exige que cada uno exista en el ABI con el MISMO
# numero. Anadir una operacion pasa a ser imposible de olvidar.
#
# ** Y DESDE EL 2026-08-16 tambien las CLASES del histograma (`SYSCALL_CLASS_*`).
# No son operaciones, pero cruzan a Ring 3 igual que ellas y tienen el mismo modo
# de fallo, que ademas es mudo: si el kernel cuenta la consola en la casilla 2 y
# Ring 3 lee la 1, sale un reparto **coherente y falso**. Entran aqui en vez de
# en un guardian nuevo porque la comprobacion es identica -- mismo nombre, mismo
# numero, en los dos lados.
# ** Y DESDE EL 2026-08-17, TODAS LAS FAMILIAS -- no dos y media.
#
# Barria `TASK_OP_*`, `ARCH_OP_*` y `SYSCALL_CLASS_*`. Las demas familias de
# operacion --las de los otros handles, y las preguntas del cursor de ESTRATOS--
# **no las miraba nadie**, y ahi vivia esto:
#
#   `MEM_OP_OFRECER` valia 0x03 en el ABI y en el userland, y el despacho del
#   kernel comparaba contra una copia local suya que decia **0x02**. Dos fallos
#   mudos: prestar memoria no entraba en su brazo, y `MEM_OP_BYTES` --que ES el
#   0x02-- entraba en el de prestar. Ninguno de los dos falla en voz alta.
#
# Es el mismo modo de fallo que ya documenta el parrafo de arriba y la misma
# cura: no una fila mas en una lista, sino **una familia menos que se escape**.
# El patron cubre `X_OP_*` sea cual sea la X, mas las tres que no se llaman asi
# (`SYSCALL_CLASS_*`, `ES_NODO_*`, `ES_TXT_*`, `DISCO_TRIM_*`).
$opsTodas = [regex]::Matches($kernelSyscalls, 'const\s+(\w+_OP_\w+|SYSCALL_CLASS_\w+|ES_NODO_\w+|ES_TXT_\w+|DISCO_TRIM_\w+)\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)')

# ** Y SE COMPARAN NUMEROS, NO CADENAS.
#
# Mientras solo entraban constantes en hexadecimal, comparar el texto valia.
# Ahora entran tambien las decimales --`ES_TXT_RUTA = 1`, `DISCO_TRIM_HECHO = 0`--
# y `0x01` contra `1` son la misma cifra escrita de dos formas: comparadas como
# texto darian un fallo FALSO, que es la peor clase de guardian. Con este
# convertidor, el lado que este escrito en hexadecimal y el que este en decimal
# dicen lo mismo cuando valen lo mismo.
function ComoNumero($t) {
    $t = $t.Replace('_', '')
    if ($t -match '^0[xX]') { return [Convert]::ToInt64($t.Substring(2), 16) }
    return [Convert]::ToInt64($t, 10)
}
foreach ($m in $opsTodas) {
    $name = $m.Groups[1].Value
    $numK = ComoNumero $m.Groups[2].Value
    $abiMatch = [regex]::Match($abiSurface, ('pub const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)'))
    if (-not $abiMatch.Success) {
        Fail ('BMO ABI surface operation contract: ' + $name + ' esta en el kernel y NO en el ABI')
    }
    if ((ComoNumero $abiMatch.Groups[1].Value) -ne $numK) {
        Fail ('BMO ABI surface operation contract mismatch: ' + $name)
    }
}
Write-Host ('    operaciones kernel<->ABI: ' + $opsTodas.Count + ' comprobadas, ninguna a mano') -ForegroundColor DarkGray

# ** Y EL TERCER LADO: EL USERLAND (2026-08-17).
#
# Las operaciones existen TRES veces, igual que la tabla de `OP_INFO` de mas
# abajo: las sirve el kernel, las declara el ABI y **las manda el userland**. El
# guardian de arriba cruza dos, asi que un numero mal escrito en `bmo::` no lo
# miraba nadie -- y ese es el lado que de verdad viaja por la puerta.
#
# El nombre se traduce con UNA regla mecanica y no con un diccionario: en el
# userland, lo que se pide sobre `CURRENT_TASK` pierde el prefijo (`OP_INFO` es
# `TASK_OP_INFO`) y lo que se pide sobre un handle se llama igual
# (`MEM_OP_OFRECER`, `FB_OP_BASE`). Si algun dia hiciera falta un diccionario,
# el problema seria el nombre y no el guardian.
$userland = Get-Content (Join-Path $root '..\Ultra_userspace\userland\src\lib.rs') -Raw
$opsUser = [regex]::Matches($userland, '(?m)^\s*pub const\s+(\w*OP_\w+|ES_NODO_\w+|ES_TXT_\w+|DISCO_TRIM_\w+)\s*:\s*u\d+\s*=\s*(0x[0-9A-Fa-f_]+|\d+)\s*;')
foreach ($m in $opsUser) {
    $name = $m.Groups[1].Value
    $numU = ComoNumero $m.Groups[2].Value
    $candidatos = @($name)
    if ($name.StartsWith('OP_')) { $candidatos += ('TASK_' + $name) }
    $enAbi = $null
    foreach ($c in $candidatos) {
        $hit = [regex]::Match($abiSurface, ('pub const\s+' + $c + '\s*:\s*u\d+\s*=\s*(0x[0-9A-Fa-f_]+|\d+)\s*;'))
        if ($hit.Success) { $enAbi = $hit.Groups[1].Value; break }
    }
    if ($null -eq $enAbi) {
        Fail ('contrato: ' + $name + ' esta en el userland y NO en el ABI (ni como TASK_' + $name + ')')
    }
    if ((ComoNumero $enAbi) -ne $numU) {
        Fail ('contrato: ' + $name + ' vale distinto en el userland y en el ABI')
    }
}
Write-Host ('    operaciones userland<->ABI: ' + $opsUser.Count + ' comprobadas') -ForegroundColor DarkGray

# ** EL CUARTO GUARDIAN: EL FORMATO DEL HANDLE (2026-08-16).
#
# Los tres de arriba comprueban NUMEROS DE OPERACION. Ninguno miraba **como
# esta construido un handle**, y ese formato estaba escrito DOS VECES:
# `ring0/obj/cap.rs` y `abi/fundamentals/handle/opaque.rs`. El kernel ni lo
# disimulaba -- su comentario dice *"mirror of bmo-abi handle/kind.rs"*.
#
# Es la forma exacta del `#GP(0x18)` de este mismo dia: el mismo numero en dos
# ficheros que no se hablan. Alli costo un arranque; aqui habria costado peor,
# porque un desplazamiento mal puesto COMPILA, devuelve "handle invalido" y se
# lee como un permiso denegado.
#
# [!] Y el fichero del ABI NO esta en `syscalls/surface/`, asi que hay que
# leerlo aparte. Se exige que exista: si alguien lo mueve, esto PARA en vez de
# comprobar contra una cadena vacia -- que es la trampa que ya documenta el
# guardian de arriba.
$abiHandleFile = Join-Path $root '..\platform\abi\bmo-abi\src\fundamentals\handle\opaque.rs'
if (-not (Test-Path $abiHandleFile)) {
    Fail ('no esta ' + $abiHandleFile + ' -- el formato del handle no se puede comprobar, y seguir seria fingir que si')
}
$abiHandle = Get-Content $abiHandleFile -Raw
$capFile = Join-Path $root 'kernel\src\ring0\obj\cap.rs'
if (-not (Test-Path $capFile)) { Fail ('no esta ' + $capFile) }
$capSrc = Get-Content $capFile -Raw
$fmtTodos = [regex]::Matches($capSrc, 'pub const\s+(HANDLE_\w+)\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)')
if ($fmtTodos.Count -eq 0) {
    Fail 'el kernel no declara ni una constante HANDLE_* -- el formato volvio a numeros a pelo'
}
foreach ($m in $fmtTodos) {
    $name = $m.Groups[1].Value
    $numK = $m.Groups[2].Value.ToUpperInvariant().Replace('_', '')
    $abiMatch = [regex]::Match($abiHandle, ('pub const\s+' + $name + '\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)'))
    if (-not $abiMatch.Success) {
        Fail ('formato del handle: ' + $name + ' esta en el kernel y NO en el ABI')
    }
    if ($abiMatch.Groups[1].Value.ToUpperInvariant().Replace('_', '') -ne $numK) {
        Fail ('formato del handle DISTINTO entre kernel y ABI: ' + $name)
    }
}
Write-Host ('    formato del handle: ' + $fmtTodos.Count + ' campos, kernel y ABI dicen lo mismo') -ForegroundColor DarkGray

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
# (`core\report.rs`), la declara el ABI (`surface.rs`) y la consume el userland
# (`userland\src\lib.rs`). Anadir un dato es una fila -- y una fila escrita en
# dos de los tres sitios es un campo que contesta otra cosa de la que se pidio,
# sin que nada falle al compilar.
#
# No es hipotetico: al escribir esta comprobacion, `INFO_PANTALLA_DUENO` estaba
# en el kernel y en el userland y NO en el ABI. La lista no se escribe a mano
# --se saca de los tres ficheros-- porque una lista a mano es lo que ya se
# quedo congelada una vez, ahi arriba.
#
# ** Y LO MISMO PARA LOS BITS DE `USB_SALUD_*`, que no son un campo sino el
# CONTENIDO de uno. Un id que no cuadra se ve enseguida --se pide un dato y sale
# otro--; un BIT que no cuadra pinta la luz del teclado del color equivocado y
# no falla nada. La segunda mitad de la tabla necesita el mismo guardian que la
# primera, asi que se barre con el, y por eso el lado del kernel son DOS
# ficheros: `report.rs` implementa las filas y `dev\usb\salud.rs` los bits.
$infoFuentes = [ordered]@{
    'kernel'   = (Get-Content (Join-Path $root 'kernel\src\ring0\core\report.rs') -Raw) + "`n" +
                 (Get-Content (Join-Path $root 'kernel\src\ring0\dev\usb\salud.rs') -Raw) + "`n" +
                 (Get-Content (Join-Path $root 'kernel\src\ring0\syscall\presupuesto.rs') -Raw)
    'abi'      = $abiSurface
    'userland' = Get-Content (Join-Path $root '..\Ultra_userspace\userland\src\lib.rs') -Raw
}
$infoCampos = @{}
foreach ($fuente in $infoFuentes.GetEnumerator()) {
    # Los ids van en hexadecimal y los bits se escriben como `1 << n`, que es
    # como se leen. Se normaliza a decimal para poder compararlos entre si: la
    # comprobacion es sobre el VALOR, no sobre como esta escrito.
    $hallados = [regex]::Matches($fuente.Value, '(?m)^\s*(?:pub\s+)?const\s+((?:INFO|USB_SALUD|MAQ)_[A-Z0-9_]+)\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|1\s*<<\s*\d+|\d+)')
    foreach ($m in $hallados) {
        $campo = $m.Groups[1].Value
        $crudo = $m.Groups[2].Value.Replace('_', '')
        if ($crudo -match '^1\s*<<\s*(\d+)$') {
            $valor = ([uint64]1 -shl [int]$Matches[1]).ToString()
        } elseif ($crudo -match '^0[xX]') {
            $valor = ([System.Convert]::ToUInt64($crudo.Substring(2), 16)).ToString()
        } else {
            $valor = ([uint64]$crudo).ToString()
        }
        if (-not $infoCampos.ContainsKey($campo)) { $infoCampos[$campo] = [ordered]@{} }
        $infoCampos[$campo][$fuente.Key] = $valor
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

# == ** LA CARA GENERADA NO PUEDE DERIVAR DE SU `.maqueta` (2026-08-18) ======
#
# MAQUETA existe para que la cara del escritorio tenga **una sola verdad**: se
# edita el `.maqueta` y el compilador emite el Rust. Pero el `_gen.rs` esta
# COMMITEADO y **nadie lo regeneraba**, asi que hasta hoy habia dos verdades que
# coincidian por suerte:
#
#     se edita el .maqueta y se olvida regenerar
#       -> el escritorio pinta la cara VIEJA
#       -> y el fichero que dice ser la fuente dice otra cosa
#       -> no falla nada, ni al compilar ni al arrancar
#
# ** Es exactamente el modo de fallo que esta casa ya vigila en cuatro sitios
# --el ABI, `bmo.h`, el formato del handle, la tabla de `OP_INFO`-- y el mas
# ironico de todos: el sistema entero se construyo para no tener la maquetacion
# escrita dos veces.
#
# No se comprueba con una lista: se barren los `.maqueta` que hay, se regenera
# cada uno a un temporal y se compara. Un fichero nuevo entra solo.
Step 'Validating generated faces match their .maqueta'
$maqDir = Join-Path $root '../toolchain/tools/maqueta/pruebas'
$maqFiles = @(Get-ChildItem -Path $maqDir -Filter '*.maqueta' -File -ErrorAction SilentlyContinue)
if ($maqFiles.Count -eq 0) {
    Fail ('no hay ni un .maqueta en ' + $maqDir + ' -- las caras generadas no se pueden comprobar')
}
# Donde vive el Rust de cada cara. Por convencion: `<nombre>_gen.rs` en la
# escena del compositor -- la misma que declara la cabecera del generado.
$escena = Join-Path $root '../Ultra_userspace/services/director/src/scene'
$lf = [string][char]10
$crlf = [string][char]13 + $lf
foreach ($maq in $maqFiles) {
    $gen = Join-Path $escena ($maq.BaseName + '_gen.rs')
    if (-not (Test-Path $gen)) {
        Fail ('la cara ' + $maq.Name + ' no tiene su ' + $maq.BaseName + '_gen.rs -- o se genero y no se guardo, o el nombre no sigue la convencion')
    }
    $tmp = Join-Path $env:TEMP ($maq.BaseName + '_gen.comprobacion.rs')
    Push-Location (Join-Path $root '..')
    $salida = (cargo run -q -p bmo-maqueta -- $maq.FullName $tmp 2>&1 | Out-String)
    Pop-Location
    if (-not (Test-Path $tmp)) {
        Write-Host $salida
        Fail ('el compilador de maqueta no emitio nada para ' + $maq.Name)
    }
    # Los finales de linea no son la cara: se normalizan antes de comparar, o
    # este guardian gritaria por un `git config` distinto en otra maquina.
    $a = (Get-Content $gen -Raw).Replace($crlf, $lf)
    $b = (Get-Content $tmp -Raw).Replace($crlf, $lf)
    Remove-Item $tmp -ErrorAction SilentlyContinue
    if ($a -ne $b) {
        Fail ('DERIVA: ' + $maq.Name + ' y ' + $maq.BaseName + '_gen.rs dicen caras distintas. Regenera con: cargo run -p bmo-maqueta -- ' + $maq.FullName + ' ' + $gen)
    }
}
Write-Host ('    caras: ' + $maqFiles.Count + ' .maqueta, y su Rust generado dice lo mismo') -ForegroundColor DarkGray

# -- ** Y LA CUARTA COPIA: `bmo.h`, LA CARA EN C DE LA MISMA TABLA --------
#
# El 2026-08-17 se anadieron `INFO_PRESUPUESTO_MAQUINA` y `INFO_SUELO_CRUCE` a
# los tres sitios de arriba **y al `.h`**, y este guion dijo verde: no miraba el
# `.h`. Lo caz  el banco del anfitrion (`bmo_h_cruza_de_lenguaje`) minutos
# despues, en el despliegue.
#
# ** El guardian que se corre a mano no protege igual que el que se corre solo.
# `build.ps1 -BuildOnly` es lo que se ejecuta veinte veces al dia; el banco, en
# el despliegue. Un fallo que solo aparece en el segundo se descubre con el disco
# en la mano.
#
# Aqui solo se comprueba la familia `INFO_`, y por una razon concreta: es la
# unica cuyo nombre en C es **mecanicamente** el de Rust con `BMO_` delante. Las
# operaciones no (`BMO_OP_CEDER` es `TASK_OP_YIELD`), y traducirlas pide el
# diccionario a mano que ya vive --con su porque-- en la prueba del ABI. Este
# guardian no la sustituye: le quita el trabajo que puede hacer solo.
#
# ** Y DESDE EL 2026-09-02 SE SIGUE LA FACHADA, no un fichero.
#
# `bmo.h` se partio por carriles (L6g): las constantes se mudaron a
# `bmo/roja.h` y `bmo/verde.h`, y `bmo.h` quedo como fachada que las incluye.
# Este guardian leia UN fichero, encontro **cero** constantes... y paso en
# verde, porque cero de cero coinciden.
#
# *** Eso es peor que fallar. Es literalmente lo que este mismo build tiene
# escrito dos veces: *"un guardian que lee menos no avisa de menos, avisa de
# nada"*. Por eso van los DOS arreglos, y el segundo es el que vale para el
# proximo: **cero constantes es un FALLO**, no un OK.
$hDirTablas = Join-Path $root '..\toolchain\forge\sem-asm\tables'
$hRuta = Join-Path $hDirTablas 'bmo\bmo.h'
if (Test-Path $hRuta) {
    # La fachada y todo lo que trae. Solo `<bmo/...>`: `<stdlib.h>` no es la
    # superficie del kernel, y meterlo traeria constantes que no cruzan.
    $hPendientes = New-Object System.Collections.Queue
    $hPendientes.Enqueue('bmo/bmo.h') | Out-Null
    $hVistos = @{}
    $hTexto = ''
    while ($hPendientes.Count -gt 0) {
        $rel = $hPendientes.Dequeue()
        if ($hVistos.ContainsKey($rel)) { continue }
        $hVistos[$rel] = $true
        $f = Join-Path $hDirTablas ($rel -replace '/', '\')
        if (-not (Test-Path $f)) {
            Fail ('la fachada de bmo.h trae ' + $rel + ' y no esta en ' + $f)
        }
        $t = Get-Content $f -Raw
        $hTexto = $hTexto + $t + "`n"
        foreach ($inc in [regex]::Matches($t, '(?m)^\s*#include\s+<(bmo/[^>]+)>')) {
            $hPendientes.Enqueue($inc.Groups[1].Value) | Out-Null
        }
    }
    $hCampos = [regex]::Matches($hTexto, '(?m)^\s*#define\s+BMO_(INFO_[A-Z0-9_]+)\s+(0x[0-9A-Fa-f]+|\d+)')
    $hMal = 0
    foreach ($m in $hCampos) {
        $campo = $m.Groups[1].Value
        $crudo = $m.Groups[2].Value
        if ($crudo -match '^0[xX]') {
            $valor = ([System.Convert]::ToUInt64($crudo.Substring(2), 16)).ToString()
        } else {
            $valor = ([uint64]$crudo).ToString()
        }
        if (-not $infoCampos.ContainsKey($campo)) {
            Fail ('bmo.h: BMO_' + $campo + ' no existe en el ABI -- una constante de C sin pareja en Rust')
        }
        $enAbi = $infoCampos[$campo]['abi']
        if ($enAbi -ne $valor) {
            Fail ('bmo.h: BMO_' + $campo + ' vale ' + $valor + ' y el ABI dice ' + $enAbi)
        }
        $hMal++
    }
    # ** CERO ES UN FALLO. Un guardian que no encuentra nada no esta diciendo
    # que todo este bien: esta diciendo que no ha mirado, y lo dice con la misma
    # voz. Es como este se quedo mudo al partirse la cabecera.
    if ($hMal -eq 0) {
        Fail ('bmo.h: CERO constantes INFO_ leidas de la fachada y sus carriles. Un guardian que no encuentra nada no ha mirado -- revisa que ' + $hRuta + ' siga trayendo lo que dice')
    }
    Write-Host ('    bmo.h: ' + $hMal + ' constantes INFO_ (fachada + ' + ($hVistos.Count - 1) + ' carriles), el mismo valor que el ABI') -ForegroundColor DarkGray
} else {
    Fail ('no se encuentra bmo.h en ' + $hRuta + ' -- el contrato de C no se puede comprobar')
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


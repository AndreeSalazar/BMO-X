# BMO -- una sola orden.
#
# `.\bmo.ps1`              comprueba y construye TODO. No toca ningun disco.
# `.\bmo.ps1 -Desplegar`   ademas lo lleva al Kingston.
# `.\bmo.ps1 -Rapido`      se salta el banco de pruebas (para iterar).
#
# == Por que existe, siendo que `build.ps1` ya construye ==
#
# `build.ps1` construye. Lo que NO hace es **comprobar antes**: el banco de
# pruebas del compilador y los crates del anfitrion se corren a mano, y lo que
# se corre a mano se deja de correr. Esta orden los pone delante del build, que
# es el unico sitio donde no se olvidan.
#
# Y el orden importa: primero lo que se prueba en el anfitrion --rapido y
# barato-- y solo si pasa se construye lo que va a la maquina. Al reves se
# tarda tres minutos en descubrir algo que un test decia en tres segundos.

param(
    [switch]$Desplegar,
    [switch]$Rapido,
    # A que unidades va, si se despliega. Se piden EXPLICITAS y sin valor por
    # defecto util: ver la nota de abajo.
    [string]$Arranque = 'D',
    [string]$Datos = 'A',
    # ** EL KERNEL DE MEDIDA. Devuelve los dos `rdtsc` a `dispatch`, o sea el
    # REPARTO de una puerta entre su mitad Rust y el resto -- lo unico que puede
    # decir donde se van los ~945 ciclos.
    #
    # [!] NO SE DEJA PUESTO: cuesta ~112 ciclos en CADA puerta de CADA programa,
    # un 11%. Se despliega, se corre `sys/precio.bex`, se apunta el reparto y se
    # vuelve a desplegar sin esta bandera. Un instrumento que se queda puesto
    # deja de ser una medida y pasa a ser un peaje.
    [switch]$Metro
)

# [!] NADA DE `$ErrorActionPreference = 'Stop'` AQUI.
#
# Estuvo puesto y rompio DOS cosas, la segunda peor que la primera:
#
#   1. El bucle de `cargo test`, que moria en el primer warning -- cada linea
#      de stderr de un nativo es un ErrorRecord, y con `Stop` es terminante.
#   2. **`build.ps1` entero**, que se ejecuta en esta misma sesion y por tanto
#      HEREDA la preferencia. Un guion que llevaba meses funcionando empezo a
#      morirse en `warning: unused variable`, y no por nada suyo: por una linea
#      escrita en el guion que lo llama.
#
# Aqui los fallos se detectan mirando lo que las herramientas DICEN --las filas
# que pasan, el codigo de salida del build-- y no dejando que el shell decida
# que un aviso del compilador es motivo para abortar.
$raiz = $PSScriptRoot
$t0 = Get-Date

function Titulo($m) { Write-Host "`n== $m" -ForegroundColor Cyan }
function Bien($m)   { Write-Host "   OK  $m" -ForegroundColor DarkGray }
function Muere($m)  { Write-Host "   [X] $m" -ForegroundColor Red; exit 1 }

Write-Host "BMO-X -- comprobar y construir" -ForegroundColor White

# -- 1. EL BANCO, ANTES QUE NADA ----------------------------------------
#
# Se corre primero porque es lo mas barato que puede decir que no. El banco de
# `bmo-c-front` EJECUTA los programas en el emulador: no comprueba que
# compilen, comprueba que hagan lo que dicen.
if (-not $Rapido) {
    Titulo 'Banco de pruebas (anfitrion)'
    Push-Location $raiz
    try {
        # ** ESTA LISTA ERA DE SIETE, Y LO QUE FALTABA NO ERA POCO.
        #
        # Es a mano, y por tanto tiene el mismo fallo que vigila el guardian de
        # opcodes de `build.ps1`: *"un guardian con lista tiene el mismo fallo
        # que vigila"*. Se le habian quedado fuera doce crates con casillas
        # escritas que nadie corria.
        #
        #   17-08  los SEIS del almacenamiento     118 casillas
        #          aqui vive la ventana de escritura --la que deja fuera la
        #          particion de arranque-- y el empaquetado de TRIM, donde
        #          equivocarse no da un fallo: hace que el disco olvide
        #          sectores que si importaban.
        #
        #   18-08  las SEIS de MAQUETA             134 casillas
        #          el nieto calcula donde cae cada caja, y una cuenta mal
        #          puesta no da un error -- **pinta el escritorio torcido**, y
        #          eso ninguna compilacion lo caza.
        #
        #   18-08  `bmo-cobol-front`                226 casillas
        #          ** LA MAS GORDA QUE FALTABA, y la que mas duele: es el
        #          hermano de `bmo-c-front`, que si estaba desde el primer dia.
        #          Ahi dentro vive el banco que EJECUTA los programas en el
        #          emulador -- el mismo que descubrio que `IF` ejecutaba las
        #          dos ramas. Y ahi estan ahora las nueve de `calcgui`, que son
        #          las unicas que comprueban que un motor de COBOL lanzado por
        #          el escritorio contesta la cuenta que se le pidio.
        #   24-08  *** SE ACABO LA LISTA, Y ESTE ES EL MOTIVO
        #
        #          Al meter `bmo-cripto` toco anadirlo a mano, y de paso se
        #          conto lo que la lista se estaba dejando fuera:
        #
        #            en el workspace   56 crates
        #            en la lista       20
        #            sin ejecutar     911 filas que EXISTEN Y PASAN
        #
        #          El banco cantaba 1315. El numero de verdad era 2226: se
        #          callaba el 41%. Y lo que faltaba no era relleno -- eran
        #          `bmo-inti-front` (274) y `bmo-inti-x86-64` (256), o sea
        #          **INTI ENTERO**, la pieza mas nueva y la que mas se toca.
        #
        #          ** Es el mismo fallo que tuvo el guardian del contrato, que
        #          leia dos ficheros de `syscall/` POR NOMBRE y se quedo ciego
        #          cuando la carpeta paso a tener once. La forma se repite:
        #
        #            una lista escrita a mano no se queda obsoleta de golpe,
        #            se queda obsoleta cada vez que alguien no se acuerda.
        #
        #          Asi que el banco ya no lleva lista: **le pregunta al
        #          workspace**. Un crate nuevo entra probado por defecto en vez
        #          de quedarse fuera por defecto, que es al reves de como
        #          estaba y es la unica diferencia que importa.
        #
        # [!] Y lo que NO se puede probar aqui se dice con su razon, no se
        #     omite en silencio. Omitir en silencio es como empezo esto.
        $noSePueden = @{
            'bmo-kernel' = 'binario bare-metal: enlazado como test, `panic_impl` sale dos veces'
        }
        $antes = $ErrorActionPreference
        $ErrorActionPreference = 'Continue'
        $meta = (cargo metadata --no-deps --format-version 1 2>$null | Out-String | ConvertFrom-Json)
        $ErrorActionPreference = $antes
        $paquetes = $meta.packages | ForEach-Object { $_.name } |
                    Where-Object { -not $noSePueden.ContainsKey($_) } | Sort-Object
        $total = 0
        $sinBanco = @()
        foreach ($p in $paquetes) {
            # [!] AQUI SE DECIDE POR CONTEO, NO POR FRASE NI POR CODIGO DE SALIDA.
            #
            # Costo tres intentos y los tres fallaron distinto, asi que queda
            # escrito: PowerShell 5.1 envuelve cada linea de stderr de un
            # ejecutable nativo en un ErrorRecord.
            #
            #   `$LASTEXITCODE`     ensuciado por los warnings del compilador:
            #                       dio `bmo-c-front FALLA` con 321 filas en verde.
            #   buscar `test result: ok`  fallo dentro del bucle aunque el texto
            #                       estaba: los cinco paquetes en rojo.
            #   `2>$null` y `cmd /c`      lo mismo.
            #
            # Lo que SI funciona es sumar los numeros: `N passed` y `N failed`
            # salen del propio resumen de cargo y no dependen de como este shell
            # trate stderr. Un paquete esta bien si no fallo ninguna.
            #
            # ** 24-08 CERO PASADAS YA NO ES ROJO, y hubo que cambiarlo al
            # quitar la lista. Con veinte crates elegidos a dedo, cero pasadas
            # solo podia significar "no llego a ejecutar". Con los 55, hay
            # crates que legitimamente no tienen banco de anfitrion --drivers
            # no_std como `bmo-ahci` o `bmo-xhci`-- y matar por eso seria
            # cambiar una mentira por otra.
            #
            # El discriminador bueno no era el conteo: es si COMPILO. Un `^error`
            # en la salida es no haber llegado a ejecutar; cero filas habiendo
            # compilado es no tener pruebas, que es otra cosa y se anota aparte.
            # [!] Y `Stop` se BAJA para esta linea, o el guion se muere en el
            # primer warning del compilador.
            #
            # Con `$ErrorActionPreference = 'Stop'` arriba, el ErrorRecord que
            # PowerShell fabrica por cada linea de stderr de un nativo pasa a
            # ser TERMINANTE: `cargo test 2>&1` aborta el guion entero aunque
            # los tests vayan bien. Se probo este bucle suelto en una consola
            # --donde `Stop` no estaba puesto-- y paso; dentro del guion murio
            # en la primera vuelta. Otra vez lo mismo: probado en un contexto,
            # roto en otro.
            $antes = $ErrorActionPreference
            $ErrorActionPreference = 'Continue'
            $salida = (cargo test -q -p $p 2>&1 | Out-String)
            $ErrorActionPreference = $antes
            $n = ([regex]::Matches($salida, '(\d+) passed') | ForEach-Object { [int]$_.Groups[1].Value } | Measure-Object -Sum).Sum
            $rojas = ([regex]::Matches($salida, '(\d+) failed') | ForEach-Object { [int]$_.Groups[1].Value } | Measure-Object -Sum).Sum
            $compilo = -not ([regex]::IsMatch($salida, '(?m)^error'))
            if ($rojas -gt 0 -or -not $compilo) {
                Write-Host $salida
                Muere "banco de ${p}: $n pasadas, $rojas rojas, compilo=$compilo"
            }
            if ($n -eq 0) { $sinBanco += $p; continue }
            $total += $n
            Bien "$p -- $n filas"
        }
        Bien "$total filas en total, ninguna roja  ($($paquetes.Count) crates preguntados)"
        if ($sinBanco.Count -gt 0) {
            # [-] No mata, pero se VE. Un crate sin una sola prueba compila y no
            #     promete nada; que salga en la lista es lo que impide que se
            #     quede asi para siempre sin que nadie lo note.
            Write-Host "   [-] $($sinBanco.Count) crates compilan y no tienen ni una prueba:" -ForegroundColor Yellow
            Write-Host "       $($sinBanco -join ', ')" -ForegroundColor Yellow
        }
        foreach ($k in $noSePueden.Keys) {
            Write-Host "   [-] $k no se prueba aqui -- $($noSePueden[$k])" -ForegroundColor Yellow
        }
    } finally { Pop-Location }
} else {
    Write-Host "   (banco saltado: -Rapido)" -ForegroundColor Yellow
}

# -- 2. CONSTRUIR ---------------------------------------------------------
#
# `build.ps1` hace el resto y lleva sus propios guardianes de contrato dentro:
# los dos syscalls contra `bmo-abi`, la tabla `OP_INFO` en sus tres sitios,
# `KIND_AUDIO` en kernel y ABI, ningun opcode repetido, y el portico de ASCII.
Titulo 'Construir (toolchain + kernel + Ring 3 + los .bex)'
$build = Join-Path $raiz 'Ultra_kernel_x86-64\build.ps1'
if (-not (Test-Path $build)) { Muere "no esta build.ps1" }

if ($Desplegar) {
    # [!] LAS UNIDADES SE DICEN, NO SE ADIVINAN.
    #
    # En esta maquina el NVMe es el Windows del dueno y BMO vive en un Kingston
    # SATA. Un build que eligiera unidad por su cuenta seria la unica orden de
    # este repositorio capaz de estropear algo que no es suyo. El gate de
    # identidad de ESTRATOS protege el volumen de datos, pero la letra la pone
    # una persona.
    Write-Host "   se va a ESCRIBIR en $Arranque y en $Datos" -ForegroundColor Yellow
    if ($Metro) {
        Write-Host "   y va el KERNEL DE MEDIDA: acuerdate de volver sin -Metro" -ForegroundColor Yellow
    }
    & $build -Todo -Drive $Arranque -Data $Datos -Metro:$Metro
} else {
    & $build -BuildOnly -Metro:$Metro
}
if ($LASTEXITCODE -ne 0) { Muere 'fallo el build' }

$seg = [int]((Get-Date) - $t0).TotalSeconds
Write-Host "`nlisto en $seg s" -ForegroundColor Green
if (-not $Desplegar) {
    Write-Host "   (no se toco ningun disco -- para desplegar: .\bmo.ps1 -Desplegar)" -ForegroundColor DarkGray
}

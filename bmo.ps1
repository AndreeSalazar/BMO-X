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
    [string]$Datos = 'A'
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
        $paquetes = @('bmo-c-front', 'bmo-uaudio', 'bmo-abi', 'bmo-fat32', 'bmo-uhid', 'bmo-net')
        $total = 0
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
            # trate stderr. Un paquete esta bien si paso alguna y no fallo
            # ninguna; cero pasadas tambien es sospechoso --significa que no se
            # llego a ejecutar-- y por eso cuenta como rojo.
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
            if ($n -eq 0 -or $rojas -gt 0) {
                Write-Host $salida
                Muere "banco de ${p}: $n pasadas, $rojas rojas"
            }
            $total += $n
            Bien "$p -- $n filas"
        }
        Bien "$total filas en total, ninguna roja"
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
    & $build -Todo -Drive $Arranque -Data $Datos
} else {
    & $build -BuildOnly
}
if ($LASTEXITCODE -ne 0) { Muere 'fallo el build' }

$seg = [int]((Get-Date) - $t0).TotalSeconds
Write-Host "`nlisto en $seg s" -ForegroundColor Green
if (-not $Desplegar) {
    Write-Host "   (no se toco ningun disco -- para desplegar: .\bmo.ps1 -Desplegar)" -ForegroundColor DarkGray
}

# limpiar.ps1 -- que pesa aqui dentro, y que de eso se puede tirar.
#
# == Por que MIDE por defecto y no borra ==
#
# Misma forma que `desplegar.ps1`, y por el mismo motivo. `.\limpiar.ps1` a
# secas **no borra nada**: dice cuanto hay y de que. Borrar es `-Borrar`.
#
# > La orden segura es la que se teclea sola. La que no se puede deshacer se
# > teclea entera.
#
# == Y la linea que este fichero NO cruza ==
#
# Aqui SOLO se borra lo que una orden vuelve a fabricar: los arboles de
# `cargo` y el bytecode de Python. Nada mas.
#
# [!] `Ultra_kernel_x86-64\viejos\` NO se toca, y no es un olvido. Son tres
# `.bex` de versiones pasadas --`doom640`, `cpu`, `gui`-- que ningun build
# regenera: si se borran, se borran. Se INFORMA de ellos y se deja la decision
# fuera de aqui, porque **meter en una misma orden lo que se recompila y lo
# que es la unica copia es como se pierden las cosas**.
#
# ** Lo que este fichero mide, dicho por delante: no hay basura en el arbol
# versionado. `git status` sale vacio. Lo que pesa es lo que nunca se ve.

param([switch]$Borrar)

$ErrorActionPreference = 'Stop'
$raiz = $PSScriptRoot

# Los CUATRO espacios de trabajo de cargo. Estan separados a proposito --el
# kernel no comparte perfil con el userspace ni con el toolchain-- y por eso
# no hay un solo `cargo clean` que valga: son cuatro arboles distintos.
$arboles = @(
    'target',
    'Ultra_kernel_x86-64\target',
    'Ultra_kernel_x86-64\uefi_chain\target',
    'Ultra_userspace\target'
)

function Peso {
    param([string]$ruta)
    if (-not (Test-Path -LiteralPath $ruta)) { return [int64]0 }
    try {
        $s = Get-ChildItem -LiteralPath $ruta -Recurse -Force -File -ErrorAction Stop |
             Measure-Object -Property Length -Sum
    } catch { return [int64]0 }
    if ($null -eq $s.Sum) { return [int64]0 }
    return [int64]$s.Sum
}

function Bonito {
    param([int64]$b)
    if ($b -ge 1073741824) { return ('{0:N1} GB' -f ($b / 1073741824)) }
    if ($b -ge 1048576)    { return ('{0:N0} MB' -f ($b / 1048576)) }
    return ('{0:N0} KB' -f ($b / 1024))
}

Write-Host "`nBMO-X -- que pesa`n" -ForegroundColor Cyan

$total = [int64]0
$victimas = @()

foreach ($a in $arboles) {
    $ruta = Join-Path $raiz $a
    $p = Peso $ruta
    if ($p -eq 0) { continue }
    $total = $total + $p
    $victimas += $ruta
    Write-Host ('   {0,10}  {1}' -f (Bonito $p), $a) -ForegroundColor Gray
}

# El bytecode de Python de las herramientas del toolchain. Pesa poco y se
# rehace solo, pero ensucia el `find` de cualquiera que busque un fichero.
$caches = @(Get-ChildItem -LiteralPath $raiz -Recurse -Force -Directory `
                -Filter '__pycache__' -ErrorAction SilentlyContinue |
            Where-Object { $_.FullName -notmatch '\\target\\' })
if ($caches.Count -gt 0) {
    $pc = [int64]0
    foreach ($c in $caches) { $pc = $pc + (Peso $c.FullName) }
    $total = $total + $pc
    $victimas += $caches.FullName
    Write-Host ('   {0,10}  __pycache__ ({1} carpetas)' -f (Bonito $pc), $caches.Count) -ForegroundColor Gray
}

Write-Host ('   {0,10}  ---- TOTAL que se puede rehacer' -f (Bonito $total)) -ForegroundColor Yellow

# -- Lo que NO se borra aqui, pero se dice.
$viejos = Join-Path $raiz 'Ultra_kernel_x86-64\viejos'
if (Test-Path -LiteralPath $viejos) {
    $pv = Peso $viejos
    Write-Host ''
    Write-Host ('   {0,10}  viejos\  <- NO lo borra esta orden: no se regenera' -f (Bonito $pv)) -ForegroundColor DarkYellow
    Get-ChildItem -LiteralPath $viejos -Recurse -Force -File |
        ForEach-Object { Write-Host ('               ' + $_.FullName.Substring($raiz.Length + 1)) -ForegroundColor DarkGray }
}

if (-not $Borrar) {
    Write-Host ''
    Write-Host '   (no se borro nada -- para borrar: .\limpiar.ps1 -Borrar)' -ForegroundColor DarkGray
    Write-Host '   el siguiente build sera COMPLETO, unos tres minutos.' -ForegroundColor DarkGray
    Write-Host ''
    exit 0
}

Write-Host ''
foreach ($v in $victimas) {
    Write-Host ('   fuera: ' + $v.Substring($raiz.Length + 1)) -ForegroundColor DarkGray
    Remove-Item -LiteralPath $v -Recurse -Force -Confirm:$false -ErrorAction SilentlyContinue
}
Write-Host ''
Write-Host ('   liberado: ' + (Bonito $total)) -ForegroundColor Green
Write-Host '   el siguiente `.\bmo.ps1` reconstruye desde cero.' -ForegroundColor DarkGray
Write-Host ''

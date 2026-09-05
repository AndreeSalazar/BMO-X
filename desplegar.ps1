# desplegar.ps1 -- construir Y llevarlo al Kingston, en una palabra.
#
# == Por que existe, en vez de quitar el flag ==
#
# El dueno lo pidio asi: *"el build.ps1, puedes quitar la escritura? da
# flojera"*. Y tenia razon en la molestia -- escribir `.\bmo.ps1 -Desplegar`
# veinte veces al dia cansa -- pero el arreglo NO es que `bmo.ps1` escriba por
# defecto, y el motivo no es el dueno: es quien MAS lo ejecuta.
#
# `bmo.ps1` se corre una docena de veces por sesion solo para ver si compila.
# Si desplegar fuera el defecto, **cada comprobacion de compilacion seria una
# escritura en disco**. El flag no esta ahi para que el dueno lo teclee: esta
# para que un `bmo.ps1` suelto --tecleado por quien sea, o por un script-- no
# toque nunca un disco.
#
# > En esta maquina el NVMe es el Windows del dueno. La orden que escribe es la
# > unica de este repositorio capaz de estropear algo que no es suyo.
#
# Asi que se separa lo que de verdad estaba junto: **la SEGURIDAD se queda y la
# MOLESTIA se va.** Comprobar es `.\bmo.ps1`; desplegar es `.\desplegar.ps1`, y
# con el tabulador son tres teclas.
#
# [!] Todo lo demas se pasa tal cual: `-Rapido`, `-Metro`, y las letras. Este
# fichero no decide nada -- si empezara a decidir seria un segundo `bmo.ps1`, y
# entonces habria dos sitios donde arreglar cada cosa.

param(
    [switch]$Rapido,
    [switch]$Metro,
    [string]$Arranque = 'D',
    [string]$Datos = 'A'
)

& "$PSScriptRoot\bmo.ps1" -Desplegar -Rapido:$Rapido -Metro:$Metro `
    -Arranque $Arranque -Datos $Datos
exit $LASTEXITCODE

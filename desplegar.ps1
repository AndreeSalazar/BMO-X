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
    # Pasa de largo las dos frases de confirmacion. Ver `bmo.ps1`.
    [switch]$Si,
    [switch]$Metro,
    [string]$Arranque = 'D',
    [string]$Datos = 'A'
)

# == ** EL RELEVO, Y AQUI SE PERDIA UNA (2026-09-07) ==========================
#
# Este fichero DECLARABA `-Si` y NO se lo pasaba a `bmo.ps1`. O sea que
# `.\desplegar.ps1 -Si ...` se aceptaba sin protestar, la bandera se la tragaba
# el relevo, y el despliegue preguntaba igual.
#
# ** Y es la peor clase de fallo de esta casa: **algo que se acepta y no hace
# nada**. Ni un error, ni un aviso -- el dueno tecleo la bandera que el mismo
# habia pedido y penso que la pregunta era irremediable.
#
# La cabecera de arriba lo decia y no lo cumplia: *"todo lo demas se pasa tal
# cual"*. Lo decia de `-Rapido` y `-Metro`, que si viajaban, y se olvido del
# tercero.
#
# [!] Desde hoy lo vigila `toolchain/tools/relevo`: una bandera declarada aqui
# que no aparezca en la llamada de abajo **para el build**. Un relevo con un
# hueco no se ve leyendo -- se ve cuando algo no pasa, y para entonces ya se ha
# buscado en el sitio equivocado.
& "$PSScriptRoot\bmo.ps1" -Desplegar -Rapido:$Rapido -Si:$Si -Metro:$Metro `
    -Arranque $Arranque -Datos $Datos
exit $LASTEXITCODE

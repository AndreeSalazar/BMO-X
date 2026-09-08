"""relevo -- el guardian de que una bandera declarada LLEGUE al otro lado.

Por que existe
==============

`desplegar.ps1` es un relevo: declara unas banderas y se las pasa a `bmo.ps1`.
El 2026-09-07 se descubrio que declaraba `-Si` y **no se lo pasaba**.

    el dueno tecleo    .\\desplegar.ps1 -Si -Arranque A -Datos A
    lo que paso        se acepto sin protestar, la bandera se la trago el
                       relevo, y el despliegue pregunto igual

** Y esa es la peor clase de fallo de esta casa: algo que se ACEPTA Y NO HACE
NADA. Ni un error, ni un aviso. PowerShell no se queja de un `param` que no se
use, asi que el hueco solo se ve cuando algo no pasa -- y para entonces ya se ha
buscado en el sitio equivocado.

La propia cabecera del fichero lo decia y no lo cumplia: *"todo lo demas se pasa
tal cual"*. Lo decia de `-Rapido` y `-Metro`, que si viajaban, y se olvido del
tercero. Un comentario que describe una politica que su propio codigo no cumple.

Lo que comprueba
================

    para cada relevo declarado abajo:
      1. se leen los nombres del bloque `param(...)`
      2. se lee la linea de llamada al otro script
      3. TODOS los nombres tienen que aparecer en esa llamada

Lo que NO comprueba, y hay que decirlo
=======================================

    que el otro lado los USE.

Aqui se comprueba que la bandera CRUCE, no que sirva de algo al llegar. Que
`bmo.ps1` haga caso de `-Si` es cosa suya, y de eso responde `discos.ps1` con su
techo. Este guardian cierra el hueco del CAMINO, que era el que no tenia nadie.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))

# (fichero relevo, a quien llama). Hoy hay uno; la lista existe para que anadir
# otro no sea inventar el guardian otra vez.
RELEVOS = [("desplegar.ps1", "bmo.ps1")]


def nombres_de_param(texto):
    """Los nombres del bloque `param(...)`, sin el `$`."""
    m = re.search(r"param\s*\((.*?)\n\)", texto, re.S)
    if not m:
        return None
    return re.findall(r"\[\w+\]\s*\$(\w+)", m.group(1))


def linea_de_llamada(texto, destino):
    """La llamada al otro script, con sus continuaciones de linea."""
    lineas = texto.splitlines()
    for i, l in enumerate(lineas):
        if destino in l and l.lstrip().startswith("&"):
            junto = l
            # PowerShell continua con una comilla invertida al final.
            while junto.rstrip().endswith("`") and i + 1 < len(lineas):
                i += 1
                junto = junto.rstrip().rstrip("`") + " " + lineas[i]
            return junto
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    quejas = []
    pasadas = 0
    for relevo, destino in RELEVOS:
        ruta = os.path.join(RAIZ, relevo)
        if not os.path.exists(ruta):
            quejas.append("%s no existe: el relevo que este guardian vigila no esta"
                          % relevo)
            continue
        with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
            texto = fh.read()

        params = nombres_de_param(texto)
        # ** UN `param()` QUE NO SE ENCUENTRA NO ES UN RELEVO SIN BANDERAS. La
        # leccion del `Guardian` de build.ps1: un guardian que no encuentra lo
        # que mira tiene que PARAR, no aprobar.
        if params is None:
            quejas.append("%s: no se encuentra su bloque `param(...)`" % relevo)
            continue
        if not params:
            quejas.append("%s: su `param(...)` no declara ni una bandera. O es "
                          "cierto y sobra el relevo, o el formato cambio" % relevo)
            continue

        llamada = linea_de_llamada(texto, destino)
        if llamada is None:
            quejas.append("%s: no se encuentra su llamada a %s" % (relevo, destino))
            continue

        for nombre in params:
            # `-Nombre` a secas, o `-Nombre:$Nombre`. Las dos valen.
            if not re.search(r"-%s\b" % re.escape(nombre), llamada):
                quejas.append(
                    "%s declara `-%s` y NO se lo pasa a %s -- se acepta y no hace "
                    "nada" % (relevo, nombre, destino))
            else:
                pasadas += 1

    if quejas:
        print("hay banderas que se pierden en el relevo:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  una bandera que se declara y no viaja se ACEPTA SIN PROTESTAR,")
        print("  y el fallo solo se ve cuando algo no pasa. Ver la cabecera de")
        print("  desplegar.ps1.")
        return 1 if args.check else 0

    print("clean: %d relevo(s), %d bandera(s) y todas cruzan al otro lado"
          % (len(RELEVOS), pasadas))
    return 0


if __name__ == "__main__":
    sys.exit(main())

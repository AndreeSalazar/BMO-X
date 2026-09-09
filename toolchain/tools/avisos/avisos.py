"""avisos -- el trinquete de los avisos del compilador.

Por que existe
==============

El 2026-09-08, buscando deudas, se conto por primera vez lo que el compilador
venia diciendo del kernel:

    97 avisos, y el build los ESCONDIA

`bmo.ps1` filtra la salida de cargo para que quepa en pantalla, asi que nadie los
habia leido nunca. Y dentro estaba esto:

    NR_INVOKE => invoke(frame),   <- "matches any value", dijo rustc
    NR_WAIT   => wait(frame),     <- "no value can reach this"

*** `NR_INVOKE` no existia en el kernel --vivia solo en el ABI-- y en Rust un
nombre desconocido dentro de un patron NO es un error: es un BINDING, y casa con
todo. Consecuencia: **`WAIT`, la segunda puerta congelada del sistema, se
despachaba a `invoke` y no habia bloqueado NUNCA en la vida del proyecto.** Y
cualquier numero de syscall desconocido tambien entraba por ahi, en vez de que lo
rechazara `unsupported()`.

    Las dos puertas eran UNA que aceptaba cualquier numero.

** El compilador lo dijo con todas las letras. Lo que fallo no fue la red: fue
que 97 avisos sin leer son un sitio donde esconderse.

Lo que hace
===========

    cuenta los avisos y los compara con LINEA_BASE.txt

      suben  -> PARA. Y ensena cuales son los nuevos
      bajan  -> lo dice, y pide bajar la linea base

**Trinquete, no muro**, que es la regla que escribio `censo_modular` y repite
L6a: exigir cero el primer dia seria un guardian gritando cuarenta veces, y uno
que grita sin motivo se apaga en una semana. Se empieza donde se esta y **solo se
puede bajar**.

Lo que NO hace, y hay que decirlo
==================================

    no mira los avisos de las LIBRERIAS de `platform/`, solo el kernel.

Son otro arbol y otra linea base; meterlos aqui haria que una limpieza en fat32
tapara un aviso nuevo en Ring 0, que es justo lo contrario de para lo que existe
esto. El dia que hagan falta, se anaden con su propia cifra.

[!] Y si `cargo` no esta o el target no esta instalado, este guardian **lo dice y
no mata**. Mismo trato que el sello de VALKYRIE: un guardian que no puede mirar
tiene que confesarlo, no aprobar en silencio -- pero tampoco puede impedir que se
construya en una maquina que no es la del dueno.
"""

import argparse
import os
import re
import subprocess
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
KERNEL = os.path.join(RAIZ, "Ultra_kernel_x86-64", "kernel")
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "LINEA_BASE.txt")

# El resumen (`X generated N warnings`) no es un aviso: es el recuento de cargo.
# Contarlo haria que la cifra subiera al partir un crate en dos.
RESUMEN = re.compile(r"^warning: .* generated \d+ warning")
AVISO = re.compile(r"^warning: ")


def leer_base():
    if not os.path.exists(BASE):
        return None
    with open(BASE, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if linea and not linea.startswith("#"):
                return int(linea)
    return None


def contar():
    """`(cuantos, [textos])` o `None` si no se puede mirar."""
    try:
        p = subprocess.run(
            ["cargo", "build", "--release", "--target", "x86_64-unknown-none"],
            cwd=KERNEL, capture_output=True, text=True, timeout=900,
        )
    except (OSError, subprocess.TimeoutExpired):
        return None
    if p.returncode != 0:
        return None
    fuera = []
    for linea in p.stderr.splitlines():
        if AVISO.match(linea) and not RESUMEN.match(linea):
            fuera.append(linea[len("warning: "):].strip())
    return fuera


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    base = leer_base()
    if base is None:
        print("no hay LINEA_BASE.txt: este guardian no sabe contra que comparar")
        return 1 if args.check else 0

    avisos = contar()
    # ** NO PUEDE MIRAR: lo dice y sigue. Ver la cabecera.
    if avisos is None:
        print("[i] no se pudo compilar el kernel para contarlos (cargo, el target")
        print("    `x86_64-unknown-none`, o un fallo de build). El trinquete NO")
        print("    corrio: la cifra de esta pasada no vale.")
        return 0

    n = len(avisos)
    if n > base:
        print("los avisos del compilador SUBIERON: %d, y la linea base es %d" % (n, base))
        print("")
        # Un recuento por clase dice mas que la lista entera, y cabe.
        clases = {}
        for a in avisos:
            k = re.sub(r"`[^`]*`", "X", a)
            clases[k] = clases.get(k, 0) + 1
        for k in sorted(clases, key=lambda x: -clases[x])[:8]:
            print("  %3d  %s" % (clases[k], k))
        print("")
        print("  *** El 08-09 uno de estos avisos escondia que `WAIT` no habia")
        print("  bloqueado NUNCA: un nombre sin definir en un patron es un")
        print("  BINDING que casa con todo, y el compilador lo dijo. Ver la")
        print("  cabecera de este fichero.")
        return 1 if args.check else 0

    if n < base:
        print("clean: %d aviso(s) del compilador, y la linea base dice %d --"
              " BAJA LA CIFRA en toolchain/tools/avisos/LINEA_BASE.txt" % (n, base))
        return 0

    print("clean: %d aviso(s) del compilador en el kernel, y no suben (L6a: "
          "trinquete, no muro)" % n)
    return 0


if __name__ == "__main__":
    sys.exit(main())

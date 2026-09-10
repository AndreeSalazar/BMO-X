#!/usr/bin/env python3
"""planes -- el mapa de lo que FALTA, y no puede envejecer.

Por que existe
==============

El 2026-09-10 el dueno dijo: *"los planes me gustaria que se mezclen [...] pero
dividiendo los planes que faltan [...] me esta fastidiando, me gustaria que
organices profesionalmente"*.

Se midio antes de mover nada, y el numero explica el fastidio:

    26 planes, 10.803 lineas, 77 casillas hechas y 126 PENDIENTES

*** Y no habia ni una vista de conjunto. Para saber que falta hay que abrir
veintiseis ficheros y contar a mano, asi que **nadie lo hace**, asi que la
respuesta a *"que queda"* sale de la memoria en vez de del arbol.

    > Un plan que hay que abrir para saber si tiene algo pendiente es un plan
    > que solo se consulta cuando ya te acordabas de el.

# ** POR QUE UN INDICE GENERADO Y NO UNA CARPETA NUEVA

La tentacion era repartir los 26 en subcarpetas --vivos, cumplidos, archivo--.
Se midio eso tambien:

    282 citas apuntan a ficheros de `plan/` y `metal/`
    CERO ficheros sin citar

O sea que mover cualquiera cuesta arreglar sus citas, y lo que se compra es que
un `ls` salga mas bonito. `NEUTRO/ORDEN.md` ya escribio la regla:

    "Reorganizar es mover lo que esta en el sitio equivocado. Cuando no hay nada
     en el sitio equivocado, reorganizar es estropear algo."

*** Los 26 planes contestan la pregunta de `plan/` --*que casillas faltan*-- y
por eso ninguno se mueve. Lo que faltaba no era una carpeta: **era el indice**.

# Y LO QUE SI ESTABA MAL, dicho por este mismo guardian

Seis de los veintiseis **no traen ni una casilla**, y tres de ellos son los mas
gordos del arbol. Un fichero de 981 lineas en `plan/` que no dice que falta no
contesta la pregunta de su carpeta: es un maestro, o una identidad, con la
palabra PLAN delante. Este guardian los nombra en cada build para que la
diferencia se note.

Modos
=====

    --check     el indice y los planes dicen lo mismo? (lo que corre en el build)
    --apply     regenera el indice
    --dry-run   ensena lo que escribiria
"""

import argparse
import io
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PLANES = os.path.join(RAIZ, "docs", "plan")
INDICE = os.path.join(PLANES, "ABIERTO.md")

# == *** UNA CASILLA TAMBIEN PUEDE SER UN ENCABEZADO (2026-09-10) =========
#
# La primera version pedia que la linea empezara por `- [ ]`, y por eso dijo
# que `PLAN_ALMACENAMIENTO` no tenia ni una casilla. **Las tiene, y las cinco
# estan hechas**: las escribe como `### [x] Paso 0 -- UNA SOLA PUERTA`.
#
# ** Casi cuesta caro: se iba a reescribir un plan que ya estaba bien. El
# guardian no cazo una deuda, INVENTO una.
#
#     > Un contador que no reconoce una forma legitima no cuenta de menos:
#     > acusa. Y lo que acusa es a quien lo hizo bien de otra manera.
#
# [!] Y por eso el `(?:#{1,6}\s*)?` va DELANTE del guion y no en su lugar: las
# dos formas valen, y una casilla dentro de un encabezado es la que se usa
# cuando el escalon trae parrafos debajo.
HECHA = re.compile(r"^\s*(?:#{1,6}\s*)?(?:[-*]\s*)?\[[xX]\]\s*(.*)$")
FALTA = re.compile(r"^\s*(?:#{1,6}\s*)?(?:[-*]\s*)?\[ \]\s*(.*)$")
TITULO = re.compile(r"^#\s+(.*)$")

# El indice se genera, asi que no se edita a mano. Se dice arriba del todo.
CABECERA = "<!-- GENERADO por toolchain/tools/planes. No se edita a mano. -->"


def limpia(t):
    """El texto de una casilla, sin markdown y acotado."""
    t = re.sub(r"\*\*(.*?)\*\*", r"\1", t)
    t = re.sub(r"`(.*?)`", r"\1", t)
    t = re.sub(r"\[(.*?)\]\(.*?\)", r"\1", t)
    t = " ".join(t.split())
    return t[:96]


def censo():
    """[(fichero, titulo, hechas, [pendientes]), ...] ordenado por pendientes."""
    filas = []
    for n in sorted(os.listdir(PLANES)):
        if not n.endswith(".md") or n == os.path.basename(INDICE):
            continue
        p = os.path.join(PLANES, n)
        with io.open(p, encoding="utf-8", errors="replace") as fh:
            L = fh.read().splitlines()
        titulo = ""
        for l in L:
            m = TITULO.match(l)
            if m:
                titulo = limpia(m.group(1))
                break
        hechas = sum(1 for l in L if HECHA.match(l))
        faltan = []
        for l in L:
            m = FALTA.match(l)
            if m:
                texto = limpia(m.group(1))
                if texto:
                    faltan.append(texto)
        filas.append((n, titulo, hechas, faltan, len(L)))
    filas.sort(key=lambda r: (-len(r[3]), r[0]))
    return filas


def pinta(filas):
    vivos = [f for f in filas if f[3]]
    cumplidos = [f for f in filas if not f[3] and f[2]]
    mudos = [f for f in filas if not f[3] and not f[2]]
    total = sum(len(f[3]) for f in filas)
    hechas = sum(f[2] for f in filas)

    o = [CABECERA, ""]
    o.append("# LO QUE FALTA -- las casillas abiertas de los %d planes" % len(filas))
    o.append("")
    o.append("> Generado por `toolchain/tools/planes`. **El build comprueba que")
    o.append("> este fichero y los planes dicen lo mismo**, asi que no puede")
    o.append("> envejecer sin que algo se ponga rojo.")
    o.append("")
    o.append("```text")
    o.append("   %3d casillas ABIERTAS en %d planes" % (total, len(vivos)))
    o.append("   %3d hechas" % hechas)
    o.append("   %3d planes CUMPLIDOS (ni una casilla pendiente)" % len(cumplidos))
    o.append("   %3d en plan/ SIN NI UNA CASILLA -- ver el final" % len(mudos))
    o.append("```")
    o.append("")
    o.append("---")
    o.append("")
    o.append("# Los planes VIVOS, el que mas debe primero")
    o.append("")

    for n, titulo, h, faltan, lin in vivos:
        o.append("## [`%s`](%s) -- %d abiertas, %d hechas" % (n, n, len(faltan), h))
        o.append("")
        if titulo:
            o.append("*%s*" % titulo)
            o.append("")
        for t in faltan[:3]:
            o.append("- [ ] %s" % t)
        if len(faltan) > 3:
            o.append("- ... y %d mas" % (len(faltan) - 3))
        o.append("")

    if cumplidos:
        o.append("---")
        o.append("")
        o.append("# CUMPLIDOS -- todas sus casillas marcadas")
        o.append("")
        o.append("** No se archivan ni se mueven: siguen siendo la razon por la")
        o.append("que algo se hizo asi, y eso se consulta mas que la casilla.")
        o.append("")
        for n, titulo, h, _f, lin in cumplidos:
            o.append("- [`%s`](%s) -- %d hechas, %d lineas" % (n, n, h, lin))
        o.append("")

    if mudos:
        o.append("---")
        o.append("")
        o.append("# [!] ESTOS ESTAN EN `plan/` Y NO TRAEN NI UNA CASILLA")
        o.append("")
        o.append("`docs/README.md` dice que `plan/` contesta *\"que casillas")
        o.append("faltan, que las bloquea, como se sabe que quedo hecha\"*. Un")
        o.append("fichero sin casillas **no contesta esa pregunta**: es un")
        o.append("maestro o una identidad con la palabra PLAN delante.")
        o.append("")
        o.append("*** No se mueven solos. Cada uno se decide a mano: o se le")
        o.append("ponen sus casillas, o se muda a la carpeta cuya pregunta si")
        o.append("contesta. Este guardian los nombra en cada build para que la")
        o.append("deuda tenga nombre en vez de ser un fichero mas.")
        o.append("")
        for n, titulo, _h, _f, lin in mudos:
            o.append("- [`%s`](%s) -- %d lineas" % (n, n, lin))
        o.append("")

    return "\n".join(o) + "\n"


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group(required=True)
    g.add_argument("--check", action="store_true")
    g.add_argument("--apply", action="store_true")
    g.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()

    nuevo = pinta(censo())

    if a.dry_run:
        sys.stdout.write(nuevo)
        return 0

    if a.apply:
        with io.open(INDICE, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(nuevo)
        print("planes: indice reescrito -> docs/plan/ABIERTO.md")
        return 0

    if not os.path.exists(INDICE):
        print("planes: falta docs/plan/ABIERTO.md."
              " Se genera con `python toolchain/tools/planes/planes.py --apply`")
        return 1
    with io.open(INDICE, encoding="utf-8") as fh:
        viejo = fh.read()
    if viejo.replace("\r\n", "\n") != nuevo:
        print("planes: el indice y los planes NO dicen lo mismo.")
        print("  alguien marco o anadio una casilla y el indice se quedo atras.")
        print("  se arregla con: python toolchain/tools/planes/planes.py --apply")
        return 1

    filas = censo()
    abiertas = sum(len(f[3]) for f in filas)
    vivos = sum(1 for f in filas if f[3])
    mudos = [f[0] for f in filas if not f[3] and not f[2]]
    msg = ("clean: el indice de planes cuadra -- %d casillas abiertas en %d de"
           " %d planes" % (abiertas, vivos, len(filas)))
    if mudos:
        msg += ("; y %d en plan/ sin ni una casilla (%s)"
                % (len(mudos), ", ".join(m.replace("PLAN_", "").replace(".md", "")
                                         for m in mudos)))
    print(msg)
    return 0


if __name__ == "__main__":
    sys.exit(main())

# -*- coding: utf-8 -*-
"""tamano -- que le hizo este cambio a los 25 programas, de un vistazo.

Por que existe
==============

El 2026-09-09 tres mirillas del codegen de C encogieron TODOS los `.bex` un 6 %
--DOOM perdio 54 KB-- y eso se supo **sumando a mano dos listados del build**.
El dato estaba en pantalla las dos veces y nadie los comparaba.

    el build imprime el tamano de los 25 programas en cada pasada
    y nadie los compara con los de ayer

*** Es la misma forma que ya tienen `avisos` y `fases`: el numero existia, se
imprimia, y no habia nada que se acordara del anterior. Un dato que hay que
apuntar a mano no se apunta.

Lo que hace
===========

    lee el tamano de cada `.bex` de `staging/` y lo compara con LINEA_BASE.txt

      cambia   lo dice, programa por programa, con el signo y el porcentaje
      igual    una linea y a otra cosa

[!] Y NO ES UN TRINQUETE, a proposito. `avisos` y `fases` PARAN el build porque
un aviso nuevo o una etiqueta perdida son siempre malos. Un programa que crece
puede estar creciendo por una razon excelente --una funcion nueva, una tabla mas
grande-- y un guardian que grita cada vez que el proyecto avanza se apaga en una
semana. Esto **REPORTA**.

⚠ Y el aviso que hay que llevar puesto al leerlo
=================================================

    EL TAMANO NO ES LA VELOCIDAD.

Van juntos en un caso concreto --pasar de maquina de pila a registros quita
instrucciones, y menos instrucciones son menos bytes-- y no en general: desplegar
un bucle lo hace mas grande y mas rapido a la vez.

*** El juez de la velocidad sigue siendo el metal: `expansion N us` del `[perf]`
de DOOM. Esto solo dice **que se movio**, no si se movio a mejor.
"""

import argparse
import os
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
STAGING = os.path.join(RAIZ, "Ultra_kernel_x86-64", "staging", "BMO-DATA")
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "LINEA_BASE.txt")


def medidos():
    """`{nombre: bytes}` de todo lo ejecutable que hay en `staging/`."""
    fuera = {}
    if not os.path.isdir(STAGING):
        return None
    for dp, _dn, fn in os.walk(STAGING):
        for n in sorted(fn):
            if not (n.endswith(".bex") or n.endswith(".ibex")):
                continue
            ruta = os.path.join(dp, n)
            rel = os.path.relpath(ruta, STAGING).replace(os.sep, "/")
            fuera[rel] = os.path.getsize(ruta)
    return fuera


def leer_base():
    if not os.path.exists(BASE):
        return None
    fuera = {}
    with open(BASE, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            nombre, _, tam = linea.rpartition(" ")
            fuera[nombre.strip()] = int(tam)
    return fuera


def escribir_base(m):
    with open(BASE, "w", encoding="utf-8", newline="\n") as f:
        f.write("# Tamano de cada ejecutable de staging/, en bytes.\n")
        f.write("# REPORTERO, no trinquete: ver toolchain/tools/tamano/tamano.py\n")
        f.write("# Se regenera con:  py toolchain/tools/tamano/tamano.py --fijar\n")
        for n in sorted(m):
            f.write("%s %d\n" % (n, m[n]))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    ap.add_argument("--fijar", action="store_true",
                    help="guarda los tamanos de ahora como la nueva linea base")
    args = ap.parse_args()

    m = medidos()
    # Un guardian que no encuentra lo que mira tiene que DECIRLO. Pero aqui no
    # mata: `bmo.ps1` sin construir deja `staging/` vacio, y eso es legitimo.
    if m is None or not m:
        print("[i] no hay ejecutables en staging/: este reportero no tiene que"
              " comparar. Construye primero.")
        return 0

    if args.fijar:
        escribir_base(m)
        print("linea base fijada: %d ejecutable(s), %d bytes en total"
              % (len(m), sum(m.values())))
        return 0

    base = leer_base()
    if base is None:
        escribir_base(m)
        print("no habia LINEA_BASE.txt: se ha creado con los %d de ahora" % len(m))
        return 0

    cambios = []
    for n in sorted(set(m) | set(base)):
        a, b = base.get(n), m.get(n)
        if a is None:
            cambios.append((n, 0, b, "NUEVO"))
        elif b is None:
            cambios.append((n, a, 0, "YA NO ESTA"))
        elif a != b:
            cambios.append((n, a, b, ""))

    if not cambios:
        print("clean: los %d ejecutables miden lo mismo que la linea base"
              % len(m))
        return 0

    ta, tb = sum(base.values()), sum(m.values())
    print("los ejecutables CAMBIARON de tamano -- %d de %d, y el total %+d B"
          " (%+.1f %%)" % (len(cambios), len(m), tb - ta,
                           (tb - ta) * 100.0 / ta if ta else 0.0))
    for n, a, b, nota in cambios:
        if nota:
            print("  %-34s %s" % (n, nota))
        else:
            print("  %-34s %8d -> %8d  %+7d  %+6.1f %%"
                  % (n, a, b, b - a, (b - a) * 100.0 / a if a else 0.0))
    print("")
    print("  [!] El tamano NO es la velocidad. El juez sigue siendo el metal:")
    print("      `expansion N us` en el [perf] de DOOM.")
    print("  Para aceptar estos numeros como los nuevos de referencia:")
    print("      py toolchain/tools/tamano/tamano.py --fijar")
    # REPORTERO: nunca mata el build. Ver la cabecera.
    return 0


if __name__ == "__main__":
    sys.exit(main())

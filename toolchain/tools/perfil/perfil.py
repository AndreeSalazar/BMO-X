"""perfil -- el guardian de que todo perfil diga A QUIEN EXPONE, y que exista.

Por que existe
==============

Idea del dueno, 2026-09-07:

    "que esos PERFIL digan exposicion, para que explique cada perfil que fallan
     para facilitar procesos. Es como que uno perfila, pero si falla el guardian
     lo frena por motivos"

Y esa es la diferencia entre lo que habia y lo que hace falta. Los perfiles ya
llevaban una seccion `SI ESTE PERFIL ESTA MAL, ASI SE NOTA` -- pero eso es PROSA:
la lee un humano, y solo si abre el fichero.

    lo que dice        "si esto miente, aquello se rompe"
    lo que faltaba     que AQUELLO estuviera nombrado, y que alguien comprobara
                       que sigue existiendo

** Un perfil que expone a un fichero borrado no avisa de nada: describe una
maquina que ya no esta y suena igual de seguro.

Lo que comprueba
================

    1. TODO perfil de PERFIL/ declara al menos un `expone:`
       -- un perfil que no expone a nadie es un perfil que nadie usa, y eso es
          una respuesta que hay que dar, no un silencio
    2. cada ruta de `expone:` EXISTE (fichero o carpeta)
    3. y se dice cuantas exposiciones hay en total, para que la cobertura se
       vea crecer o encogerse

Lo que NO comprueba, y hay que decirlo
=======================================

    que el consumidor de verdad USE el perfil.

Eso exigiria entender el codigo. Aqui se comprueba que la flecha apunta a algo
que existe -- que es poco, y es exactamente lo que hoy se puede afirmar sin
mentir. El resto lo hacen los guardianes especificos: `perfil-placa` compara
campo por campo, y ese es el modelo al que tienden los demas.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
CARPETA = os.path.join(RAIZ, "PERFIL")

EXPONE = re.compile(r"^expone:\s+(\S+)\s*$", re.M)


def perfiles():
    """Todos los .txt de PERFIL/, incluidas sus subcarpetas."""
    fuera = []
    for dirpath, _, files in os.walk(CARPETA):
        for f in sorted(files):
            if f.endswith(".txt"):
                fuera.append(os.path.join(dirpath, f))
    return sorted(fuera)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    if not os.path.isdir(CARPETA):
        print("la carpeta NO EXISTE: falta PERFIL/")
        return 1 if args.check else 0

    lista = perfiles()
    # ** CERO PERFILES NO ES CERO PROBLEMAS. La leccion del `Guardian` de
    # build.ps1: un path mal escrito dejo un guardian muerto y el build dijo
    # COMPLETE igual.
    if not lista:
        print("PERFIL/ no tiene ni un perfil. O esta vacia, o este guardian")
        print("dejo de encontrarlos. Ninguna de las dos cosas es 'todo bien'.")
        return 1 if args.check else 0

    quejas = []
    total = 0
    for ruta in lista:
        rel = os.path.relpath(ruta, RAIZ).replace("\\", "/")
        with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
            texto = fh.read()
        destinos = EXPONE.findall(texto)
        if not destinos:
            quejas.append(
                "%s no declara ni un `expone:` -- si de verdad no lo usa nadie, "
                "dilo; si lo usa alguien, nombralo" % rel)
            continue
        total += len(destinos)
        for d in destinos:
            if not os.path.exists(os.path.join(RAIZ, d.replace("/", os.sep))):
                quejas.append("%s expone a %s, y eso NO EXISTE" % (rel, d))

    if quejas:
        print("los perfiles y lo que exponen NO cuadran:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  el formato esta en PERFIL/README.md, seccion 7. Un perfil que")
        print("  expone a un fichero borrado no avisa de nada: describe una")
        print("  maquina que ya no esta y suena igual de seguro.")
        return 1 if args.check else 0

    print("clean: %d perfil(es), %d exposicion(es) y todas apuntan a algo que existe"
          % (len(lista), total))
    return 0


if __name__ == "__main__":
    sys.exit(main())

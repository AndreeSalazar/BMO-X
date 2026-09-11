# -*- coding: utf-8 -*-
"""fases -- EL EJE PROPIO DEL COMPILADOR: donde nace un fallo y DONDE APARECE.

Por que existe, y por que NO es roja/amarilla/verde
====================================================

El dueno lo pidio el 2026-09-09 con estas palabras:

    "el modulo nivel 3 es bueno PERO no es suficiente. Hablo de modular propio
     que tenga enfoque en C, porque si es archivo y codegen hasta AST TODO SON
     valiosos, en sentido de POR QUE cada uno, para no tener sorpresas. El
     compilador de C es algo que considero DELICADO."

Y tiene razon, porque **un compilador falla distinto a un kernel**.

    en Ring 0   un fallo se paga DONDE ESTA: la maquina se para, o se corrompe.
                Por eso el semaforo pregunta "que arriesgo si lo TOCO"
    en un       un fallo se paga LEJOS: el compilador acaba en verde, el `.bex`
    compilador  se escribe, el emulador pasa, y el sintoma sale dentro de un
                juego de 900 KB, tres semanas despues y en otro fichero

*** Esa DISTANCIA es el problema entero de un compilador, y ningun eje de los
que ya tenia esta casa la nombra. `[cuesta]` dice cuanto duele; `[riesgo]` dice
por que fallara; **ninguno dice DONDE LO VAS A VER**.

Las dos etiquetas
=================

    [fase]     LEXICO SINTAXIS ARBOL TIPOS EMISION IMAGEN
               en que mitad del camino vive la pieza

    [aparece]  AQUI BANCO EMULADOR METAL DENTRO
               *** ORDENADO POR LO QUE CUESTA ENCONTRARLO

Y esa segunda escala es toda la herramienta:

    AQUI       el compilador lo dice, en la linea que lo trae.       gratis
    BANCO      una de las 500 filas se pone roja.                    segundos
    EMULADOR   compila, y el emulador lo caza al ejecutarlo.         minutos
    METAL      compila, el emulador pasa, y falla en el Ryzen.       un arranque
    DENTRO     todo pasa y el sintoma sale dentro de un programa
               grande, lejos de su causa.                            DIAS

** Los cinco fallos de codegen de la semana del 01 al 04-09 eran `DENTRO` los
cinco. Y el del 09-09 --un `imul` para multiplicar uno por ocho-- vivio meses
porque nadie lo buscaba: no fallaba, solo costaba.

Lo que este guardian hace
=========================

    cuenta los ficheros que declaran las dos etiquetas y NO deja que bajen,
    comprueba que los valores estan en el vocabulario CERRADO,
    y ENSENA LA LISTA DE LOS `DENTRO`

*** Lo tercero es lo que de verdad se viene a buscar. Esa lista es **el mapa de
las sorpresas**: los ficheros donde un error no va a avisar. Antes de tocar uno,
se sabe que el banco no protege ahi -- y eso cambia como se toca.

Trinquete y no muro, que es la regla de L6a y la que escribio `avisos`: se
empieza donde se esta y solo se puede subir.

Por que el vocabulario vive AQUI y no en `contrato_ley.py`
===========================================================

Porque `contrato_ley.py` guarda lo que **todos** los guardianes comparten, y
esto lo usa uno solo: es el eje de UN arbol. Meterlo alli daria a entender que
un fichero de Ring 0 tambien deberia declarar `[aparece]`, y no -- en Ring 0 el
fallo aparece donde esta, que es justo la diferencia que este fichero explica.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
ARBOL = os.path.join(RAIZ, "toolchain", "lang", "c", "src")
BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "LINEA_BASE.txt")

# En que mitad del camino vive la pieza. Cerrado, por el mismo motivo que
# `COSTES`: dos ficheros de la misma fase tienen que decirlo igual.
FASES = ("LEXICO", "SINTAXIS", "ARBOL", "TIPOS", "EMISION", "IMAGEN")

# *** ORDENADO POR LO QUE CUESTA ENCONTRARLO. El orden ES la informacion.
APARECE = ("AQUI", "BANCO", "EMULADOR", "METAL", "DENTRO")

RE_FASE = re.compile(r"^//!\s*\[fase\]\s+([A-Z]+)\s*$", re.M)
RE_APARECE = re.compile(r"^//!\s*\[aparece\]\s+([A-Z]+)\b", re.M)
RE_CARRIL = re.compile(r"^//!\s*\[carril\]\s+([A-Z]+)\b", re.M)

# *** EL SEMAFORO DEL COMPILADOR SE DERIVA, NO SE OPINA.
#
# El dueno lo pidio el 09-09 asi: "la regla en el compilador, el estandar es
# semaforo de rojo y verde, EL PORQUE". Y el porque ya estaba medido: el color
# de una pieza de compilador es **quien te sujeta si la rompes**.
#
#    AQUI      -> VERDE      el compilador te lo dice antes de que salga
#    BANCO     -> VERDE      500 filas te sujetan
#    EMULADOR  -> AMARILLO   hay que ejecutar para verlo
#    METAL     -> AMARILLO   hace falta un arranque
#    DENTRO    -> ROJO       nadie te sujeta; sale lejos de la causa
#
# ** Y por eso este guardian lo COMPRUEBA en vez de leerlo. Un `[carril]` que no
# cuadra con su `[aparece]` es una de las dos etiquetas mintiendo, y con dos
# escritas a mano eso pasa el primer dia que alguien tenga prisa.
#
# *** La diferencia con el semaforo de Ring 0 es toda la idea: alli el color se
# elige --"que arriesgo si lo toco"-- porque el fallo se paga donde esta. Aqui
# el color se DEDUCE, porque lo que decide el riesgo de tocar una pieza de
# compilador no es lo que hace: es **quien la va a cazar**.
COLOR_DE = {"AQUI": "VERDE", "BANCO": "VERDE", "EMULADOR": "AMARILLO",
            "METAL": "AMARILLO", "DENTRO": "ROJO"}


def ficheros():
    """Todo `.rs` del compilador de C, menos su banco de pruebas.

    El banco se salta a proposito: son las pruebas, no las piezas. Pedirle a una
    prueba que declare donde aparece su fallo es una vuelta de mas -- una prueba
    que falla aparece siempre en el mismo sitio: en el banco.
    """
    fuera = []
    for dp, dn, fn in os.walk(ARBOL):
        dn[:] = [d for d in dn if d not in ("tests", "target")]
        for n in sorted(fn):
            if n.endswith(".rs"):
                fuera.append(os.path.join(dp, n))
    return fuera


def leer_base():
    if not os.path.exists(BASE):
        return None
    with open(BASE, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if linea and not linea.startswith("#"):
                return int(linea)
    return None


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    if not os.path.isdir(ARBOL):
        # Un guardian que no encuentra lo que mira tiene que PARAR, no aprobar.
        # Es la leccion de `perfil.py` y del `Guardian` con la ruta mal escrita.
        print("el arbol de BMO C no esta donde dice: %s" % ARBOL)
        return 1

    quejas = []
    marcados = 0
    por_fase = {}
    dentro = []
    for ruta in ficheros():
        with open(ruta, "r", encoding="utf-8", errors="replace") as f:
            txt = f.read()
        rel = os.path.relpath(ruta, RAIZ).replace(os.sep, "/")
        mf = RE_FASE.search(txt)
        ma = RE_APARECE.search(txt)
        if not mf and not ma:
            # == *** ESTO ERA UN `continue`, Y ERA UN AGUJERO (2026-09-10) ===
            #
            # Un fichero sin ninguna de las dos etiquetas se saltaba en
            # SILENCIO. Sumado al trinquete, eso quiere decir que **un fichero
            # NUEVO sin etiqueta pasaba**: `marcados` no bajaba --el nuevo
            # nunca conto-- asi que el suelo se cumplia y el build decia clean.
            #
            # ** El guardian contaba lo que hay, no lo que falta. Es la misma
            # forma que `EMBUDO.txt` el 10-09 y que `planes.py` el 09-09: un
            # contador que solo mira lo declarado no cuenta de menos, **deja de
            # ser un guardian**.
            #
            # *** Y se puede exigir a TODOS, sin trinquete, por la razon que
            # R10 escribio para Ring 0: la cobertura ya es 39 de 39. Una regla
            # que se cumple entera no necesita un suelo que tolere lo que ya
            # estaba mal -- el trinquete de abajo se queda para que el numero
            # no baje, pero el muro es este.
            quejas.append(
                "%s no declara [fase] ni [aparece]. Todo fichero de BMO C dice "
                "en que fase falla y QUIEN lo caza -- si no, su fallo es una "
                "sorpresa por definicion" % rel)
            continue
        marcados += 1
        if not mf:
            quejas.append("%s declara [aparece] y no [fase]" % rel)
            continue
        if not ma:
            quejas.append("%s declara [fase] y no [aparece]" % rel)
            continue
        if mf.group(1) not in FASES:
            quejas.append("%s inventa una fase: %s. Las que hay son %s"
                          % (rel, mf.group(1), ", ".join(FASES)))
        if ma.group(1) not in APARECE:
            quejas.append("%s inventa un [aparece]: %s. Los que hay son %s"
                          % (rel, ma.group(1), ", ".join(APARECE)))
        mc = RE_CARRIL.search(txt)
        if mc is None:
            quejas.append("%s declara [fase] y no [carril]. El semaforo del "
                          "compilador sale de su [aparece]: %s -> %s"
                          % (rel, ma.group(1), COLOR_DE[ma.group(1)]))
        elif mc.group(1) != COLOR_DE[ma.group(1)]:
            quejas.append("%s dice [aparece] %s y [carril] %s, y no cuadran: "
                          "%s se sujeta con %s. Una de las dos miente"
                          % (rel, ma.group(1), mc.group(1), ma.group(1),
                             COLOR_DE[ma.group(1)]))
        por_fase.setdefault(mf.group(1), []).append(rel)
        if ma.group(1) == "DENTRO":
            dentro.append(rel)

    if quejas:
        for q in quejas:
            print("  " + q)
        return 1 if args.check else 0

    base = leer_base()
    if base is None:
        print("no hay LINEA_BASE.txt: este guardian no sabe contra que comparar")
        return 1 if args.check else 0
    if marcados < base:
        print("los ficheros de BMO C con [fase] BAJARON: %d, y la linea base es %d"
              % (marcados, base))
        print("  Un fichero que pierde su etiqueta es un sitio donde vuelve a")
        print("  no saberse si el fallo va a avisar. Trinquete: solo puede subir.")
        return 1 if args.check else 0

    orden = [f for f in FASES if f in por_fase]
    # ** El recuento de `DENTRO` va en la PRIMERA linea a proposito: el
    # envoltorio del build solo ensena esa, y ese numero es el que hay que ver
    # sin ir a buscarlo. La lista entera sale al correr el guardian a mano.
    # ** SE DICE "N DE N". Un numero suelto se lee como cobertura completa
    # aunque no lo sea; con el denominador delante, el dia que no cuadren se
    # ve en la misma linea y sin ir a contar nada.
    print("clean: %d de los %d fichero(s) de BMO C declaran su fase (%s) -- y "
          "en %d de ellos el fallo APARECE LEJOS (`DENTRO`): ahi el banco no protege"
          % (marcados, len(ficheros()),
             " ".join("%s %d" % (f, len(por_fase[f])) for f in orden),
             len(dentro)))
    # *** LA LISTA QUE SE VIENE A BUSCAR: donde un error NO va a avisar.
    print("       [aparece] DENTRO en %d: el banco no protege ahi, y el sintoma"
          % len(dentro))
    print("       sale lejos de la causa. Es el mapa de las sorpresas:")
    for r in dentro:
        print("         %s" % r)
    if marcados > base:
        print("       y SUBIERON de %d: baja la cifra en"
              " toolchain/tools/fases/LINEA_BASE.txt" % base)
    return 0


if __name__ == "__main__":
    sys.exit(main())

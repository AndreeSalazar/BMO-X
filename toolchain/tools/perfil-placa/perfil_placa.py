"""perfil-placa -- el guardian de que el PERFIL y los RODEOS digan lo mismo.

Por que existe
==============

`PERFIL/PLACA/PERFIL.txt` lista las manas del firmware de esta placa y, por cada una,
el fichero que la rodea. Y los rodeos viven en TRES CAPAS distintas -- el sobre
del traspaso, la etapa s1 y el kernel -- que no saben las unas de las otras.

Dos listas de lo mismo que pueden separarse sin que nadie avise. Es el
`[riesgo] ESPEJO`, y ya se pago con las constantes del ABI y con el censo del
neutro.

Y aqui separarse tiene una forma concreta y silenciosa:

    se quita un rodeo del codigo    y el perfil sigue diciendo que se paga
    se renombra un fichero          y el perfil apunta a un sitio que no existe
    se cambia de placa              y el perfil sigue nombrando a la vieja

** Ninguna de las tres rompe el build por si sola. Las tres dejan a alguien
leyendo un perfil que describe una maquina que ya no esta puesta.

Lo que comprueba
================

    1. los cuatro campos (fabricante, modelo, socket, firmware) dicen lo mismo
       en `PERFIL/PLACA/PERFIL.txt` y en `plat/perfil_placa.rs`
    2. el `donde` de cada mana EXISTE
    3. y ese fichero todavia NOMBRA el modelo -- si el rodeo se quito, deja de
       nombrarlo y aqui se ve
    4. el numero de manas del perfil es el que declara el kernel (`MANAS`)

Lo que NO comprueba, y hay que decirlo
=======================================

    que la placa que hay puesta sea de verdad esta.

Eso no lo sabe el build: lo sabria el firmware, y preguntarselo es otro trabajo
(`plat/placa.rs` lee ACPI). El perfil se DECLARA, no se detecta -- y por eso el
kernel lo dice en CABINA como AVISO y no como dato.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
PERFIL = os.path.join(RAIZ, "PERFIL", "PLACA", "PERFIL.txt")
KERNEL = os.path.join(
    RAIZ, "Ultra_kernel_x86-64", "kernel", "src", "ring0", "plat", "perfil_placa.rs")

CAMPOS = ("fabricante", "modelo", "socket", "firmware")


def del_perfil():
    """(campos, manas) o (None, None) si el perfil no esta."""
    if not os.path.exists(PERFIL):
        return None, None
    with open(PERFIL, "r", encoding="utf-8", errors="replace") as fh:
        lineas = fh.read().splitlines()
    campos = {}
    manas = []
    actual = None
    for linea in lineas:
        pelada = linea.strip()
        m = re.match(r"^(\w+):\s+(.*)$", pelada)
        if not m:
            continue
        clave, valor = m.group(1), m.group(2).strip()
        if clave in CAMPOS and not manas and actual is None:
            campos[clave] = valor
        elif clave == "mana":
            actual = {"mana": valor}
            manas.append(actual)
        elif actual is not None and clave in ("que", "rodeo", "donde", "coste"):
            # Solo el primero: `que` y `rodeo` siguen en las lineas de abajo y
            # no hacen falta aqui.
            actual.setdefault(clave, valor)
    return campos, manas


def del_kernel():
    if not os.path.exists(KERNEL):
        return None
    with open(KERNEL, "r", encoding="utf-8", errors="replace") as fh:
        texto = fh.read()
    fuera = {}
    for nombre, clave in (("FABRICANTE", "fabricante"), ("MODELO", "modelo"),
                          ("SOCKET", "socket"), ("FIRMWARE", "firmware")):
        m = re.search(r'pub const %s: &str = "([^"]*)"' % nombre, texto)
        if m:
            fuera[clave] = m.group(1)
    m = re.search(r"pub const MANAS: u32 = (\d+)", texto)
    fuera["manas"] = int(m.group(1)) if m else None
    return fuera


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    campos, manas = del_perfil()
    # ** UN PERFIL QUE FALTA NO ES UN PERFIL LIMPIO. La leccion del `Guardian`
    # de build.ps1: un path mal escrito dejo un guardian muerto y el build dijo
    # COMPLETE igual.
    if campos is None:
        print("el perfil NO EXISTE: falta PERFIL/PLACA/PERFIL.txt")
        return 1 if args.check else 0
    kern = del_kernel()
    if kern is None:
        print("falta el otro lado: plat/perfil_placa.rs")
        return 1 if args.check else 0
    if not manas:
        print("el perfil no declara ni una mana: o esta vacio o su formato")
        print("cambio y este guardian dejo de entenderlo. Ninguna de las dos")
        print("cosas es 'todo en orden'.")
        return 1 if args.check else 0

    quejas = []

    # 1. los cuatro campos, en los dos sitios
    for c in CAMPOS:
        a, b = campos.get(c), kern.get(c)
        if a is None:
            quejas.append("PERFIL/PLACA/PERFIL.txt no dice `%s`" % c)
        elif b is None:
            quejas.append("perfil_placa.rs no declara %s" % c.upper())
        elif a != b:
            quejas.append("%s: el perfil dice '%s' y el kernel dice '%s'" % (c, a, b))

    modelo = campos.get("modelo")

    # 2 y 3. cada mana: su fichero existe y todavia nombra el modelo
    for m in manas:
        nombre = m.get("mana", "?")
        donde = m.get("donde")
        if not donde:
            quejas.append("la mana `%s` no dice DONDE se rodea" % nombre)
            continue
        ruta = os.path.join(RAIZ, donde.replace("/", os.sep))
        if not os.path.exists(ruta):
            quejas.append("`%s`: %s no existe" % (nombre, donde))
            continue
        if modelo:
            with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                if modelo not in fh.read():
                    quejas.append(
                        "`%s`: %s ya NO nombra la placa '%s' -- si el rodeo se "
                        "quito, quitalo tambien del perfil" % (nombre, donde, modelo))

    # 4. la cuenta
    if kern.get("manas") is None:
        quejas.append("perfil_placa.rs no declara MANAS")
    elif kern["manas"] != len(manas):
        quejas.append("el perfil trae %d mana(s) y el kernel declara MANAS = %d"
                      % (len(manas), kern["manas"]))

    if quejas:
        print("el perfil de la placa y el codigo NO dicen lo mismo:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  el perfil esta en PERFIL/PLACA/PERFIL.txt y su otra mitad en")
        print("  kernel/src/ring0/plat/perfil_placa.rs. Al cambiar de placa se")
        print("  tocan LOS DOS, y las manas se quitan de los dos a la vez.")
        return 1 if args.check else 0

    print("clean: el perfil de la placa cuadra -- %s %s (%s, %s), %d mana(s) "
          "rodeada(s) y todas con su fichero"
          % (campos["fabricante"], campos["modelo"], campos["socket"],
             campos["firmware"], len(manas)))
    return 0


if __name__ == "__main__":
    sys.exit(main())

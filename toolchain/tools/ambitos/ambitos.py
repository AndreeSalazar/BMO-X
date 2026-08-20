"""ambitos -- el guardian del AMBITO de un commit, y solo del ambito.

Por que existe
==============

El asunto de un commit de esta casa es `tipo(ambito): LA LECCION`. El formato se
cumple solo --375 de los ultimos 400-- asi que forzarlo no compra nada.

Lo que NO se cumple solo es el ambito. Contados el 2026-08-20:

    gui          69      el compositor
    director     13      el compositor, renombrado el 19-08 (`3e9552a4`)
    escritorio    7      el compositor, otra vez
    ui            3      y otra

Cuatro nombres para el mismo componente. Buscar "todo lo que toco el compositor"
exige sabersete los cuatro, y el que llegue en enero no se los sabe. En total,
**111 ambitos distintos** -- eso ya no es una lista cerrada, es un desague.

Lo que este guardian hace, y lo que NO hace
===========================================

Comprueba lo VERIFICABLE y nada mas:

    1. el asunto sigue `tipo(ambito): algo`
    2. `tipo` esta en los siete que se usan
    3. `ambito` esta en la lista, o es un alias con destino conocido
    4. el mensaje entero es ASCII

** Y NO MIRA LA PROSA. Ni la longitud del titulo, ni si hay `VERIFICADO`, ni si
el cuerpo explica el por que. Lo que hace valiosos a estos commits es que
cuentan una leccion, y una leccion no se puede plantillar: una regla que
intentara forzarla los convertiria en un formulario que se rellena sin pensar.
La plantilla de `.gitmessage` recuerda esas secciones; esto no las exige.

El trinquete, igual que el censo
================================

No juzga el pasado. `[DESDE]` guarda el commit en el que se cerro la lista y
solo se miran los que vienen despues -- si no, los 69 `gui` de ayer harian
fallar el guardian desde el primer dia, y un guardian que falla siempre se apaga
en una semana. Es el mismo razonamiento que `censo_modular.py` escribe entero.

Y la lista arranca con TODO lo que ya se usaba, ruido incluido. Podarla es
trabajo del dueno con la lista delante, no un juicio de quien escribio esto:
borrar una linea de `AMBITOS.txt` es lo que hace que ese nombre deje de valer.

Uso
===

    py ambitos.py --check              juzga de [DESDE] a HEAD; sale 1 si algo falla
    py ambitos.py --msg FICHERO        juzga UN mensaje (lo llama el hook)
    py ambitos.py --sellar             mueve [DESDE] a HEAD y recuenta
"""

import argparse
import os
import re
import subprocess
import sys

AQUI = os.path.dirname(os.path.abspath(__file__))
LISTA = os.path.join(AQUI, "AMBITOS.txt")

# Los siete que se usan. No es una eleccion: es lo que dice el historial.
TIPOS = ("feat", "fix", "docs", "refactor", "perf", "test", "chore")

ASUNTO = re.compile(r"^([a-z]+)\(([a-z0-9_-]+)\):\s+\S")


def leer_lista(ruta):
    """Devuelve (ambitos, alias, desde)."""
    ambitos, alias, desde = set(), {}, None
    seccion = None
    if not os.path.isfile(ruta):
        return ambitos, alias, desde
    with open(ruta, "r", encoding="utf-8") as fh:
        for linea in fh:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            if linea.startswith("["):
                seccion = linea.strip("[]")
                continue
            if seccion == "DESDE":
                desde = linea.split()[0]
            elif seccion == "AMBITOS":
                ambitos.add(linea.split()[0])
            elif seccion == "ALIAS":
                # `viejo -> nuevo   por que`
                partes = linea.split("->", 1)
                if len(partes) == 2:
                    viejo = partes[0].strip()
                    resto = partes[1].strip().split(None, 1)
                    alias[viejo] = (resto[0], resto[1] if len(resto) > 1 else "")
    return ambitos, alias, desde


def juzgar(mensaje, ambitos, alias):
    """Devuelve una lista de motivos. Vacia si el mensaje pasa."""
    motivos = []
    lineas = mensaje.splitlines()
    asunto = lineas[0] if lineas else ""

    # Un merge no lo escribe una persona: no se juzga lo que no se redacta.
    if asunto.startswith("Merge ") or asunto.startswith("Revert "):
        return motivos

    malos = sorted({c for c in mensaje if ord(c) > 126})
    if malos:
        motivos.append("el mensaje trae bytes no-ASCII: " + " ".join(malos))

    m = ASUNTO.match(asunto)
    if not m:
        motivos.append('el asunto no es `tipo(ambito): algo` -- "%s"' % asunto[:60])
        return motivos

    tipo, ambito = m.group(1), m.group(2)
    if tipo not in TIPOS:
        motivos.append("`%s` no es uno de los siete tipos: %s"
                       % (tipo, ", ".join(TIPOS)))
    if ambito in alias:
        destino, porque = alias[ambito]
        motivos.append("`%s` ya no se usa: es `%s`%s"
                       % (ambito, destino, ("  -- " + porque) if porque else ""))
    elif ambito not in ambitos:
        cerca = [a for a in sorted(ambitos) if a.startswith(ambito[:3])]
        pista = ("  quiza: " + ", ".join(cerca[:4])) if cerca else ""
        motivos.append("`%s` no esta en AMBITOS.txt.%s" % (ambito, pista))
        motivos.append("    si de verdad es un componente nuevo, anadelo alli con"
                       " una linea que diga que es.")
    return motivos


def commits_desde(desde):
    """(sha, mensaje) de cada commit posterior a `desde`."""
    if not desde:
        return []
    rango = desde + "..HEAD"
    out = subprocess.run(
        ["git", "log", "--format=%H%x00%B%x01", rango],
        capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if out.returncode != 0:
        return []
    fuera = []
    for trozo in out.stdout.split("\x01"):
        trozo = trozo.strip("\n")
        if not trozo:
            continue
        sha, _, cuerpo = trozo.partition("\x00")
        fuera.append((sha.strip(), cuerpo))
    return fuera


def sellar(ambitos, alias):
    """Recuenta el uso y mueve [DESDE] a HEAD."""
    head = subprocess.run(["git", "rev-parse", "HEAD"],
                          capture_output=True, text=True).stdout.strip()
    usos = {}
    out = subprocess.run(["git", "log", "--format=%s"],
                         capture_output=True, text=True,
                         encoding="utf-8", errors="replace").stdout
    for linea in out.splitlines():
        m = ASUNTO.match(linea)
        if m:
            usos[m.group(2)] = usos.get(m.group(2), 0) + 1

    with open(LISTA, "r", encoding="utf-8") as fh:
        lineas = fh.readlines()
    fuera, seccion = [], None
    for linea in lineas:
        s = linea.strip()
        if s.startswith("["):
            seccion = s.strip("[]")
            fuera.append(linea)
            continue
        if seccion == "DESDE" and s and not s.startswith("#"):
            fuera.append(head + "   sellado a mano\n")
            continue
        if seccion == "AMBITOS" and s and not s.startswith("#"):
            nombre = s.split()[0]
            resto = s.split(None, 1)[1] if len(s.split(None, 1)) > 1 else ""
            # La cuenta se regenera: es lo que hace visible el ruido al podar.
            resto = re.sub(r"^\(\d+\)\s*", "", resto)
            fuera.append("%-16s (%d) %s\n" % (nombre, usos.get(nombre, 0), resto))
            continue
        fuera.append(linea)
    with open(LISTA, "w", encoding="utf-8", newline="") as fh:
        fh.writelines(fuera)
    print("lista sellada en " + head[:12] + "; %d ambitos" % len(ambitos))
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="juzga de [DESDE] a HEAD y sale 1 si algo falla")
    ap.add_argument("--msg", metavar="FICHERO",
                    help="juzga UN mensaje de commit (lo llama el hook)")
    ap.add_argument("--sellar", action="store_true",
                    help="mueve [DESDE] a HEAD y recuenta el uso")
    args = ap.parse_args()

    ambitos, alias, desde = leer_lista(LISTA)
    if not ambitos:
        print("[X] no se pudo leer " + LISTA)
        return 1

    if args.sellar:
        return sellar(ambitos, alias)

    if args.msg:
        try:
            with open(args.msg, "r", encoding="utf-8", errors="replace") as fh:
                mensaje = fh.read()
        except OSError as e:
            print("[X] no se pudo leer el mensaje: %s" % e)
            return 1
        motivos = juzgar(mensaje, ambitos, alias)
        if motivos:
            print("el ambito de este commit no vale:")
            for m in motivos:
                print("  " + m)
            print("")
            print("  la lista esta en toolchain/tools/ambitos/AMBITOS.txt")
            return 1
        return 0

    malos = []
    total = 0
    for sha, mensaje in commits_desde(desde):
        total += 1
        motivos = juzgar(mensaje, ambitos, alias)
        if motivos:
            malos.append((sha, mensaje.splitlines()[0] if mensaje else "", motivos))

    if malos:
        print("commits con un ambito que no vale:")
        for sha, asunto, motivos in malos:
            print("  %s  %s" % (sha[:12], asunto[:64]))
            for m in motivos:
                print("      " + m)
        print("")
        print("%d de %d commits desde la linea base" % (len(malos), total))
        print("  si el ambito es bueno y falta, anadelo a AMBITOS.txt.")
        print("  si ya entro y no se puede arreglar, `--sellar` mueve la base.")
        return 1 if args.check else 0

    print("clean: los %d commits desde la base usan %d ambitos conocidos"
          % (total, len(ambitos)))
    return 0


if __name__ == "__main__":
    sys.exit(main())

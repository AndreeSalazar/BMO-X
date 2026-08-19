#!/usr/bin/env python3
"""censo_modular -- the meter for L6a, and a ratchet so the tree can only improve.

Why this exists
===============

MODULAR is the house rule. It is written as L6 in `META-KERNEL_HARD.md`, it has
four obligations, and L6a is a number: **a module over ~1.000 lines is split.
It is not a suggestion.**

Nothing measured it.

That is not a small gap, because a rule with a number and no meter behaves
exactly like a rule without a number. The proof is in the tree's own history:
`gui/main.rs` **grew 1.244 lines between 08-04 and 08-12** while a written plan
to split it already existed. Nobody decided that. It happened one commit at a
time, and no step was big enough to be worth stopping for.

    L4 of META-KERNEL_HARD: a rule is proven by saying NO.

So this tool exists to say NO on the day it happens, not three weeks later.

The ratchet, and why it is not a wall
=====================================

Nineteen files break L6a today. A guardian that fails on all nineteen from the
first run gets switched off within a day, and then it protects nothing -- the
same reasoning `enlaces.py` writes down about false alarms.

So the guardian does not judge the past. It judges the DELTA against a sealed
baseline (`LINEA_BASE.txt`), and it only ever says NO to two things:

    NUEVO     a file crosses 1.000 lines and was not on the list
    CRECIO    a file already on the list got bigger

Everything else is good news: a file that shrinks, or drops off the list, is
reported and the baseline is asked to be re-sealed. **The tree can only get
better, one commit at a time** -- which is the property that makes it safe to
change many things at once without them colliding in the same 3.000-line file.

The two species, because only one can be split without design
=============================================================

Lines alone do not say what a split costs. Lines per function does, and it
separates two different animals (measured across the tree on 2026-08-13):

    media ~30    a DRAWER: many small functions in a big file. Splitting is
                 moving text -- mechanical, and provable byte for byte (L6d)
    media 150+   ONE GIANT FUNCTION: the shared local state has to become a
                 struct first. That is a design change and a hash cannot prove
                 it. Another day, and another method.

The count is only emitted for languages where a function has an unambiguous
opening token (`fn`, `def`, `function`). For the rest the column says `-`,
because inventing a number here would be worse than not having one.

How this file is built, and it is on purpose
============================================

L7 says the generations are `abuelo -> padre -> hijo -> nieto` and that
**knowledge only goes down**. The meter for L7 is built by L7:

    abuelo   `medir`     lines and functions of ONE file. Does not know what big means
    padre    `Ficha`     names that file. Does not know there are others
    hijo     `especie`   relates two numbers of the padre. Does not judge them
    nieto    `juicio`    the verdict and the exit code. The only one with an opinion

Usage
=====

    py censo_modular.py               the report
    py censo_modular.py --check       exit 1 if something is new or grew
    py censo_modular.py --sellar      re-record the baseline as it is today
"""

import argparse
import io
import os
import re
import subprocess
import sys

# -- The number L6a puts, and where it comes from ------------------------------
#
# 1.000 lines is not a taste: it is the figure written in L6a of
# `META-KERNEL_HARD.md`. It lives here as one constant so that the day it
# changes, it changes in one place and the baseline is re-sealed against it.
LIMITE = 1000

# Below this a file is not reported at all. It exists so the report shows what
# is CLOSE to the limit -- a file at 980 lines is the next problem, and seeing
# it before it crosses is the whole point of a ratchet.
AVISO = 900

EXTS = ('.rs', '.py', '.c', '.h', '.cob', '.ps1')

# Where a function starts, per language. A language that is not here gets no
# count and says so with a `-`.
APERTURA = {
    '.rs': re.compile(
        r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?'
        r'(?:extern\s+"[^"]*"\s+)?fn\s', re.M),
    '.py': re.compile(r'^\s*def\s', re.M),
    '.ps1': re.compile(r'^\s*function\s', re.M | re.I),
}

BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'LINEA_BASE.txt')


# == ABUELO ====================================================================
# The raw fact: how many lines and how many functions this file has. It does not
# know what a limit is, it does not know there are other files, and it has no
# opinion about the numbers it returns.

def medir(ruta):
    """(lineas, funciones, generado) de UN fichero. `funciones` None si no se sabe."""
    try:
        s = io.open(ruta, encoding='utf-8', errors='replace').read()
    except OSError:
        return None
    lineas = s.count('\n') + 1
    patron = APERTURA.get(os.path.splitext(ruta)[1])
    # La marca que ya usa el arbol para lo que no escribio una persona
    # (`cobol-gen`, `c-gen`). Se busca solo en la cabecera: un fichero que la
    # mencione a mitad esta hablando de otro, no declarandose.
    generado = 'AUTO-GENERADO' in s[:400] or 'AUTO-GENERATED' in s[:400]
    return lineas, (len(patron.findall(s)) if patron else None), generado


# == PADRE =====================================================================
# Names the fact: this measurement belongs to this path. It does not know that
# other Fichas exist, which is why nothing here compares or sorts.

class Ficha:
    def __init__(self, ruta, lineas, funciones, generado=False):
        self.ruta = ruta
        self.lineas = lineas
        self.funciones = funciones
        self.generado = generado

    @property
    def media(self):
        """Lineas por funcion, o None cuando no hay cuenta de funciones."""
        if not self.funciones:
            return None
        return self.lineas // self.funciones


# == HIJO ======================================================================
# Relates two numbers of the padre -- lines against functions -- and gives the
# relation a name. It does NOT say whether that is good or bad: a drawer of
# 2.900 lines and a drawer of 300 get the same word here.

# Solo donde un fichero ES un modulo de funciones. Un guion de PowerShell son
# ordenes sueltas de arriba abajo, asi que dividir sus lineas entre sus
# `function` daria una media que no significa nada -- y una media que no
# significa nada es peor que una columna vacia.
CON_ESPECIE = ('.rs', '.py')


def especie(ficha):
    if not ficha.ruta.endswith(CON_ESPECIE):
        return 'desconocida'
    # Cero funciones en un lenguaje que SI se sabe contar no es ignorancia: es
    # un fichero de datos. Decir "desconocida" ahi seria echarle la culpa al
    # metro de algo que el metro sabe perfectamente.
    if ficha.funciones == 0:
        return 'TABLA'
    m = ficha.media
    if m is None:
        return 'desconocida'
    if m < 60:
        return 'CAJON'
    if m >= 150:
        return 'GIGANTE'
    return 'mixto'


COMO_SE_PARTE = {
    'CAJON': 'mecanico: mover texto, y demostrable byte a byte (L6d)',
    'GIGANTE': 'pide DISENO: el estado local tiene que volverse un struct',
    'mixto': 'a mano: hay funciones grandes entre las pequenas',
    'TABLA': 'son datos, no logica: mirar si lo deberia emitir una fabrica',
    'desconocida': 'sin cuenta de funciones para este lenguaje',
}


# -- Reading the tree and the baseline (plumbing, no generation) ---------------

def ficheros_del_repo(raiz):
    salida = subprocess.run(
        ['git', '-C', raiz, 'ls-files'],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    return [f for f in salida if f.endswith(EXTS)]


def censar(raiz):
    fichas = []
    for f in ficheros_del_repo(raiz):
        m = medir(os.path.join(raiz, f))
        if m is None:
            continue
        lineas, funciones, generado = m
        if lineas < AVISO:
            continue
        fichas.append(Ficha(f, lineas, funciones, generado))
    fichas.sort(key=lambda x: -x.lineas)
    return fichas


def leer_linea_base():
    """{ruta: lineas} de los techos, y {ruta: motivo} de los exentos."""
    techos, exentos = {}, {}
    if not os.path.isfile(BASE):
        return techos, exentos
    seccion = None
    for cruda in io.open(BASE, encoding='utf-8'):
        linea = cruda.strip()
        if not linea or linea.startswith('#'):
            continue
        if linea.startswith('[') and linea.endswith(']'):
            seccion = linea[1:-1].upper()
            continue
        partes = linea.split(None, 1)
        if seccion == 'TECHOS' and len(partes) == 2:
            techos[partes[1].strip()] = int(partes[0])
        elif seccion == 'EXENTOS' and len(partes) == 2:
            exentos[partes[0]] = partes[1].strip()
    return techos, exentos


def sellar(fichas, exentos):
    hoy = []
    hoy.append('# LINEA BASE del censo modular -- el techo de cada fichero que')
    hoy.append('# hoy incumple L6a. La regla del trinquete: un fichero de esta')
    hoy.append('# lista solo puede ENCOGER. Si crece, o si aparece uno nuevo por')
    hoy.append('# encima de %d lineas, `censo_modular.py --check` dice NO.' % LIMITE)
    hoy.append('#')
    hoy.append('# No se edita a mano: se regenera con `--sellar` cuando un')
    hoy.append('# reparto baja un numero, y el commit ensena cuanto bajo.')
    hoy.append('')
    hoy.append('[TECHOS]')
    for f in fichas:
        if f.lineas > LIMITE and f.ruta not in exentos and not f.generado:
            hoy.append('%6d  %s' % (f.lineas, f.ruta))
    hoy.append('')
    hoy.append('# EXENTOS -- un "no" con motivo escrito, que se puede discutir.')
    hoy.append('# Aqui solo entra lo que NO lo escribio una persona: si la')
    hoy.append('# modularidad de un fichero la decide su fabrica, la regla se le')
    hoy.append('# aplica a la fabrica y no a lo que emite.')
    hoy.append('')
    hoy.append('[EXENTOS]')
    for ruta, motivo in sorted(exentos.items()):
        hoy.append('%s  %s' % (ruta, motivo))
    hoy.append('')
    io.open(BASE, 'w', encoding='utf-8', newline='\n').write('\n'.join(hoy))


# == NIETO =====================================================================
# The verdict. The only generation with an opinion: it is the one that knows
# what the limit means, what the baseline promised, and what deserves an exit
# code of 1. It lives at the end because nothing above it may ask what it thinks.

def juicio(fichas, techos, exentos):
    nuevos, crecidos, encogidos, salidos = [], [], [], []
    vistos = set()

    for f in fichas:
        if f.ruta in exentos or f.generado:
            continue
        vistos.add(f.ruta)
        techo = techos.get(f.ruta)
        if techo is None:
            if f.lineas > LIMITE:
                nuevos.append(f)
        elif f.lineas > techo:
            crecidos.append((f, techo))
        elif f.lineas < techo:
            encogidos.append((f, techo))

    for ruta, techo in techos.items():
        if ruta not in vistos:
            salidos.append((ruta, techo))

    return nuevos, crecidos, encogidos, salidos


def informe(fichas, techos, exentos, nuevos, crecidos, encogidos, salidos):
    print('%7s %5s %6s  %-58s %s' % ('lineas', 'fns', 'media', 'fichero', 'especie'))
    fuera = []
    for f in fichas:
        if f.ruta in exentos or f.generado:
            fuera.append(f)
            continue
        marca = '!' if f.lineas > LIMITE else ' '
        print('%7d %5s %6s %s %-58s %s' % (
            f.lineas,
            f.funciones if f.funciones is not None else '-',
            f.media if f.media is not None else '-',
            marca, f.ruta, especie(f)))

    pasan = [f for f in fichas
             if f.lineas > LIMITE and f.ruta not in exentos and not f.generado]
    cajones = [f for f in pasan if especie(f) == 'CAJON']
    print()
    print('%d ficheros incumplen L6a (>%d lineas). %d son CAJON, o sea que se'
          % (len(pasan), LIMITE, len(cajones)))
    print('parten moviendo texto y el reparto se demuestra con un hash (L6d).')
    for f in fuera:
        motivo = exentos.get(f.ruta) or 'lo emite una fabrica: dice AUTO-GENERADO'
        print('  [-] fuera del censo  %s (%d) -- %s' % (f.ruta, f.lineas, motivo))

    for f, techo in encogidos:
        print('  [+] ENCOGIO   %s: %d -> %d' % (f.ruta, techo, f.lineas))
    for ruta, techo in salidos:
        print('  [+] YA NO ESTA %s (estaba en %d)' % (ruta, techo))
    if encogidos or salidos:
        print('      -> `--sellar` para que el trinquete no permita volver atras.')

    for f in nuevos:
        print('  [X] NUEVO     %s: %d lineas, y no estaba en la linea base' % (f.ruta, f.lineas))
        print('                %s -> %s' % (especie(f), COMO_SE_PARTE[especie(f)]))
    for f, techo in crecidos:
        print('  [X] CRECIO    %s: %d -> %d (+%d)' % (f.ruta, techo, f.lineas, f.lineas - techo))


def main():
    ap = argparse.ArgumentParser(description='El metro de L6a, con trinquete.')
    ap.add_argument('--check', action='store_true',
                    help='sale con 1 si algo es nuevo o crecio')
    ap.add_argument('--sellar', action='store_true',
                    help='vuelve a grabar la linea base tal como esta hoy')
    ap.add_argument('--raiz', default=None, help='raiz del repo (por defecto, la de este fichero)')
    args = ap.parse_args()

    raiz = args.raiz or os.path.abspath(
        os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))

    fichas = censar(raiz)
    techos, exentos = leer_linea_base()

    if args.sellar:
        sellar(fichas, exentos)
        print('linea base sellada: %s' % BASE)
        return 0

    nuevos, crecidos, encogidos, salidos = juicio(fichas, techos, exentos)
    informe(fichas, techos, exentos, nuevos, crecidos, encogidos, salidos)

    if not techos:
        print('\n[!] no hay linea base todavia: `--sellar` la graba.')
        return 0

    if nuevos or crecidos:
        print('\nL6a: %d nuevos, %d crecidos.' % (len(nuevos), len(crecidos)))
        return 1 if args.check else 0

    print('\nclean: ningun fichero nuevo por encima de %d y ninguno crecio.' % LIMITE)
    return 0


if __name__ == '__main__':
    sys.exit(main())

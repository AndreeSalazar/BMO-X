#!/usr/bin/env python3
"""herencia -- the meter for L7, read from Cargo.toml and never guessed.

Why this exists, and why it does NOT parse `use`
================================================

L7 says the generations are `abuelo -> padre -> hijo -> nieto` and that
**knowledge only goes down**: no generation knows who consumes it. It was
doctrine in four documents and nothing checked it.

The obvious meter -- read every `use` in every file -- is the wrong one, and one
grep proves it. `syscall/entry.rs` is labelled **abuelo** in `CENSO_DE_EJES.md`,
and line 41 of that file says:

    use super::dispatch;

The grandfather names the father. That is not a bug in the kernel: it is two
different relations wearing the same four words.

    a PIPELINE of data    the consumer imports the producer
                          -> knowledge and dependency point the SAME way
    a CHAIN of calls      the caller imports the callee
                          -> the least-knowing piece imports the most-knowing one

In the door, `entry.rs` knows least (it cannot know which operation was asked
for) and calls first. Its generation label is a claim about **what it knows**,
which is what makes a measurement falsifiable -- *"the 246 cycles cannot be in
the stub, because the stub does not know the operation"*. It is not a claim
about who imports whom.

So a `use`-based checker would have condemned correct code on its first run,
which is the fastest way to get a guardian switched off.

What this checks instead: CRATES, and it is exact
=================================================

Where a generation IS a crate, the relation is declared, not inferred: it is the
`[dependencies]` block of a `Cargo.toml`. Nothing is parsed out of code, nothing
is heuristic, and a `pub use` cannot hide an edge.

That is also where L7 already lives in this tree. MAQUETA is five crates with
one generation each, `bmo-disco-juicio` is a nieto that lives outside the binary
it judges (L7b), and the ciclos meter ends in `bmo-juicio`.

The rule, and the only thing it says NO to
==========================================

    rango:  abuelo 0 · padre 1 · hijo 2 · nieto 3 · bisnieto 4

    bisnieto -> nieto     baja: correcto
    nieto    -> nieto     mismo escalon: L7a lo permite
    abuelo   -> padre     SUBE: el abuelo sabria quien lo consume  -> NO

Two declared non-generations exist so that "unlabelled" never means two things
at once:

    ninguna   no es una generacion (un consumidor, una utilidad compartida)
    varias    el crate lleva mas de una generacion DENTRO, por modulos. El
              chequeo de crates no puede juzgarlo, y lo dice en vez de callar

How this file is built, and it is on purpose
============================================

    abuelo   `leer_cargo`   nombre y dependencias de UN Cargo.toml. Nada mas
    padre    `Crate`        junta ese crate con su generacion declarada
    hijo     `aristas`      relaciona dos crates del padre. No dice si esta mal
    nieto    `juicio`       el veredicto y el codigo de salida
"""

import io
import os
import re
import subprocess
import sys

RANGO = {'abuelo': 0, 'padre': 1, 'hijo': 2, 'nieto': 3, 'bisnieto': 4}
DECLARADAS_SIN_RANGO = ('ninguna', 'varias')

ETIQUETA = re.compile(r'^\s*//!\s*generacion:\s*([a-z]+)', re.M)
# `nombre = { path = "..." }` dentro de [dependencies]. Solo las de ruta: una
# dependencia de crates.io no es de este arbol y no tiene generacion que juzgar.
DEP_RUTA = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}]*\bpath\s*=\s*"([^"]+)"', re.M)
NOMBRE = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)


# == ABUELO ====================================================================
# El hecho crudo de UN Cargo.toml: como se llama y a que rutas depende. No sabe
# que es una generacion ni que existen otros crates.

def leer_cargo(ruta):
    try:
        s = io.open(ruta, encoding='utf-8', errors='replace').read()
    except OSError:
        return None
    m = NOMBRE.search(s)
    if not m:
        return None
    deps = []
    dentro = s.split('[dependencies]', 1)
    if len(dentro) == 2:
        bloque = re.split(r'^\[', dentro[1], maxsplit=1, flags=re.M)[0]
        deps = [d for _, d in DEP_RUTA.findall(bloque)]
    return m.group(1), deps


def leer_generacion(dir_crate):
    """La etiqueta `//! generacion: X` de la cabecera del crate, o None."""
    for cabeza in ('src/lib.rs', 'src/main.rs'):
        p = os.path.join(dir_crate, cabeza)
        if not os.path.isfile(p):
            continue
        try:
            s = io.open(p, encoding='utf-8', errors='replace').read(4000)
        except OSError:
            continue
        m = ETIQUETA.search(s)
        if m:
            return m.group(1)
    return None


# == PADRE =====================================================================
# Nombra el hecho: este crate, en esta carpeta, declara esta generacion. No sabe
# que hay otros crates ni compara con nadie.

class Crate:
    def __init__(self, nombre, carpeta, deps, generacion):
        self.nombre = nombre
        self.carpeta = carpeta
        self.deps = deps
        self.generacion = generacion

    @property
    def rango(self):
        return RANGO.get(self.generacion)


def censar(raiz):
    salida = subprocess.run(
        ['git', '-C', raiz, 'ls-files'],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    crates = {}
    for f in salida:
        if os.path.basename(f) != 'Cargo.toml':
            continue
        carpeta = os.path.dirname(f)
        leido = leer_cargo(os.path.join(raiz, f))
        if leido is None:
            continue
        nombre, deps = leido
        absolutas = [os.path.normpath(os.path.join(carpeta, d)).replace(os.sep, '/')
                     for d in deps]
        crates[carpeta] = Crate(nombre, carpeta, absolutas,
                                leer_generacion(os.path.join(raiz, carpeta)))
    return crates


# == HIJO ======================================================================
# Relaciona dos crates del padre: de quien a quien va cada arista, y con que
# rangos. No dice si eso esta bien -- solo que existe y que se puede comparar.

def aristas(crates):
    juzgables, con_varias = [], []
    for c in crates.values():
        if c.rango is None:
            continue
        for destino in c.deps:
            otro = crates.get(destino)
            if otro is None or otro.generacion is None:
                continue
            if otro.generacion == 'varias':
                con_varias.append((c, otro))
            elif otro.rango is not None:
                juzgables.append((c, otro))
    return juzgables, con_varias


# == NIETO =====================================================================
# El veredicto. El unico que sabe que significa que un rango sea mayor que otro,
# y el unico que decide el codigo de salida.

def juicio(juzgables):
    return [(o, d) for o, d in juzgables if d.rango > o.rango]


def informe(crates, juzgables, con_varias, rotas):
    etiquetados = [c for c in crates.values() if c.generacion]
    con_rango = [c for c in etiquetados if c.rango is not None]
    print('%d crates en el arbol, %d con generacion declarada, %d con rango.'
          % (len(crates), len(etiquetados), len(con_rango)))
    print('%d aristas juzgadas.' % len(juzgables))

    for c in sorted(con_rango, key=lambda x: (x.rango, x.nombre)):
        abajo = [d.nombre for o, d in juzgables if o is c]
        print('  %-9s %-24s -> %s' % (c.generacion, c.nombre,
                                      ', '.join(abajo) if abajo else '(no depende de nadie)'))

    for o, d in con_varias:
        print('  [-] sin juzgar  %s -> %s: el destino declara `varias`' % (o.nombre, d.nombre))

    for o, d in rotas:
        print('  [X] SUBE  %s (%s) depende de %s (%s)'
              % (o.nombre, o.generacion, d.nombre, d.generacion))
        print('            el conocimiento solo baja: o el dato sube como')
        print('            parametro, o las dos son la misma generacion (L7a)')

    if not rotas:
        print('\nclean: ninguna dependencia sube de generacion.')


def etiquetas_sueltas(raiz, crates):
    """`generacion:` en ficheros que NO son la cabeza de un crate.

    No es un fallo y no puede serlo: L7c dice que la generacion se comprueba
    entre crates. Se cuentan y se dicen **porque el silencio seria la trampa**
    -- alguien que etiquete un modulo esperando que lo juzguen tiene derecho a
    saber que no lo juzga nadie, y por que.
    """
    cabezas = set()
    for c in crates.values():
        for cabeza in ('src/lib.rs', 'src/main.rs'):
            cabezas.add((c.carpeta + '/' + cabeza).replace('//', '/'))
    salida = subprocess.run(
        ['git', '-C', raiz, 'ls-files'],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    sueltas = []
    for f in salida:
        if not f.endswith('.rs') or f in cabezas:
            continue
        try:
            s = io.open(os.path.join(raiz, f), encoding='utf-8', errors='replace').read(4000)
        except OSError:
            continue
        m = ETIQUETA.search(s)
        if m:
            sueltas.append((f, m.group(1)))
    return sueltas


def revisar(raiz):
    """Devuelve 0 o 1, con el informe impreso. Lo usa `censo_modular --check`."""
    crates = censar(raiz)
    juzgables, con_varias = aristas(crates)
    rotas = juicio(juzgables)
    informe(crates, juzgables, con_varias, rotas)

    # El vocabulario esta CERRADO. Una palabra inventada no rompe el arbol,
    # pero empieza la deriva de siempre: dos listas de lo mismo que dejan de
    # decir lo mismo. Se avisa en cuanto aparece.
    validas = set(RANGO) | set(DECLARADAS_SIN_RANGO)
    raras = [c for c in crates.values()
             if c.generacion and c.generacion not in validas]
    for c in raras:
        print('  [!] palabra no declarada  %s dice `%s`. El vocabulario es: %s'
              % (c.nombre, c.generacion, ', '.join(sorted(validas))))

    sueltas = etiquetas_sueltas(raiz, crates)
    if sueltas:
        cuenta = {}
        for _, gen in sueltas:
            cuenta[gen] = cuenta.get(gen, 0) + 1
        print('\n%d ficheros llevan `generacion:` sin ser la cabeza de un crate'
              % len(sueltas))
        print('(%s). Se documentan y NO se juzgan: L7c.'
              % ', '.join('%s %d' % (g, n) for g, n in sorted(cuenta.items())))

    return 1 if rotas else 0


def main():
    raiz = os.path.abspath(
        os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))
    codigo = revisar(raiz)
    if '--check' not in sys.argv:
        return 0
    return codigo


if __name__ == '__main__':
    sys.exit(main())

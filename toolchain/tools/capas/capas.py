#!/usr/bin/env python3
"""capas -- el metro de L8: EL PRINCIPAL NO SE NOMBRA DESDE ABAJO.

Por que existe (2026-09-13)
===========================

Eddi: *"analizar todos los que conectan con el principal -- Ring 3, el intermedio,
el kernel y Ring 0 -- y construir el guardian que obligue CLARO que dependencia
es, para facilitar y matar el espagueti"*.

Se midio antes de escribir una linea, y el espagueti NO estaba donde parecia:

    entre CRATES (81, en 8 workspaces)   casi limpio: 3 aristas que cruzaban
                                         mal, y las tres eran lo mismo -- logica
                                         PURA viviendo en `platform/drivers/`
    DENTRO del kernel                    un solo NUDO de 13 de sus 16
                                         subsistemas y 27 parejas que se
                                         importan en los dos sentidos
    DENTRO del DIRECTOR                  un nudo de 4 (commands, desktop,
                                         scene, watch)
    DENTRO de bmo-userland               limpio

Por eso este guardian tiene DOS varas, y no son la misma:

    CRATES   MURO. Las capas se declaran por carpeta, la direccion permitida es
             una tabla, y una arista que sube no pasa NUNCA. Es exacto: sale de
             `[dependencies]`, que un `pub use` no puede esconder (L7c).
    NUDOS    TRINQUETE. Una pareja de subsistemas que se importan en los dos
             sentidos no se deshace en una tarde -- el kernel tiene 27 --, asi
             que las de hoy van a `LINEA_BASE.txt` y lo que se prohibe es una
             pareja NUEVA. Solo pueden bajar.

Las capas, de abajo arriba
==========================

    puro         platform/shared    logica sin hardware, probada en el anfitrion
    contrato     platform/abi       el ABI: lo que Ring 0 y Ring 3 firman
    driver       platform/drivers   codigo que ENLAZA el kernel y toca aparatos
    arranque     boot_context, faggin, uefi_chain
    nucleo       el kernel          EL PRINCIPAL. Nadie lo enlaza
    ring3        Ultra_userspace    habla con el principal por el ABI y nada mas
    herramienta  toolchain          no corre en la maquina

    nucleo       -> nucleo arranque driver contrato puro
    driver       -> driver contrato puro
    contrato     -> contrato puro
    puro         -> puro
    arranque     -> arranque
    ring3        -> ring3 contrato puro          ** NUNCA driver ni nucleo
    herramienta  -> herramienta contrato puro driver

** La regla que mas importa es la de `ring3`: **Ring 3 no enlaza codigo que corre
en Ring 0.** Si dos anillos necesitan la misma logica, esa logica es `puro` y vive
en `platform/shared`; si necesitan hablar, hablan por el ABI.

La capa se DECLARA, y la declaracion se COMPRUEBA
=================================================

Un crate puede declarar en su cabecera `//! capa: puro -- motivo` aunque viva en
otra carpeta: `bmo-rtc` DECIDE fechas y no toca un puerto. Pero una declaracion
que no se comprueba es una opinion, asi que `puro` exige que el crate no tenga
un solo `unsafe` o que lleve `#![forbid(unsafe_code)]`. Solo se puede declarar
`puro`: declarar que algo SUBE de capa no le quita a nadie una restriccion.

Lo que NO mide, dicho
=====================

Los nudos se cuentan por `crate::a::b`. Un `super::super::` que cruce carpeta no
se ve, ni un `use` dentro de una macro. Es una cota inferior, y como trinquete
basta: lo que se ve no puede empeorar.

Como esta construido (L7)
=========================

    abuelo   leer_cargo, leer_capa, contar_unsafe, usos   un fichero, un hecho
    padre    Crate, censar, medir_nudos                   junta los hechos
    hijo     aristas                                      relaciona dos crates
    nieto    juzgar, probar                               el veredicto
"""

import argparse
import collections
import io
import os
import re
import subprocess
import sys
import tempfile

AQUI = os.path.dirname(os.path.abspath(__file__))
BASE = os.path.join(AQUI, 'LINEA_BASE.txt')

PERMITIDO = {
    'nucleo': {'nucleo', 'arranque', 'driver', 'contrato', 'puro'},
    'arranque': {'arranque'},
    'driver': {'driver', 'contrato', 'puro'},
    'contrato': {'contrato', 'puro'},
    'puro': {'puro'},
    'ring3': {'ring3', 'contrato', 'puro'},
    'herramienta': {'herramienta', 'contrato', 'puro', 'driver'},
}

# El orden importa: el primer prefijo que case gana.
POR_CARPETA = (
    ('Ultra_kernel_x86-64/kernel', 'nucleo'),
    ('Ultra_kernel_x86-64/', 'arranque'),
    ('Ultra_userspace/', 'ring3'),
    ('platform/abi/', 'contrato'),
    ('platform/shared/', 'puro'),
    ('platform/drivers/', 'driver'),
    ('platform/services/', 'driver'),
    ('toolchain/', 'herramienta'),
)

DECLARABLES = {'puro'}

# Donde se miden los nudos: (nombre, carpeta de fuentes, niveles de ruta que
# hacen un subsistema).
BINARIOS = (
    ('kernel', 'Ultra_kernel_x86-64/kernel/src', 2),
    ('director', 'Ultra_userspace/services/director/src', 1),
    ('userland', 'Ultra_userspace/userland/src', 1),
)

DEP_RUTA = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}\n]*\bpath\s*=\s*"([^"]+)"', re.M)
NOMBRE = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)
CAPA = re.compile(r'^\s*//!\s*capa:\s*([a-z0-9]+)(?:\s*--\s*(\S.*))?$', re.M)
UNSAFE = re.compile(r'\bunsafe\b')
FORBID = re.compile(r'#!\[\s*forbid\(\s*unsafe_code\s*\)\s*\]')
USO = re.compile(r'\bcrate::((?:[a-z_][a-z0-9_]*::)*[a-z_][a-z0-9_]*)')


# == ABUELO ====================================================================

def leer(ruta, tope=None):
    try:
        with io.open(ruta, encoding='utf-8-sig', errors='replace') as f:
            return f.read(tope) if tope else f.read()
    except OSError:
        return None


def seccion(texto, nombre):
    trozos = texto.split('[%s]' % nombre, 1)
    if len(trozos) != 2:
        return ''
    return re.split(r'^\[', trozos[1], maxsplit=1, flags=re.M)[0]


def leer_cargo(ruta):
    """(nombre, [rutas de dependencias]) de un Cargo.toml con [package], o None."""
    t = leer(ruta)
    if t is None:
        return None
    paquete = seccion(t, 'package')
    m = NOMBRE.search(paquete)
    if not m:
        return None
    return m.group(1), [p for _, p in DEP_RUTA.findall(seccion(t, 'dependencies'))]


def leer_capa(dir_crate):
    """(capa, motivo) declarada en la cabecera, o (None, None)."""
    for cabeza in ('src/lib.rs', 'src/main.rs'):
        t = leer(os.path.join(dir_crate, cabeza), 6000)
        if t:
            m = CAPA.search(t)
            if m:
                return m.group(1), (m.group(2) or '').strip()
    return None, None


def sin_comentarios(t):
    t = re.sub(r'/\*.*?\*/', '', t, flags=re.S)
    return re.sub(r'//[^\n]*', '', t)


def contar_unsafe(dir_crate):
    """(cuantos `unsafe` hay en el codigo, si lleva forbid(unsafe_code))."""
    n, prohibe = 0, False
    for d, _, fs in os.walk(os.path.join(dir_crate, 'src')):
        for f in fs:
            if f.endswith('.rs'):
                t = leer(os.path.join(d, f)) or ''
                prohibe = prohibe or bool(FORBID.search(t))
                n += len(UNSAFE.findall(sin_comentarios(t)))
    return n, prohibe


def usos(texto):
    return [m.group(1).split('::') for m in USO.finditer(sin_comentarios(texto))]


# == PADRE =====================================================================

class Crate:
    def __init__(self, nombre, carpeta, deps, capa, declarada, motivo, n_unsafe, prohibe):
        self.nombre, self.carpeta, self.deps = nombre, carpeta, deps
        self.capa, self.declarada, self.motivo = capa, declarada, motivo
        self.n_unsafe, self.prohibe = n_unsafe, prohibe


def capa_por_carpeta(carpeta):
    c = carpeta.replace(os.sep, '/') + '/'
    for prefijo, capa in POR_CARPETA:
        if c.startswith(prefijo.rstrip('/') + '/'):
            return capa
    return None


def censar(raiz):
    # ** Con `--others`: un crate NUEVO, que todavia no esta en git, tambien se
    # juzga. Sin esto el guardian no vio `bmo-foco` el dia que se escribio -- y lo
    # que se escapa es justo lo que se esta anadiendo, que es lo que hay que mirar.
    ficheros = subprocess.run(['git', '-C', raiz, 'ls-files', '--cached', '--others', '--exclude-standard'],
                              capture_output=True, text=True, check=True).stdout.splitlines()
    crates = {}
    for f in ficheros:
        if os.path.basename(f) != 'Cargo.toml':
            continue
        leido = leer_cargo(os.path.join(raiz, f))
        if leido is None:
            continue
        carpeta = os.path.dirname(f).replace(os.sep, '/')
        nombre, deps = leido
        absolutas = [os.path.normpath(os.path.join(carpeta, d)).replace(os.sep, '/') for d in deps]
        dir_abs = os.path.join(raiz, carpeta)
        declarada, motivo = leer_capa(dir_abs)
        n, prohibe = contar_unsafe(dir_abs)
        crates[carpeta] = Crate(nombre, carpeta, absolutas, capa_por_carpeta(carpeta),
                                declarada, motivo, n, prohibe)
    return crates


def subsistema(rel, nivel):
    partes = rel.replace(os.sep, '/').split('/')
    partes[-1] = partes[-1][:-3]
    if partes[-1] in ('mod', 'lib', 'main'):
        partes = partes[:-1]
    return '/'.join(partes[:nivel])


def medir_nudos(src, nivel):
    """({(a, b): usos}, parejas mutuas) de los subsistemas bajo `src`."""
    aristas, nodos = collections.Counter(), set()
    for d, _, fs in os.walk(src):
        for f in fs:
            if not f.endswith('.rs'):
                continue
            rel = os.path.relpath(os.path.join(d, f), src)
            a = subsistema(rel, nivel)
            if not a:
                continue
            nodos.add(a)
            for ruta in usos(leer(os.path.join(d, f)) or ''):
                b = '/'.join(ruta[:nivel])
                if b and b != a and not a.startswith(b + '/') and not b.startswith(a + '/'):
                    aristas[(a, b)] += 1
    aristas = collections.Counter({k: v for k, v in aristas.items() if k[1] in nodos})
    mutuas = {tuple(sorted(k)) for k in aristas if (k[1], k[0]) in aristas}
    return aristas, mutuas


# == HIJO ======================================================================

def aristas(crates):
    for c in crates.values():
        for d in c.deps:
            if d in crates:
                yield c, crates[d]


# == NIETO =====================================================================

def capa_efectiva(c):
    return c.declarada if c.declarada in DECLARABLES else c.capa


def por_que(o, d):
    if d == 'nucleo':
        return 'nadie enlaza el nucleo: es el principal, y se le habla por el ABI'
    if o == 'ring3' and d in ('driver', 'arranque'):
        return 'Ring 3 no enlaza codigo que corre en Ring 0: lo compartido es `puro`, lo demas va por el ABI'
    if o == 'puro':
        return 'un crate puro no sabe de nadie: si necesita ese tipo, el tipo tambien es puro'
    return '%s solo puede depender de: %s' % (o, ', '.join(sorted(PERMITIDO.get(o, ()))))


def juzgar_crates(crates):
    """Lista de textos, uno por cada cosa que esta mal. Vacia = limpio."""
    malas = []
    for c in sorted(crates.values(), key=lambda c: c.carpeta):
        if c.capa is None:
            malas.append('%s (%s): su carpeta no es de ninguna capa -- anadela a POR_CARPETA' % (c.nombre, c.carpeta))
        if c.declarada is not None:
            if c.declarada not in DECLARABLES:
                malas.append('%s declara `capa: %s`, y solo se puede declarar: %s'
                             % (c.nombre, c.declarada, ', '.join(sorted(DECLARABLES))))
            elif not c.motivo:
                malas.append('%s declara `capa: %s` sin motivo (`//! capa: puro -- por que`)' % (c.nombre, c.declarada))
            elif c.n_unsafe and not c.prohibe:
                malas.append('%s declara `capa: puro` y tiene %d `unsafe`: puro es sin `unsafe` o con forbid(unsafe_code)'
                             % (c.nombre, c.n_unsafe))
    for o, d in aristas(crates):
        co, cd = capa_efectiva(o), capa_efectiva(d)
        if co is None or cd is None:
            continue
        if cd not in PERMITIDO.get(co, ()):
            malas.append('%s [%s] -> %s [%s]: %s' % (o.nombre, co, d.nombre, cd, por_que(co, cd)))
    return malas


def leer_linea_base(ruta=BASE):
    base = collections.defaultdict(set)
    t = leer(ruta)
    for linea in (t or '').splitlines():
        partes = linea.split()
        if len(partes) == 4 and partes[0] == 'nudo':
            base[partes[1]].add((partes[2], partes[3]))
    return base, t is not None


def juzgar_nudos(hoy, base):
    """(nuevas, deshechas) por binario."""
    nuevas, deshechas = {}, {}
    for nombre, mutuas in hoy.items():
        nuevas[nombre] = sorted(mutuas - base.get(nombre, set()))
        deshechas[nombre] = sorted(base.get(nombre, set()) - mutuas)
    return nuevas, deshechas


# == LA AUTOPRUEBA: un guardian que no sabe decir NO no guarda nada ============

def probar():
    fallos = []

    def exige(nombre, condicion):
        if not condicion:
            fallos.append(nombre)

    def cr(nombre, carpeta, deps=(), declarada=None, motivo='x', n_unsafe=0, prohibe=False):
        return Crate(nombre, carpeta, list(deps), capa_por_carpeta(carpeta), declarada, motivo, n_unsafe, prohibe)

    def juicio(*cs):
        return juzgar_crates({c.carpeta: c for c in cs})

    drv = cr('drv', 'platform/drivers/x')
    pur = cr('pur', 'platform/shared/p')
    ker = cr('ker', 'Ultra_kernel_x86-64/kernel', ['platform/drivers/x', 'platform/shared/p'])
    exige('el nucleo puede enlazar drivers y puros', juicio(drv, pur, ker) == [])
    exige('Ring 3 -> driver dice NO', juicio(drv, cr('app', 'Ultra_userspace/a', ['platform/drivers/x'])) != [])
    exige('Ring 3 -> nucleo dice NO', juicio(ker, drv, pur, cr('app', 'Ultra_userspace/a', ['Ultra_kernel_x86-64/kernel'])) != [])
    exige('herramienta -> nucleo dice NO', juicio(ker, drv, pur, cr('t', 'toolchain/t', ['Ultra_kernel_x86-64/kernel'])) != [])
    exige('puro -> driver dice NO', juicio(drv, cr('j', 'platform/shared/j', ['platform/drivers/x'])) != [])
    exige('Ring 3 -> puro pasa', juicio(pur, cr('app', 'Ultra_userspace/a', ['platform/shared/p'])) == [])
    exige('un driver que DECLARA puro, sin unsafe, lo es',
          juicio(cr('rtc', 'platform/drivers/rtc', declarada='puro'), cr('app', 'Ultra_userspace/a', ['platform/drivers/rtc'])) == [])
    exige('declarar puro con unsafe dice NO', juicio(cr('m', 'platform/drivers/m', declarada='puro', n_unsafe=3)) != [])
    exige('declarar puro con unsafe pero forbid pasa', juicio(cr('m', 'platform/drivers/m', declarada='puro', n_unsafe=1, prohibe=True)) == [])
    exige('declarar puro sin motivo dice NO', juicio(cr('m', 'platform/drivers/m', declarada='puro', motivo='')) != [])
    exige('declarar nucleo dice NO', juicio(cr('m', 'platform/drivers/m', declarada='nucleo')) != [])
    exige('carpeta sin capa dice NO', juicio(cr('z', 'otra/cosa')) != [])

    nuevas, deshechas = juzgar_nudos({'k': {('a', 'b')}}, {})
    exige('una pareja mutua nueva dice NO', nuevas['k'] == [('a', 'b')])
    nuevas, deshechas = juzgar_nudos({'k': {('a', 'b')}}, {'k': {('a', 'b'), ('c', 'd')}})
    exige('la de la base pasa, y la deshecha se ve', nuevas['k'] == [] and deshechas['k'] == [('c', 'd')])

    with tempfile.TemporaryDirectory() as t:
        for rel, texto in (('a/mod.rs', 'use crate::b::x;'), ('b/uno.rs', 'fn f() { crate::a::y(); }'),
                           ('c.rs', '// use crate::a::z;\nuse crate::b::w;')):
            os.makedirs(os.path.dirname(os.path.join(t, rel)) or t, exist_ok=True)
            io.open(os.path.join(t, rel), 'w', encoding='utf-8').write(texto)
        aristas_, mutuas = medir_nudos(t, 1)
        exige('mide la pareja a<->b', mutuas == {('a', 'b')})
        exige('un `crate::` comentado no cuenta', ('c', 'a') not in aristas_ and ('c', 'b') in aristas_)
    return fallos


# == LA SALIDA =================================================================

def main():
    ap = argparse.ArgumentParser(description='El metro de L8: capas entre crates y nudos dentro.')
    ap.add_argument('--check', action='store_true', help='sale con 1 si algo sube de capa o hay un nudo nuevo')
    ap.add_argument('--mapa', action='store_true', help='ensena el mapa entero: capas, aristas y parejas')
    ap.add_argument('--sellar', action='store_true', help='graba las parejas de hoy en la linea base')
    ap.add_argument('--motivo', default='', help='POR QUE entra una pareja nueva en la base. Sin esto, se rechaza')
    ap.add_argument('--raiz', default=None)
    args = ap.parse_args()
    raiz = args.raiz or os.path.abspath(os.path.join(AQUI, '..', '..', '..'))

    fallos = probar()
    if fallos:
        print('el guardian NO sabe decir que no -- autoprueba rota:')
        for f in fallos:
            print('  [x] ' + f)
        return 1

    crates = censar(raiz)
    malas = juzgar_crates(crates)
    hoy, detalle = {}, {}
    for nombre, src, nivel in BINARIOS:
        detalle[nombre], hoy[nombre] = medir_nudos(os.path.join(raiz, src), nivel)
    base, hay_base = leer_linea_base()
    nuevas, deshechas = juzgar_nudos(hoy, base)

    por_capa = collections.Counter(capa_efectiva(c) for c in crates.values())
    tipos = collections.Counter((capa_efectiva(o), capa_efectiva(d)) for o, d in aristas(crates))

    if args.mapa:
        print('== CAPAS (%d crates)' % len(crates))
        for c in sorted(crates.values(), key=lambda c: (str(capa_efectiva(c)), c.nombre)):
            extra = '  (declara %s: %s)' % (c.declarada, c.motivo) if c.declarada else ''
            print('  %-12s %-28s %s%s' % (capa_efectiva(c), c.nombre, c.carpeta, extra))
        print('\n== ARISTAS POR CAPA')
        for (o, d), n in sorted(tipos.items()):
            print('  %-12s -> %-12s %3d' % (o, d, n))
        for nombre, _, _ in BINARIOS:
            ar = detalle[nombre]
            print('\n== NUDOS de %s: %d parejas en los dos sentidos' % (nombre, len(hoy[nombre])))
            for a, b in sorted(hoy[nombre], key=lambda p: -(ar[p] + ar[(p[1], p[0])])):
                print('  %-24s <-> %-24s %4d / %-4d' % (a, b, ar[(a, b)], ar[(b, a)]))

    if args.sellar:
        if malas:
            print('no se sella con aristas que suben de capa: eso es un muro, no un trinquete')
            for m in malas:
                print('  [x] ' + m)
            return 1
        suben = sum(len(v) for v in nuevas.values())
        if suben and not args.motivo.strip():
            print('entran %d parejas nuevas en la base: hace falta --motivo' % suben)
            return 1
        lineas = ['# LINEA_BASE de capas.py (L8) -- las parejas de subsistemas que HOY se importan',
                  '# en los dos sentidos. Solo pueden bajar: una pareja nueva rompe el build.',
                  '# Se regenera con `py toolchain/tools/capas/capas.py --sellar`.']
        viejo = leer(BASE) or ''
        lineas += [l for l in viejo.splitlines() if l.startswith('# subida')]
        if suben:
            lineas.append('# subida: %s' % args.motivo.strip())
        for nombre, _, _ in BINARIOS:
            for a, b in sorted(hoy[nombre]):
                lineas.append('nudo %s %s %s' % (nombre, a, b))
        io.open(BASE, 'w', encoding='utf-8', newline='\n').write('\n'.join(lineas) + '\n')
        print('linea base sellada: %s' % BASE)
        return 0

    fallo = False
    if malas:
        fallo = True
        print('L8: %d dependencia(s) que no respetan las capas:' % len(malas))
        for m in malas:
            print('  [x] ' + m)
    if not hay_base:
        print('[!] no hay linea base de nudos todavia: `--sellar` la graba.')
    for nombre, _, _ in BINARIOS:
        for a, b in nuevas.get(nombre, []) if hay_base else []:
            fallo = True
            ar = detalle[nombre]
            print('  [x] %s: %s <-> %s se importan en los DOS sentidos (%d / %d usos) y no estaba en la base'
                  % (nombre, a, b, ar[(a, b)], ar[(b, a)]))
        for a, b in deshechas.get(nombre, []):
            print('  [+] %s: %s <-> %s ya NO es un nudo. Sella (`--sellar`) para que no pueda volver.' % (nombre, a, b))
    if fallo:
        print('\nComo se arregla: la dependencia baja de capa (el tipo compartido se muda a `puro`),')
        print('o el dato sube como parametro. Ver L8 en FUERO/META-KERNEL_HARD.md.')
        return 1

    declaradas = sorted(c.nombre for c in crates.values() if c.declarada)
    print('clean: %d crates en %d capas, %d aristas, y ninguna sube de capa (%s declaran puro y lo son)'
          % (len(crates), len(por_capa), sum(tipos.values()), ', '.join(declaradas) or 'ningun crate'))
    print('clean: nudos -- %s; ninguno nuevo'
          % ', '.join('%s %d (base %d)' % (n, len(hoy[n]), len(base.get(n, ()))) for n, _, _ in BINARIOS))
    return 0


if __name__ == '__main__':
    sys.exit(main())

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

Por eso este guardian tiene TRES varas, y no son la misma:

    CRATES    MURO. Las capas se declaran por carpeta, la direccion permitida es
              una tabla, y una arista que sube no pasa NUNCA. Es exacto: sale de
              `[dependencies]`, que un `pub use` no puede esconder (L7c).
    NUDOS     TRINQUETE. Una pareja de subsistemas que se importan en los dos
              sentidos no se deshace en una tarde, asi que las de hoy van a
              `LINEA_BASE.txt` y lo que se prohibe es una pareja NUEVA.
    FAMILIAS  (L8b, solo el kernel -- el principal). Cada subsistema DICE en su
              cabecera que familia es, en que nivel esta y a quien conecta:

                  //! [familia] cabina  nivel 1 -- el registro que todos llaman
                  //! [conecta] reloj

              y el guardian lo EXIGE PRIMERO: sin esa declaracion no juzga nada
              mas. Despues comprueba que la cabecera no miente (cada `use` a
              otra familia esta en `[conecta]`, y todo lo de `[conecta]` se usa)
              y que las aristas BAJAN de nivel. Las que hoy suben van a la base
              como `sube kernel a b`: trinquete, solo pueden bajar.

** Eddi, sobre la tercera: *"comentario especial que diga que familia es, que el
kernel empieza en 0 y conecta con el siguiente, para guiar en los puntos
importantes -- y que el guardian lo exija primero"*. La cabecera es el mapa que
se lee sin abrir el codigo, y el guardian es lo que impide que el mapa se
quede viejo.

Las capas de crates, de abajo arriba
====================================

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

Un crate puede declarar `//! capa: puro -- motivo` aunque viva en otra carpeta,
y la declaracion se comprueba: cero `unsafe` o `#![forbid(unsafe_code)]`.

Lo que NO mide, dicho
=====================

Los usos se cuentan por `crate::a::b` (con los grupos `crate::a::{b, c}`
expandidos). Un `super::super::` que cruce carpeta no se ve, ni un `use` dentro
de una macro. Es una cota inferior, y como trinquete basta.

Como esta construido (L7)
=========================

    abuelo   leer_cargo, leer_capa, contar_unsafe, usos, leer_familia
    padre    Crate, censar, medir_nudos, leer_familias
    hijo     aristas
    nieto    juzgar_crates, juzgar_nudos, juzgar_familias, probar
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

# (nombre, carpeta de fuentes, niveles de ruta que hacen un subsistema,
#  raices que no son familia -- o None si ese binario no declara familias).
BINARIOS = (
    ('kernel', 'Ultra_kernel_x86-64/kernel/src', 2, {'ring0'}),
    ('director', 'Ultra_userspace/services/director/src', 1, None),
    ('userland', 'Ultra_userspace/userland/src', 1, None),
)

DEP_RUTA = re.compile(r'^\s*([A-Za-z0-9_-]+)\s*=\s*\{[^}\n]*\bpath\s*=\s*"([^"]+)"', re.M)
NOMBRE = re.compile(r'^\s*name\s*=\s*"([^"]+)"', re.M)
CAPA = re.compile(r'^\s*//!\s*capa:\s*([a-z0-9]+)(?:\s*--\s*(\S.*))?$', re.M)
UNSAFE = re.compile(r'\bunsafe\b')
FORBID = re.compile(r'#!\[\s*forbid\(\s*unsafe_code\s*\)\s*\]')
IDENT = re.compile(r'[A-Za-z_][A-Za-z0-9_]*')
FAMILIA = re.compile(r'^\s*//!\s*\[familia\]\s+([a-z_][a-z0-9_]*)\s+nivel\s+(\d+)\s*--\s*(\S.*)$', re.M)
CONECTA = re.compile(r'^\s*//!\s*\[conecta\][ \t]*(.*)$', re.M)


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
    m = NOMBRE.search(seccion(t, 'package'))
    if not m:
        return None
    return m.group(1), [p for _, p in DEP_RUTA.findall(seccion(t, 'dependencies'))]


def leer_capa(dir_crate):
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
    n, prohibe = 0, False
    for d, _, fs in os.walk(os.path.join(dir_crate, 'src')):
        for f in fs:
            if f.endswith('.rs'):
                t = leer(os.path.join(d, f)) or ''
                prohibe = prohibe or bool(FORBID.search(t))
                n += len(UNSAFE.findall(sin_comentarios(t)))
    return n, prohibe


def _ruta(t, i, prefijo):
    """Las rutas que salen de `t[i:]`: `a::b::{c, d::e}` -> [a,b,c] y [a,b,d,e].

    ** Hasta el 2026-09-13 esto era una expresion regular que se paraba en la
    llave: `use crate::ring0::{obj, task};` contaba como UNA arista hacia `ring0`
    y escondia las dos de verdad. Se vio al preparar L8b del kernel.
    """
    ruta = list(prefijo)
    while True:
        m = IDENT.match(t, i)
        if m:
            ruta.append(m.group(0))
            i = m.end()
            if t.startswith('::', i):
                i += 2
                if t.startswith('{', i):
                    return _grupo(t, i + 1, ruta)
                continue
            return [ruta]
        if t.startswith('{', i):
            return _grupo(t, i + 1, ruta)
        return [ruta] if ruta else []


def _grupo(t, i, prefijo):
    trozos, prof, inicio, j = [], 0, i, i
    while j < len(t):
        c = t[j]
        if c == '{':
            prof += 1
        elif c == '}':
            if prof == 0:
                trozos.append(t[inicio:j])
                break
            prof -= 1
        elif c == ',' and prof == 0:
            trozos.append(t[inicio:j])
            inicio = j + 1
        j += 1
    salida = []
    for tr in trozos:
        tr = tr.strip()
        if tr == 'self':
            salida.append(list(prefijo))
        elif tr:
            salida.extend(_ruta(tr, 0, prefijo))
    return salida


def usos(texto):
    t = sin_comentarios(texto)
    salida = []
    for m in re.finditer(r'\bcrate::', t):
        salida.extend(_ruta(t, m.end(), []))
    return salida


def cabecera_de(src, sub):
    """El texto donde un subsistema declara su familia.

    ** PRIMERO el bloque EN LINEA del padre (`pub mod obj { //! [familia] ... }`):
    en el kernel, `obj`, `task`, `fsys`, `plat`, `core` y `dev` no tienen
    `mod.rs` -- se declaran dentro de `ring0/mod.rs`, y un `dev/mod.rs` que
    exista al lado NO se compila. Leer el fichero primero habria juzgado una
    cabecera muerta.
    """
    partes = sub.split('/')
    padre_dir = os.path.join(src, *partes[:-1])
    for padre in ('mod.rs', 'lib.rs', 'main.rs'):
        t = leer(os.path.join(padre_dir, padre))
        if not t:
            continue
        m = re.search(r'^[ \t]*pub(?:\([a-z]+\))?[ \t]+mod[ \t]+%s[ \t]*\{[ \t]*\n' % re.escape(partes[-1]), t, re.M)
        if m:
            lineas = []
            for linea in t[m.end():].splitlines():
                if not linea.strip().startswith('//'):
                    break
                lineas.append(linea.strip())
            return '\n'.join(lineas)
    for p in (os.path.join(src, *partes, 'mod.rs'), os.path.join(src, *partes) + '.rs'):
        t = leer(p, 8000)
        if t is not None:
            return t
    return None


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
    # juzga. Sin esto el guardian no vio `bmo-foco` el dia que se escribio.
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
    """({(a, b): usos}, parejas mutuas, subsistemas) bajo `src`."""
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
    return aristas, mutuas, nodos


def leer_familias(src, nodos, raices):
    """({subsistema: familia}, [subsistemas sin declaracion])."""
    familias, faltan = {}, []
    for n in sorted(nodos):
        if n in raices:
            continue
        t = cabecera_de(src, n) or ''
        m = FAMILIA.search(t)
        if not m:
            faltan.append(n)
            continue
        c = CONECTA.search(t)
        conecta = set()
        if c:
            conecta = {x for x in re.split(r'[\s,]+', c.group(1).strip()) if x and x != '-'}
        familias[n] = {'nombre': m.group(1), 'nivel': int(m.group(2)), 'que': m.group(3).strip(),
                       'conecta': conecta, 'tiene_conecta': c is not None}
    return familias, faltan


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


def corto(sub):
    return sub.split('/')[-1]


def juzgar_familias(aristas_, familias, faltan, raices):
    """(errores, subidas). Los errores rompen siempre; las subidas, contra la base.

    ** EL ORDEN ES LA REGLA: si falta una declaracion no se juzga nada mas. Una
    familia sin nivel no se puede comparar con nadie, y un informe de subidas
    medio calculado sobre un mapa medio escrito confunde mas que ayuda.
    """
    if faltan:
        return (['%s no declara su familia. Ponle en la cabecera:\n'
                 '        //! [familia] %s  nivel N -- que es y para quien\n'
                 '        //! [conecta] las familias que usa, o -' % (n, corto(n)) for n in faltan], set())
    errores, subidas = [], set()
    for n, f in sorted(familias.items()):
        if f['nombre'] != corto(n):
            errores.append('%s declara `[familia] %s`: el nombre es el de su carpeta o fichero' % (n, f['nombre']))
        if not f['tiene_conecta']:
            errores.append('%s no tiene `//! [conecta]` (con `-` si no usa a nadie)' % n)
    usados = collections.defaultdict(set)
    for (a, b) in aristas_:
        if a in raices or b in raices or a not in familias or b not in familias:
            continue
        usados[a].add(corto(b))
        if corto(b) not in familias[a]['conecta']:
            errores.append('%s usa %s y su `[conecta]` no lo dice' % (a, corto(b)))
        if familias[b]['nivel'] >= familias[a]['nivel']:
            subidas.add((corto(a), corto(b)))
    for n, f in sorted(familias.items()):
        for s in sorted(f['conecta'] - usados[n]):
            errores.append('%s declara `[conecta] %s` y no lo usa: la cabecera miente' % (n, s))
    return errores, subidas


def leer_linea_base(ruta=BASE):
    """(nudos por binario, subidas por binario, si habia base)."""
    nudos, subidas = collections.defaultdict(set), collections.defaultdict(set)
    t = leer(ruta)
    for linea in (t or '').splitlines():
        p = linea.split()
        if len(p) == 4 and p[0] == 'nudo':
            nudos[p[1]].add((p[2], p[3]))
        elif len(p) == 4 and p[0] == 'sube':
            subidas[p[1]].add((p[2], p[3]))
    return nudos, subidas, t is not None


def comparar(hoy, base):
    """(nuevas, deshechas) por binario, para cualquier conjunto de parejas."""
    nuevas, deshechas = {}, {}
    for nombre, actuales in hoy.items():
        nuevas[nombre] = sorted(actuales - base.get(nombre, set()))
        deshechas[nombre] = sorted(base.get(nombre, set()) - actuales)
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

    nuevas, deshechas = comparar({'k': {('a', 'b')}}, {})
    exige('una pareja nueva dice NO', nuevas['k'] == [('a', 'b')])
    nuevas, deshechas = comparar({'k': {('a', 'b')}}, {'k': {('a', 'b'), ('c', 'd')}})
    exige('la de la base pasa, y la deshecha se ve', nuevas['k'] == [] and deshechas['k'] == [('c', 'd')])
    exige('un grupo con llaves son VARIAS rutas',
          usos('use crate::ring0::{obj::cap, task::{proc, self}};') ==
          [['ring0', 'obj', 'cap'], ['ring0', 'task', 'proc'], ['ring0', 'task']])

    def arbol(ficheros):
        t = tempfile.mkdtemp()
        for rel, texto in ficheros.items():
            p = os.path.join(t, *rel.split('/'))
            os.makedirs(os.path.dirname(p), exist_ok=True)
            io.open(p, 'w', encoding='utf-8').write(texto)
        return t

    t = arbol({'a/mod.rs': 'use crate::b::x;', 'b/uno.rs': 'fn f() { crate::a::y(); }',
               'c.rs': '// use crate::a::z;\nuse crate::b::w;'})
    ar, mutuas, _ = medir_nudos(t, 1)
    exige('mide la pareja a<->b', mutuas == {('a', 'b')})
    exige('un `crate::` comentado no cuenta', ('c', 'a') not in ar and ('c', 'b') in ar)

    base_ok = {
        'alto/mod.rs': '//! [familia] alto  nivel 2 -- el de arriba\n//! [conecta] bajo\nuse crate::bajo::x;',
        'bajo.rs': '//! [familia] bajo  nivel 0 -- el suelo\n//! [conecta] -\nfn f() {}',
    }

    def familias_de(ficheros):
        t = arbol(ficheros)
        ar, _, nodos = medir_nudos(t, 1)
        fams, faltan = leer_familias(t, nodos, set())
        return juzgar_familias(ar, fams, faltan, set())

    errores, subidas = familias_de(base_ok)
    exige('una familia bien declarada que baja pasa', errores == [] and subidas == set())
    errores, _ = familias_de(dict(base_ok, **{'otro.rs': 'use crate::bajo::x;'}))
    exige('un subsistema SIN [familia] se exige primero', len(errores) == 1 and 'otro' in errores[0])
    errores, _ = familias_de(dict(base_ok, **{'bajo.rs': '//! [familia] bajo  nivel 0 -- el suelo\n//! [conecta] -\nuse crate::alto::y;'}))
    exige('usar una familia que [conecta] no dice es NO', any('no lo dice' in e for e in errores))
    errores, subidas = familias_de(dict(base_ok, **{'bajo.rs': '//! [familia] bajo  nivel 0 -- el suelo\n//! [conecta] alto\nuse crate::alto::y;'}))
    exige('declarada y usada hacia arriba es una SUBIDA, no un error', errores == [] and subidas == {('bajo', 'alto')})
    errores, _ = familias_de(dict(base_ok, **{'alto/mod.rs': '//! [familia] alto  nivel 2 -- x\n//! [conecta] bajo, fantasma\nuse crate::bajo::x;'}))
    exige('un [conecta] que no se usa es una cabecera que miente', any('miente' in e for e in errores))
    return fallos


# == LA SALIDA =================================================================

def main():
    ap = argparse.ArgumentParser(description='El metro de L8: capas entre crates, nudos y familias dentro.')
    ap.add_argument('--check', action='store_true', help='sale con 1 si algo sube de capa, hay un nudo o una subida nueva, o falta una familia')
    ap.add_argument('--mapa', action='store_true', help='ensena el mapa entero')
    ap.add_argument('--conecta', action='store_true', help='imprime, por subsistema del kernel, a que familias conecta HOY')
    ap.add_argument('--sellar', action='store_true', help='graba nudos y subidas de hoy en la linea base')
    ap.add_argument('--motivo', default='', help='POR QUE entra algo nuevo en la base. Sin esto, se rechaza')
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
    nudos_hoy, subidas_hoy, detalle, familias, errores_fam = {}, {}, {}, {}, {}
    for nombre, src, nivel, raices in BINARIOS:
        src_abs = os.path.join(raiz, src)
        ar, mutuas, nodos = medir_nudos(src_abs, nivel)
        detalle[nombre], nudos_hoy[nombre] = ar, mutuas
        if raices is not None:
            fams, faltan = leer_familias(src_abs, nodos, raices)
            familias[nombre] = fams
            errores_fam[nombre], subidas_hoy[nombre] = juzgar_familias(ar, fams, faltan, raices)
    base_nudos, base_subidas, hay_base = leer_linea_base()
    nudos_nuevos, nudos_idos = comparar(nudos_hoy, base_nudos)
    sub_nuevas, sub_idas = comparar(subidas_hoy, base_subidas)

    if args.conecta:
        for nombre, src, nivel, raices in BINARIOS:
            if raices is None:
                continue
            ar = detalle[nombre]
            por = collections.defaultdict(set)
            for (a, b) in ar:
                if a not in raices and b not in raices:
                    por[a].add(corto(b))
            nodos = {a for a, _ in ar} | {b for _, b in ar}
            for n in sorted(nodos - raices):
                print('%-22s %s' % (n, ', '.join(sorted(por[n])) or '-'))
        return 0

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
        for nombre, _, _, _ in BINARIOS:
            ar = detalle[nombre]
            print('\n== NUDOS de %s: %d parejas en los dos sentidos' % (nombre, len(nudos_hoy[nombre])))
            for a, b in sorted(nudos_hoy[nombre], key=lambda p: -(ar[p] + ar[(p[1], p[0])])):
                print('  %-24s <-> %-24s %4d / %-4d' % (a, b, ar[(a, b)], ar[(b, a)]))
            if nombre in familias:
                print('\n== FAMILIAS de %s, de abajo arriba' % nombre)
                for n, f in sorted(familias[nombre].items(), key=lambda kv: (kv[1]['nivel'], kv[0])):
                    print('  nivel %2d  %-12s %s' % (f['nivel'], f['nombre'], f['que']))
                    print('            conecta: %s' % (', '.join(sorted(f['conecta'])) or '-'))
                print('\n== SUBIDAS de %s: %d aristas van a un nivel igual o mas alto' % (nombre, len(subidas_hoy[nombre])))
                pares = [(a, b) for (a, b) in ar if corto(a) != corto(b)]
                for a, b in sorted(subidas_hoy[nombre]):
                    usos_ = sum(v for (x, y), v in ar.items() if corto(x) == a and corto(y) == b)
                    print('  %-12s -> %-12s %4d usos' % (a, b, usos_))

    if args.sellar:
        if malas or any(errores_fam.values()):
            print('no se sella con errores: una capa que sube o una familia mal declarada es un muro, no un trinquete')
            for m in malas + [e for es in errores_fam.values() for e in es]:
                print('  [x] ' + m)
            return 1
        entran = sum(len(v) for v in nudos_nuevos.values()) + sum(len(v) for v in sub_nuevas.values())
        if entran and not args.motivo.strip():
            print('entran %d parejas nuevas en la base: hace falta --motivo' % entran)
            return 1
        lineas = ['# LINEA_BASE de capas.py (L8) -- lo que HOY se tolera, y solo puede bajar.',
                  '#   nudo <binario> <a> <b>   dos subsistemas que se importan en los dos sentidos',
                  '#   sube <binario> <a> <b>   una familia que usa otra de su nivel o mas alta (L8b)',
                  '# Se regenera con `py toolchain/tools/capas/capas.py --sellar --motivo "..."`.']
        viejo = leer(BASE) or ''
        lineas += [l for l in viejo.splitlines() if l.startswith('# subida')]
        if entran:
            lineas.append('# subida: %s' % args.motivo.strip())
        for nombre, _, _, _ in BINARIOS:
            for a, b in sorted(nudos_hoy[nombre]):
                lineas.append('nudo %s %s %s' % (nombre, a, b))
            for a, b in sorted(subidas_hoy.get(nombre, ())):
                lineas.append('sube %s %s %s' % (nombre, a, b))
        io.open(BASE, 'w', encoding='utf-8', newline='\n').write('\n'.join(lineas) + '\n')
        print('linea base sellada: %s' % BASE)
        return 0

    fallo = False
    for nombre, errores in errores_fam.items():
        if errores:
            fallo = True
            print('L8b: las familias de %s -- esto se exige PRIMERO:' % nombre)
            for e in errores:
                print('  [x] ' + e)
    if malas:
        fallo = True
        print('L8: %d dependencia(s) que no respetan las capas:' % len(malas))
        for m in malas:
            print('  [x] ' + m)
    if not hay_base:
        print('[!] no hay linea base todavia: `--sellar` la graba.')
    for nombre, _, _, _ in BINARIOS:
        ar = detalle[nombre]
        for a, b in (nudos_nuevos.get(nombre, []) if hay_base else []):
            fallo = True
            print('  [x] %s: %s <-> %s se importan en los DOS sentidos (%d / %d usos) y no estaba en la base'
                  % (nombre, a, b, ar[(a, b)], ar[(b, a)]))
        for a, b in nudos_idos.get(nombre, []):
            print('  [+] %s: %s <-> %s ya NO es un nudo. Sella (`--sellar`) para que no pueda volver.' % (nombre, a, b))
        if errores_fam.get(nombre):
            continue
        for a, b in (sub_nuevas.get(nombre, []) if hay_base else []):
            fallo = True
            print('  [x] %s: la familia %s usa %s, que esta en su nivel o mas arriba, y no estaba en la base'
                  % (nombre, a, b))
        for a, b in sub_idas.get(nombre, []):
            print('  [+] %s: %s ya no sube a %s. Sella (`--sellar`) para que no pueda volver.' % (nombre, a, b))
    if fallo:
        print('\nComo se arregla: la dependencia baja (el dato sube como parametro, o lo compartido')
        print('se muda a una familia mas baja), y la cabecera dice la verdad. Ver L8 y L8b en')
        print('FUERO/META-KERNEL_HARD.md.')
        return 1

    declaradas = sorted(c.nombre for c in crates.values() if c.declarada)
    print('clean: %d crates en %d capas, %d aristas, y ninguna sube de capa (%s declaran puro y lo son)'
          % (len(crates), len(por_capa), sum(tipos.values()), ', '.join(declaradas) or 'ningun crate'))
    print('clean: nudos -- %s; ninguno nuevo'
          % ', '.join('%s %d (base %d)' % (n, len(nudos_hoy[n]), len(base_nudos.get(n, ()))) for n, _, _, _ in BINARIOS))
    for nombre, fams in familias.items():
        print('clean: familias de %s -- %d declaradas y todas dicen la verdad; %d aristas suben (base %d), ninguna nueva'
              % (nombre, len(fams), len(subidas_hoy[nombre]), len(base_subidas.get(nombre, ()))))
    return 0


if __name__ == '__main__':
    sys.exit(main())

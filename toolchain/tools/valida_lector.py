# -*- coding: utf-8 -*-
"""Valida el lector de simbolos de Ring 3 SIN el Ryzen.

`simbolos.rs` vive en el compositor, que es `no_std`/`no_main`: no admite
`#[cfg(test)]` que corra (misma leccion que el kernel esta manana). Asi que se
reproduce AQUI su algoritmo exacto --los mismos desplazamientos codificados a
mano-- contra un `.bex` de verdad, y se compara con lo que dice `--map`.

Si un desplazamiento esta mal, se ve ahora y no en el arranque.
"""
import io, struct, subprocess, sys, glob, os

# Las MISMAS constantes que `simbolos.rs`. Si divergen, este script miente.
SECTION_SYMBOLS = 0x08
SECTION_ENTRY = 48
SYMBOL = 32
TABLA_CADENAS = 8
KIND_FUNCTION = 0x01

def localizar(b):
    if b[:4] != b'BEF1':
        return None
    tabla = struct.unpack_from('<Q', b, 32)[0]
    count = struct.unpack_from('<I', b, 40)[0]
    if count == 0 or count > 255:
        return None
    sec_off = sec_len = 0
    for i in range(count):
        e = tabla + i * SECTION_ENTRY
        if b[e] == SECTION_SYMBOLS:
            sec_off, sec_len = struct.unpack_from('<QQ', b, e + 8)
            break
    if sec_len < TABLA_CADENAS:
        return None
    n = struct.unpack_from('<I', b, sec_off)[0]
    fin = TABLA_CADENAS + n * SYMBOL
    if n == 0 or fin > sec_len:
        return None
    return sec_off, n, sec_off + fin

def resolver(b, desp):
    loc = localizar(b)
    if not loc:
        return None
    sec_off, n, cadenas = loc
    for i in range(n):
        e = sec_off + TABLA_CADENAS + i * SYMBOL
        if b[e + 24] != KIND_FUNCTION:
            continue
        addr = struct.unpack_from('<Q', b, e + 8)[0]
        size = struct.unpack_from('<Q', b, e + 16)[0]
        if not (addr <= desp < addr + size):
            continue
        name_off = struct.unpack_from('<I', b, e)[0]
        p = cadenas + name_off
        fin = b.index(b'\0', p)
        return b[p:fin].decode('ascii', 'replace'), desp - addr
    return None

EXE = os.path.abspath('target/release/bmo-c-front.exe')
EJ = 'toolchain/lang/c/examples'
S = os.environ['S']

fallos = 0
for bex in sorted(glob.glob(S + '/*.bex')):
    nombre = os.path.basename(bex)[:-4]
    fuente = os.path.join(EJ, nombre + '.c')
    if not os.path.exists(fuente):
        continue
    b = io.open(bex, 'rb').read()
    salida = subprocess.run([EXE, fuente, '--map'], capture_output=True, text=True).stdout
    mapa = []
    for l in salida.splitlines():
        t = l.split()
        if len(t) >= 2:
            try:
                mapa.append((int(t[0], 16), t[1]))
            except ValueError:
                pass
    if not mapa:
        print('  %-14s (--map no dio nada)' % nombre)
        continue
    mapa.sort()
    ok = malos = 0
    for k, (off, fn) in enumerate(mapa):
        # Justo el inicio, y un byte mas adentro.
        for pruebo, esperado_sobra in ((off, 0), (off + 1, 1)):
            r = resolver(b, pruebo)
            if r is None:
                continue          # puede caer fuera si la funcion mide 1 byte
            got, sobra = r
            if got == fn and sobra == esperado_sobra:
                ok += 1
            else:
                malos += 1
                if malos <= 2:
                    print('    %s: %#x deberia ser %s+%d y el lector dice %s+%d'
                          % (nombre, pruebo, fn, esperado_sobra, got, sobra))
    fallos += malos
    print('  %-14s %3d funciones  %3d aciertos  %d fallos' % (nombre, len(mapa), ok, malos))

print()
print('TOTAL DE FALLOS DEL LECTOR: %d' % fallos)
sys.exit(1 if fallos else 0)

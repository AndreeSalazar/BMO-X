# -*- coding: utf-8 -*-
"""Que funcion es ese `rip` de RING 3, leido de la tabla de simbolos del .bex.

`toolchain/tools/simbolo` resuelve contra el ELF del KERNEL. Este `rip` es de
Ring 3 -- vive dentro de `doom.bex`-- y el compilador de C escribe ahi su propia
tabla de simbolos (seccion 0x08).
"""
import io, struct, sys

BEX = r"C:\Users\Salazar\Documents\BMO\Ultra_kernel_x86-64\staging\BMO-DATA\apps\doom.bex"
RIP = int(sys.argv[1], 16) if len(sys.argv) > 1 else 0x004000725C

d = open(BEX, "rb").read()
magic, vmaj, vmin, flags = struct.unpack_from("<IHHI", d, 0)
entry_off, sec_off = struct.unpack_from("<QQ", d, 24)
sec_n, total = struct.unpack_from("<II", d, 40)
print("BEF magic=%08X v%d.%d  secciones=%d  fichero=%d B" % (magic, vmaj, vmin, sec_n, len(d)))
print()

secs = []
for i in range(sec_n):
    o = sec_off + i * 48
    kind = d[o]
    fl, = struct.unpack_from("<I", d, o + 4)
    fo, fs, ms, va = struct.unpack_from("<QQQQ", d, o + 8)
    secs.append((kind, fo, fs, ms, va))
    nombre = {1: ".code", 2: ".rodata", 3: ".data", 4: ".bss", 5: "imports",
              6: "exports", 7: "relocs", 8: "SIMBOLOS", 9: "manifest",
              11: "recursos"}.get(kind, "0x%02X" % kind)
    print("  %-10s file=+%-8d %-8d B   virt=0x%08X  mem=%d" % (nombre, fo, fs, va, ms))

sim = [s for s in secs if s[0] == 8]
if not sim:
    print("\n[X] este .bex NO lleva tabla de simbolos")
    raise SystemExit(1)

_, fo, fs, _, _ = sim[0]
blob = d[fo:fo + fs]
count, = struct.unpack_from("<I", blob, 0)
CAB = 8            # TablaCadenas: count u32 + reservado, alineado a 8
ENT = 32           # Symbol::SIZE
ini = CAB
fin = ini + count * ENT
cadenas = blob[fin:]
print("\n  %d simbolos, %d B de cadenas" % (count, len(cadenas)))


def nombre_de(off):
    z = cadenas.find(b"\0", off)
    return cadenas[off:z if z >= 0 else None].decode("ascii", "replace")


syms = []
for i in range(count):
    o = ini + i * ENT
    name_off, name_hash = struct.unpack_from("<II", blob, o)
    va, size = struct.unpack_from("<QQ", blob, o + 8)
    kind, binding, vis, sidx = struct.unpack_from("<BBBB", blob, o + 24)
    syms.append((va, size, name_off, sidx))

syms.sort()
base = 0x40000000
off = RIP - base
print("\n  rip = 0x%X   ->  +0x%X dentro de la imagen\n" % (RIP, off))

# El que lo CONTIENE, y si no, el anterior mas cercano.
dentro = [s for s in syms if s[0] <= off < s[0] + s[1]]
if dentro:
    va, size, n, _ = dentro[0]
    print("  *** %s + 0x%X   (empieza en +0x%X, mide %d B)"
          % (nombre_de(n), off - va, va, size))
else:
    antes = [s for s in syms if s[0] <= off]
    if antes:
        va, size, n, _ = antes[-1]
        print("  el anterior: %s en +0x%X, mide %d B  ->  acaba en +0x%X"
              % (nombre_de(n), va, size, va + size))
        print("  [!] el rip cae FUERA de el: +0x%X" % off)
    else:
        print("  [!] antes del primer simbolo")

print("\n  -- los cinco de alrededor --")
for va, size, n, _ in syms:
    if abs(va - off) < 0x400:
        marca = "  <-- AQUI" if va <= off < va + size else ""
        print("    +0x%-8X %-6d B  %s%s" % (va, size, nombre_de(n), marca))

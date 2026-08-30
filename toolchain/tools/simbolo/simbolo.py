#!/usr/bin/env python3
"""simbolo -- que funcion es ese `rip`, leido del kernel que hay AHORA.

Por que esto existe
===================

La pantalla azul da una direccion:

    rip=0x00000000004111B5

Y una direccion no manda a ningun sitio. El 2026-08-30 esa de ahi era
`destroy_address_space +0x385`, y saberlo fue la mitad de la investigacion --
pero se saco con un script de usar y tirar, escrito a mano, con la ruta del ELF
metida dentro. Eso es exactamente lo que esta casa no hace:

    una herramienta con la ruta escrita dentro contesta con confianza
    sobre el binario de la semana pasada.

Asi que esto **busca el ELF**, no lo sabe. Y dice cual encontro, cuando se
construyo y con que hash -- porque un `rip` resuelto contra otro build no da un
error: **da un nombre de funcion equivocado**, que es peor que no dar ninguno.

Uso
===

    python simbolo.py 0x4111B5
    python simbolo.py 4111B5 cr2=0xFFFFBD352B3AC000
    <pega la pantalla azul entera por la entrada estandar>
    python simbolo.py

Con la pantalla pegada saca los `rip=` y los `cr2=` el solo, que es como se usa
de verdad: se copia lo que se ve y no se transcribe un numero de dieciseis
digitos a mano.

Lo que NO hace
==============

No adivina el build. Si el kernel que corria en el Ryzen no es el que hay aqui
compilado, los nombres estan mal y esta herramienta no puede saberlo. Por eso
imprime la fecha y el hash del ELF que uso: **la comparacion es del que mira**.
"""

import os
import re
import struct
import sys
import hashlib

# La raiz del repo desde aqui: toolchain/tools/simbolo -> ../../..
RAIZ = os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))

# Donde han vivido los ELF del kernel. Se BUSCA en todas y gana la mas reciente:
# una lista de sitios donde mirar no es lo mismo que una ruta escrita dentro --
# la lista sobrevive a que cargo cambie de carpeta, la ruta no.
DONDE_MIRAR = (
    "Ultra_kernel_x86-64/target/kernel/x86_64-unknown-none/release",
    "Ultra_kernel_x86-64/target/x86_64-unknown-none/release",
    "Ultra_kernel_x86-64/target/release",
)
NOMBRE = "bmo-kernel"

# La base a la que se carga el kernel. Sale del propio ELF (`e_entry` y las
# secciones ya vienen en direcciones de carga), asi que aqui no se suma nada:
# esta constante esta para DECIRLA, no para usarla.
CARGA_DECLARADA = 0x400000

# De donde salen los dos numeros del physmap. NO se copian aqui: se LEEN del
# fuente, por el mismo motivo que el ELF se busca en vez de saberse.
#
# *** El 2026-08-30 la pantalla azul se resolvio restando `cr2 - HIGH_MEM_BASE`
# a mano y comparando con `PHYSMAP_SIZE` de memoria. Los dos numeros viven en un
# fichero y **uno de ellos ya habia cambiado de sitio una vez**: el dia que
# `s2_mem` espeje 1 TiB en vez de 16 GiB, una copia aqui contestaria que 60 TiB
# esta fuera cuando estaria dentro. Un diagnostico con la constante pegada
# dentro es el mismo fallo que persigue este proyecto, en la herramienta que lo
# persigue.
MM_MOD = "Ultra_kernel_x86-64/kernel/src/ring0/mm/mod.rs"


def buscar_elf():
    """El `bmo-kernel` mas reciente de los sitios conocidos. `None` si no hay."""
    hallados = []
    for rel in DONDE_MIRAR:
        ruta = os.path.join(RAIZ, rel.replace("/", os.sep), NOMBRE)
        if os.path.isfile(ruta):
            hallados.append((os.path.getmtime(ruta), ruta))
    if not hallados:
        return None
    hallados.sort()
    return hallados[-1][1]


def simbolos_de(ruta):
    """`[(valor, tamano, nombre)]` de las tablas de simbolos del ELF, ordenado.

    Se leen SYMTAB y DYNSYM y se mezclan. Un ELF sin tabla de simbolos --con
    `strip`-- devuelve la lista vacia, y eso se dice: es distinto de "no
    encontre la funcion".
    """
    d = open(ruta, "rb").read()
    if d[:4] != b"\x7fELF":
        raise SystemExit("no es un ELF: " + ruta)
    if d[4] != 2:
        raise SystemExit("solo se lee ELF de 64 bits")

    e_shoff = struct.unpack_from("<Q", d, 0x28)[0]
    e_shentsize = struct.unpack_from("<H", d, 0x3A)[0]
    e_shnum = struct.unpack_from("<H", d, 0x3C)[0]

    def seccion(i):
        o = e_shoff + i * e_shentsize
        (_n, typ, _f, _addr, off, size, link, _info, _al, entsize) = struct.unpack_from(
            "<IIQQQQIIQQ", d, o
        )
        return typ, off, size, link, entsize

    fuera = []
    for i in range(e_shnum):
        typ, off, size, link, entsize = seccion(i)
        if typ not in (2, 11) or entsize == 0:  # SYMTAB, DYNSYM
            continue
        _t, stroff, _s, _l, _e = seccion(link)
        for k in range(size // entsize):
            o = off + k * entsize
            st_name, _info, _other, _shndx, st_value, st_size = struct.unpack_from(
                "<IBBHQQ", d, o
            )
            if st_value == 0:
                continue
            b = d[stroff + st_name:]
            fin = b.find(b"\0")
            nom = b[:fin].decode("utf-8", "replace") if fin >= 0 else ""
            if nom:
                fuera.append((st_value, st_size, nom))
    fuera.sort()
    return fuera


def legible(mangled):
    """El nombre v0 de Rust, en algo que se pueda leer. **Crudo aparte.**

    Es un demanglador CORTO y lo dice: parte el nombre en sus componentes
    `<largo><texto>` y se salta el desambiguador de crate (`Cs...._`). No cubre
    genericos ni `impl` anidados.

    ** Y por eso el crudo se imprime SIEMPRE al lado, que es la misma regla que
    el `PHYstatus` de la red: el byte entero es la prueba y la funcion es la
    opinion. Un demanglador a medias que sustituya al original convierte un
    nombre raro en un nombre bonito y EQUIVOCADO.
    """
    s = mangled
    if not s.startswith("_R"):
        return s
    i = 2
    partes = []
    while i < len(s):
        c = s[i]
        if c == "C" and i + 1 < len(s) and s[i + 1] == "s":
            j = s.find("_", i + 2)
            if j < 0:
                break
            i = j + 1
            continue
        if c.isdigit():
            j = i
            while j < len(s) and s[j].isdigit():
                j += 1
            try:
                n = int(s[i:j])
            except ValueError:
                break
            partes.append(s[j:j + n])
            i = j + n
            continue
        i += 1
    return "::".join(p for p in partes if p) or s


def physmap():
    """`(HIGH_MEM_BASE, PHYSMAP_SIZE)` leidos del fuente. `None` si no se pueden."""
    ruta = os.path.join(RAIZ, MM_MOD.replace("/", os.sep))
    if not os.path.isfile(ruta):
        return None
    txt = open(ruta, "r", encoding="utf-8", errors="replace").read()
    def const(nombre):
        m = re.search(
            r"pub const " + nombre + r"\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d[\d_]*)", txt
        )
        if not m:
            return None
        t = m.group(1).replace("_", "")
        return int(t, 16) if t.lower().startswith("0x") else int(t)
    base, tam = const("HIGH_MEM_BASE"), const("PHYSMAP_SIZE")
    if base is None or tam is None:
        return None
    return base, tam


def humano(n):
    """Un tamano en la unidad que se lee de un vistazo."""
    for u, d in (("TiB", 1 << 40), ("GiB", 1 << 30), ("MiB", 1 << 20), ("KiB", 1 << 10)):
        if n >= d:
            return "%.1f %s" % (n / float(d), u)
    return "%d B" % n


def por_el_physmap(addr, mapa):
    """Que es esta direccion si se mira como una del physmap. Lista de lineas.

    *** ES LA RESTA QUE CONTESTO LA PANTALLA AZUL DEL 30-08, y se hizo a mano.
    Un `cr2` alto no dice nada por si mismo; `cr2 - HIGH_MEM_BASE` comparado con
    lo que el espejo alcanza de verdad dice si el kernel intentaba tocar RAM que
    existe o RAM que no.
    """
    if mapa is None:
        return ["   (no se pudieron leer HIGH_MEM_BASE/PHYSMAP_SIZE del fuente)"]
    base, tam = mapa
    if addr < base:
        return []
    fisica = addr - base
    out = ["   por el PHYSMAP:  fisica = 0x%X  (%s)" % (fisica, humano(fisica))]
    if fisica < tam:
        out.append("      DENTRO del espejo (%s). Es RAM que el kernel puede tocar."
                   % humano(tam))
    else:
        out.append("      [!] FUERA del espejo, que llega a %s." % humano(tam))
        out.append("      [!] `phys_to_virt` la calcula igual y no la mapea nadie:")
        out.append("          eso es un #PF de no-presente desde el kernel.")
        out.append("      -> el numero salio de una tabla de paginas o de una")
        out.append("         ranura pisada. Ver L6f `AJENO` en mm/vmm.rs.")
    return out


def contexto(simbolos, objetivo):
    """El que CONTIENE la direccion, y sus vecinos por si no la contiene nadie."""
    dentro = [s for s in simbolos if s[0] <= objetivo < s[0] + max(s[1], 1)]
    antes = [s for s in simbolos if s[0] <= objetivo][-3:]
    despues = [s for s in simbolos if s[0] > objetivo][:2]
    return dentro, antes, despues


def direcciones(argv, entrada):
    """Los numeros a resolver: de los argumentos, o sacados del texto pegado."""
    saca = []
    for a in argv:
        m = re.search(r"(?:0x)?([0-9A-Fa-f]{4,16})", a)
        if m:
            saca.append((a if "=" in a else "rip", int(m.group(1), 16)))
    if saca:
        return saca
    # Nada por argumento: se lee la pantalla pegada. `rip` y `cr2` son las dos
    # que mandan a sitios distintos y por eso se cogen las dos.
    for etiqueta, valor in re.findall(r"\b(rip|cr2|rsp)\s*=\s*0x([0-9A-Fa-f]+)", entrada):
        saca.append((etiqueta, int(valor, 16)))
    return saca


def main():
    elf = buscar_elf()
    if not elf:
        print("[X] no encuentro ningun `%s` compilado." % NOMBRE)
        print("    Mirado en:")
        for rel in DONDE_MIRAR:
            print("      " + rel)
        print("    Construye primero:  .\\bmo.ps1 -Rapido")
        return 1

    entrada = ""
    if not sys.stdin.isatty():
        entrada = sys.stdin.read()
    objetivos = direcciones(sys.argv[1:], entrada)
    if not objetivos:
        print("uso:  python simbolo.py 0x4111B5")
        print("      ...o pega la pantalla azul por la entrada estandar")
        return 2

    sha = hashlib.sha256(open(elf, "rb").read()).hexdigest()[:16]
    import datetime
    cuando = datetime.datetime.fromtimestamp(os.path.getmtime(elf))
    rel = os.path.relpath(elf, RAIZ).replace(os.sep, "/")
    print("kernel   %s" % rel)
    print("         %s   sha256:%s" % (cuando.strftime("%Y-%m-%d %H:%M"), sha))
    print("         [!] si el Ryzen no corria ESTE build, los nombres son de otro")
    print("             binario y esta herramienta no puede saberlo.")

    mapa = physmap()
    if mapa:
        print("         physmap  base 0x%X  espeja %s   (leido de %s)"
              % (mapa[0], humano(mapa[1]), MM_MOD.rsplit("/", 3)[-1]))

    simbolos = simbolos_de(elf)
    if not simbolos:
        print("[X] el ELF no trae tabla de simbolos (esta `strip`ado).")
        return 1
    print("         %d simbolos, carga declarada 0x%X" % (len(simbolos), CARGA_DECLARADA))

    for etiqueta, addr in objetivos:
        print("")
        print("== %s = 0x%X ==" % (etiqueta, addr))
        if addr < simbolos[0][0] or addr > simbolos[-1][0] + max(simbolos[-1][1], 1):
            # Decirlo es lo que impide leer un vecino lejano como si fuera la
            # respuesta. Un `cr2` casi nunca cae en el codigo: cae en datos.
            print("   no es codigo de este binario (0x%X..0x%X)"
                  % (simbolos[0][0], simbolos[-1][0]))
            for linea in por_el_physmap(addr, mapa):
                print(linea)
            if etiqueta == "rsp":
                # La misma frase que imprime `plat/faults.rs`, para que la
                # pantalla y la herramienta no llamen distinto a lo mismo.
                alta = mapa is not None and addr >= mapa[0]
                print("   pila: %s" % ("de HILO DEL KERNEL" if alta else "baja"))
            continue
        dentro, antes, despues = contexto(simbolos, addr)
        for v, sz, n in dentro:
            print("   DENTRO DE  %s" % legible(n))
            print("              0x%X +0x%X  (la funcion mide 0x%X)" % (v, addr - v, sz))
            print("              crudo: %s" % n)
        if not dentro:
            print("   ningun simbolo la contiene por tamano. Vecinos:")
        print("   -- antes --")
        for v, sz, n in antes:
            print("      0x%X  +0x%-6X %s" % (v, addr - v, legible(n)))
        print("   -- despues --")
        for v, sz, n in despues:
            print("      0x%X  -0x%-6X %s" % (v, v - addr, legible(n)))
    return 0


if __name__ == "__main__":
    sys.exit(main())

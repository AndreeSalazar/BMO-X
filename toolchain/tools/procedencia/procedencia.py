#!/usr/bin/env python3
"""PROCEDENCIA -- delata este artefacto donde se construyo?

*** LA PREGUNTA, Y POR QUE NO LA CONTESTA COMPILAR DOS VECES

`PLAN_SEGURIDAD.md` C7 pide compilacion reproducible: que el mismo fuente de el
mismo binario **en dos maquinas distintas**. Lo que se puede hacer con UNA
maquina es compilar dos veces y comparar -- y eso pasa siempre, incluso cuando
el binario lleva dentro la ruta del que lo construyo. La ruta es la misma en
las dos pasadas.

    compilar dos veces AQUI   ->  no ve la ruta incrustada. Sale verde
    compilar en OTRA maquina  ->  bytes distintos, y no hay otra maquina

*** Asi que esto mide el PROXY que si se puede medir con una sola: **que el
artefacto no lleve dentro nada que solo exista en esta maquina.** No demuestra
que sea reproducible; demuestra que no es IRREPRODUCIBLE por el motivo mas
comun, y por el unico que ademas filtra el nombre de un usuario a todo el que
reciba el binario.

=== LO QUE SE MIDIO EL 2026-08-25, ANTES DE QUE ESTO EXISTIERA ===========

    d.bex    off=0x87f32  'C:\\Users\\Salazar\\Documents\\BMO\\...\\recorte.rs'
    d.bex    off=0x88466  'C:\\Users\\Salazar\\Documents\\BMO\\...\\foco.rs'
    gui.bex  las mismas dos

Son `core::panic::Location`: los mete `panic!`, `assert!` o un indice fuera de
rango, y llevan la ruta **tal y como la vio el compilador**. Se cerro con
`trim-paths = "all"` en `Ultra_userspace/Cargo.toml`, que las reescribe a
`/cargo/deps/...` -- **sin borrarlas**, para que una autopsia siga sabiendo de
que fichero habla.

** Es la misma leccion que MAQUETA ya pago con su emisor:

    "un artefacto que depende de quien lo genera NO SE PUEDE COMPARAR"

=== [!] LO QUE ESTO NO PUEDE VER ========================================

  - Un binario que dependa de la FECHA, del orden de un `HashMap` o de una
    version del compilador. Para eso hace falta la otra maquina.
  - Rutas de OTRA maquina. Solo sabe buscar las de esta.

Uso:
    python toolchain/tools/procedencia/procedencia.py --check
"""

import argparse
import os
import re
import subprocess
import sys

# Lo que se mira. Son los artefactos que SALEN de aqui hacia el metal o hacia
# otro; los intermedios de `target/` no cuentan, porque no viaja ninguno.
PATRONES = [
    "Ultra_kernel_x86-64/staging/BMO-DATA/**/*.bex",
    "Ultra_kernel_x86-64/kernel/src/ring0/task/payloads/*.bex",
    # ** El kernel y el arranque van tambien, aunque hoy salgan limpios.
    #
    # Un guardian que solo mire lo que ya se sabe sucio no es un trinquete: es
    # una lista de arreglos hechos. Estos dos estan bien AHORA, y lo que esto
    # compra es enterarse el dia que dejen de estarlo -- que es el dia en que
    # alguien anada un `assert!` en un sitio nuevo.
    "Ultra_kernel_x86-64/target/kernel/x86_64-unknown-none/release/bmo-kernel",
    "Ultra_kernel_x86-64/target/**/x86_64-unknown-uefi/release/*.efi",
]

# Un `.bex` puede llevar dentro un WAD o una fuente, y ahi puede haber
# cualquier byte. Se buscan cadenas ASCII largas, que es la forma que tiene una
# ruta -- no bytes sueltos.
CADENA = re.compile(rb"[\x20-\x7e]{8,}")


def raiz():
    """La raiz del repo, preguntada a git y no supuesta."""
    try:
        r = subprocess.run(
            ["git", "rev-parse", "--show-toplevel"],
            capture_output=True, text=True, check=True,
        )
        return os.path.normpath(r.stdout.strip())
    except Exception:
        return os.path.normpath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))


def huellas(root):
    """Las cadenas que solo existen en ESTA maquina.

    Se generan en las dos formas de barra porque un binario puede llevar la que
    quiera: `rustc` en Windows escribe `\\` en unos sitios y `/` en otros, y
    buscar solo una deja pasar la mitad.
    """
    fuera = []
    casa = os.path.expanduser("~")
    for base in (root, casa):
        if not base:
            continue
        for forma in (base, base.replace("\\", "/"), base.replace("/", "\\")):
            if forma and forma not in fuera:
                fuera.append(forma)
    # El nombre de usuario suelto: aparece en rutas que no empiezan por la raiz.
    usuario = os.path.basename(casa)
    if usuario and len(usuario) >= 3:
        fuera.append(usuario)
    return fuera


def ficheros(root):
    import glob
    out = []
    for p in PATRONES:
        out.extend(sorted(glob.glob(os.path.join(root, p), recursive=True)))
    return out


def revisar(root):
    marcas = huellas(root)
    bajas = [m.lower().encode("utf-8", "ignore") for m in marcas]
    sucios = []
    mirados = 0
    for f in ficheros(root):
        mirados += 1
        try:
            with open(f, "rb") as fh:
                datos = fh.read()
        except OSError:
            continue
        for m in CADENA.finditer(datos):
            s = m.group().lower()
            for i, marca in enumerate(bajas):
                if marca and marca in s:
                    sucios.append((f, m.start(), m.group().decode("ascii", "replace"), marcas[i]))
                    break
    return mirados, sucios


def main():
    ap = argparse.ArgumentParser(add_help=True)
    ap.add_argument("--check", action="store_true", help="falla si algun artefacto delata la maquina")
    args = ap.parse_args()

    root = raiz()
    mirados, sucios = revisar(root)

    if mirados == 0:
        # No es un aprobado: es que no hay nada construido.
        print("  [-] no hay artefactos que mirar (construye antes)")
        return 0

    if not sucios:
        print("clean: los %d artefactos no llevan dentro la ruta de esta maquina" % mirados)
        print("       (esto NO prueba que sean reproducibles -- ver la cabecera)")
        return 0

    print("ARTEFACTOS QUE DELATAN DONDE SE CONSTRUYERON:")
    vistos = set()
    for f, off, cad, marca in sucios:
        rel = os.path.relpath(f, root)
        clave = (rel, cad)
        if clave in vistos:
            continue
        vistos.add(clave)
        print("  %s  +%#x" % (rel, off))
        print("      %s" % cad[:120])
    print()
    print("  %d cadena(s) en %d fichero(s)." % (len(vistos), len(set(v[0] for v in vistos))))
    print("  Un binario con la ruta de su constructor dentro NO se puede comparar")
    print("  con el mismo binario hecho en otra maquina -- y lleva el nombre de un")
    print("  usuario a todo el que lo reciba.")
    print("  Para Rust: `trim-paths = \"all\"` en el perfil del workspace.")
    return 1 if args.check else 0


if __name__ == "__main__":
    sys.exit(main())

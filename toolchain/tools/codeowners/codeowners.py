"""codeowners -- que `.github/CODEOWNERS` diga lo que el kernel ENLAZA de verdad.

== De donde sale, y con fecha ==

El 2026-09-17 el dueno cerro Ring 0 a contribuciones externas (ver
CONTRIBUTING.md), y "Ring 0" quedo escrito como una LISTA en CODEOWNERS: lo que
el kernel enlaza, sacado de `cargo tree`. Ese mismo dia se dejo dicho que la
lista se desfasaria el dia que el kernel enlazara un crate nuevo, porque
mantenerla dependia de acordarse.

Tardo UN dia. El 2026-09-18 `bmo-cola` entro en Ring 0 (A0 de
PLAN_EL_BUS_APARTE) y CODEOWNERS no se entero hasta que alguien lo vio pasar en
la salida del build. Una lista de seguridad que depende de la memoria de quien
la lleva es un texto, y un texto se incumple sin ruido.

== Que comprueba, en las dos direcciones ==

  1. Todo crate que el kernel ENLAZA tiene dueno en CODEOWNERS.
  2. Ninguno es de TERCEROS -- hoy son cero, y la regla del 17-09 es lo que
     mantiene ese cero (el modelo es xz). Si algun dia el dueno decide
     enlazar uno, va a `PERMITIDOS` con fecha y motivo, no por la puerta de
     atras.
  3. Toda linea de CODEOWNERS apunta a algo que EXISTE. Una linea que ya no
     cubre nada (una carpeta renombrada) es un cerrojo sobre una puerta que ya
     no esta ahi, y la puerta de verdad se quedo abierta.

== Antes de juzgar, demuestra que ve ==

Si `cargo tree` no devuelve el propio kernel, el guardian no ha mirado nada y lo
dice: MUERTO, no "limpio".
"""

import argparse
import os
import re
import shutil
import subprocess
import sys

# Crates de TERCEROS que el dueno decidio enlazar en Ring 0, cada uno con su
# fecha y su motivo. Vacia a proposito: el 2026-09-17 el kernel enlazaba 37
# crates y los 37 eran del repositorio.
PERMITIDOS = {}

RE_LINEA_TREE = re.compile(r"^(\S+) v\S+ \((.+)\)\s*(\(\*\))?\s*$")


def raiz():
    return os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def cargo():
    c = shutil.which("cargo")
    if c:
        return c
    casa = os.path.join(os.path.expanduser("~"), ".cargo", "bin", "cargo.exe")
    return casa if os.path.exists(casa) else None


def enlazados():
    """`[(nombre, ruta_relativa_o_None)]` de todo lo que enlaza bmo-kernel.

    `None` en la ruta = no es del repositorio: viene de crates.io o de git.
    """
    c = cargo()
    if not c:
        return None
    kernel = os.path.join(raiz(), "Ultra_kernel_x86-64", "kernel")
    r = subprocess.run(
        [c, "+nightly", "tree", "--target", "x86_64-unknown-none", "--prefix", "none",
         "--edges", "normal"],
        cwd=kernel, capture_output=True, text=True, encoding="utf-8", errors="replace",
    )
    if r.returncode != 0:
        return None
    vistos = {}
    for linea in r.stdout.splitlines():
        linea = linea.strip()
        if not linea:
            continue
        m = RE_LINEA_TREE.match(linea)
        if m:
            nombre, donde = m.group(1), m.group(2)
            ruta = os.path.normpath(donde)
            if ruta.lower().startswith(raiz().lower()):
                rel = os.path.relpath(ruta, raiz()).replace(os.sep, "/")
                vistos[nombre] = rel
            else:
                vistos[nombre] = None
        else:
            # Sin parentesis: un crate de crates.io se imprime "nombre vX.Y.Z".
            partes = linea.split()
            if len(partes) >= 2 and partes[1].startswith("v"):
                vistos.setdefault(partes[0], None)
    return sorted(vistos.items())


def patrones():
    """Los patrones de CODEOWNERS, en orden, sin comentarios."""
    p = os.path.join(raiz(), ".github", "CODEOWNERS")
    salida = []
    with open(p, encoding="utf-8") as f:
        for n, linea in enumerate(f, 1):
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            salida.append((n, linea.split()[0]))
    return salida


def cubre(patron, rel):
    """Un patron anclado (`/a/b/` o `/a/b`) cubre la carpeta `rel`?"""
    p = patron.lstrip("/")
    carpeta = rel.rstrip("/") + "/"
    if p.endswith("/"):
        return carpeta.startswith(p)
    return carpeta == p + "/" or carpeta.startswith(p + "/")


def juzgar(crates, pats, existe):
    """Las quejas. Aparte de `comprobar` para poder probar el juicio sin cargo."""
    quejas = []
    for nombre, rel in crates:
        if rel is None:
            if nombre not in PERMITIDOS:
                quejas.append("%s lo enlaza el kernel y es de TERCEROS: Ring 0 no enlaza nada "
                              "que no sea del repositorio salvo decision del dueno, con fecha "
                              "y motivo en PERMITIDOS" % nombre)
            continue
        if not any(cubre(p, rel) for _, p in pats):
            quejas.append("%s (%s) lo enlaza el kernel y NO tiene dueno en CODEOWNERS"
                          % (nombre, rel))
    for n, p in pats:
        if not existe(p.lstrip("/").rstrip("/")):
            quejas.append("CODEOWNERS:%d  `%s` no apunta a nada que exista: un cerrojo sobre "
                          "una puerta que ya no esta" % (n, p))
    return quejas


def autoprueba():
    """El juicio tiene que decir que NO a lo que debe, y que SI a lo que debe."""
    pats = [(1, "/Ultra_kernel_x86-64/"), (2, "/platform/drivers/"), (3, "/bmo.ps1")]
    todo = lambda p: True
    casos = [
        ("un crate cubierto pasa", juzgar([("k", "Ultra_kernel_x86-64/kernel")], pats, todo), False),
        ("un crate sin dueno se dice", juzgar([("c", "platform/shared/bmo-cola")], pats, todo), True),
        ("uno de terceros se dice", juzgar([("x", None)], pats, todo), True),
        ("un patron que no existe se dice", juzgar([], pats, lambda p: p != "bmo.ps1"), True),
        ("un prefijo no es la carpeta", juzgar([("n", "platform/drivers-x/a")], pats, todo), True),
    ]
    return [nombre for nombre, quejas, espera in casos if bool(quejas) != espera]


def comprobar():
    fallos = autoprueba()
    if fallos:
        print("guardian MUERTO: el juicio falla en: " + "; ".join(fallos))
        return 1
    crates = enlazados()
    if crates is None:
        print("guardian MUERTO: `cargo +nightly tree` no respondio sobre el kernel")
        return 1
    if not any(n == "bmo-kernel" for n, _ in crates):
        print("guardian MUERTO: `cargo tree` no devolvio ni el propio kernel")
        return 1
    pats = patrones()
    existe = lambda rel: os.path.exists(os.path.join(raiz(), rel.replace("/", os.sep)))
    quejas = juzgar(crates, pats, existe)
    if quejas:
        for q in quejas:
            print("  [X] " + q)
        print("codeowners: %d incumplimiento(s)" % len(quejas))
        return 1
    terceros = sum(1 for _, rel in crates if rel is None)
    print("clean: el kernel enlaza %d crate(s), %d de terceros, y todos tienen dueno; "
          "%d linea(s) de CODEOWNERS y todas apuntan a algo que existe"
          % (len(crates), terceros, len(pats)))
    return 0


def main():
    ap = argparse.ArgumentParser(description="CODEOWNERS contra lo que el kernel enlaza.")
    ap.add_argument("--check", action="store_true", help="lo que llama el build")
    ap.parse_args()
    return comprobar()


if __name__ == "__main__":
    sys.exit(main())

#!/usr/bin/env python3
"""casillas -- una casilla que no dice DONDE MIRAR no la comprueba nadie.

Por que existe
==============

El 2026-08-24 el dueno pregunto: *"no verificas que faltan eso las escaleras
para completar?"*. Se recontaron las escaleras de los planes contra el codigo y
**ocho casillas de cincuenta y cuatro estaban mal**:

    PLAN_MAQUETA        cinco escalones [ ] con su crate hecho y su banco verde
                        (lex 24, node 36, cascade 22, layout 20, verdict 20)
    PLAN_ALMACENAMIENTO "FAT32, 2.453 lineas en un fichero" -> hoy son CINCO
                        ficheros y el mayor tiene 971
    PYTHON_MAESTRO      "una fila de intrinsics.toml (RDRAND)" -> ya estaba,
                        con sus opcodes; y listar directorio desde Ring 3
                        tambien

*** Y NO ES DESCUIDO, ES LA FORMA: se marca la casilla el dia que se acaba el
trabajo, y ese dia lo que hay en la cabeza es el codigo, no el documento. El
arbol ya tiene quien cuente ficheros (`censo_modular`), quien cuente crates
(el banco de `bmo.ps1`) y quien compruebe que las citas resuelven (`enlaces`).
**Las casillas no las contaba nadie.**

*** EL PRIMER INTENTO DE ESTE GUARDIAN NO CAZO NINGUNA, Y ESO ENSENO LA REGLA

Se escribio buscando casillas `[ ]` que nombraran un crate existente entre
comillas invertidas. Se probo contra el documento de ANTES del arreglo y
contesto **limpio**. Las cinco lineas eran:

    [ ] 1   abuelo   lexer de dos modos (marcado / estilo)

**No citan nada.** Ni el crate, ni el fichero, ni una fecha. La heuristica no
podia fallar de otra forma.

Y ahi estaba el defecto de verdad, que no era el que se fue a buscar:

> Una casilla que no dice DONDE MIRAR no la puede comprobar nadie -- ni una
> maquina ni una persona. No esta desactualizada: **es incomprobable**.

Asi que la regla se dio la vuelta. En vez de adivinar si una casilla miente
--que no se puede-- se exige que sea COMPROBABLE: que cite un crate, un
fichero o una fecha. Una escalera cuyos escalones citan su prueba se recuenta
en un rato; una que no, se queda como estaba durante meses.

** Y la comprobacion de "nombra codigo que existe y esta probado" se queda
igual, como segunda senal. Caza otra forma --la casilla que cita bien y se
quedo atras-- y las dos juntas cubren las dos maneras de mentir.

[!] Las dos AVISAN y ninguna mata. Un guardian que mata con una heuristica
obliga a poner excusas en los documentos, y un documento con excusas para el
guardian es peor que uno desactualizado. La regla la escribio `censo_modular`:
**trinquete, no muro.**
"""

import json
import re
import subprocess
import sys
from pathlib import Path

RAIZ = Path(__file__).resolve().parents[3]

#: Donde viven las escaleras. No se barre el arbol entero a proposito: una
#: casilla en un README de ejemplo no es una promesa del proyecto.
CARPETAS = ["docs/plan", "docs/maestro"]

#: `[ ]` sin hacer, `[~]` a medias, `[x]`/`[X]` hecho.
CASILLA = re.compile(r"\[([ xX~])\]")

#: Lo que va entre comillas invertidas dentro de la linea de una casilla.
CITADO = re.compile(r"`([^`\n]+)`")

#: Una fecha tambien vale como prueba: dice CUANDO mirar.
FECHA = re.compile(r"20\d\d-\d\d-\d\d")


def crates():
    """Los crates del workspace, por nombre."""
    try:
        salida = subprocess.run(
            ["cargo", "metadata", "--no-deps", "--format-version", "1"],
            cwd=RAIZ, capture_output=True, text=True, check=True,
        ).stdout
    except Exception:
        return set()
    return {p["name"] for p in json.loads(salida)["packages"]}


def con_pruebas(nombre):
    """Ese crate tiene banco? Se pregunta a `cargo`, no se adivina.

    ** Un crate que existe y NO tiene pruebas no dispara el aviso: existir no es
    estar hecho, y el arbol tiene trece crates asi, contados y dichos por el
    banco de `bmo.ps1`. Lo sospechoso es existir **y estar probado**.
    """
    try:
        r = subprocess.run(
            ["cargo", "test", "-q", "-p", nombre],
            cwd=RAIZ, capture_output=True, text=True,
        )
    except Exception:
        return False
    total = sum(int(n) for n in re.findall(r"(\d+) passed", r.stdout))
    return total > 0


def revisar():
    del_workspace = crates()
    sospechosas = []
    cuenta = {" ": 0, "~": 0, "x": 0}

    mudas = []

    for carpeta in CARPETAS:
        d = RAIZ / carpeta
        if not d.is_dir():
            continue
        for doc in sorted(d.glob("*.md")):
            for n, linea in enumerate(doc.read_text(encoding="utf-8", errors="replace").splitlines(), 1):
                # ** UNA LINEA CON VARIAS CASILLAS ES LA LEYENDA, no un
                # escalon: `PLAN_DOOM` explica arriba que significa cada
                # simbolo, y contarla como pendiente seria denunciar el
                # diccionario por no citar a nadie.
                if len(CASILLA.findall(linea)) > 1:
                    continue
                m = CASILLA.search(linea)
                if not m:
                    continue
                estado = m.group(1).lower()
                cuenta[estado if estado in cuenta else "x"] += 1
                if estado != " ":
                    continue

                # ** LA SENAL PRINCIPAL: la casilla no dice donde mirar.
                #
                # Ni un crate, ni un fichero, ni una fecha. Nadie puede
                # comprobarla -- y por eso se quedan como estan durante meses.
                # Ver la cabecera: es el fallo que el primer intento no vio.
                citas = [c.strip() for c in CITADO.findall(linea)]
                if not citas and not FECHA.search(linea):
                    mudas.append((doc, n, linea.strip()))

                for cita in citas:
                    if cita in del_workspace and con_pruebas(cita):
                        sospechosas.append((doc, n, cita, linea.strip()))
                    # Y un fichero citado que existe: la otra mitad del patron.
                    elif "/" in cita and (RAIZ / cita).exists():
                        sospechosas.append((doc, n, cita, linea.strip()))
    return cuenta, sospechosas, mudas


def main():
    cuenta, sospechosas, mudas = revisar()
    total = sum(cuenta.values())
    print(f"    casillas: {total} en los planes  "
          f"({cuenta['x']} hechas, {cuenta['~']} a medias, {cuenta[' ']} sin hacer)")

    if mudas:
        print(f"    [-] {len(mudas)} casilla(s) sin hacer NO DICEN DONDE MIRAR:")
        for doc, n, linea in mudas[:12]:
            print(f"        {doc.relative_to(RAIZ).as_posix()}:{n}  {linea[:80]}")
        if len(mudas) > 12:
            print(f"        ... y {len(mudas) - 12} mas")
        print("        Una casilla sin crate, fichero ni fecha no la comprueba nadie.")

    if not sospechosas:
        if not mudas:
            print("    clean: todas las casillas sin hacer dicen donde mirar.")
        return 0

    # ** AVISA Y NO MATA. Ver la cabecera: matar con una heuristica llena los
    # documentos de excusas para el guardian.
    print(f"    [-] {len(sospechosas)} casilla(s) SIN HACER nombran algo que ya existe:")
    for doc, n, cita, linea in sospechosas:
        rel = doc.relative_to(RAIZ).as_posix()
        print(f"        {rel}:{n}  `{cita}`")
        print(f"            {linea[:96]}")
    print("        Comprobar si el escalon esta hecho y la casilla se quedo atras.")
    return 0


if __name__ == "__main__":
    sys.exit(main())

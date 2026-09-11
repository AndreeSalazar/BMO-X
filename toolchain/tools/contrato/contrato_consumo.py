"""R21 -- EL CONSUMO EN REPOSO (L6h): que hace cada fichero cuando la maquina
no hace nada.

Vive en su propio fichero **y no dentro de `contrato.py`**, y el motivo es el
de `contrato_drivers.py`, repetido palabra por palabra el 2026-09-11:

  *** AL ANADIR R21 AHI DENTRO, `contrato.py` CRUZO LAS 1.000 LINEAS y L6a lo
  cazo en el mismo build.

[!] Y esta vez con un agravante que hay que dejar escrito: la cabecera de
`contrato_drivers.py` cuenta exactamente este fallo, y estaba al lado. Se
repitio igual. Por eso la regla nueva nace aqui, y por eso esta nota: la
proxima regla del contrato empieza en un fichero propio, no en el monolito.

La regla y el porque estan en `r21_el_consumo`; la ley, en L6h de
`FUERO/META-KERNEL_HARD.md`.
"""

import os
import re

from contrato_ley import CARRILES_FUERA_DEL_KERNEL, CONSUMO, raiz

# `//!` o `//`: los dos ficheros de la fuente llevan `//` porque `texto.rs` los
# mete con `include!` dentro de un `static`. El letrero es el mismo deber.
RE_CONSUMO = re.compile(r"^//!? \[consumo\]\s+(\S+)", re.M)


def r21_el_consumo(ficheros, fuera=None):
    """L6h -- **todo fichero de Ring 0 dice que gasta en reposo**. `{ruta: texto}`.

    Peticion del dueno, 2026-09-11: *"dividir en archivos que consumen y no,
    por motivos"*. Tres exigencias en Ring 0, y una fuera:

      1. declara `[consumo]`. Sin trinquete, como R10: se empieza en 180 de 180.
      2. la clase es una de `CONSUMO`. Inventarse otra es volver a no saber.
      3. UNA sola clase. Dos es un fichero que late y se pide a la vez: esta mal
         cortado, y el corte va justo por donde cambia la clase (L6h).
      4. fuera del kernel no se exige; quien lo declare, usa el vocabulario.

    [!] Lo que NO comprueba: que la clase sea la CORRECTA. Deducirla de los
    `loop` seria adivinar -- `plat/spin.rs` gira y es NADA -- y un guardian que
    adivina da permiso con autoridad.
    """
    quejas = []
    for ruta in sorted(ficheros):
        clases = RE_CONSUMO.findall(ficheros[ruta])
        if not clases:
            quejas.append("%s no declara [consumo]. Las clases son: %s (L6h)"
                          % (ruta, ", ".join(CONSUMO)))
            continue
        quejas += _clases_validas(ruta, clases)
    for ruta in sorted(fuera or {}):
        clases = RE_CONSUMO.findall(fuera[ruta])
        if clases:
            quejas += _clases_validas(ruta, clases)
    return quejas


def _clases_validas(ruta, clases):
    malas = [c for c in clases if c not in CONSUMO]
    if malas:
        return ["%s dice [consumo] %s, que no es una clase. Son: %s (L6h)"
                % (ruta, malas[0], ", ".join(CONSUMO))]
    if len(set(clases)) > 1:
        return ["%s declara DOS clases de consumo (%s): late y se pide a la vez, "
                "y eso se parte (L6h)" % (ruta, " y ".join(sorted(set(clases))))]
    return []


def lo_que_gasta_en_reposo(ficheros):
    """**La nota de cada build**: los ficheros de Ring 0 que NO son NADA.

    Es la mitad de "siempre alertar" que se puede decir sin arrancar: la lista
    de lo que gasta con la maquina quieta, saliendo sola. Si crece, se ve.
    """
    if not ficheros:
        return []
    por = {}
    for ruta in sorted(ficheros):
        m = RE_CONSUMO.search(ficheros[ruta])
        if m and m.group(1) in CONSUMO and m.group(1) != "NADA":
            por.setdefault(m.group(1), []).append(ruta.rsplit("/ring0/", 1)[-1])
    total = sum(len(v) for v in por.values())
    lineas = ["R21 L6h: en reposo gastan %d de %d ficheros de Ring 0"
              % (total, len(ficheros))]
    for c in ("LATE", "APARATO", "APAGA"):
        if c in por:
            lineas.append("        %-8s %s" % (c, ", ".join(por[c])))
    return lineas


def comprobar_consumo(r0):
    """Lo que `comprobar()` necesita de R21, en UNA llamada: `(quejas, notas)`.

    Asi `contrato.py` gana tres lineas por esta regla y no ocho -- que era
    justo lo que le sobraba para volver a cruzar las 1.000.
    """
    quejas = [("R21 L6h el consumo en reposo", q)
              for q in r21_el_consumo(r0, ficheros_de_consumo_fuera())]
    reposo = lo_que_gasta_en_reposo(r0)
    return quejas, (["\n".join(reposo)] if reposo else [])


def ficheros_de_consumo_fuera():
    """`{ruta: texto}` de los arboles de Ring 3 vigilados. Solo se juzga a quien
    declara: exigirlo a todo el compositor seria un muro, no un letrero."""
    fuera = {}
    for arbol in CARRILES_FUERA_DEL_KERNEL:
        d = os.path.join(raiz(), arbol.replace("/", os.sep))
        if not os.path.isdir(d):
            continue  # R18 ya grita si un arbol vigilado desaparece
        for dp, dn, fn in os.walk(d):
            dn[:] = [x for x in dn if x != "target"]
            for n in sorted(fn):
                if not n.endswith(".rs"):
                    continue
                ruta = os.path.join(dp, n)
                with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                    fuera[os.path.relpath(ruta, raiz()).replace(os.sep, "/")] = f.read()
    return fuera

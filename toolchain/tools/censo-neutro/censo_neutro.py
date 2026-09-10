"""censo-neutro -- el guardian de que el CENSO y el CODIGO digan lo mismo.

Por que existe
==============

`NEUTRO/CENSO.txt` lista los aparatos que alcanzan la RAM por su cuenta, y la
regla N1 dice que uno sin fila **es una parte del sistema que nadie sabe que
existe**. Pero el censo se escribe A MANO, y el codigo que etiqueta sus marcos
esta en otro sitio.

Dos listas de lo mismo que pueden separarse sin que nadie avise. Es el
`[riesgo] ESPEJO` de esta casa, y ya se ha pagado antes: las constantes del ABI
viven tres veces y hay un guardian por eso mismo.

    el censo dice     quien alcanza la RAM
    el codigo dice    quien etiqueta sus marcos como `Titular::Neutro`

Si esas dos frases se separan, la que gana es la peor: **un aparato escribiendo
en marcos sin etiquetar**, que es exactamente lo que costo la carpeta entera.

Lo que comprueba, y lo que NO
==============================

    1. todo fichero que etiqueta `Titular::Neutro` tiene fila en el censo
    2. toda fila que nombra un fichero, ese fichero existe y etiqueta
    3. la cuenta `xN` de la fila es el numero de sitios que etiquetan
    4. y dice CUANTAS filas se saltaron, para que la cobertura parcial no se
       disfrace de salud

** LO QUE NO PUEDE COMPROBAR, y hay que decirlo porque es la mitad de R5:

    el censo contra LA MAQUINA de verdad.

Esto corre en el anfitrion, sin bus PCI delante. Saber si hay un aparato con DMA
que no esta en el censo **solo se puede contestar arrancando**, y ese numero ya
existe: es el `sincodigo` del portero del bus (`dev/portero.rs`).

    R5a  codigo <-> censo    ESTE guardian. Corre en cada build
    R5b  censo  <-> maquina  el portero, y hay que arrancar para verlo

Confundirlas seria decir que el build garantiza algo que no puede ver.
"""

import argparse
import os
import re
import sys

RAIZ = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", "..", ".."))
CENSO = os.path.join(RAIZ, "NEUTRO", "CENSO.txt")
# Las rutas del censo son relativas a AQUI. Esta escrito tambien en CENSO.txt:
# un formato que solo conoce el guardian es un formato que nadie puede cumplir.
BASE = os.path.join(RAIZ, "Ultra_kernel_x86-64", "kernel", "src", "ring0")

# La clase que etiqueta. Su definicion vive en `mm/titular.rs`, que por eso se
# excluye: ahi la palabra aparece porque se DECLARA, no porque se use.
MARCA = "Titular::Neutro"
DEFINICION = os.path.join(BASE, "mm", "titular.rs")

# Una fila del censo que nombra un fichero: `dev/disk/mod.rs  x2`.
FILA = re.compile(r"^\s{2,}(\S+)\s+.*?([\w/]+\.rs)\s*(?:x(\d+))?\s", re.M)


def marcas_en_codigo():
    """{ruta relativa a BASE: cuantas veces etiqueta}."""
    encontrado = {}
    for dirpath, _, files in os.walk(BASE):
        for f in files:
            if not f.endswith(".rs"):
                continue
            ruta = os.path.join(dirpath, f)
            # La definicion no cuenta: ahi la palabra aparece porque se DECLARA.
            if os.path.normcase(ruta) == os.path.normcase(DEFINICION):
                continue
            with open(ruta, "r", encoding="utf-8", errors="replace") as fh:
                texto = fh.read()
            # ** Solo las lineas que NO son comentario. Media docena de ficheros
            # NOMBRAN la clase para explicarse, y contar esas seria exigir una
            # fila de censo a quien solo la menciono.
            n = 0
            for linea in texto.splitlines():
                pelada = linea.strip()
                if pelada.startswith("//"):
                    continue
                # == *** Y COMPARAR TAMPOCO ES ETIQUETAR (2026-09-09) =====
                #
                # ** El razonamiento de arriba se extiende solo. Un comentario
                # que nombra la clase no la pone; **una linea que la COMPARA,
                # tampoco**:
                #
                #     titular_de(p) == Titular::Neutro    PREGUNTA
                #     alloc_frames_contig_de(n, Titular::Neutro)   ETIQUETA
                #
                # *** Lo trajo el paso N2: al cablear `bmo-dma-juicio` en el
                # disco, `dev/disk/transfer.rs` tuvo que preguntar si un marco
                # es de un aparato -- y este guardian le pidio una fila de
                # censo por leer la etiqueta que el propio censo vigila.
                #
                # [!] Y la diferencia importa: exigirle censo a los LECTORES
                # castiga justo a quien usa el dato. Un guardian que hace mas
                # caro consultar su propia tabla se convierte en el motivo de
                # que nadie la consulte.
                if "==" in pelada or "!=" in pelada:
                    continue
                if MARCA in pelada:
                    n += 1
            if n:
                rel = os.path.relpath(ruta, BASE).replace("\\", "/")
                encontrado[rel] = n
    return encontrado


def filas_del_censo():
    """[(aparato, ruta, veces)] de las filas que nombran un fichero, y cuantas
    se saltaron por no nombrarlo."""
    if not os.path.exists(CENSO):
        return None, 0
    with open(CENSO, "r", encoding="utf-8", errors="replace") as fh:
        texto = fh.read()
    filas = []
    saltadas = 0
    dentro = False
    for linea in texto.splitlines():
        # La tabla empieza tras la linea de guiones y acaba en la primera
        # linea vacia despues de haber empezado.
        if re.match(r"^\s{2,}-{3,}\s+-{3,}", linea):
            dentro = True
            continue
        if not dentro:
            continue
        if not linea.strip():
            if filas or saltadas:
                break
            continue
        m = FILA.match(linea + " ")
        if m:
            aparato, ruta, veces = m.group(1), m.group(2), m.group(3)
            filas.append((aparato, ruta, int(veces) if veces else 1))
        else:
            saltadas += 1
    return filas, saltadas


# == *** LA CITA DEL PORTERO, Y POR QUE LLEVA JUEZ ==========================
#
# `dev/portero.rs` compara los maestros del bus PCI contra cuantos aparatos
# declara este censo (paso N3 / R5b). Para eso necesita el numero, y un numero
# copiado a mano es lo que R19 del contrato acaba de nombrar: el `OP_PID` de
# `medida/coste` que llevaba semanas siendo `CONSOLE_READ`.
#
# > Un numero copiado CON guardian es una cita. Sin guardian es una
# > suposicion con cara de dato.
PORTERO = os.path.join(BASE, "dev", "portero.rs")
RE_CENSADOS = re.compile(r"APARATOS_CENSADOS:\s*u32\s*=\s*(\d+)")


def la_cita_del_portero(filas):
    """Queja si el numero de `portero.rs` no es el de filas con fichero."""
    if not os.path.exists(PORTERO):
        return ["falta dev/portero.rs: la cita del censo no se puede comprobar"]
    with open(PORTERO, "r", encoding="utf-8", errors="replace") as fh:
        m = RE_CENSADOS.search(fh.read())
    if not m:
        return ["dev/portero.rs ya no declara APARATOS_CENSADOS: la cita"
                " desaparecio y con ella el aviso del arranque"]
    dice = int(m.group(1))
    if dice != len(filas):
        return ["dev/portero.rs dice %d aparatos censados y el censo tiene %d"
                " fila(s) con fichero" % (dice, len(filas))]
    return []


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--check", action="store_true")
    args = ap.parse_args()

    filas, saltadas = filas_del_censo()
    # ** UN CENSO QUE FALTA NO ES UN CENSO LIMPIO. La leccion del `Guardian`
    # de build.ps1: un path mal escrito dejo un guardian muerto y el build dijo
    # COMPLETE igual.
    if filas is None:
        print("el censo NO EXISTE: falta NEUTRO/CENSO.txt")
        return 1 if args.check else 0
    if not filas:
        print("el censo no tiene ni una fila con fichero: o esta vacio o su")
        print("formato cambio y este guardian dejo de entenderlo. Las dos cosas")
        print("son un fallo, y ninguna es 'todo en orden'.")
        return 1 if args.check else 0

    codigo = marcas_en_codigo()
    del_censo = {ruta: (aparato, veces) for aparato, ruta, veces in filas}

    quejas = []

    # 1. codigo -> censo
    for ruta, n in sorted(codigo.items()):
        if ruta not in del_censo:
            quejas.append(
                "%s etiqueta %d marco(s) como NEUTRO y NO tiene fila en el censo (N1)"
                % (ruta, n))

    # 2 y 3. censo -> codigo, y la cuenta
    for ruta, (aparato, veces) in sorted(del_censo.items()):
        abs_ruta = os.path.join(BASE, ruta.replace("/", os.sep))
        if not os.path.exists(abs_ruta):
            quejas.append("%s: el censo nombra %s y ese fichero no existe"
                          % (aparato, ruta))
            continue
        n = codigo.get(ruta, 0)
        if n == 0:
            quejas.append(
                "%s: el censo dice que %s etiqueta, y ese fichero NO etiqueta (N2)"
                % (aparato, ruta))
        elif n != veces:
            quejas.append(
                "%s: el censo dice x%d en %s y el codigo etiqueta %d sitio(s)"
                % (aparato, veces, ruta, n))

    # ** Y la cita del portero: el numero que `dev/portero.rs` compara en el
    # arranque contra los maestros del bus PCI (N3 / R5b).
    quejas += la_cita_del_portero(filas)

    if quejas:
        print("el censo del neutro y el codigo NO dicen lo mismo:")
        for q in quejas:
            print("  " + q)
        print("")
        print("  el censo esta en NEUTRO/CENSO.txt y su ley en NEUTRO/LEY.md.")
        print("  N1: un aparato que alcanza la RAM y no esta escrito es una")
        print("  parte del sistema que nadie sabe que existe.")
        return 1 if args.check else 0

    print("clean: el censo del neutro cuadra con el codigo -- %d aparato(s) con"
          " fichero, %d sitio(s) que etiquetan, %d fila(s) sin codigo que mirar,"
          " y el portero cita el mismo numero"
          % (len(del_censo), sum(codigo.values()), saltadas))
    return 0


if __name__ == "__main__":
    sys.exit(main())

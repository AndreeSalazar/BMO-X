#!/usr/bin/env python3
"""contrato -- the compatibility contract, isolated so it can be applied.

Why this exists
===============

`docs/identidad/LA_COMPATIBILIDAD.md` writes down what BMO-X promises not to
break, and it writes down the toll: six things every new piece of surface has
to pay. Prose cannot collect a toll. This tool is the same six rules with the
prose removed, so that adding to the surface is checked instead of remembered.

The rules were not invented here. Four of them already ran -- inline, inside
`Ultra_kernel_x86-64/build.ps1`. That file is 1.606 lines and its own entry in
the modular census says, in the fourth repetition of the same prediction:

    "The next guardian is NOT added: split this file first."

So the checks move out. That is the first half of what this tool is for, and
the reason it is a folder and not thirty more lines of PowerShell.

The second half: two of the rules had NOBODY checking them
==========================================================

Table 6 of that document lists who watches each promise, and two rows say
NOBODY. They are the two this tool adds:

  * **A kind that does not fit its field.** `HANDLE_KIND_MASK` is seven bits.
    `KIND_TAREA` was `0x80` and therefore every handle of that type failed to
    resolve -- not sometimes: always, since the day the family was written.
    A number that does not fit its field COMPILES.

  * **The two kind tables contradicting each other.** The kernel's `KIND_*`
    and `bmo-abi`'s `HandleKind` are two lists of the same taxonomy, in two
    crates that do not talk to each other, and **they have already diverged**.

A ratchet, not a wall
=====================

The divergence exists today and cannot be fixed by a build failing: renumbering
a live capability is a contract change, and this tool exists to make those
deliberate. So it follows L6a: it does not judge what is already wrong -- it
judges the DELTA against `LINEA_BASE.txt`.

    what is on the baseline    tolerated, with its reason written next to it
    a NEW divergence           fails the build
    one that gets FIXED        says so, and asks to be removed from the baseline

The tree can only improve. That is the answer to the thing the owner named:
*if it accumulates while it grows, that is what matters now.*

Proving a guardian
==================

L4 of META-KERNEL_HARD: a rule is proven by saying NO. A guardian that has
never rejected anything is a guardian nobody has tested, so `--autoprueba`
runs every rule against synthetic tables built to break exactly one of them,
and fails if any rule stays quiet. It is the toll this tool pays itself.

Usage
=====

    python contrato.py --check        # the build calls this
    python contrato.py --sellar       # rewrite the baseline from the tree
    python contrato.py --autoprueba   # prove each rule can say NO
"""

import argparse
import os
import re
import sys

# -- Donde vive cada lado del contrato --------------------------------------
#
# Rutas relativas a la raiz del repo. Estan aqui arriba y no repartidas por el
# fichero porque son la unica parte que cambia cuando algo se muda, y una ruta
# mal escrita deja un guardian MUERTO que dice COMPLETE igual.
CAP_KERNEL = "Ultra_kernel_x86-64/kernel/src/ring0/obj/cap.rs"
KIND_ABI = "platform/abi/bmo-abi/src/fundamentals/handle/kind.rs"
OPS_KERNEL = "Ultra_kernel_x86-64/kernel/src/ring0/syscall/ops.rs"
OBJ_KERNEL = "Ultra_kernel_x86-64/kernel/src/ring0/obj"
SURFACE_ABI = "platform/abi/bmo-abi/src/syscalls/surface"
USERLAND = "Ultra_userspace/userland/src/lib.rs"

# -- L6e: MODULAR PRECISA. El vocabulario CERRADO de lo que cuesta un fallo.
#
# Ordenado de barato a peor. Es cerrado a proposito: una clase inventada al
# vuelo hace que dos ficheros que cuestan lo mismo lo digan de dos formas, y
# entonces la etiqueta deja de poder compararse -- que era todo el punto.
#
# La ley y de donde sale cada clase: `META-KERNEL_HARD.md`, L6e.
COSTES = ("NADA", "TAREA", "APARATO", "DATO", "MAQUINA", "PUERTA")

# -- L6f: MODULAR PRECISA NIVEL 2. Por que ESA pieza es la que va a fallar.
#
# *** L6e contesta "que cuesta si se equivoca". Esto contesta la otra mitad:
# **por que es probable que se equivoque**. Son preguntas distintas y la
# segunda es la que ahorra el tiempo -- un fallo no dice en que fichero mirar,
# y con esto la lista de sospechosos deja de ser el arbol entero.
#
# Peticion del dueno, con sus palabras: *"no se trata de cortar codigo sino ES
# capturar cual de ellas SON potenciales que pueden sufrir bug y eso elimina la
# posibilidad de la aguja en el pajar."*
#
# CERRADO por el mismo motivo que `COSTES`, y cada clase es un fallo que este
# proyecto YA PAGO. Ver `META-KERNEL_HARD.md`, L6f.
RIESGOS = ("AJENO", "ESPEJO", "SILENCIO", "RELOJ", "UNICO")

BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), "LINEA_BASE.txt")
CUESTAS = os.path.join(os.path.dirname(os.path.abspath(__file__)), "CUESTAS.txt")
RIESGOS_TXT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "RIESGOS.txt")


def raiz():
    return os.path.abspath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", ".."))


def leer(rel):
    ruta = os.path.join(raiz(), rel.replace("/", os.sep))
    if not os.path.exists(ruta):
        raise SystemExit("guardian MUERTO: falta " + rel)
    with open(ruta, "r", encoding="utf-8", errors="replace") as f:
        return f.read()


def leer_dir(rel, sufijo=".rs"):
    d = os.path.join(raiz(), rel.replace("/", os.sep))
    if not os.path.isdir(d):
        raise SystemExit("guardian MUERTO: falta el directorio " + rel)
    trozos = []
    for n in sorted(os.listdir(d)):
        if n.endswith(sufijo):
            with open(os.path.join(d, n), "r", encoding="utf-8", errors="replace") as f:
                trozos.append(f.read())
    return "\n".join(trozos)


def como_numero(t):
    t = t.replace("_", "")
    return int(t, 16) if t.lower().startswith("0x") else int(t)


# ===========================================================================
#  LO QUE SE LEE DEL ARBOL
# ===========================================================================

RE_KIND_KERNEL = re.compile(r"pub const KIND_(\w+)\s*:\s*u8\s*=\s*(0x[0-9A-Fa-f]+)\s*;")
RE_KIND_ABI = re.compile(r"^\s{4}(\w+)\s*=\s*(0x[0-9A-Fa-f]+)\s*,", re.M)
RE_MASK = re.compile(r"HANDLE_KIND_MASK\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f]+)")
# Las familias de operacion, y las tres que no se llaman `X_OP_*`. Por PATRON y
# no por lista: un objeto nuevo entra solo, que es la leccion que ya costo tres
# operaciones del directorio sin contrato.
RE_OPS = re.compile(
    r"const\s+(\w+_OP_\w+|SYSCALL_CLASS_\w+|ES_NODO_\w+|ES_TXT_\w+|DISCO_TRIM_\w+)"
    r"\s*:\s*u64\s*=\s*(0x[0-9A-Fa-f_]+|\d+)"
)
RE_OPS_USER = re.compile(
    r"(?m)^\s*pub const\s+(\w*OP_\w+|ES_NODO_\w+|ES_TXT_\w+|DISCO_TRIM_\w+)"
    r"\s*:\s*u\d+\s*=\s*(0x[0-9A-Fa-f_]+|\d+)\s*;"
)


def kinds_del_kernel(txt):
    return {como_numero(v): "KIND_" + n for n, v in RE_KIND_KERNEL.findall(txt)}


def kinds_del_abi(txt):
    return {como_numero(v): n for n, v in RE_KIND_ABI.findall(txt)}


def mascara(txt):
    m = RE_MASK.search(txt)
    if not m:
        raise SystemExit("guardian MUERTO: no se encuentra HANDLE_KIND_MASK en " + CAP_KERNEL)
    return como_numero(m.group(1))


# ===========================================================================
#  LAS SIETE REGLAS. Cada una devuelve una lista de quejas.
# ===========================================================================

def r1_caben_en_su_campo(kern, abi, mask):
    """Un numero que no cabe en su campo COMPILA, y falla siempre en silencio."""
    quejas = []
    for tabla, donde in ((kern, "kernel"), (abi, "bmo-abi")):
        for num, nombre in sorted(tabla.items()):
            if num > mask:
                quejas.append(
                    "%s: %s = 0x%02X no cabe en HANDLE_KIND_MASK (0x%02X). "
                    "Todo handle de ese tipo se codifica truncado y NUNCA resuelve."
                    % (donde, nombre, num, mask)
                )
    return quejas


def r2_las_dos_tablas(kern, abi, base):
    """Las dos listas de la misma taxonomia, en dos crates que no se hablan."""
    quejas = []
    notas = []
    for num in sorted(set(kern) & set(abi)):
        nk, na = kern[num], abi[num]
        esperado = base.get(num)
        if esperado is None:
            quejas.append(
                "0x%02X lo usan AHORA las dos tablas (%s / %s) y no esta en la linea "
                "base. Si significan lo mismo, sellalo; si no, elige otro numero."
                % (num, nk, na)
            )
            continue
        if (esperado["kernel"], esperado["abi"]) != (nk, na):
            quejas.append(
                "0x%02X cambio de pareja: la linea base dice %s / %s y el arbol dice "
                "%s / %s" % (num, esperado["kernel"], esperado["abi"], nk, na)
            )
    # Los que estaban en la linea base y ya no chocan: se arreglaron.
    for num, e in sorted(base.items()):
        if num not in kern or num not in abi:
            notas.append(
                "0x%02X (%s / %s) ya no lo usan las dos: quitalo de la linea base "
                "con --sellar" % (num, e["kernel"], e["abi"])
            )
    return quejas, notas


def r3_operaciones_kernel(ops_kernel, ops_abi):
    quejas = []
    for nombre, valor in ops_kernel.items():
        if nombre not in ops_abi:
            quejas.append("%s esta en el kernel y NO en el ABI" % nombre)
        elif ops_abi[nombre] != valor:
            quejas.append(
                "%s: el kernel dice 0x%X y el ABI 0x%X" % (nombre, valor, ops_abi[nombre])
            )
    return quejas


def r4_operaciones_userland(ops_user, ops_abi):
    """En el userland, lo que se pide sobre `CURRENT_TASK` pierde el prefijo."""
    quejas = []
    for nombre, valor in ops_user.items():
        candidatos = [nombre] + (["TASK_" + nombre] if nombre.startswith("OP_") else [])
        hallado = None
        for c in candidatos:
            if c in ops_abi:
                hallado = c
                break
        if hallado is None:
            quejas.append("%s esta en el userland y NO en el ABI" % nombre)
        elif ops_abi[hallado] != valor:
            quejas.append(
                "%s: el userland dice 0x%X y el ABI (%s) 0x%X"
                % (nombre, valor, hallado, ops_abi[hallado])
            )
    return quejas


def _familia(nombre):
    """A que enumeracion pertenece este nombre.

    ** LA PRIMERA VERSION CORTABA POR EL PRIMER GUION BAJO Y DIO CINCO FALSOS.

    `DISCO_OP_TRIM_LIBRE` es una OPERACION del disco; `DISCO_TRIM_SIN_DISCO` es
    un CODIGO DE ESTADO del TRIM. Comparten las cinco primeras letras y nada
    mas, asi que meterlos en la misma bolsa los declaraba repetidos por valer los
    dos `0x1` -- que es correcto, porque son dos enumeraciones distintas. Igual
    con `ES_NODO_*` contra `ES_TXT_*`.

    *** Un guardian que da un falso positivo se apaga en una semana, y entonces
    deja de avisar tambien de lo verdadero. La cabecera de `build.ps1` ya lo
    tiene escrito con estas palabras: *"comparadas como texto darian un fallo
    FALSO, que es la peor clase de guardian."*
    """
    if "_OP_" in nombre:
        return nombre.split("_OP_")[0] + "_OP"
    partes = nombre.split("_")
    return "_".join(partes[:2]) if len(partes) > 2 else nombre


RE_CUESTA = re.compile(r"^//!\s*\[cuesta\]\s+(\w+)", re.M)


def r6_el_coste_declarado(declarantes, minimo):
    """L6e -- MODULAR PRECISA: si lo declaras, usa el vocabulario. Y no bajes.

    # Por que es un trinquete y no un muro

    Hay ~150 ficheros solo en `ring0`. Exigirles la etiqueta a todos de golpe
    seria un guardian que grita 150 veces el primer dia, y **uno que grita sin
    motivo se apaga en una semana** -- lo dice el guardian de los enlaces y lo
    repite L6a. Asi que se exigen dos cosas mucho mas pequenas:

      * quien la declare, que la declare BIEN (vocabulario cerrado);
      * y que el numero de los que la declaran **no baje nunca**.

    *** La segunda es la que hace que la ley avance sola: cada fichero nuevo que
    la ponga sube el suelo, y el suelo no se puede volver a bajar.
    """
    quejas = []
    for ruta, clase in sorted(declarantes.items()):
        if clase not in COSTES:
            quejas.append(
                "%s declara [cuesta] %s, que no esta en el vocabulario. "
                "Las clases son: %s" % (ruta, clase, ", ".join(COSTES))
            )
    if len(declarantes) < minimo:
        quejas.append(
            "los ficheros que declaran [cuesta] bajaron de %d a %d. "
            "La ley L6e solo puede avanzar: si uno se borro, sella con --sellar"
            % (minimo, len(declarantes))
        )
    return quejas


RE_RIESGO = re.compile(r"^//!\s*\[riesgo\]\s+([A-Z ]+)", re.M)


def r7_el_riesgo_declarado(declarantes, minimo):
    """L6f -- MODULAR PRECISA NIVEL 2: por que ESA pieza es la que va a fallar.

    # La diferencia con R6, que no es un matiz

    `[cuesta]` dice **cuanto duele** si la pieza se equivoca. `[riesgo]` dice
    **por que se va a equivocar**. Un fichero puede costar `MAQUINA` y no tener
    ningun riesgo declarado: es caro y es tranquilo. Y al reves.

    *** Y la que sirve el dia del fallo es esta. Una pantalla azul da un `rip`,
    y de ahi sale UNA funcion; lo que no da es en cual de sus cuatro niveles
    mirar. La clase lo dice: `AJENO` manda al numero que entro de fuera,
    `ESPEJO` manda a buscar al otro que juzga lo mismo. El 2026-08-30 esas dos
    eran exactamente las dos respuestas.

    # Se admiten VARIAS clases, y no es una comodidad

    Un mismo trozo puede esconder dos agujas --`destroy_address_space` es
    `AJENO` y `ESPEJO` a la vez-- y obligar a elegir una haria que la etiqueta
    mintiera por la mitad. Cada palabra se juzga contra el vocabulario por
    separado, asi que la comparabilidad no se pierde.

    # Trinquete, igual que L6e

    No se le exige la etiqueta a nadie. Se exige que quien la ponga use el
    vocabulario, y que el numero de los que la ponen **no baje nunca**.
    """
    quejas = []
    for ruta, clases in sorted(declarantes.items()):
        for clase in clases:
            if clase not in RIESGOS:
                quejas.append(
                    "%s declara [riesgo] %s, que no esta en el vocabulario. "
                    "Las clases son: %s" % (ruta, clase, ", ".join(RIESGOS))
                )
    if len(declarantes) < minimo:
        quejas.append(
            "los ficheros que declaran [riesgo] bajaron de %d a %d. "
            "La ley L6f solo puede avanzar: si uno se borro, sella con --sellar"
            % (minimo, len(declarantes))
        )
    return quejas


def r5_sin_numeros_repetidos(ops_kernel):
    """Dos operaciones con el mismo numero: una de las dos entra en el brazo de
    la otra, y ninguna de las dos falla en voz alta."""
    quejas = []
    familias = {}
    for nombre, valor in ops_kernel.items():
        familias.setdefault(_familia(nombre), {}).setdefault(valor, []).append(nombre)
    for fam, porNum in sorted(familias.items()):
        for valor, nombres in sorted(porNum.items()):
            if len(nombres) > 1:
                quejas.append(
                    "familia %s: 0x%X lo usan %s" % (fam, valor, " y ".join(sorted(nombres)))
                )
    return quejas


# ===========================================================================
#  LA LINEA BASE
# ===========================================================================

def linea_base_leer():
    base = {}
    if not os.path.exists(BASE):
        return base
    with open(BASE, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            partes = linea.split(None, 3)
            if len(partes) < 3:
                continue
            base[como_numero(partes[0])] = {
                "kernel": partes[1],
                "abi": partes[2],
                "nota": partes[3] if len(partes) > 3 else "",
            }
    return base


def linea_base_escribir(kern, abi, previa):
    filas = []
    for num in sorted(set(kern) & set(abi)):
        nota = previa.get(num, {}).get("nota", "")
        if not nota:
            nota = "COINCIDEN" if _parecen_lo_mismo(kern[num], abi[num]) else "DIVERGEN -- deuda"
        filas.append("0x%02X %-18s %-18s %s" % (num, kern[num], abi[num], nota))
    with open(BASE, "w", encoding="utf-8", newline="\n") as f:
        f.write(CABECERA_BASE)
        f.write("\n".join(filas))
        f.write("\n")
    return len(filas)


def _parecen_lo_mismo(nk, na):
    """Una heuristica, y SOLO para proponer al sellar -- nunca para juzgar.

    Dos nombres en dos idiomas no se pueden comparar de verdad: `KIND_ARCHIVO` y
    `File` son el mismo objeto y no comparten una letra. Lo que si se puede es
    ADIVINAR y dejar que una persona corrija la nota. Juzgar con esto seria
    inventar un veredicto; proponerlo ahorra escribir catorce lineas a mano.
    """
    return nk.replace("KIND_", "").replace("_", "").lower()[:4] == na.replace("_", "").lower()[:4]


CABECERA_CUESTAS = """# EL SUELO DE L6e -- cuantos ficheros declaran `[cuesta]`.
#
# La ley esta en `META-KERNEL_HARD.md`, L6e (MODULAR PRECISA): el corte se elige
# tambien por lo que cuesta que la pieza se equivoque, y la cabecera lo declara.
#
# ** Esto NO exige la etiqueta a los ~150 ficheros de `ring0`. Exige dos cosas
# mas pequenas: que quien la declare use el vocabulario cerrado, y que este
# numero **no baje nunca**. Cada fichero nuevo que la ponga sube el suelo, y el
# suelo no se vuelve a bajar.
#
# El numero va solo en la primera linea util. Lo de abajo es el inventario, y es
# comentario: esta para leerlo, no para juzgarlo.

"""

CABECERA_RIESGOS = """# EL SUELO DE L6f -- cuantos ficheros declaran `[riesgo]`.
#
# La ley esta en `META-KERNEL_HARD.md`, L6f (MODULAR PRECISA NIVEL 2). L6e dice
# lo que CUESTA que una pieza se equivoque; esto dice POR QUE es probable que se
# equivoque, que es la mitad que sirve el dia del fallo.
#
# ** Un `rip` da UNA funcion. Lo que no da es en cual de sus cuatro niveles
# mirar. La clase lo dice, y por eso el vocabulario es cerrado: son los sitios
# donde este proyecto ya encontro la aguja.
#
# El numero va solo en la primera linea util. Lo de abajo es el inventario, y es
# comentario: esta para leerlo, no para juzgarlo.

"""

CABECERA_BASE = """# LINEA BASE del contrato -- los numeros que USAN LAS DOS TABLAS.
#
# El kernel (`obj/cap.rs`) y `bmo-abi` (`handle/kind.rs`) son dos listas de la
# misma taxonomia en dos crates que no se hablan. Cuando un numero aparece en
# las dos, es una AFIRMACION de que significan lo mismo -- y hoy hay cinco donde
# eso es falso.
#
# ** ESTA LISTA ESTA PARA ENCOGERSE. Es un trinquete, como el de L6a: lo que ya
# esta aqui se tolera con su motivo escrito al lado; un numero NUEVO en las dos
# tablas para el build hasta que alguien decida cual de las dos cosas es.
#
# Hoy no hace dano porque el `kind` del handle **solo lo interpreta el kernel**:
# el ABI declara la taxonomia y no la usa para resolver nada. Es deuda, no
# fallo. El dia que alguien de Ring 3 mire ese byte, deja de serlo.
#
# Formato:  numero  nombre_kernel  nombre_abi  nota
# Se regenera con `--sellar`, y la nota se escribe A MANO: una herramienta no
# puede saber si `KIND_ARCHIVO` y `File` son el mismo objeto.

"""


# ===========================================================================
#  AUTOPRUEBA -- L4: una regla se demuestra diciendo NO
# ===========================================================================

def autoprueba():
    """Cada regla contra una tabla hecha para romperla EXACTAMENTE a ella."""
    fallos = []
    casos = [0]

    def exige(nombre, quejas, debe_quejarse=True):
        casos[0] += 1
        if debe_quejarse and not quejas:
            fallos.append("la regla %s NO dijo nada y tenia que decir que NO" % nombre)
        if not debe_quejarse and quejas:
            fallos.append("la regla %s se quejo de algo correcto: %s" % (nombre, quejas))

    # R1 -- un kind por encima de la mascara.
    exige("R1", r1_caben_en_su_campo({0x80: "KIND_MALO"}, {}, 0x7F))
    exige("R1(bueno)", r1_caben_en_su_campo({0x7F: "KIND_JUSTO"}, {}, 0x7F), False)

    # R2 -- un numero nuevo en las dos tablas, sin sellar.
    q, _ = r2_las_dos_tablas({0x33: "KIND_X"}, {0x33: "Otro"}, {})
    exige("R2", q)
    q, _ = r2_las_dos_tablas(
        {0x33: "KIND_X"}, {0x33: "Otro"}, {0x33: {"kernel": "KIND_X", "abi": "Otro", "nota": "ok"}}
    )
    exige("R2(sellado)", q, False)
    # Y una pareja que CAMBIA sin pasar por la linea base.
    q, _ = r2_las_dos_tablas(
        {0x33: "KIND_Y"}, {0x33: "Otro"}, {0x33: {"kernel": "KIND_X", "abi": "Otro", "nota": "ok"}}
    )
    exige("R2(pareja cambiada)", q)

    # R3 -- una operacion del kernel que el ABI no tiene, y una con otro numero.
    exige("R3(falta)", r3_operaciones_kernel({"TASK_OP_X": 1}, {}))
    exige("R3(numero)", r3_operaciones_kernel({"TASK_OP_X": 1}, {"TASK_OP_X": 2}))
    exige("R3(bueno)", r3_operaciones_kernel({"TASK_OP_X": 1}, {"TASK_OP_X": 1}), False)

    # R4 -- el userland pierde el prefijo, asi que las dos formas tienen que valer.
    exige("R4(falta)", r4_operaciones_userland({"OP_X": 1}, {}))
    exige("R4(prefijo)", r4_operaciones_userland({"OP_X": 1}, {"TASK_OP_X": 1}), False)
    exige("R4(numero)", r4_operaciones_userland({"OP_X": 1}, {"TASK_OP_X": 9}))

    # R5 -- dos operaciones de la misma familia con el mismo numero.
    exige("R5", r5_sin_numeros_repetidos({"TASK_OP_A": 3, "TASK_OP_B": 3}))
    exige("R5(distintas familias)", r5_sin_numeros_repetidos({"TASK_OP_A": 3, "FB_OP_B": 3}), False)
    # *** Y el falso positivo que la primera version SI daba: una operacion del
    # disco y un codigo de estado del TRIM comparten prefijo y no son la misma
    # enumeracion. Se queda como prueba para que nadie "simplifique" `_familia`.
    exige(
        "R5(prefijo compartido, enumeraciones distintas)",
        r5_sin_numeros_repetidos({"DISCO_OP_TRIM_LIBRE": 1, "DISCO_TRIM_SIN_DISCO": 1}),
        False,
    )
    exige(
        "R5(ES_NODO contra ES_TXT)",
        r5_sin_numeros_repetidos({"ES_NODO_HIJOS": 1, "ES_TXT_RUTA": 1}),
        False,
    )

    # R6 -- L6e: una clase inventada, y un suelo que baja.
    exige("R6(clase inventada)", r6_el_coste_declarado({"x.rs": "CARISIMO"}, 0))
    exige("R6(clase buena)", r6_el_coste_declarado({"x.rs": "MAQUINA"}, 0), False)
    exige("R6(el suelo baja)", r6_el_coste_declarado({"x.rs": "NADA"}, 5))
    exige("R6(el suelo sube)", r6_el_coste_declarado({"x.rs": "NADA", "y.rs": "TAREA"}, 1), False)

    # R7 -- L6f: lo mismo, y ademas que la SEGUNDA clase tambien se juzgue.
    exige("R7(clase inventada)", r7_el_riesgo_declarado({"x.rs": ("RARO",)}, 0))
    exige("R7(clase buena)", r7_el_riesgo_declarado({"x.rs": ("AJENO",)}, 0), False)
    # *** La que de verdad importa: dos clases, la primera buena y la segunda no.
    # Un juez que solo mire la primera palabra deja pasar la mitad de cada linea.
    exige("R7(la segunda tambien se mira)", r7_el_riesgo_declarado({"x.rs": ("AJENO", "RARO")}, 0))
    exige("R7(dos buenas)", r7_el_riesgo_declarado({"x.rs": ("AJENO", "ESPEJO")}, 0), False)
    exige("R7(el suelo baja)", r7_el_riesgo_declarado({"x.rs": ("AJENO",)}, 5))

    if fallos:
        for f in fallos:
            print("  [X] " + f)
        print("autoprueba: %d regla(s) no saben decir que NO" % len(fallos))
        return 1
    # ** El numero se CUENTA, no se escribe. La version anterior decia "21
    # casos" y habia 19: un guardian con una cifra a mano dentro es un guardian
    # que dice un numero viejo con toda la confianza del mundo.
    print("clean: las SIETE reglas saben decir que NO (%d casos)" % casos[0])
    return 0


# ===========================================================================

def _declarantes(regex, leer_grupo):
    """Todo `.rs` del arbol cuya cabecera case con `regex`.

    Se barre por PATRON y no por lista, que es la leccion que ya costo tres
    operaciones del directorio sin contrato: un fichero nuevo entra solo.

    ** UN barrido y no dos. Cuando entro L6f esto estaba a punto de ser la
    segunda copia del mismo `os.walk` con otra expresion regular, que es
    exactamente lo que L6e dice que no se haga: tres copias de una regla son
    tres sitios donde arreglarla, y el dia que alguien arregle dos se notara en
    el tercero.
    """
    hallados = {}
    raiz_ = raiz()
    for sub in ("Ultra_kernel_x86-64", "platform", "Ultra_userspace", "toolchain"):
        base = os.path.join(raiz_, sub)
        if not os.path.isdir(base):
            continue
        for dirpath, dirnames, filenames in os.walk(base):
            dirnames[:] = [d for d in dirnames if d not in ("target", ".git")]
            for n in filenames:
                if not n.endswith(".rs"):
                    continue
                ruta = os.path.join(dirpath, n)
                with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                    cab = f.read(4000)
                m = regex.search(cab)
                if m:
                    rel = os.path.relpath(ruta, raiz_).replace(os.sep, "/")
                    hallados[rel] = leer_grupo(m.group(1))
    return hallados


def declarantes_de_coste():
    """Los `[cuesta]` -- L6e. Una clase por fichero."""
    return _declarantes(RE_CUESTA, lambda g: g)


def declarantes_de_riesgo():
    """Los `[riesgo]` -- L6f. VARIAS clases por fichero, separadas por espacios."""
    return _declarantes(RE_RIESGO, lambda g: tuple(g.split()))


def _suelo(ruta):
    """El primer numero util de un fichero de trinquete. Ausente = 0."""
    if not os.path.exists(ruta):
        return 0
    with open(ruta, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if linea and not linea.startswith("#"):
                try:
                    return int(linea.split()[0])
                except ValueError:
                    return 0
    return 0


def minimo_de_costes():
    return _suelo(CUESTAS)


def minimo_de_riesgos():
    return _suelo(RIESGOS_TXT)


def cargar():
    cap = leer(CAP_KERNEL)
    kern = kinds_del_kernel(cap)
    abi = kinds_del_abi(leer(KIND_ABI))
    mask = mascara(cap)
    ops_kernel_txt = leer(OPS_KERNEL) + "\n" + leer_dir(OBJ_KERNEL)
    ops_kernel = {n: como_numero(v) for n, v in RE_OPS.findall(ops_kernel_txt)}
    ops_abi = {n: como_numero(v) for n, v in RE_OPS.findall(leer_dir(SURFACE_ABI))}
    ops_user = {n: como_numero(v) for n, v in RE_OPS_USER.findall(leer(USERLAND))}
    return kern, abi, mask, ops_kernel, ops_abi, ops_user


def comprobar():
    kern, abi, mask, ops_kernel, ops_abi, ops_user = cargar()
    base = linea_base_leer()

    quejas = []
    quejas += [("R1 un kind que no cabe en su campo", q) for q in r1_caben_en_su_campo(kern, abi, mask)]
    q2, notas = r2_las_dos_tablas(kern, abi, base)
    quejas += [("R2 las dos tablas de kinds", q) for q in q2]
    quejas += [("R3 operacion del kernel sin contrato", q) for q in r3_operaciones_kernel(ops_kernel, ops_abi)]
    quejas += [("R4 operacion del userland sin contrato", q) for q in r4_operaciones_userland(ops_user, ops_abi)]
    quejas += [("R5 dos operaciones con el mismo numero", q) for q in r5_sin_numeros_repetidos(ops_kernel)]
    decl = declarantes_de_coste()
    quejas += [("R6 L6e el coste declarado", q) for q in r6_el_coste_declarado(decl, minimo_de_costes())]
    ries = declarantes_de_riesgo()
    quejas += [("R7 L6f el riesgo declarado", q) for q in r7_el_riesgo_declarado(ries, minimo_de_riesgos())]

    for nota in notas:
        print("  [i] " + nota)

    if quejas:
        for regla, q in quejas:
            print("  [X] %s: %s" % (regla, q))
        print("contrato: %d incumplimiento(s)" % len(quejas))
        return 1

    divergen = sum(1 for e in base.values() if e["nota"].startswith("DIVERGEN"))
    print("clean: %d kinds del kernel, %d del ABI, todos caben en 0x%02X"
          % (len(kern), len(abi), mask))
    print("clean: %d operaciones del kernel y %d del userland, todas en el contrato"
          % (len(ops_kernel), len(ops_user)))
    if divergen:
        print("clean: %d divergencia(s) en la linea base -- toleradas, y solo pueden bajar"
              % divergen)
    print("clean: %d fichero(s) declaran [cuesta] (L6e) y ninguno inventa una clase"
          % len(decl))
    print("clean: %d fichero(s) declaran [riesgo] (L6f) -- %d clase(s) en total"
          % (len(ries), sum(len(c) for c in ries.values())))
    return 0


def main():
    p = argparse.ArgumentParser(description="El contrato de compatibilidad, comprobado.")
    g = p.add_mutually_exclusive_group(required=True)
    g.add_argument("--check", action="store_true", help="lo que llama el build")
    g.add_argument("--sellar", action="store_true", help="reescribe la linea base desde el arbol")
    g.add_argument("--autoprueba", action="store_true", help="demuestra que cada regla sabe decir NO")
    a = p.parse_args()

    if a.autoprueba:
        return autoprueba()
    if a.sellar:
        kern, abi, _, _, _, _ = cargar()
        n = linea_base_escribir(kern, abi, linea_base_leer())
        decl = declarantes_de_coste()
        with open(CUESTAS, "w", encoding="utf-8", newline="\n") as f:
            f.write(CABECERA_CUESTAS)
            f.write("%d\n" % len(decl))
            for ruta, clase in sorted(decl.items()):
                f.write("# %-9s %s\n" % (clase, ruta))
        ries = declarantes_de_riesgo()
        with open(RIESGOS_TXT, "w", encoding="utf-8", newline="\n") as f:
            f.write(CABECERA_RIESGOS)
            f.write("%d\n" % len(ries))
            for ruta, clases in sorted(ries.items()):
                f.write("# %-18s %s\n" % (" ".join(clases), ruta))
        print("sellado el suelo de L6e: %d fichero(s) declaran [cuesta]" % len(decl))
        print("sellado el suelo de L6f: %d fichero(s) declaran [riesgo]" % len(ries))
        print("sellada la linea base: %d numero(s) en las dos tablas" % n)
        print("[!] revisa las notas A MANO: una herramienta no sabe si KIND_ARCHIVO y File")
        print("    son el mismo objeto.")
        return 0
    return comprobar()


if __name__ == "__main__":
    sys.exit(main())

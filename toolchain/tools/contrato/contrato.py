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

# -- L6g nivel 3: LOS CARRILES ----------------------------------------------
#
# ** Aqui vivia `critic/`, una carpeta GLOBAL con nombre de carril, y era mi
# primera lectura --equivocada-- de L6g. Se retiro el 2026-08-31 y el dueno lo
# dijo por su nombre: *"no me gusta esa palabra ahi"*.
#
# *** Y el nombre solo era el sintoma. **Un color solo significa algo DENTRO de
# un modulo**: `critic/amarilla.rs` era "amarilla respecto a que?". Una senal
# ilegible justo en el sitio donde la senal ERA el objetivo.
#
# Sus dos inquilinas volvieron a casa --`mm/vmm/amarilla.rs` y
# `mm/phys/amarilla.rs`-- y lo que las ataba viaja ahora donde tiene que viajar:
# en su `[riesgo] ESPEJO`, no en una carpeta.

# -- L6g, LA OTRA MITAD: los carriles POR MODULO. -----------------------------
#
# ** `critic/` de arriba es una CARPETA GLOBAL, y por eso sus carriles no
# incluyen el verde: alli dentro todo es critico por definicion. Pero el modelo
# que de verdad usa el arbol --y el que pidio el dueno-- es otro: **un fichero
# de Ring 0 se parte DENTRO DE SU PROPIA CARPETA**, y ahi el verde es la mitad
# del mensaje. `mm/vmm/verde.rs` no dice "esto no importa": dice **"esto se
# puede tocar sin miedo"**, que es justo lo que hace falta saber el dia que la
# maquina esta rota y hay que cambiar algo deprisa.
#
# *** Y hasta hoy la ley se cobraba EN EL UNICO SITIO QUE USA EL MODELO VIEJO y
# en ninguno de los cuatro que usan el bueno. `mm/vmm/`, `plat/faults/`,
# `task/scheduler/` y `obj/fb/` son doce ficheros con letrero y sin guardian:
# los doce lo declaran hoy porque se escribieron a mano, y el primero que se
# anadiera sin `[cuesta]` no lo habria dicho nadie.
VIAS_MODULO = ("roja", "amarilla", "verde")

# -- EL SEMAFORO, y es lo que el dueno pidio con esas palabras ---------------
#
#    ROJO      critico. Cambiarlo puede parar la maquina o corromperla callando
#    AMARILLO  posible cambio: esta en obras, o es un instrumento que si se
#              equivoca no falla -- CONVENCE, que es peor
#    VERDE     normal y seguro. Se puede jugar
#
# ** El `[carril]` no es lo mismo que el `[cuesta]` y por eso son dos etiquetas.
# `[cuesta]` dice **que se pierde si esto falla**; `[carril]` dice **que arriesgo
# si lo TOCO**. `core/autopsy.rs` es la prueba de que no coinciden: su coste es
# NADA --no rompe nada al fallar-- y su carril es AMARILLO, porque si miente
# manda la investigacion al sitio equivocado. Ya paso tres veces en una semana.
SEMAFORO = ("ROJO", "AMARILLO", "VERDE")
# El nombre del fichero de un carril y su etiqueta tienen que decir lo mismo.
# Es lo que caza un renombrado a medias, que es como una pieza cambia de color
# sin que nadie lo decida.
COLOR_DEL_NOMBRE = {"roja": "ROJO", "amarilla": "AMARILLO", "verde": "VERDE"}
RING0_DIR = "Ultra_kernel_x86-64/kernel/src/ring0"

# -- L6g EN LA PUERTA DE LOS TERCEROS: las cabeceras de REX -------------------
#
# ** El semaforo cubria los 162 ficheros de Ring 0 y NINGUNA de las diez
# cabeceras con las que se escribe una app. O sea que la unica parte del arbol
# que un tercero abre de verdad era la unica sin letrero.
#
# La ley es la misma; lo que cambia es el idioma del comentario. Rust marca con
# `//!` y C no tiene doc-comment, asi que la marca vive dentro del bloque de
# cabecera del propio fichero:
#
#      * [carril]  ROJO      ...
#
# [!] Y una diferencia de FORMA que el juez tiene que conocer: una linea de
# `[riesgo]` puede llevar VARIAS clases, asi que cuando lleva mas de una **la
# linea es solo para ellas** y el motivo baja a la siguiente. Con el motivo
# pegado detras no hay forma de saber donde acaba la clase y empieza la prosa
# --`AJENO ESPEJO AJENO: los offsets...`-- y un juez que adivina da permiso con
# autoridad.
REX_DIR = "toolchain/forge/sem-asm/tables/bmo"
ESPEJO = os.path.join(os.path.dirname(os.path.abspath(__file__)), "REX_ESPEJO.txt")

# Las constantes cuyo valor este juez NO sabe evaluar exacto. Se llevan a la
# vista a proposito: una lista vacia dice "lo lei todo", y una con nombres dice
# "estas no las juzgo" -- que es una respuesta, y esconderla no lo seria.
SIN_EVALUAR = []

FRONTERA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "FRONTERA_REX.txt")
COBERTURA = os.path.join(os.path.dirname(os.path.abspath(__file__)), "COBERTURA.txt")

# -- R14: las APPS del arbol propio. -----------------------------------------
#
# No cubre `mods/` ni `$BMO_MODS`: **un tercero TIENE DERECHO a redefinir un
# numero**, que es literalmente lo que promete el buscador de cabeceras. Esta
# regla es para lo que el proyecto publica como ejemplo -- que es lo que la
# gente copia.
APPS_C_DIR = "toolchain/lang/c/examples"
RE_DEFINE_C = re.compile(r"^#define\s+([A-Za-z_][A-Za-z0-9_]*)\s+([^\n]*?)\s*(?:/\*.*)?$", re.M)
# La OPERACION es el segundo argumento de la puerta. Lo demas --capability,
# a0, a1, a2-- son datos; solo este dice QUE se pide.
RE_PUERTA_C = re.compile(r"bmo_(?:valor|codigo)\s*\(\s*[^,()]+,\s*([^,]+),")
RE_IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
RE_SOLO_NUMERO = re.compile(r"^\s*(0[xX][0-9A-Fa-f]+|\d+)\s*$")

# -- R15: el ABI no puede repetir un numero DENTRO de una familia -------------
#
# ** Este proyecto ya lo pago una vez, y el kernel lo dejo escrito al elegir
# `PANTALLA_SOLTAR`: *"0x1D elegido tras listar los opcodes ORDENADOS, que es la
# regla desde que MEMORIA_PEDIR se puso en 0x12 --ya ocupado por REINICIAR-- y
# pedir memoria habria reiniciado la maquina"*.
#
# R5 ya vigila eso en el KERNEL. Nadie lo vigilaba en el ABI, que es la lista
# que lee quien escribe una app.
ABI_FAMILIAS_OP = (
    "TASK_OP_", "ARCH_OP_", "INPUT_OP_", "FB_OP_", "AUDIO_OP_", "MEM_OP_",
    "CONSOLA_OP_", "DIR_OP_", "DISCO_OP_", "RED_OP_", "PLACA_OP_",
    "TAREA_OP_", "PRESTADO_OP_", "CHANNEL_OP_", "LIENZO_OP_", "APARATO_OP_",
    # ** `INFO_TXT_` ANTES que `INFO_`, y no es orden alfabetico: los campos de
    # texto entran por `TASK_OP_INFO_TEXTO` (0x14) y no por `TASK_OP_INFO`
    # (0x13), asi que son OTRO espacio de numeracion. Sin esta linea, R15
    # gritaria que `INFO_TSC_HZ` y `INFO_TXT_EXT_NOMBRE` comparten el 0x05 --y
    # comparten el numero, y no es un choque.
    "INFO_TXT_", "INFO_",
)

# Los choques que hoy existen y se toleran, con su motivo. ** TIENE QUE LLEGAR A
# CERO: no es una lista de excepciones, es una deuda escrita.
ABI_CHOQUES_TOLERADOS = {
    # ** VACIA, Y ESE ES EL ESTADO CORRECTO. (2026-09-02)
    #
    # Tuvo una entrada exactamente un dia: `TASK_OP_LIENZO_REFLEJO` compartia el
    # `0x1C` con `TASK_OP_TOMAR`. Era una constante MUERTA --de `KIND_LIENZO`,
    # un diseno que salio del kernel-- okupando un numero VIVO, asi que la
    # salida no fue tolerarla: fue borrarla.
    #
    # [!] Si algo vuelve aqui, la fila lleva su motivo y **la lista solo puede
    # encoger**: `r15_el_abi_no_repite_numero` se queja de una tolerancia que ya
    # no hace falta, para que una deuda saldada no se quede escrita como si
    # todavia existiera.
}

# -- R13: EL ESPEJO. Los mismos numeros, escritos dos veces --------------------
#
# `<bmo/paquete.h>` lo confiesa por escrito: *"los numeros del formato viven en
# `bmo_abi::bef` y aqui se repiten porque C no puede importar de Rust"*. Eso es
# verdad de TODA cabecera de REX, no solo de aquella: 90 constantes escritas dos
# veces, en dos lenguajes, sin nadie que las compare.
#
# ** Y es el patron que ya costo caro: una tabla con dos lectores, y crecer por
# uno la rompe para el otro. El 22-08 fueron `intrinsics.toml` y cinco frontends.
#
# El emparejamiento es JUICIO y no deduccion: los dos lados se llaman distinto a
# proposito --uno habla ingles de kernel (`TASK_OP_FRAMEBUFFER_CLAIM`), el otro
# espanol de app (`BMO_OP_PANTALLA_RECLAMAR`)-- asi que un juez que dedujera la
# pareja estaria adivinando. Lo que hay aqui es el mapa de familias que cubre lo
# mecanico, y `REX_A_MANO` para lo que no tiene cola comun.
REX_FAMILIAS = (
    ("ARCH_OP_", "BMO_ARCH_"),
    ("INPUT_OP_", "BMO_ENTRADA_"),
    ("INFO_", "BMO_INFO_"),
    ("MEM_OP_", "BMO_MEM_"),
    ("TASK_OP_", "BMO_OP_"),
    ("FB_OP_", "BMO_FB_"),
)

# Las parejas que no comparten cola. Cada una es una persona diciendo "estas dos
# son la misma cosa", que es justo lo que una herramienta no puede decir.
REX_A_MANO = {
    "AUDIO_OP_DEVICES": "BMO_SONIDO_APARATO",
    "AUDIO_OP_BEEP": "BMO_SONIDO_PITAR",
    "AUDIO_OP_VOLUME": "BMO_SONIDO_VOLUMEN",
    "AUDIO_OP_SILENCE": "BMO_SONIDO_CALLAR",
    "TASK_OP_FRAMEBUFFER_CLAIM": "BMO_OP_PANTALLA_RECLAMAR",
    "TASK_OP_INPUT_CLAIM": "BMO_OP_ENTRADA_RECLAMAR",
    "TASK_OP_AUDIO_CLAIM": "BMO_OP_SONIDO_RECLAMAR",
    "TASK_OP_AUDIO_RELEASE": "BMO_OP_SONIDO_SOLTAR",
    "TASK_OP_GET_PID": "BMO_OP_PID",
    "TASK_OP_GET_TID": "BMO_OP_TID",
    "TASK_OP_YIELD": "BMO_OP_CEDER",
    "TASK_OP_EXIT": "BMO_OP_SALIR",
    "TASK_OP_CONSOLE_WRITE": "BMO_OP_CONSOLA_ESCRIBIR",
    "TASK_OP_CONSOLE_READ": "BMO_OP_CONSOLA_LEER",
    # ** Estas TRES no podian emparejarse hasta el 2026-09-02, y no por falta de
    # mapa: el extractor truncaba sus valores. `CURRENT_TASK` es
    # `0xFFFF_FFFF_FFFF_FFFE` y se leia `0xFFFF`; `DEVICE_HDA` es `1 << 1` y se
    # leia `1`. Emparejarlas entonces habria dado un choque FALSO.
    "CURRENT_TASK": "BMO_TAREA_ACTUAL",
    "DEVICE_SPEAKER": "BMO_APARATO_ALTAVOZ",
    "DEVICE_HDA": "BMO_APARATO_HDA",
}

# ** SE CAPTURA LA EXPRESION ENTERA, y luego se evalua o se descarta.
#
# La primera version cazaba `(0x[0-9A-Fa-f]+|\d+)` y paraba ahi. Con eso:
#
#     pub const DEVICE_HDA: u64 = 1 << 1;                 leia 1, vale 2
#     pub const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE; leia 0xFFFF
#
# **Dos truncamientos en silencio, dentro del juez que existe para cazar
# numeros que no coinciden.** Si `CURRENT_TASK` hubiera tenido pareja, R13
# habria cantado un choque falso -- y peor: un truncamiento que POR CASUALIDAD
# coincida da un "clean" que nadie puede distinguir de uno de verdad.
#
# > Un extractor que trunca no lee de menos: lee MAL, y lo hace en voz de dato.
RE_CONST_RS = re.compile(r"pub const ([A-Z0-9_]+): u64 = ([^;]+);")
RE_CONST_H = re.compile(r"^#define ([A-Z0-9_]+)[ \t]+([^\n]+?)[ \t]*$", re.M)
# Lo que SI se sabe evaluar exacto: un literal, o un desplazamiento de literales.
RE_LITERAL = re.compile(r"^(0[xX][0-9A-Fa-f_]+|\d[\d_]*)$")
RE_DESPL = re.compile(r"^(0[xX][0-9A-Fa-f_]+|\d[\d_]*)\s*<<\s*(\d+)$")


def valor_exacto(txt):
    """El valor de una constante, o `None` si no se puede saber SIN suponer.

    ** `None` no es un fallo: es la unica respuesta honesta para una expresion
    que este juez no sabe evaluar. Lo que no se puede leer exacto **no se
    empareja**, porque una pareja con un valor adivinado es peor que no tenerla.
    """
    t = txt.strip()
    # los sufijos de C (`ULL`, `UL`, `U`) y los comentarios de linea sobran
    corte = t.find("/*")
    if corte >= 0:
        t = t[:corte].strip()
    for suf in ("ULL", "ull", "UL", "ul", "LL", "ll", "U", "u", "L", "l"):
        if t.endswith(suf) and len(t) > len(suf):
            t = t[:-len(suf)].strip()
            break
    m = RE_LITERAL.match(t)
    if m:
        return int(m.group(1).replace("_", ""), 0)
    m = RE_DESPL.match(t)
    if m:
        return int(m.group(1).replace("_", ""), 0) << int(m.group(2))
    return None
RE_SELLO_H = re.compile(r"^ \* \[(carril|cuesta|riesgo)\]\s+(.*)$", re.M)
# Una clase es una palabra ENTERA en mayusculas. Ver `sellos_de_cabecera`.
RE_MAYUSCULAS = re.compile(r"^[A-Z]+$")
VOCABULARIO = {"carril": SEMAFORO, "cuesta": COSTES, "riesgo": RIESGOS}


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
#  LAS DIEZ REGLAS. Cada una devuelve una lista de quejas.
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


# L6g regla 2: quien PRUEBA a esta pieza. Ver `r8_lo_que_vive_en_critic`.
RE_PRUEBA = re.compile(r"^//!\s*\[prueba\]\s+([a-z0-9-]+)", re.M)

RE_RIESGO = re.compile(r"^//!\s*\[riesgo\]\s+([A-Z ]+)", re.M)


def r8_el_juez_nombrado_existe(ficheros):
    """L6g -- **si dices quien te prueba, ese crate existe**. `{ruta: texto}`.

    Lo que queda de la vieja R8 cuando la carpeta desaparece, y es la mitad que
    valia. Se aplica ahora a **todo Ring 0** en vez de a dos ficheros.

    ** El kernel NO PUEDE tener pruebas: `bmo.ps1` lo excluye del banco con su
    motivo escrito --*"binario bare-metal: enlazado como test, `panic_impl` sale
    dos veces"*--. Asi que una pieza de Ring 0 que quiera demostrar algo saca su
    juez a un crate propio, sin dependencias y sin `unsafe`, que SI corre bajo
    `cargo test`, y lo NOMBRA:

        //! [prueba]  bmo-fisica-juicio

    Y se comprueba que exista. Un nombre que no resuelve es la misma mentira que
    un `#[cfg(test)]` que no corre nunca: **una garantia que se ve y no esta**.

    [!] Declararlo NO es obligatorio, y eso es deliberado. La mayoria de Ring 0
    no tiene juez que sacar --pintar una fuente no se prueba con un crate-- y
    exigirlo a los 162 seria pedir un banco de pruebas por cortesia. Lo que no se
    tolera es prometerlo y que no este.
    """
    quejas = []
    for ruta in sorted(ficheros):
        m = RE_PRUEBA.search(ficheros[ruta])
        if not m:
            continue
        nombre = m.group(1).strip()
        if not os.path.isdir(os.path.join(raiz(), "platform", "shared", nombre)):
            quejas.append(
                "%s declara [prueba] %s y ese crate no existe en "
                "platform/shared/ (L6g)" % (ruta, nombre)
            )
    return quejas


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

CABECERA_COBERTURA = """# EL SUELO DE R16 -- cuantas constantes del ABI tienen cabecera en REX.
#
# La pregunta del dueno era *"que reglas para que el ABI se aproveche TODO?"*, y
# la respuesta honesta no es "se expone todo": es **el hueco es este numero, y
# no puede crecer**.
#
# ** El denominador sale de `FRONTERA_REX.txt`, no del ABI entero. Un
# porcentaje contra las constantes enteras contaria como pendiente cosas que
# nunca van a estar -- y eso no es una medida, es una excusa que se ve bien.
#
# El numero va solo en la primera linea util. Se resella con `--sellar`.
#
# [!] Lo que esto NO mide: si la cabecera es BUENA. Mide que exista.
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

    # R11/R12 -- L6g en REX. Las cabeceras son C, asi que la marca va dentro
    # del bloque de comentario y no en un `//!`. Se arman por trozos por el
    # mismo motivo que las de R8.
    CAR = " * [carril]  ROJO      x" + chr(10)
    CUE = " * [cuesta]  DATO      x" + chr(10)
    RIE = " * [riesgo]  AJENO ESPEJO" + chr(10)
    entera = CAR + CUE + RIE
    exige("R11(cabecera entera)", r11_el_semaforo_de_rex({"a/roja.h": entera}), False)
    exige("R11(sin carril)", r11_el_semaforo_de_rex({"a/x.h": CUE + RIE}))
    exige("R11(color inventado)",
          r11_el_semaforo_de_rex({"a/x.h": " * [carril]  AZUL x" + chr(10) + CUE + RIE}))
    # *** La regla de corte de L6e, que es la que crea las carpetas.
    exige("R11(dos costes = mal cortado)",
          r11_el_semaforo_de_rex({"a/x.h": CAR + " * [cuesta]  DATO PUERTA" + chr(10) + RIE}))
    # *** El renombrado a medias: se llama verde y dice ROJO.
    exige("R11(nombre contra etiqueta)", r11_el_semaforo_de_rex({"a/verde.h": entera}))
    # *** Y que la SEGUNDA clase de [riesgo] tambien se juzgue.
    exige("R11(la segunda clase tambien)",
          r11_el_semaforo_de_rex({"a/roja.h": CAR + CUE + " * [riesgo]  AJENO RARO" + chr(10)}))

    # R14 -- la operacion sale de REX, no del fichero.
    REXN = {"BMO_ARCH_LEER": (0x01, "archivo/roja.h")}
    D = chr(35) + "define "
    malo = D + "FB_BASE 0x01" + chr(10) + "x = bmo_valor(p, FB_BASE, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": malo}, REXN)
    exige("R14(un numero copiado)", q)
    bueno = "x = bmo_valor(p, BMO_ARCH_LEER, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": bueno}, REXN)
    exige("R14(el nombre de REX)", q, False)
    # *** Un alias que apunta a REX NO es pecado: el numero sigue en un sitio.
    alias = D + "MIO BMO_ARCH_LEER" + chr(10) + "x = bmo_valor(p, MIO, 0, 0, 0);"
    q, _ = r14_ninguna_app_inventa_un_numero({"a.c": alias}, REXN)
    exige("R14(alias de un nombre de REX)", q, False)
    # *** Y un literal desnudo INFORMA, no falla: la sonda de seguridad llama a
    # operaciones que no existen a proposito, y gritarle la apagaria.
    q, n = r14_ninguna_app_inventa_un_numero(
        {"a.c": "x = bmo_codigo(p, 0x7777, 0, 0, 0);"}, REXN)
    exige("R14(un literal no falla)", q, False)
    exige("R14(pero se informa)", n)

    # *** Y una tolerancia que ya no hace falta se dice tambien: una deuda
    # saldada que sigue escrita miente igual que una oculta.
    _guardadas = dict(ABI_CHOQUES_TOLERADOS)
    ABI_CHOQUES_TOLERADOS[("TASK_OP_", 0xEE)] = "una deuda que ya no existe"
    q, _ = r15_el_abi_no_repite_numero({"TASK_OP_A": (0x30, "t.rs")})
    exige("R15(tolerancia que sobra)", q)
    ABI_CHOQUES_TOLERADOS.clear()
    ABI_CHOQUES_TOLERADOS.update(_guardadas)

    # R16 -- la cobertura no baja.
    exige("R16(baja)", r16_la_cobertura_solo_sube(80, 197, 93))
    exige("R16(igual)", r16_la_cobertura_solo_sube(93, 197, 93), False)
    exige("R16(sube)", r16_la_cobertura_solo_sube(94, 197, 93), False)
    # *** Que la SUPERFICIE crezca no es un fallo: el ABI gana operaciones antes
    # de que exista la cabecera. Lo que no puede es perderse una que ya estaba.
    exige("R16(la superficie crece)", r16_la_cobertura_solo_sube(93, 240, 93), False)
    # *** Y la frontera es la que hace honesto el denominador.
    _abi = {"TASK_OP_X": (1, "t.rs"), "CABINA_Y": (2, "c.rs")}
    _c, _s = cobertura_de_rex(_abi, {"TASK_OP_X": {}}, [("CABINA_", "el panel")])
    exige("R16(la frontera no cuenta)",
          [] if (_c, _s) == (1, 1) else ["conto %d de %d" % (_c, _s)], False)

    # R15 -- el ABI no repite dentro de una familia.
    q, _ = r15_el_abi_no_repite_numero(
        {"TASK_OP_A": (0x30, "t.rs"), "TASK_OP_B": (0x30, "t.rs")})
    exige("R15(dos ops con el mismo numero)", q)
    q, _ = r15_el_abi_no_repite_numero(
        {"TASK_OP_A": (0x30, "t.rs"), "TASK_OP_B": (0x31, "t.rs")})
    exige("R15(numeros distintos)", q, False)
    # *** Familias distintas SI pueden compartir: `ARCH_OP_LEER` y
    # `INPUT_OP_PUNTERO` valen las dos 0x01 y no se estorban.
    q, _ = r15_el_abi_no_repite_numero(
        {"ARCH_OP_X": (0x01, "o.rs"), "INPUT_OP_Y": (0x01, "e.rs")})
    exige("R15(familias distintas)", q, False)
    # *** Y el 0x05 de INFO_TSC_HZ contra INFO_TXT_EXT_NOMBRE: NO es choque,
    # porque los TXT entran por otra operacion del kernel.
    q, _ = r15_el_abi_no_repite_numero(
        {"INFO_TSC_HZ": (0x05, "i.rs"), "INFO_TXT_EXT_NOMBRE": (0x05, "i.rs")})
    exige("R15(INFO contra INFO_TXT)", q, False)

    # -- El EXTRACTOR, que es de quien depende R13 entero -------------------
    # *** Los dos truncamientos que tuvo de verdad, cada uno con su nombre.
    exige("valor(desplazamiento)",
          [] if valor_exacto("1 << 1") == 2 else ["1<<1 no da 2"], False)
    exige("valor(guiones bajos)",
          [] if valor_exacto("0xFFFF_FFFF_FFFF_FFFE") == 0xFFFFFFFFFFFFFFFE
          else ["los _ truncan"], False)
    exige("valor(sufijo ULL)",
          [] if valor_exacto("0x8000000000000000ULL") == 0x8000000000000000
          else ["el sufijo estorba"], False)
    exige("valor(decimal)",
          [] if valor_exacto("64") == 64 else ["decimal mal"], False)
    # *** Y lo que NO se sabe evaluar contesta None en vez de inventarse algo.
    exige("valor(lo que no se sabe)",
          [] if valor_exacto("(1024 * 1024)") is None else ["se invento un valor"], False)
    exige("valor(un nombre)",
          [] if valor_exacto("OTRA_CONSTANTE") is None else ["se invento un valor"], False)

    # R13 -- el espejo. Se arman a mano, que es lo que permite probar los casos
    # que el arbol real no tiene (y ojala no tenga nunca).
    ABI = {"TASK_OP_X": (0x09, "tarea.rs")}
    REX = {"BMO_OP_X": (0x09, "bmo/roja.h")}
    PAR = {"TASK_OP_X": "BMO_OP_X"}
    SELLO = {"TASK_OP_X": {"c": "BMO_OP_X", "valor": 0x09, "nota": "ok"}}
    exige("R13(sellado y coincide)", r13_el_espejo_de_rex(ABI, REX, PAR, SELLO), False)
    # Los dos lados discrepan: es el caso obvio.
    exige("R13(discrepan)",
          r13_el_espejo_de_rex(ABI, {"BMO_OP_X": (0x0A, "bmo/roja.h")}, PAR, SELLO))
    # Una pareja nueva que nadie miro: gate de revision.
    exige("R13(pareja sin sellar)", r13_el_espejo_de_rex(ABI, REX, PAR, {}))
    # Un nombre que desaparece de un lado.
    exige("R13(el nombre de C se fue)", r13_el_espejo_de_rex(ABI, {}, {}, SELLO))
    exige("R13(el nombre del ABI se fue)", r13_el_espejo_de_rex({}, REX, {}, SELLO))
    # *** LA QUE JUSTIFICA LA TABLA: cambian los DOS a la vez. Una comparacion
    # en vivo diria que coinciden --y coinciden-- y se le escaparia el unico
    # cambio que rompe los `.bex` que ya estan firmados.
    exige("R13(cambian los dos a la vez)",
          r13_el_espejo_de_rex({"TASK_OP_X": (0x77, "tarea.rs")},
                               {"BMO_OP_X": (0x77, "bmo/roja.h")}, PAR, SELLO))

    FACH = "#include <bmo/x/roja.h>" + chr(10) + "#include <bmo/x/verde.h>" + chr(10)
    exige("R12(carpeta limpia)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "verde.h"]}, {"t/bmo/x.h": FACH}),
          False)
    exige("R12(un colado entre carriles)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "ayudas.h"]}, {"t/bmo/x.h": FACH}))
    exige("R12(sin fachada)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h"]}, {}))
    # *** La que paga el silencio: la fachada existe y se deja un carril fuera.
    exige("R12(carril fuera de la fachada)",
          r12_los_carriles_de_rex({"t/bmo/x": ["roja.h", "verde.h"]},
                                  {"t/bmo/x.h": "#include <bmo/x/roja.h>" + chr(10)}))

    # R8 -- L6g: las cuatro exigencias de critic/, cada una por separado. Se
    # arman por trozos y no con literales de varias lineas: el fichero que
    # contiene esta prueba se lee y se parchea a menudo, y una cadena con saltos
    # dentro es lo primero que se rompe al hacerlo.
    CU = "//! [cuesta] MAQUINA" + chr(10)
    RI = "//! [riesgo] AJENO" + chr(10)
    BA = "//! [prueba]  bmo-mmio-juicio" + chr(10)
    bueno = (CU + RI + BA, 10)
    # -- R8: el juez nombrado existe ----------------------------------------
    exige("R8(juez que existe)",
          r8_el_juez_nombrado_existe({"mm/phys/amarilla.rs": BA}), False)
    exige("R8(juez que no existe)",
          r8_el_juez_nombrado_existe(
              {"mm/phys/amarilla.rs": "//! [prueba]  bmo-no-existe" + chr(10)}))
    # No declararlo NO es un fallo: la mayoria de Ring 0 no tiene juez que sacar.
    exige("R8(sin declarar juez)",
          r8_el_juez_nombrado_existe({"core/gato/neon.rs": "//! un gato"}), False)

    # -- R9: los carriles POR MODULO ---------------------------------------
    #
    # ** El caso que justifica la regla entera es el tercero: un `.rs` sin
    # nombre de carril viviendo entre carriles. Los otros dos son el letrero;
    # ese es la carpeta.
    sano = {"roja.rs": CU + RI, "verde.rs": CU + RI, "mod.rs": ""}
    exige("R9(carpeta sana)",
          r9_los_carriles_del_modulo({"mm/vmm": sano}), False)
    exige("R9(carril sin cuesta)",
          r9_los_carriles_del_modulo({"mm/vmm": {"roja.rs": RI}}))
    exige("R9(carril sin riesgo)",
          r9_los_carriles_del_modulo({"mm/vmm": {"roja.rs": CU}}))
    exige("R9(colado entre carriles)",
          r9_los_carriles_del_modulo(
              {"mm/vmm": {"roja.rs": CU + RI, "ayudas.rs": CU + RI}}))
    # El verde SI es un carril aqui, y en `critic/` no. No es una incoherencia:
    # son dos jurisdicciones con dos listas, y este caso lo deja fijado.
    exige("R9(el verde es carril de modulo)",
          r9_los_carriles_del_modulo({"obj/fb": {"verde.rs": CU + RI}}), False)
    exige("R9(mod.rs no lleva letrero)",
          r9_los_carriles_del_modulo({"obj/fb": {"mod.rs": ""}}), False)
    exige("R9(sin carpetas)", r9_los_carriles_del_modulo({}), False)

    # -- R10: el semaforo ---------------------------------------------------
    CA = "//! [carril]  ROJO      porque si" + chr(10)
    exige("R10(con color)", r10_el_semaforo({"plat/spin.rs": CA}), False)
    exige("R10(sin color)", r10_el_semaforo({"plat/spin.rs": "//! un fichero"}))
    exige("R10(color inventado)",
          r10_el_semaforo({"plat/spin.rs": "//! [carril]  AZUL x" + chr(10)}))
    # *** El que de verdad guarda algo: un `verde.rs` que dice ROJO. Es un
    # renombrado a medias, y es como una pieza cambia de color sin que nadie lo
    # decida -- justo lo contrario de para lo que existe un semaforo.
    exige("R10(el nombre y la etiqueta se contradicen)",
          r10_el_semaforo({"obj/fb/verde.rs": CA}))
    exige("R10(el nombre y la etiqueta coinciden)",
          r10_el_semaforo({"obj/fb/roja.rs": CA}), False)
    exige("R10(sin ficheros)", r10_el_semaforo({}), False)

    if fallos:
        for f in fallos:
            print("  [X] " + f)
        print("autoprueba: %d regla(s) no saben decir que NO" % len(fallos))
        return 1
    # ** El numero se CUENTA, no se escribe. La version anterior decia "21
    # casos" y habia 19: un guardian con una cifra a mano dentro es un guardian
    # que dice un numero viejo con toda la confianza del mundo.
    print("clean: las DIECISEIS reglas saben decir que NO (%d casos)" % casos[0])
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


# `//!` o `//`. Los dos ficheros de fuente los mete `texto.rs` con `include!`
# dentro de un `static`, o sea que su contenido es una EXPRESION y no admite
# documentacion de modulo. El letrero es obligatorio igual: lo que cambia es el
# vehiculo, no el deber.
RE_CARRIL = re.compile(r"^//!? \[carril\]\s+(\S+)", re.M)


def r10_el_semaforo(ficheros):
    """L6g -- **TODO fichero de Ring 0 lleva su color**. `{ruta: texto}`.

    Es el trabajo del 2026-08-31, y el dueno lo dijo mejor que la ley: *"es como
    poner titulos"*. No hay que partir 162 ficheros -- hay que **etiquetarlos**,
    para que el dia que haya que cambiar algo deprisa se sepa de un vistazo si
    se puede jugar o si hay que ir con las dos manos.

    Tres exigencias:

      1. declara `[carril]`. Sin excepciones y sin trinquete: un fichero NUEVO
         sin color es exactamente la sorpresa que esto viene a evitar.
      2. el color es uno de los tres. Inventarse un cuarto es volver a no tener
         semaforo.
      3. si el fichero SE LLAMA como un carril, el nombre y la etiqueta dicen lo
         mismo. Caza el renombrado a medias -- que es como una pieza cambia de
         color sin que nadie lo haya decidido.

    [!] Sin trinquete a proposito, al reves que L6a. Un trinquete tolera lo que
    ya estaba mal; aqui no hay nada que tolerar porque **se empieza en 162 de
    162**, y una regla que se cumple entera el primer dia no necesita suelo.
    """
    quejas = []
    for ruta in sorted(ficheros):
        m = RE_CARRIL.search(ficheros[ruta])
        if not m:
            quejas.append("%s no declara [carril]. Los colores son: %s (L6g)"
                          % (ruta, ", ".join(SEMAFORO)))
            continue
        color = m.group(1)
        if color not in SEMAFORO:
            quejas.append("%s dice [carril] %s, que no es un color. Son: %s (L6g)"
                          % (ruta, color, ", ".join(SEMAFORO)))
            continue
        tallo = ruta.rsplit("/", 1)[-1][:-3]
        debido = COLOR_DEL_NOMBRE.get(tallo)
        if debido and color != debido:
            quejas.append(
                "%s se llama `%s.rs` y declara [carril] %s. El nombre y la "
                "etiqueta tienen que decir lo mismo (L6g)" % (ruta, tallo, color))
    return quejas


def ficheros_de_ring0():
    """`{ruta: texto}` de todo `.rs` de Ring 0. El semaforo los cubre TODOS."""
    d = os.path.join(raiz(), RING0_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for dirpath, dirnames, filenames in os.walk(d):
        dirnames[:] = [x for x in dirnames if x not in ("target", ".git")]
        for n in sorted(filenames):
            if not n.endswith(".rs"):
                continue
            ruta = os.path.join(dirpath, n)
            with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                fuera[os.path.relpath(ruta, raiz()).replace(os.sep, "/")] = f.read()
    return fuera


def r9_los_carriles_del_modulo(carpetas):
    """L6g -- los carriles dentro del modulo. `{carpeta: {fichero: texto}}`.

    Dos exigencias, y las dos son de LETRERO, no de tamano:

      1. **una carpeta de carriles no mezcla.** Si hay un `roja.rs`, todo `.rs`
         de al lado (menos `mod.rs`) es un carril. Un `ayudas.rs` colado entre
         carriles es exactamente la aguja volviendo al pajar: una pieza sin
         semaforo en el sitio donde el semaforo es la razon de existir.
      2. **un carril declara lo que cuesta y por que falla** (L6e y L6f). El
         nombre del fichero dice el color; el `[cuesta]` dice a que atenerse.

    ** Lo que NO se exige aqui, y hay que decirlo: ni el tope de 300 lineas ni
    el `[prueba]`. Los dos son de `critic/`, que guarda JUECES -- piezas
    pequenas, puras y con banco. Un carril de modulo no es un juez: es la mitad
    de un fichero de Ring 0 que ya existia. `task/scheduler/roja.rs` son 744
    lineas de cambio de contexto y no puede ser otra cosa. Poner el tope de un
    juez a un carril seria pedirle a la ley que mienta.
    """
    quejas = []
    for carpeta in sorted(carpetas):
        for n in sorted(carpetas[carpeta]):
            if n == "mod.rs":
                continue
            tallo = n[:-3]
            if tallo not in VIAS_MODULO:
                quejas.append(
                    "%s/%s esta en una carpeta de carriles y no es uno. Los "
                    "carriles son: %s (L6g)"
                    % (carpeta, n, ", ".join(v + ".rs" for v in VIAS_MODULO))
                )
                continue
            txt = carpetas[carpeta][n]
            if not RE_CUESTA.search(txt):
                quejas.append("%s/%s es un carril y no declara [cuesta] (L6e)" % (carpeta, n))
            if not RE_RIESGO.search(txt):
                quejas.append("%s/%s es un carril y no declara [riesgo] (L6f)" % (carpeta, n))
    return quejas


def carpetas_de_carriles():
    """`{carpeta: {fichero: texto}}` de toda carpeta de Ring 0 con carriles.

    Una carpeta ES de carriles si tiene al menos un `.rs` con nombre de carril.
    No hay lista que mantener: **el arbol se declara solo**, que es lo que hace
    que partir un fichero manana ya venga vigilado sin tocar esto.

    [!] Ya no hay excepciones. La habia --`critic/`, que juzgaba R8 con reglas
    mas duras-- y desaparecio con la carpeta el 2026-08-31: un color solo
    significa algo dentro de un modulo.
    """
    d = os.path.join(raiz(), RING0_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for dirpath, dirnames, filenames in os.walk(d):
        dirnames[:] = [x for x in dirnames if x not in ("target", ".git")]
        rs = [n for n in filenames if n.endswith(".rs")]
        if not any(n[:-3] in VIAS_MODULO for n in rs):
            continue
        rel = os.path.relpath(dirpath, raiz()).replace(os.sep, "/")
        grupo = {}
        for n in sorted(rs):
            with open(os.path.join(dirpath, n), "r", encoding="utf-8",
                      errors="replace") as f:
                grupo[n] = f.read()
        fuera[rel] = grupo
    return fuera


def sellos_de_cabecera(txt):
    """`{etiqueta: [clases]}` de una cabecera de C. Lo que no este, no aparece.

    ** LAS CLASES SON LAS PALABRAS EN MAYUSCULAS QUE ABREN LA LINEA, y se
    reclaman TODAS antes de juzgar ninguna. La primera version tomaba palabras
    *mientras estuvieran en el vocabulario* y paraba en la primera que no --y
    con eso `[riesgo] AJENO RARO` colaba: `RARO` no era una clase mala, era
    "el motivo, que empieza por RARO". **La autoprueba lo cazo el dia que se
    escribio.**

    Es la trampa que L6f ya tenia contada en R7 con estas palabras: *un juez
    que solo mire la primera palabra deja pasar la mitad de cada linea*. La
    volvi a escribir igual.

    [!] El precio, dicho: un motivo que EMPIECE por una palabra en mayusculas
    se lee como una clase y R11 se queja. Es a proposito -- se falla hacia la
    queja y no hacia el permiso, y la salida es la que ya manda el formato:
    cuando hay mas de una clase, la linea es solo para ellas.
    """
    fuera = {}
    for etiqueta, resto in RE_SELLO_H.findall(txt):
        clases = []
        for w in resto.split():
            if not RE_MAYUSCULAS.match(w):
                break
            clases.append(w)
        if not clases and resto.split():
            clases = [resto.split()[0]]
        fuera[etiqueta] = clases
    return fuera


def cabeceras_de_rex():
    """`{ruta: texto}` de todo `.h` de `tables/bmo/`, carriles incluidos."""
    d = os.path.join(raiz(), REX_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for dirpath, dirnames, filenames in os.walk(d):
        for n in sorted(filenames):
            if not n.endswith(".h"):
                continue
            ruta = os.path.join(dirpath, n)
            with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                fuera[os.path.relpath(ruta, raiz()).replace(os.sep, "/")] = f.read()
    return fuera


def carpetas_de_carriles_rex():
    """`{carpeta: [ficheros]}` de las carpetas de carriles de REX.

    Una carpeta ES de carriles si tiene al menos un `.h` con nombre de carril.
    No hay lista que mantener: **el arbol se declara solo**, igual que en Ring
    0. Partir manana `entrada.h` ya viene vigilado sin tocar esto.
    """
    d = os.path.join(raiz(), REX_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for dirpath, dirnames, filenames in os.walk(d):
        hs = [n for n in filenames if n.endswith(".h")]
        if not any(n[:-2] in VIAS_MODULO for n in hs):
            continue
        rel = os.path.relpath(dirpath, raiz()).replace(os.sep, "/")
        fuera[rel] = sorted(hs)
    return fuera


def r11_el_semaforo_de_rex(ficheros):
    """L6g en REX -- toda cabecera de `<bmo/...>` lleva sus TRES etiquetas.

    Las mismas exigencias que R10 en Ring 0, y una cuarta que aqui muerde de
    verdad:

      1. declara `[carril]`, y el color es uno de los tres.
      2. si el fichero SE LLAMA como un carril, nombre y etiqueta coinciden --
         caza el renombrado a medias, que es como una pieza cambia de color sin
         que nadie lo decida.
      3. declara `[cuesta]` (L6e) y `[riesgo]` (L6f), con su vocabulario, y
         **la segunda clase se juzga igual que la primera**.
      4. ** UNA sola clase de `[cuesta]`. Es la regla de corte de L6e --*"un
         fichero cuya cabecera necesita declarar DOS clases esta mal
         cortado"*-- y las cuatro carpetas de `tables/bmo/` existen justamente
         porque su fichero necesitaba declarar dos.

    [!] Sin trinquete, igual que R10 y por el mismo motivo: se empieza cubriendo
    las 18 de 18, y una regla que se cumple entera el primer dia no necesita un
    suelo que tolere lo que ya estaba mal.
    """
    quejas = []
    for ruta in sorted(ficheros):
        sellos = sellos_de_cabecera(ficheros[ruta])
        n = ruta.rsplit("/", 1)[-1]
        for etiqueta in ("carril", "cuesta", "riesgo"):
            if etiqueta not in sellos:
                quejas.append("%s no declara [%s] (L6%s)"
                              % (ruta, etiqueta, {"carril": "g", "cuesta": "e",
                                                  "riesgo": "f"}[etiqueta]))
                continue
            for clase in sellos[etiqueta]:
                if clase not in VOCABULARIO[etiqueta]:
                    quejas.append(
                        "%s declara [%s] %s, que no esta en el vocabulario. "
                        "Son: %s" % (ruta, etiqueta, clase,
                                     ", ".join(VOCABULARIO[etiqueta])))
        if len(sellos.get("cuesta", [])) > 1:
            quejas.append(
                "%s declara DOS clases de [cuesta] (%s). Un fichero que las "
                "necesita esta mal cortado: el corte va por donde cambia el "
                "coste (L6e)" % (ruta, " ".join(sellos["cuesta"])))
        if len(sellos.get("carril", [])) > 1:
            quejas.append("%s declara DOS colores. Un fichero tiene UNO (L6g)" % ruta)
        debido = COLOR_DEL_NOMBRE.get(n[:-2])
        if debido and sellos.get("carril", [None])[0] != debido:
            quejas.append(
                "%s se llama `%s` y declara [carril] %s. El nombre y la "
                "etiqueta tienen que decir lo mismo (L6g)"
                % (ruta, n, " ".join(sellos.get("carril", ["nada"]))))
    return quejas


def r12_los_carriles_de_rex(carpetas, ficheros):
    """L6g -- una carpeta de carriles de REX no MEZCLA, y su fachada la trae.

    Tres exigencias, y la tercera es la que de verdad hace falta aqui:

      1. **todo `.h` de una carpeta de carriles ES un carril.** Un `ayudas.h`
         colado entre carriles es la aguja volviendo al pajar.
      2. **la fachada existe**: `bmo/<X>/` obliga a que haya `bmo/<X>.h`. Sin
         ella `#include <bmo/X.h>` deja de resolver, y eso cuesta PUERTA -- se
         lleva por delante fuentes que ya existen.
      3. ** **la fachada trae TODOS sus carriles.** Uno que se quede fuera no da
         un `fichero no encontrado`: da un simbolo no declarado a nueve capas de
         distancia. Es exactamente el fallo que `<bmo/bloque.h>` ya conto una
         vez, cuando `superficie.h` leia `__bmo_bloque_cap` sin traerlo y el
         error hablaba de un simbolo que el programa no habia escrito nunca.
    """
    quejas = []
    for carpeta in sorted(carpetas):
        tallo = carpeta.rsplit("/", 1)[-1]
        for n in carpetas[carpeta]:
            if n[:-2] not in VIAS_MODULO:
                quejas.append(
                    "%s/%s esta en una carpeta de carriles y no es uno. Los "
                    "carriles son: %s (L6g)"
                    % (carpeta, n, ", ".join(v + ".h" for v in VIAS_MODULO)))
        fachada = carpeta.rsplit("/", 1)[0] + "/" + tallo + ".h"
        if fachada not in ficheros:
            quejas.append(
                "hay carriles en %s/ y no hay fachada `%s`. Sin ella "
                "`#include <bmo/%s.h>` deja de resolver, y eso cuesta PUERTA (L6g)"
                % (carpeta, fachada, tallo))
            continue
        texto = ficheros[fachada]
        for n in carpetas[carpeta]:
            if n[:-2] not in VIAS_MODULO:
                continue
            if ("<bmo/%s/%s>" % (tallo, n)) not in texto:
                quejas.append(
                    "la fachada `%s` no trae `%s/%s`. Un carril fuera de la "
                    "fachada no da un fichero no encontrado: da un simbolo sin "
                    "declarar a nueve capas (L6g)" % (fachada, tallo, n))
    return quejas


def constantes_del_abi():
    """`{nombre: (valor, fichero)}` de la superficie de `bmo-abi`."""
    fuera = {}
    d = os.path.join(raiz(), SURFACE_ABI.replace("/", os.sep))
    if not os.path.isdir(d):
        return fuera
    for n in sorted(os.listdir(d)):
        if not n.endswith(".rs"):
            continue
        with open(os.path.join(d, n), "r", encoding="utf-8", errors="replace") as f:
            for nombre, valor in RE_CONST_RS.findall(f.read()):
                v = valor_exacto(valor)
                if v is not None:
                    fuera[nombre] = (v, n)
                else:
                    SIN_EVALUAR.append("%s (%s)" % (nombre, valor.strip()[:40]))
    return fuera


def constantes_de_rex():
    """`{nombre: (valor, fichero)}` de las cabeceras de REX."""
    fuera = {}
    d = os.path.join(raiz(), REX_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return fuera
    for dirpath, dirnames, filenames in os.walk(d):
        for n in sorted(filenames):
            if not n.endswith(".h"):
                continue
            ruta = os.path.join(dirpath, n)
            rel = os.path.relpath(ruta, d).replace(os.sep, "/")
            with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                for nombre, valor in RE_CONST_H.findall(f.read()):
                    v = valor_exacto(valor)
                    if v is not None:
                        fuera[nombre] = (v, rel)
                    else:
                        SIN_EVALUAR.append("%s (%s)" % (nombre, valor.strip()[:40]))
    return fuera


def parejas_de_rex(abi, rex):
    """`{nombre_abi: nombre_c}` -- lo que el mapa de familias sabe emparejar."""
    fuera = {}
    for na in sorted(abi):
        nc = REX_A_MANO.get(na)
        if nc is None:
            for pre_a, pre_c in REX_FAMILIAS:
                if na.startswith(pre_a):
                    cand = pre_c + na[len(pre_a):]
                    if cand in rex:
                        nc = cand
                    break
        if nc and nc in rex:
            fuera[na] = nc
    return fuera


def espejo_leer():
    """`{nombre_abi: {"c":.., "valor":.., "nota":..}}` de `REX_ESPEJO.txt`."""
    fuera = {}
    if not os.path.exists(ESPEJO):
        return fuera
    with open(ESPEJO, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            trozos = linea.split(None, 3)
            if len(trozos) < 3:
                continue
            fuera[trozos[1]] = {
                "c": trozos[2],
                "valor": int(trozos[0], 0),
                "nota": trozos[3] if len(trozos) > 3 else "",
            }
    return fuera


def espejo_escribir(abi, parejas, previa):
    """Reescribe `REX_ESPEJO.txt`. **Conserva la nota** de lo que no cambia.

    ** La nota es lo unico de este fichero que no puede regenerarse, porque es
    lo unico que no sale del arbol: dice POR QUE dos nombres distintos son la
    misma cosa. Perderla al resellar convertiria el trinquete en una lista de
    numeros -- y una lista de numeros no se puede revisar.

    Una pareja nueva entra con la nota vacia a proposito: asi el que sella ve
    en el diff exactamente lo que le toca escribir.
    """
    filas = []
    for na in sorted(parejas, key=lambda x: (abi[x][1], abi[x][0], x)):
        nota = previa.get(na, {}).get("nota", "")
        filas.append("0x%02X %-30s %-32s %s"
                     % (abi[na][0], na, parejas[na], nota))
    cabecera = ""
    if os.path.exists(ESPEJO):
        with open(ESPEJO, "r", encoding="utf-8") as f:
            for linea in f:
                if linea.strip() and not linea.startswith("#"):
                    break
                cabecera += linea
    with open(ESPEJO, "w", encoding="utf-8", newline="\n") as f:
        f.write(cabecera)
        f.write("\n".join(filas) + "\n")
    return len(filas)


def r13_el_espejo_de_rex(abi, rex, parejas, sellado):
    """R13 -- los numeros de REX dicen lo mismo que los del ABI.

    Cuatro exigencias, y la cuarta es la razon de que haya una TABLA en vez de
    una comparacion en vivo:

      1. una pareja SELLADA sigue existiendo en los dos lados. Si un nombre
         desaparece, o alguien lo renombra, se dice.
      2. las dos siguen valiendo lo que se sello.
      3. una pareja que el mapa encuentra y **nadie ha sellado** para el build.
         Es un gate de revision: un numero nuevo compartido lo mira una persona
         antes de que exista en dos sitios para siempre.
      4. ** una pareja que cambia **en los dos lados a la vez** tambien se caza.
         Una comparacion en vivo diria que coinciden --porque coinciden-- y se
         le escaparia justo el cambio que rompe todos los `.bex` ya firmados.
         Contra el numero SELLADO no hay forma de que pase.

    [!] Lo que R13 NO puede comprobar, y hay que decirlo: **que la pareja sea
    la correcta**. Que `TASK_OP_FRAMEBUFFER_CLAIM` y `BMO_OP_PANTALLA_RECLAMAR`
    sean la misma cosa lo dice una persona en la nota, y una herramienta que lo
    dedujera estaria adivinando. Es la misma frontera que ya tiene `--sellar`
    para la linea base de kinds.
    """
    quejas = []
    for na in sorted(sellado):
        fila = sellado[na]
        nc = fila["c"]
        if na not in abi:
            quejas.append(
                "%s esta sellado en el espejo y ya no existe en el ABI. Si se "
                "renombro, la fila se actualiza con --sellar (R13)" % na)
            continue
        if nc not in rex:
            quejas.append(
                "%s esta sellado contra %s, y %s ya no existe en REX (R13)"
                % (na, nc, nc))
            continue
        va = abi[na][0]
        vc = rex[nc][0]
        if va != fila["valor"] or vc != fila["valor"]:
            quejas.append(
                "el espejo sello %s = %s = 0x%02X, y hoy el ABI dice 0x%02X y "
                "REX dice 0x%02X. Cambiar un numero de la puerta rompe binarios "
                "que YA existen (R13)"
                % (na, nc, fila["valor"], va, vc))
    for na in sorted(parejas):
        if na not in sellado:
            quejas.append(
                "%s y %s son el mismo numero en dos sitios y nadie lo ha "
                "sellado. Miralo y sella con --sellar (R13)"
                % (na, parejas[na]))
    return quejas


def fuentes_de_apps():
    """`{ruta: texto}` de los `.c` que el proyecto publica como ejemplo."""
    d = os.path.join(raiz(), APPS_C_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for n in sorted(os.listdir(d)):
        if not n.endswith(".c"):
            continue
        with open(os.path.join(d, n), "r", encoding="utf-8", errors="replace") as f:
            fuera["%s/%s" % (APPS_C_DIR, n)] = f.read()
    return fuera


def r14_ninguna_app_inventa_un_numero(fuentes, rex):
    """R14 -- la OPERACION que cruza la puerta viene de REX, no del fichero.

    ** El pecado no es *"el nombre parece del kernel"* --eso seria adivinar-- es
    concreto y se ve: **el segundo argumento de `bmo_valor`/`bmo_codigo` es un
    macro que el propio fichero define como un numero**. Ese macro es una copia
    de un numero del kernel que nadie compara con el original.

    Es lo que paso de verdad, dos veces, en el mismo fichero:

        #define FB_BASE   0x01     REX no publicaba el framebuffer
        #define ENT_TECLA 0x03     y esta SI estaba en <bmo/entrada.h>,
                                   incluida nueve lineas mas arriba

    ** Un `#define` que apunta a un nombre de REX **no es pecado**: es un alias
    legible, y el numero sigue viniendo de un solo sitio.

    [!] Y un LITERAL desnudo no se juzga aqui, se INFORMA. `sonda_C.c` llama a
    `0x7777` y a `0xFFFFFFFF` a proposito -- su trabajo es comprobar que el
    kernel dice que no a lo que no existe. **Una regla que le grita a la sonda
    de seguridad por hacer su trabajo es una regla que se acaba apagando.**
    Devuelve `(quejas, notas)`.
    """
    quejas, notas = [], []
    for ruta in sorted(fuentes):
        txt = fuentes[ruta]
        propios = {}
        for nombre, cuerpo in RE_DEFINE_C.findall(txt):
            propios[nombre] = cuerpo.strip()
        for arg in RE_PUERTA_C.findall(txt):
            arg = arg.strip()
            if RE_SOLO_NUMERO.match(arg):
                notas.append(
                    "%s cruza la puerta con el literal %s. Si es una sonda esta "
                    "bien; si no, es una operacion que REX no publica (R14)"
                    % (ruta, arg))
                continue
            for ident in RE_IDENT.findall(arg):
                if ident not in propios:
                    continue
                cuerpo = propios[ident]
                # Un alias de un nombre de REX es legitimo: el numero sigue
                # viniendo de un sitio solo.
                if any(x in rex for x in RE_IDENT.findall(cuerpo)):
                    continue
                quejas.append(
                    "%s pasa `%s` como operacion, y lo define el mismo fichero "
                    "(`#define %s %s`). Un numero del kernel copiado en una app "
                    "es una copia que nadie compara con el original: usa el "
                    "nombre de REX (R14)" % (ruta, ident, ident, cuerpo))
    return quejas, notas


def r15_el_abi_no_repite_numero(abi):
    """R15 -- dos operaciones de la MISMA familia no pueden valer lo mismo.

    ** Este proyecto ya lo pago, y el kernel lo dejo escrito al elegir el opcode
    de `PANTALLA_SOLTAR`: *"0x1D elegido tras listar los opcodes ORDENADOS, que
    es la regla desde que `MEMORIA_PEDIR` se puso en `0x12` --ya ocupado por
    `REINICIAR`-- y pedir memoria habria reiniciado la maquina"*.

    R5 vigila eso en el KERNEL. Nadie lo vigilaba en el ABI -- que es la lista
    que lee quien escribe una app, y la que R13 acaba de sellar contra REX.

    Devuelve `(quejas, notas)`: un choque tolerado sale como nota con su motivo,
    para que la deuda se vea en cada build en vez de olvidarse.
    """
    quejas, notas = [], []
    familias = {}
    for nombre in sorted(abi):
        for pre in ABI_FAMILIAS_OP:
            if nombre.startswith(pre):
                familias.setdefault(pre, {}).setdefault(abi[nombre][0], []).append(nombre)
                break
    vivos = set()
    for pre in sorted(familias):
        for valor in sorted(familias[pre]):
            nombres = familias[pre][valor]
            if len(nombres) < 2:
                continue
            vivos.add((pre, valor))
            if (pre, valor) in ABI_CHOQUES_TOLERADOS:
                notas.append("en la familia %s la 0x%02X la comparten %s -- TOLERADO: %s"
                             % (pre, valor, " y ".join(nombres),
                                ABI_CHOQUES_TOLERADOS[(pre, valor)]))
                continue
            quejas.append(
                "en el ABI, %s valen las dos 0x%02X y son la misma familia de "
                "operaciones. Quien escriba una invocara la otra (R15)"
                % (" y ".join(nombres), valor))
    # ** Y una tolerancia que ya no hace falta TAMBIEN se dice. Una deuda
    # saldada que sigue escrita miente igual que una deuda oculta: la proxima
    # persona la lee y cree que el choque sigue ahi.
    for pre, valor in sorted(ABI_CHOQUES_TOLERADOS):
        if (pre, valor) not in vivos:
            quejas.append(
                "%s0x%02X esta en ABI_CHOQUES_TOLERADOS y ya NO choca. Borra "
                "esa linea: la lista solo puede encoger (R15)" % (pre, valor))
    return quejas, notas


def frontera_leer():
    """`[(prefijo, motivo)]` de `FRONTERA_REX.txt`. Lo que NO es de REX."""
    fuera = []
    if not os.path.exists(FRONTERA):
        return fuera
    with open(FRONTERA, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            trozos = linea.split(None, 1)
            fuera.append((trozos[0], trozos[1] if len(trozos) > 1 else ""))
    return fuera


def cobertura_de_rex(abi, sellado, frontera):
    """`(cubiertas, superficie)` -- cuanto del ABI que es DE APP tiene cabecera.

    ** El denominador NO son las constantes del ABI enteras: son las que quedan
    tras quitar la frontera. Un porcentaje contra el total contaria como
    "pendiente" cosas que nunca van a estar, y eso no es una medida, es una
    excusa que se ve bien.
    """
    prefijos = tuple(p for p, _ in frontera)
    superficie = [n for n in abi if not n.startswith(prefijos)]
    cubiertas = [n for n in superficie if n in sellado]
    return len(cubiertas), len(superficie)


def r16_la_cobertura_solo_sube(cubiertas, superficie, suelo):
    """R16 -- cuantas operaciones del contrato tienen funcion en REX.

    ** Convierte *"REX no tiene X"* de sensacion en cifra con trinquete. Es la
    respuesta a la pregunta del dueno --*"que reglas para que el ABI se
    aproveche TODO?"*-- y la respuesta honesta no es "se expone todo": es **el
    hueco es este numero, y no puede crecer**.

    Una sola exigencia, y es la del trinquete: **las cubiertas no bajan**. Que
    la superficie crezca no es un fallo --el ABI gana operaciones antes de que
    haya cabecera para ellas-- pero perder una cabecera que ya existia si lo es.

    [!] Y lo que R16 NO mide: si la cabecera es BUENA. Mide que exista. Una
    cabecera que compila y no hace lo que dice cuenta igual que una buena, y
    ninguna maquina va a distinguirlas.
    """
    quejas = []
    if cubiertas < suelo:
        quejas.append(
            "la cobertura de REX bajo de %d a %d constantes con cabecera. El "
            "trinquete solo sube: si una cabecera se ha retirado a proposito, "
            "resella con --sellar (R16)" % (suelo, cubiertas))
    return quejas


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
    r0 = ficheros_de_ring0()
    quejas += [("R8 L6g el juez nombrado existe", q) for q in r8_el_juez_nombrado_existe(r0)]
    vias = carpetas_de_carriles()
    quejas += [("R9 L6g los carriles del modulo", q)
               for q in r9_los_carriles_del_modulo(vias)]
    quejas += [("R10 L6g el semaforo de Ring 0", q) for q in r10_el_semaforo(r0)]
    rex = cabeceras_de_rex()
    quejas += [("R11 L6g el semaforo de REX", q) for q in r11_el_semaforo_de_rex(rex)]
    vias_rex = carpetas_de_carriles_rex()
    quejas += [("R12 L6g los carriles de REX", q)
               for q in r12_los_carriles_de_rex(vias_rex, rex)]
    abi_c = constantes_del_abi()
    rex_c = constantes_de_rex()
    par = parejas_de_rex(abi_c, rex_c)
    quejas += [("R13 el espejo de REX", q)
               for q in r13_el_espejo_de_rex(abi_c, rex_c, par, espejo_leer())]
    q14, n14 = r14_ninguna_app_inventa_un_numero(fuentes_de_apps(), rex_c)
    quejas += [("R14 una app inventa un numero del kernel", q) for q in q14]
    notas += n14
    q15, n15 = r15_el_abi_no_repite_numero(abi_c)
    quejas += [("R15 el ABI repite un numero", q) for q in q15]
    notas += n15
    frontera = frontera_leer()
    cub, sup = cobertura_de_rex(abi_c, espejo_leer(), frontera)
    quejas += [("R16 la cobertura de REX bajo", q)
               for q in r16_la_cobertura_solo_sube(cub, sup, _suelo(COBERTURA))]

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
    jueces = sorted({RE_PRUEBA.search(x).group(1) for x in r0.values()
                     if RE_PRUEBA.search(x)})
    if jueces:
        print("clean: %d fichero(s) nombran a su juez (L6g) -- %s"
              % (sum(1 for x in r0.values() if RE_PRUEBA.search(x)), ", ".join(jueces)))
    if vias:
        print("clean: %d carpeta(s) de carriles (L6g), %d carril(es), todos con letrero"
              % (len(vias), sum(len([n for n in g if n != "mod.rs"]) for g in vias.values())))
    if rex:
        cr = {c: 0 for c in SEMAFORO}
        for txt in rex.values():
            sel = sellos_de_cabecera(txt).get("carril", [])
            if sel and sel[0] in cr:
                cr[sel[0]] += 1
        print("clean: el semaforo cubre las %d cabeceras de REX -- %s"
              % (len(rex), "  ".join("%s %d" % (c, cr[c]) for c in SEMAFORO)))
    if vias_rex:
        print("clean: %d carpeta(s) de carriles en REX, y sus fachadas las traen enteras"
              % len(vias_rex))
    if par:
        print("clean: %d constante(s) escritas en REX y en el ABI dicen el mismo numero"
              % len(par))
    if sup:
        print("clean: REX cubre %d de las %d constantes del ABI que son DE APP "
              "(%d%%); la frontera deja fuera %d"
              % (cub, sup, (100 * cub) // sup, len(abi_c) - sup))
    if SIN_EVALUAR:
        print("  [i] %d constante(s) con un valor que este juez no evalua exacto, "
              "y por eso NO se emparejan: %s"
              % (len(SIN_EVALUAR), ", ".join(sorted(set(SIN_EVALUAR))[:6])))
    if r0:
        colores = {c: 0 for c in SEMAFORO}
        for txt in r0.values():
            m = RE_CARRIL.search(txt)
            if m and m.group(1) in colores:
                colores[m.group(1)] += 1
        print("clean: el semaforo cubre los %d ficheros de Ring 0 -- %s"
              % (len(r0), "  ".join("%s %d" % (c, colores[c]) for c in SEMAFORO)))
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
        abi_c = constantes_del_abi()
        rex_c = constantes_de_rex()
        n_esp = espejo_escribir(abi_c, parejas_de_rex(abi_c, rex_c), espejo_leer())
        print("sellado el espejo de REX: %d pareja(s) ABI <-> C" % n_esp)
        _fr = frontera_leer()
        _cub, _sup = cobertura_de_rex(abi_c, espejo_leer(), _fr)
        with open(COBERTURA, "w", encoding="utf-8", newline="\n") as f:
            f.write(CABECERA_COBERTURA)
            f.write("%d\n" % _cub)
            f.write("# de %d constantes del ABI que son de app; la frontera deja fuera %d\n"
                    % (_sup, len(abi_c) - _sup))
        print("sellada la cobertura de REX: %d de %d" % (_cub, _sup))
        print("sellado el suelo de L6e: %d fichero(s) declaran [cuesta]" % len(decl))
        print("sellado el suelo de L6f: %d fichero(s) declaran [riesgo]" % len(ries))
        print("sellada la linea base: %d numero(s) en las dos tablas" % n)
        print("[!] revisa las notas A MANO: una herramienta no sabe si KIND_ARCHIVO y File")
        print("    son el mismo objeto, ni si TASK_OP_FRAMEBUFFER_CLAIM es")
        print("    BMO_OP_PANTALLA_RECLAMAR. Una fila NUEVA sale con la nota vacia.")
        return 0
    return comprobar()


if __name__ == "__main__":
    sys.exit(main())

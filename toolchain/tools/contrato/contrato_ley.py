# -*- coding: utf-8 -*-
"""EL VOCABULARIO CERRADO de la ley, y donde vive cada cosa.

Aqui no hay ninguna regla: hay las listas contra las que las reglas juzgan
--`COSTES`, `RIESGOS`, `SEMAFORO`, `VIAS_MODULO`-- y las rutas de los ficheros
que se leen. Es lo unico que TODOS los guardianes comparten, y por eso esta
solo.

** Que sea cerrado es la mitad del valor: una clase inventada al vuelo hace que
dos ficheros que cuestan lo mismo lo digan de dos formas, y entonces la etiqueta
deja de poder compararse -- que era todo el punto.

** Este fichero salio de partir `contrato.py` el 2026-09-02, cuando L6a dijo
que pasaba de las 1.000 lineas. El guardian clasifico el fichero como CAJON y
prescribio el remedio: *"mecanico: mover texto, y demostrable byte a byte"*.

Eso es exactamente lo que se hizo -- **ni una linea de logica cambio**, y se
comprobo contra la salida de `--check` y `--autoprueba` de antes del corte.

[!] Y el corte NO se eligio por gusto: se midieron las masas. `autoprueba` eran
274 lineas, los guardianes de REX 676, y el vocabulario cerrado 114. Tres masas
con nombre, y el resto --el contrato kernel<->ABI y el mando-- se queda en
`contrato.py`.
"""
import os


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

# -- R17: la CARA RUST del ABI, que tampoco llevaba letrero -------------------
#
# `fundamentals/` son los tipos que cruzan la frontera: `BmoStatus` en rax/rdx,
# `BmoHandle` con su generacion, los bits de `BmoCap`. Un tercero que escriba
# Rust para BMO-X abre ESTE directorio, igual que quien escribe C abre `tables/`.
#
# ** Y estaba en la misma situacion que REX antes de R11: cubierto por ninguna
# regla, porque L6g decia *"todo `.rs` de Ring 0"* y esto no es Ring 0.
FUNDAMENTALS_DIR = "platform/abi/bmo-abi/src/fundamentals"

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



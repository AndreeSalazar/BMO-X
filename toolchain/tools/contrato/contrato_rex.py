# -*- coding: utf-8 -*-
"""LOS GUARDIANES DE REX -- la puerta de los terceros. R11 a R16.

`contrato.py` vigila que el kernel y el ABI digan lo mismo. Esto vigila la
TERCERA cara: las cabeceras `<bmo/...>` con las que se escribe una app, que
repiten en C los mismos numeros porque C no puede importar de Rust.

    R11  el semaforo de REX      cada cabecera dice que cuesta y que arrastro
    R12  los carriles            una carpeta de carriles no mezcla, y hay fachada
    R13  el espejo               los 98 numeros escritos dos veces coinciden
    R14  una app no inventa      la operacion sale de REX, no de un #define
    R15  el ABI no repite        dos ops de una familia no valen lo mismo
    R16  la cobertura            cuanto del ABI que es de app tiene cabecera

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
import re

from contrato_ley import *  # noqa: F401,F403 -- el vocabulario cerrado


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
    ("PRESTADO_OP_", "BMO_PRESTADO_"),
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



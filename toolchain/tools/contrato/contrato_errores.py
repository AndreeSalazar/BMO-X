# -*- coding: utf-8 -*-
"""R23 -- UN NUMERO, UN ERROR: ningun codigo significa dos cosas.

Pedida por el dueno el 2026-09-12: *"dale con la tabla de errores tambien, que
no se repitan; no romper todas las reglas, busca y redefinir, y otras si romper
si no entran"*.

Fichero propio, como R21 y R22, y por la misma razon escrita tres veces ya:
`contrato.py` toco las 1.000 lineas de L6a al enganchar R22.

# El hueco que tapa

L6i (R22) arreglo que una puerta contestara EXITO al negar. Y al arreglarlo
aparecio el mismo defecto **un piso mas arriba**: el NO llegaba, y llegaba con
un numero que significaba otra cosa.

```text
   ERROR_BUSY         valia 16, 21 y 22 segun quien lo dijera
   ERROR_NOT_THERE    valia 20, 26 y 28
   ERROR_NO_FREE_SLOT valia 24, 25 y 27
```

*** Y el que mordia: el `21` de un canal lleno --`endpoint::ERROR_BUSY`-- lo lee
Ring 3 como `ERROR_GATE`, o sea **"rechazado: la firma no cuadra"**. Un mensaje
que manda a mirar la firma de un `.bex` cuando lo que pasaba era que la cola
estaba llena. Decir que no y que se entienda OTRA COSA no es mejor que callarse.

# [!] LA DISTINCION QUE HACE QUE ESTA REGLA NO SEA UN MURO

Un numero en DOS ficheros no es un choque: casi siempre es una **pareja**, el
kernel y Ring 3 nombrando lo mismo. Hay once:

```text
   0x14 ERROR_NOT_THERE     ring0/task/launch.rs  +  userland/proceso.rs
   0x1c ERROR_ARCH_NO_ESTA  ring0/obj/file.rs     +  userland/proceso.rs
   ...
```

** Lo que se prohibe no es compartir numero: es **compartir numero con OTRO
NOMBRE**. Por eso la comprobacion es sobre el par `(numero, nombre)` y no sobre
el numero a secas -- y por eso el arreglo del 12-09 no movio ni un numero de
esas once: **les cambio el nombre en el kernel para que fuera el de Ring 3**,
que es el que se lee cuando algo falla.

# Redefinir y romper, que son cosas distintas

```text
   REDEFINIR   el numero se queda, cambia el nombre. No lo nota nadie:
               file.rs, directory.rs, console.rs -- 10 constantes
   ROMPER      el numero cambia. Solo donde NADIE de Ring 3 lo lee, y se
               midio antes: endpoint (20->18, 21->19) y mmio (7->12)
```

*** Esa medida es la regla entera: `launch` tambien tenia `ERROR_BUSY`, y NO se
movio, porque el DIRECTOR imprime sus tres codigos en la caja de Ejecutar.
**Se rompe donde no mira nadie; donde alguien mira, se redefine.**

La ley esta en L6j de `FUERO/META-KERNEL_HARD.md`.
"""

import os
import re

from contrato_ley import raiz

#: Donde se declaran codigos de error. Ring 0, el userland y lo que enlaza el
#: kernel: son los tres sitios desde los que un numero puede llegar a una app.
ARBOLES_DE_ERRORES = (
    "Ultra_kernel_x86-64/kernel/src",
    "Ultra_userspace/userland/src",
    "Ultra_userspace/services",
    "platform",
)

#: `pub const ERROR_X: u32 = 7;` y su gemelo en hexadecimal.
#:
#: [!] EL HEXADECIMAL NO ES UN DETALLE: el primer censo de este arbol se escribio
#: con `= (\d+)`, caso `0xE001` como si fuera un CERO, y dio por hecho que cuatro
#: errores de `memory.rs` valian lo mismo que EXITO. Eran 0xE001..0xE004 y
#: estaban bien. Un guardian que lee mal acusa a quien cumple.
RE_ERROR = re.compile(
    r"pub(?:\(crate\))?\s+const\s+(ERROR_\w+)\s*:\s*u32\s*=\s*(0[xX][0-9a-fA-F]+|\d+)")


def codigos_de_error(ficheros):
    """`[(numero, nombre, ruta)]` de cada constante declarada. `{ruta: texto}`."""
    fuera = []
    for ruta in sorted(ficheros):
        for m in RE_ERROR.finditer(ficheros[ruta]):
            crudo = m.group(2)
            valor = int(crudo, 16) if crudo.lower().startswith("0x") else int(crudo)
            fuera.append((valor, m.group(1), ruta))
    return fuera


def r23_un_numero_un_error(ficheros):
    """L6j -- **ningun numero significa dos cosas, ningun nombre vale dos numeros**.

    Dos deberes simetricos, y los dos son hechos comprobables, no opiniones:

      1. un NUMERO tiene un solo nombre. Si dos nombres comparten numero, el que
         lo reciba no puede saber cual le hablo.
      2. un NOMBRE tiene un solo numero. Si el mismo nombre vale tres cosas,
         buscarlo da tres respuestas y ninguna pista de cual es la tuya.

    ** SIN TRINQUETE, al reves que R22: el arbol quedo en CERO el dia que nacio
    la regla, asi que no hay deuda que tolerar. Una regla que se estrena limpia
    se puede permitir ser estricta -- y esta es justo la que el dueno pidio
    estricta.
    """
    codigos = codigos_de_error(ficheros)
    por_num, por_nom = {}, {}
    for valor, nombre, ruta in codigos:
        por_num.setdefault(valor, {}).setdefault(nombre, []).append(ruta)
        por_nom.setdefault(nombre, {}).setdefault(valor, []).append(ruta)

    quejas = []
    for valor in sorted(por_num):
        if len(por_num[valor]) > 1:
            partes = ["%s (%s)" % (n, ", ".join(sorted(
                os.path.basename(r) for r in por_num[valor][n])))
                for n in sorted(por_num[valor])]
            quejas.append(
                "el codigo %d significa DOS cosas: %s. O se unifica el nombre, o "
                "se mueve el que no lea nadie (L6j)" % (valor, " y ".join(partes)))
    for nombre in sorted(por_nom):
        if len(por_nom[nombre]) > 1:
            partes = ["%d en %s" % (v, ", ".join(sorted(
                os.path.basename(r) for r in por_nom[nombre][v])))
                for v in sorted(por_nom[nombre])]
            quejas.append(
                "%s vale %d numeros distintos: %s. Un nombre que vale tres cosas "
                "manda a buscar a tres sitios (L6j)"
                % (nombre, len(por_nom[nombre]), "; ".join(partes)))
    return quejas


def las_parejas(ficheros):
    """**La nota de cada build**: cuantos codigos hay y cuantos son PAREJA.

    Una pareja es un numero declarado con el MISMO nombre en el kernel y en el
    userland -- o sea, los dos lados del contrato diciendo lo mismo. Es la mitad
    buena de la cuenta y por eso se ensena: sin ella, "31 codigos" no dice si el
    contrato esta emparejado o si cada lado va por su cuenta.
    """
    codigos = codigos_de_error(ficheros)
    if not codigos:
        return []
    por_par = {}
    for valor, nombre, ruta in codigos:
        por_par.setdefault((valor, nombre), set()).add(ruta)
    parejas = sum(1 for k in por_par if len(por_par[k]) > 1)
    return ["R23 L6j: %d codigo(s) de error, %d numero(s) distintos, "
            "%d pareja(s) kernel<->Ring 3 -- ninguno con doble significado"
            % (len(codigos), len(set(v for v, _, _ in codigos)), parejas)]


def comprobar_errores(ficheros=None):
    """Lo que `comprobar()` necesita de R23 en UNA llamada: `(quejas, notas)`."""
    ficheros = ficheros_con_errores() if ficheros is None else ficheros
    quejas = [("R23 L6j un numero, un error", q)
              for q in r23_un_numero_un_error(ficheros)]
    nota = las_parejas(ficheros)
    return quejas, (["\n".join(nota)] if nota else [])


def ficheros_con_errores():
    """`{ruta: texto}` de todo `.rs` de los arboles que declaran codigos."""
    fuera = {}
    for arbol in ARBOLES_DE_ERRORES:
        d = os.path.join(raiz(), arbol.replace("/", os.sep))
        if not os.path.isdir(d):
            continue
        for dp, dn, fn in os.walk(d):
            dn[:] = [x for x in dn if x not in ("target", ".git")]
            for n in sorted(fn):
                if not n.endswith(".rs"):
                    continue
                ruta = os.path.join(dp, n)
                with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                    fuera[os.path.relpath(ruta, raiz()).replace(os.sep, "/")] = f.read()
    return fuera

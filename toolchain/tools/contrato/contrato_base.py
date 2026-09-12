# -*- coding: utf-8 -*-
"""La LINEA BASE del contrato, y los suelos de las reglas que solo pueden subir.

Salio de `contrato.py` el 2026-09-12, y **el motivo es que la casa ya lo habia
predicho tres veces**. Su cabecera lleva escrito, desde que nacio R21:

    *"cada guardian nuevo lo engorda, asi que o se parte o esta lista tendra una
    entrada por guardian"*

R21 se fue a su fichero por eso. R22 tambien. Y al enganchar R23 el fichero
toco **1.002 lineas de codigo**, o sea que la prediccion dejo de serlo por
cuarta vez. Esta vez no se afeitan dos lineas: se parte.

# Que es esto, y por que es UNA cosa

Todo lo que sigue contesta la misma pregunta -- **cual era el suelo, y se puede
bajar de el?**:

```text
   LINEA_BASE.txt   los numeros que usan las DOS tablas de kinds (R2)
   CUESTAS.txt      cuantos ficheros declaran `[cuesta]` (L6e)
   RIESGOS.txt      cuantos declaran `[riesgo]` (L6f)
   COBERTURA.txt    cuantas constantes del ABI tienen cabecera en REX (R16)
```

Cuatro trinquetes, cuatro ficheros, una sola idea: **lo que se gano no se
devuelve**. Vivian mezclados con las reglas que los usan; aqui viven juntos, que
es donde se ve que son la misma maquina cuatro veces.

[!] Y `_parecen_lo_mismo` sigue siendo lo que era: una HEURISTICA para PROPONER
al sellar, jamas para juzgar. Esta aqui y no al lado de R2 justamente para que
no se confunda con un juez.

** Se movio TEXTO, no logica: ni una linea cambio de contenido.
"""

import os

from contrato_ley import BASE, CUESTAS, como_numero, raiz  # noqa: F401

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
# ** El denominador sale de `VALKYRIE-ABI/FRONTERA.txt`, no del ABI entero. Un
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


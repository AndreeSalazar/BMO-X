"""R20 -- EL SEMAFORO DE LOS DRIVERS, que SI son Ring 0.

Vive en su propio fichero **y no dentro de `contrato.py`**, por dos motivos y
el segundo es el que manda:

  1. `contrato_ley.py` y `contrato_rex.py` ya lo hacian. Una regla nueva que se
     mete en el monolito porque "es una mas" es como el monolito se hizo.
  2. *** AL ANADIR R20 AHI DENTRO, `contrato.py` CRUZO LAS 1.000 LINEAS y L6a
     lo caza en el mismo build. O sea que la ley de esta casa dijo que no a la
     forma de aplicar otra ley de esta casa, **el mismo dia**.

    > Un guardian que crece cada vez que se le anade una regla acaba siendo el
    > modulo mas grande del arbol. Y entonces nadie lo toca.

La regla y el porque estan en `r20_el_semaforo_de_los_drivers`.
"""

import os
import re

from contrato_ley import DRIVERS_DIR, DRIVERS_TXT, SEMAFORO


def _raiz():
    """La raiz del repo: cuatro niveles por encima de este fichero.

    Se calcula aqui y no se importa de `contrato.py` a proposito: importarla
     seria una dependencia circular --`contrato.py` importa esto--, y ademas
    es lo que permite correr este modulo solo.
    """
    d = os.path.dirname(os.path.abspath(__file__))
    return os.path.abspath(os.path.join(d, "..", "..", ".."))


# ** La MISMA expresion que `contrato.py`, y esta duplicada A SABIENDAS.
#
# Importarla de alli seria una dependencia circular --`contrato.py` importa
# este modulo-- y sacarla a `contrato_ley.py` obligaria a mover tambien las
# otras cuatro. Dos lineas iguales que se pueden leer de un vistazo son mejor
# negocio que una mudanza que nadie pidio; el dia que haya una tercera copia,
# ese dia si.
RE_CARRIL = re.compile(r"^//!? \[carril\]\s+(\S+)", re.M)

def ficheros_de_drivers():
    """`{ruta: texto}` de todo `.rs` de `platform/drivers`, menos sus pruebas.

    El banco se salta por lo mismo que en `fases.py`: son las pruebas, no las
    piezas. Una prueba que falla aparece siempre en el mismo sitio.
    """
    d = os.path.join(_raiz(), DRIVERS_DIR.replace("/", os.sep))
    if not os.path.isdir(d):
        return {}
    fuera = {}
    for dirpath, dirnames, filenames in os.walk(d):
        dirnames[:] = [x for x in dirnames if x not in ("target", "tests", ".git")]
        for n in sorted(filenames):
            if not n.endswith(".rs") or n in ("pruebas.rs", "tests.rs"):
                continue
            ruta = os.path.join(dirpath, n)
            with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                fuera[os.path.relpath(ruta, _raiz()).replace(os.sep, "/")] = f.read()
    return fuera


def minimo_de_drivers():
    """El suelo del trinquete. Ausente = 0, igual que los demas."""
    if not os.path.exists(DRIVERS_TXT):
        return 0
    with open(DRIVERS_TXT, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if linea and not linea.startswith("#"):
                try:
                    return int(linea.split()[0])
                except ValueError:
                    return 0
    return 0


def r20_el_semaforo_de_los_drivers(ficheros, suelo):
    """R20 -- **los drivers TAMBIEN son Ring 0**, y no los cubria nadie.

    R11 lo dijo de REX y R17 de `fundamentals/`, las dos con las mismas
    palabras: *"no lo cubria ninguna regla porque esto no es Ring 0"*. Aqui esa
    frase es **falsa**: `platform/drivers` son catorce crates que el kernel
    ENLAZA y que ejecutan con privilegio de Ring 0. Lo unico que no es de Ring
    0 es su carpeta.

    *** Y es donde vive el DMA: los catorce sitios de `NEUTRO/DMA/EMBUDO.txt`
    --los que le dan una fisica a un aparato-- estan todos aqui dentro. El
    codigo que habla con los maestros del bus era el menos senalizado del
    arbol, con Ring 0 al 100%.

      > Un semaforo cuyo alcance es una CARPETA y no un PRIVILEGIO deja fuera
      > justo lo que se mudo de carpeta.

    [!] CON TRINQUETE, y es la diferencia con R10, R11 y R17. Las tres dicen
    *"sin trinquete: se empieza cubriendolos todos"* y podian decirlo porque no
    habia nada que tolerar. Aqui hay 19.000 lineas ya escritas, y un build rojo
    que hoy no se puede poner verde se acaba desactivando.

    Dos exigencias, y la segunda es la que hace util a la primera:

      1. el que declare `[carril]` que declare un COLOR de los tres. Un cuarto
         color es volver a no tener semaforo.
      2. **el numero no baja.** Un fichero que pierde su letrero es un sitio
         donde vuelve a no saberse que se arrastra al tocarlo.
    """
    quejas = []
    marcados = 0
    for ruta in sorted(ficheros):
        m = RE_CARRIL.search(ficheros[ruta])
        if not m:
            continue
        marcados += 1
        if m.group(1) not in SEMAFORO:
            quejas.append("%s dice [carril] %s, que no es un color. Son: %s (R20)"
                          % (ruta, m.group(1), ", ".join(SEMAFORO)))
    if marcados < suelo:
        quejas.append(
            "los drivers con [carril] BAJARON: %d, y el suelo es %d. Un driver "
            "que pierde su letrero es codigo de Ring 0 sin semaforo, y ahi "
            "dentro esta el DMA (R20)" % (marcados, suelo))
    return quejas

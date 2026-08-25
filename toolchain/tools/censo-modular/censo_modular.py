#!/usr/bin/env python3
"""censo_modular -- the meter for L6a, and a ratchet so the tree can only improve.

Why this exists
===============

MODULAR is the house rule. It is written as L6 in `META-KERNEL_HARD.md`, it has
four obligations, and L6a is a number: **a module over ~1.000 lines is split.
It is not a suggestion.**

Nothing measured it.

That is not a small gap, because a rule with a number and no meter behaves
exactly like a rule without a number. The proof is in the tree's own history:
`gui/main.rs` **grew 1.244 lines between 08-04 and 08-12** while a written plan
to split it already existed. Nobody decided that. It happened one commit at a
time, and no step was big enough to be worth stopping for.

    L4 of META-KERNEL_HARD: a rule is proven by saying NO.

So this tool exists to say NO on the day it happens, not three weeks later.

The ratchet, and why it is not a wall
=====================================

Nineteen files break L6a today. A guardian that fails on all nineteen from the
first run gets switched off within a day, and then it protects nothing -- the
same reasoning `enlaces.py` writes down about false alarms.

So the guardian does not judge the past. It judges the DELTA against a sealed
baseline (`LINEA_BASE.txt`), and it only ever says NO to two things:

    NUEVO     a file crosses 1.000 lines and was not on the list
    CRECIO    a file already on the list got bigger

Everything else is good news: a file that shrinks, or drops off the list, is
reported and the baseline is asked to be re-sealed. **The tree can only get
better, one commit at a time** -- which is the property that makes it safe to
change many things at once without them colliding in the same 3.000-line file.

The two species, because only one can be split without design
=============================================================

Lines alone do not say what a split costs. Lines per function does, and it
separates two different animals (measured across the tree on 2026-08-13):

    media ~30    a DRAWER: many small functions in a big file. Splitting is
                 moving text -- mechanical, and provable byte for byte (L6d)
    media 150+   ONE GIANT FUNCTION: the shared local state has to become a
                 struct first. That is a design change and a hash cannot prove
                 it. Another day, and another method.

The count is only emitted for languages where a function has an unambiguous
opening token (`fn`, `def`, `function`). For the rest the column says `-`,
because inventing a number here would be worse than not having one.

How this file is built, and it is on purpose
============================================

L7 says the generations are `abuelo -> padre -> hijo -> nieto` and that
**knowledge only goes down**. The meter for L7 is built by L7:

    abuelo   `medir`     lines and functions of ONE file. Does not know what big means
    padre    `Ficha`     names that file. Does not know there are others
    hijo     `especie`   relates two numbers of the padre. Does not judge them
    nieto    `juicio`    the verdict and the exit code. The only one with an opinion

Usage
=====

    py censo_modular.py               the report
    py censo_modular.py --check       exit 1 if something is new or grew
    py censo_modular.py --sellar      re-record the baseline as it is today
    py censo_modular.py --sellar --motivo "..."   ... and a ceiling may go UP

A ceiling that goes up needs a written reason, refused without one, and the
reason is kept forever in the `[SUBIDAS]` section.

A RENAME is the known gap: the file arrives as NUEVO at its new path and the
old ceiling leaves the list, so nothing looks like a rise and `--sellar` does
not ask. The tree does not get worse -- same file, same size, new path -- but
the move is only recorded in the commit. Closing it means matching by content,
and that is a bigger tool than this one. That rule is the owner's,
and it is the whole of it: **everything has its why; what has none is removed
and replaced.** It was added on 2026-08-19 after a ceiling was re-sealed with
its reason living only in a commit message -- which is where nobody looks.
"""

import argparse
import io
import os
import re
import subprocess
import sys

import herencia

# -- The number L6a puts, and where it comes from ------------------------------
#
# 1.000 lines is not a taste: it is the figure written in L6a of
# `META-KERNEL_HARD.md`. It lives here as one constant so that the day it
# changes, it changes in one place and the baseline is re-sealed against it.
LIMITE = 1000

# Below this a file is not reported at all. It exists so the report shows what
# is CLOSE to the limit -- a file at 980 lines is the next problem, and seeing
# it before it crosses is the whole point of a ratchet.
AVISO = 900

EXTS = ('.rs', '.py', '.c', '.h', '.cob', '.ps1')

# Where a function starts, per language. A language that is not here gets no
# count and says so with a `-`.
APERTURA = {
    '.rs': re.compile(
        r'^\s*(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?(?:unsafe\s+)?'
        r'(?:extern\s+"[^"]*"\s+)?fn\s', re.M),
    '.py': re.compile(r'^\s*def\s', re.M),
    '.ps1': re.compile(r'^\s*function\s', re.M | re.I),
}

BASE = os.path.join(os.path.dirname(os.path.abspath(__file__)), 'LINEA_BASE.txt')



# == ABUELO: EL ANILLO =========================================================
#
# *** POR QUE EL LIMITE NO PUEDE SER EL MISMO PARA TODO (2026-08-24)
#
# Regla de Eddi: *"el guardian limita estrictamente hasta mil, y por que? porque
# hablamos de Bare Metal Orquestal. Pero si es para Ring 3 como library OS,
# okey."*
#
# Y tiene el motivo dentro: **lo que cuesta un fallo depende del anillo.**
#
#     Ring 0    un fallo se lleva la MAQUINA. Y ahi conviven 236 `static mut`:
#               en un fichero grande, cada funcion puede tocarlos todos
#     Ring 3    un fallo mata la TAREA. El kernel recupera la pantalla y
#               imprime sus ultimas cuatro lineas -- verificado en metal
#     util      no corre en la maquina. Un compilador con un fichero grande
#               produce programas malos, que es caro; no cuelga un arranque
#
# == [!] Y AQUI ESTA LA TRAMPA QUE HAY QUE VER ANTES DE ESCRIBIR LA REGLA ======
#
# **El anillo NO se puede decidir por la carpeta.** Se midio el 2026-08-24:
#
#     platform/drivers/storage/fat32/src/lib.rs   2.537 lineas   RING 0
#     platform/drivers/usb/xhci/src/lib.rs        1.584 lineas   RING 0
#
# Los dos viven bajo `platform/`, los dos PARECEN Ring 3, y los dos son crates
# de Rust que **enlaza el kernel**. Una regla que dijera *"platform = Ring 3 =
# mas laxo"* habria relajado el limite justo sobre los dos ficheros mas grandes
# que corren en Ring 0. Lo contrario de lo que se pedia.
#
# *** Asi que el anillo sale del GRAFO DE DEPENDENCIAS DEL KERNEL, que es un
# hecho y no una convencion: si el kernel lo enlaza, corre en Ring 0. El dia que
# un driver se mude a Ring 3 de verdad --como dice la intencion de `rdna4`-- su
# `Cargo.toml` deja de estar en ese grafo y esta funcion se entera sola.

RING0, RING3, UTIL = 'ring0', 'ring3', 'util'

_CRATES_R0 = None


def _crates_del_kernel(raiz):
    """Los crates que el kernel enlaza, transitivamente. Se calcula una vez."""
    global _CRATES_R0
    if _CRATES_R0 is not None:
        return _CRATES_R0
    vistos, pend = set(), [os.path.join(raiz, 'Ultra_kernel_x86-64', 'kernel')]
    while pend:
        d = os.path.normpath(pend.pop())
        clave = os.path.relpath(d, raiz).replace(os.sep, '/')
        if clave in vistos:
            continue
        vistos.add(clave)
        cargo = os.path.join(d, 'Cargo.toml')
        if not os.path.exists(cargo):
            continue
        try:
            t = io.open(cargo, encoding='utf-8', errors='replace').read()
        except OSError:
            continue
        for p in re.findall(r'path\s*=\s*"([^"]+)"', t):
            pend.append(os.path.join(d, p))
    _CRATES_R0 = vistos
    return vistos


def anillo(ruta, raiz='.'):
    """Donde CORRE este fichero. Un hecho, no una opinion sobre el.

    ** No mira la carpeta salvo para lo que no es un crate: mira si el kernel
    enlaza el crate al que pertenece. Ver la cabecera de esta seccion.
    """
    r = ruta.replace(os.sep, '/')
    # ** Un script NUNCA corre en la maquina, viva donde viva. BMO-X no tiene
    # PowerShell ni Python, asi que `build.ps1` --que esta dentro de
    # `Ultra_kernel_x86-64/`-- es herramienta y no Ring 0. Es un hecho sobre el
    # lenguaje, no una convencion sobre la carpeta.
    #
    # [!] Se descubrio al estrenar esta funcion: el censo conto 7 ficheros de
    # Ring 0 y uno era `build.ps1`. Una clasificacion que se equivoca en el
    # primer informe no habria durado dos dias.
    if r.endswith('.ps1') or r.endswith('.py'):
        return UTIL
    if r.startswith('toolchain/'):
        return UTIL
    for c in _crates_del_kernel(raiz):
        if r.startswith(c + '/'):
            return RING0
    if r.startswith('Ultra_kernel_x86-64/'):
        return RING0
    return RING3


# == ABUELO ====================================================================
# The raw fact: how many lines and how many functions this file has. It does not
# know what a limit is, it does not know there are other files, and it has no
# opinion about the numbers it returns.

def solo_codigo(texto):
    """**Cuantas lineas de este fichero son CODIGO.** Ni comentario ni blanco.

    *** POR QUE L6a PASO A MEDIR ESTO Y NO EL TOTAL (2026-08-24)
    ===========================================================

    Lo cazo el dueno, y con razon. El 24-08 se le anadio a `syscall/mod.rs` una
    cabecera explicando **por que** se habia repartido, y el fichero cruzo las
    mil lineas y salto la regla. Al medirlo:

        977 lineas totales  ->  530 de CODIGO,  423 de DOCUMENTACION (43%)

    **Su codigo era la mitad del limite. Lo que lo empujo fue la explicacion.**

    *** Y eso hacia que el metro empujara contra lo que este proyecto mas
    valora. La regla de la casa es *"todo tiene su por que; lo que no lo tiene,
    se quita"*, y este arbol es **36% documentacion medida**. Un guardian que
    cuenta el por que como si fuera riesgo le pone precio a escribirlo -- y el
    dia que alguien tenga prisa, lo barato sera borrar el comentario.

    ## Y lo que L6a existe para manejar es CODIGO, no paginas

    El peligro de un fichero grande es el ESTADO que sus funciones comparten y
    las interacciones que esconde. Un comentario no comparte estado con nadie:
    **hace el fichero mas facil de auditar, no mas dificil.**

    ## [!] Y el cambio NO es un blanqueo, y se puede comprobar

    Medido sobre la linea base del dia que se cambio, salen CUATRO ficheros y se
    quedan los gordos de verdad:

    ```text
        SALEN      emu/mod.rs 1761 -> 974      reports.rs 1345 -> 937
                   preprocessor 1196 -> 800    declarations 1001 -> 599
        SE QUEDAN  cobol/codegen 2948 -> 1869  cobol/parser 2011 -> 1565
                   c/codegen 2321 -> 1398      cpp/parser 1621 -> 1177
                   validator 1435 -> 1179
    ```

    Los que se quedan son los que tienen codigo de verdad. Si contar codigo
    hubiera vaciado la lista, el cambio seria sospechoso; que deje dentro a los
    cinco mas grandes es lo que dice que mide lo que dice medir.

    ** El TOTAL se sigue enseNando al lado, para que nadie tenga que creerse
    esta cuenta: las dos columnas estan en el informe.
    """
    codigo = 0
    en_bloque = False
    for l in texto.split('\n'):
        t = l.strip()
        if not t:
            continue
        if en_bloque:
            if '*/' in t:
                en_bloque = False
            continue
        if t.startswith('//'):
            continue
        if t.startswith('/*'):
            if '*/' not in t:
                en_bloque = True
            continue
        codigo += 1
    return codigo


def mayor_funcion(texto, ruta):
    """**Cuanto ocupa la funcion MAS GRANDE.** `None` si no se sabe contar.

    *** POR QUE ESTA MEDIDA HACIA FALTA, y lo enseno un fichero de verdad
    ==================================================================

    El 2026-08-24 el censo llamaba CAJON a `task/proc.rs` --media 58 lineas por
    funcion-- y eso decia *"se parte moviendo texto"*. Al abrirlo:

        `admit_payload_desde` era UNA FUNCION DE 607 LINEAS

    **Diecinueve funciones pequenas y un monstruo dan la misma media que veinte
    medianas.** La media es un promedio, y un promedio esconde exactamente lo
    que este censo existe para separar: un cajon se parte mecanicamente y un
    monstruo pide un cambio de diseNo.

    ** La medida es una APROXIMACION y se dice: se toma la distancia entre dos
    aperturas de funcion consecutivas. Un `fn` anidado la partiria en dos y daria
    de menos -- nunca de mas, que es el lado seguro para una alarma. Contar
    llaves de verdad pediria un parser, y un parser por lenguaje es otro
    proyecto; esta cuenta ya distingue 607 de 58, que es lo que hacia falta.
    """
    patron = APERTURA.get(os.path.splitext(ruta)[1])
    if patron is None:
        return None
    # La linea de cada apertura, y el final del fichero como ultimo corte.
    cortes = [texto.count('\n', 0, m.start()) for m in patron.finditer(texto)]
    if not cortes:
        return None
    cortes.append(texto.count('\n'))
    pares = list(zip(cortes, cortes[1:]))
    mayor, (ini, fin) = max(((b - a, (a, b)) for a, b in pares), key=lambda x: x[0])

    # *** Y CUANTO ESTADO COMPARTE, que es lo que decide si se puede partir.
    #
    # ** Lo enseno `syscall/mod.rs` el 2026-08-24. Tenia una funcion de 795
    # lineas y este censo la marcaba `CON MONSTRUO` -- *"partirla es diseno, no
    # tijeras"*. Al medirla:
    #
    #     locales a nivel del cuerpo   0
    #     estado compartido            los tres parametros, y nada mas
    #
    # **No era un monstruo: era un DESPACHADOR con cuarenta y cinco brazos
    # independientes**, y cada brazo era una funcion esperando nombre. Se partio
    # moviendo texto en una tarde.
    #
    # *** La media dijo `mixto`, el tamano dijo `CON MONSTRUO`, y los dos se
    # equivocaron igual: **midieron lo GRANDE que es la funcion y no lo que
    # decide si se puede partir, que es su ESTADO.** El propio doc de este
    # fichero lo llevaba escrito --*"el estado local compartido tiene que
    # volverse un struct primero"*-- y nadie lo media.
    lineas = texto.split('\n')[ini:fin]
    estado = sum(1 for l in lineas
                 if l.startswith('    let ') or l.startswith('    static '))
    return mayor, estado


def medir(ruta):
    """(lineas, funciones, generado) de UN fichero. `funciones` None si no se sabe."""
    try:
        s = io.open(ruta, encoding='utf-8', errors='replace').read()
    except OSError:
        return None
    lineas = s.count('\n') + 1
    patron = APERTURA.get(os.path.splitext(ruta)[1])
    # La marca que ya usa el arbol para lo que no escribio una persona
    # (`cobol-gen`, `c-gen`). Se busca solo en la cabecera: un fichero que la
    # mencione a mitad esta hablando de otro, no declarandose.
    generado = 'AUTO-GENERADO' in s[:400] or 'AUTO-GENERATED' in s[:400]
    m = mayor_funcion(s, ruta)
    return (lineas, solo_codigo(s), (len(patron.findall(s)) if patron else None),
            generado, m[0] if m else None, m[1] if m else None)


# == PADRE =====================================================================
# Names the fact: this measurement belongs to this path. It does not know that
# other Fichas exist, which is why nothing here compares or sorts.

class Ficha:
    def __init__(self, ruta, lineas, codigo, funciones, generado=False, mayor=None, estado=None):
        # *** LO QUE L6a MIDE DESDE EL 2026-08-24: lineas de CODIGO, ni
        # comentario ni blanco. Ver `solo_codigo` para el por que y para la
        # comprobacion de que el cambio no es un blanqueo.
        self.codigo = codigo
        # **Cuanto estado comparte la funcion mas grande.** Es lo que decide si
        # se parte moviendo texto o si pide un struct antes. Ver `mayor_funcion`.
        self.estado = estado
        # **La funcion mas grande.** Ver `mayor_funcion`: la media esconde un
        # monstruo entre pequenas, y este censo existe para separarlos.
        self.mayor = mayor
        # ** El anillo se guarda en la ficha y no se pregunta cada vez: es un
        # hecho sobre el fichero, igual que sus lineas.
        self.anillo = anillo(ruta)
        self.ruta = ruta
        self.lineas = lineas
        self.funciones = funciones
        self.generado = generado

    @property
    def media(self):
        """Lineas por funcion, o None cuando no hay cuenta de funciones."""
        if not self.funciones:
            return None
        return self.lineas // self.funciones


# == HIJO ======================================================================
# Relates two numbers of the padre -- lines against functions -- and gives the
# relation a name. It does NOT say whether that is good or bad: a drawer of
# 2.900 lines and a drawer of 300 get the same word here.

# Solo donde un fichero ES un modulo de funciones. Un guion de PowerShell son
# ordenes sueltas de arriba abajo, asi que dividir sus lineas entre sus
# `function` daria una media que no significa nada -- y una media que no
# significa nada es peor que una columna vacia.
CON_ESPECIE = ('.rs', '.py')


def especie(ficha):
    if not ficha.ruta.endswith(CON_ESPECIE):
        return 'desconocida'
    # Cero funciones en un lenguaje que SI se sabe contar no es ignorancia: es
    # un fichero de datos. Decir "desconocida" ahi seria echarle la culpa al
    # metro de algo que el metro sabe perfectamente.
    if ficha.funciones == 0:
        return 'TABLA'
    # *** EL MONSTRUO SE MIRA ANTES QUE LA MEDIA (2026-08-24).
    #
    # ** Una funcion que se lleva mas de un tercio del fichero manda sobre el
    # promedio, y no al reves: `task/proc.rs` tenia media 58 --CAJON, "se parte
    # moviendo texto"-- y dentro una funcion de 607 lineas. Diecinueve pequenas
    # y un monstruo dan la misma media que veinte medianas.
    #
    # El umbral es UN TERCIO y no la mitad porque lo que se quiere cazar no es
    # "el fichero ES una funcion" --eso ya lo dice la media-- sino **"hay una
    # que no se va a poder mover sin diseNo"**.
    if ficha.mayor and ficha.codigo and ficha.mayor * 3 > ficha.codigo:
        # *** Y AQUI SE MIRA EL ESTADO, no el tamano. Una funcion enorme que no
        # declara casi nada no es un monstruo: es un DESPACHADOR, y se parte
        # moviendo texto. Ver `mayor_funcion`.
        #
        # El umbral son CINCO locales, y sale de los dos casos medidos:
        # `syscall::invoke_current_task` tenia 0 y se partio en una tarde;
        # `task::admit_payload_desde` tiene decenas y sigue entera.
        if ficha.estado is not None and ficha.estado <= 5:
            return 'DESPACHADOR'
        return 'CON MONSTRUO'
    m = ficha.media
    if m is None:
        return 'desconocida'
    if m < 60:
        return 'CAJON'
    if m >= 150:
        return 'GIGANTE'
    return 'mixto'


COMO_SE_PARTE = {
    'CAJON': 'mecanico: mover texto, y demostrable byte a byte (L6d)',
    'GIGANTE': 'pide DISENO: el estado local tiene que volverse un struct',
    'mixto': 'a mano: hay funciones grandes entre las pequenas',
    'CON MONSTRUO': 'UNA funcion se lleva >1/3 Y comparte estado: PARTIRLA es diseno',
    'DESPACHADOR': 'UNA funcion enorme SIN estado: son brazos sueltos, se parten moviendo texto',
    'TABLA': 'son datos, no logica: mirar si lo deberia emitir una fabrica',
    'desconocida': 'sin cuenta de funciones para este lenguaje',
}


# -- Reading the tree and the baseline (plumbing, no generation) ---------------

def ficheros_del_repo(raiz):
    salida = subprocess.run(
        ['git', '-C', raiz, 'ls-files'],
        capture_output=True, text=True, check=True,
    ).stdout.splitlines()
    return [f for f in salida if f.endswith(EXTS)]


def censar(raiz):
    fichas = []
    for f in ficheros_del_repo(raiz):
        m = medir(os.path.join(raiz, f))
        if m is None:
            continue
        lineas, codigo, funciones, generado, mayor, estado = m
        if codigo < AVISO:
            continue
        fichas.append(Ficha(f, lineas, codigo, funciones, generado, mayor, estado))
    fichas.sort(key=lambda x: -x.lineas)
    return fichas


def leer_linea_base():
    """Techos, exentos y **subidas con su motivo**.

    Una subida es un techo que se levanto. Se guarda para siempre, con su por
    que al lado, porque un numero que sube sin motivo escrito es justo lo que
    este fichero existe para impedir.
    """
    techos, exentos, subidas = {}, {}, []
    if not os.path.isfile(BASE):
        return techos, exentos, subidas
    seccion = None
    for cruda in io.open(BASE, encoding='utf-8'):
        linea = cruda.strip()
        if not linea or linea.startswith('#'):
            continue
        if linea.startswith('[') and linea.endswith(']'):
            seccion = linea[1:-1].upper()
            continue
        partes = linea.split(None, 1)
        if seccion == 'TECHOS' and len(partes) == 2:
            techos[partes[1].strip()] = int(partes[0])
        elif seccion == 'EXENTOS' and len(partes) == 2:
            exentos[partes[0]] = partes[1].strip()
        elif seccion == 'SUBIDAS':
            subidas.append(linea)
    return techos, exentos, subidas


def sellar(fichas, exentos, techos, subidas, motivo):
    """Regraba la linea base. **Se niega si algo sube y no trae motivo.**

    Bajar no se pregunta: un reparto que quita lineas se explica solo. Subir,
    no. Y hasta el 2026-08-19 esto no lo comprobaba nadie -- se re-sello un
    techo (`syscall/mod.rs`, +14) y el unico sitio donde quedo el por que fue
    un mensaje de commit, que es donde nadie lo va a buscar dentro de un ano.

    Es la regla del dueno aplicada a la propia herramienta: **todo tiene su por
    que; lo que no lo tiene, se quita.**
    """
    # *** PARA RING 0 NO HAY LINEA BASE NUEVA. NUNCA. (2026-08-24)
    #
    # Regla de Eddi: *"el guardian limita ESTRICTAMENTE hasta mil, y por que?
    # porque hablamos de Bare Metal Orquestal. Pero si es para Ring 3 como
    # library OS, okey."*
    #
    # ** Y la diferencia no esta en el NUMERO, esta en la SALIDA DE EMERGENCIA.
    # El limite sigue siendo 1.000 para todo el arbol -- es la cifra que pone
    # L6a y no se toca. Lo que cambia por anillo es si se puede pedir una
    # excepcion:
    #
    #     util y Ring 3   el trinquete: un fichero nuevo se puede sellar, y
    #                     desde ese dia solo puede ENCOGER
    #     Ring 0          NO SE SELLA. Un fichero del kernel que cruce las mil
    #                     lineas para el build, y no hay `--motivo` que valga
    #
    # *** El motivo es que lo que cuesta un fallo depende del anillo: en Ring 3
    # un fallo mata la tarea --el kernel recupera la pantalla y escribe sus
    # ultimas cuatro lineas, verificado en metal-- y en Ring 0 se lleva la
    # maquina. Y ahi conviven 236 `static mut`: en un fichero grande, cada
    # funcion puede tocarlos todos.
    #
    # [!] Los SEIS que ya estan --8.886 lineas de Ring 0-- se respetan: el
    # trinquete no juzga el pasado, y un guardian que falle sobre lo que ya hay
    # se apaga en un dia. Lo que se cierra es la puerta a que entre el septimo.
    nuevos_r0 = [
        f for f in fichas
        if f.anillo == RING0 and f.codigo > LIMITE
        and f.ruta not in exentos and not f.generado
        and f.ruta not in techos
    ]
    if nuevos_r0:
        print('[X] RING 0 no admite linea base nueva. Estos hay que partirlos:')
        for f in nuevos_r0:
            print('    %6d  %s' % (f.lineas, f.ruta))
        print('    Un fallo en Ring 0 se lleva la maquina, no la tarea.')
        print('    En Ring 3 el trinquete sigue valiendo; aqui no hay excepcion.')
        return 1

    suben = []
    for f in fichas:
        if f.codigo > LIMITE and f.ruta not in exentos and not f.generado:
            viejo = techos.get(f.ruta)
            # *** SE COMPARA `codigo` CONTRA `codigo` (2026-08-24).
            # ** Aqui ponia `f.lineas > viejo`: las lineas TOTALES contra un
            # techo que se guarda en lineas de CODIGO. Y total siempre es mayor
            # --los comentarios cuentan-- asi que un fichero que no habia
            # crecido ni una linea salia como subida:
            #
            #     validator.rs: 1179 -> 1179 (+0)
            #
            # *** Y eso no es cosmetico: con dos "subidas" en la lista, `--sellar`
            # se niega pidiendo un motivo por cada una -- o sea que obliga a
            # INVENTAR una excusa para un fichero que no cambio. Justo lo que la
            # regla de arriba existe para impedir.
            #
            # ** Es el mismo patron que destapo la auditoria de seguridad del
            # mismo dia, seis veces: **un limite comparado contra el numero
            # equivocado.** No faltaba la comprobacion; comparaba mal.
            if viejo is not None and f.codigo > viejo:
                suben.append((f.ruta, viejo, f.codigo))
    if suben and not motivo:
        print('[X] hay techos que SUBEN y `--sellar` no acepta una subida muda:')
        for ruta, viejo, nuevo in suben:
            print('    %s: %d -> %d (+%d)' % (ruta, viejo, nuevo, nuevo - viejo))
        print('    vuelve a intentarlo con `--motivo "por que sube"`.')
        return 1
    # *** UN MOTIVO NO PUEDE EXPLICAR TRES SUBIDAS (2026-08-24).
    #
    # ** Aqui habia un bucle que pegaba el MISMO `--motivo` a cada fichero que
    # subiera, y el 24-08 lo hizo de verdad: un sellado con el motivo del
    # emulador (AVX2 y el decodificador VEX) quedo escrito **tambien** en
    # `build.ps1` y en `bmo-abi/src/bef/validator.rs`, que habian crecido por
    # cosas que no tenian nada que ver.
    #
    # *** Y eso convierte la lista de SUBIDAS en lo contrario de lo que es. Su
    # propio comentario dice *"una lista de excusas que se puede leer entera es
    # lo que hace que sea incomodo anadirle una"*. Una excusa copiada a tres
    # sitios no es incomoda: es RUIDO, y a la tercera nadie la lee.
    #
    # La regla que faltaba: **un `--motivo` explica UNA subida.** Si suben
    # varios, se sellan de uno en uno, cada uno con el suyo.
    if len(suben) > 1 and motivo:
        print('[X] suben %d techos y solo hay UN motivo:' % len(suben))
        for ruta, viejo, nuevo in suben:
            print('    %s: %d -> %d (+%d)' % (ruta, viejo, nuevo, nuevo - viejo))
        print('    Un motivo explica UNA subida. Pegarlo a todas deja escrita')
        print('    una excusa falsa en los otros, y la lista de SUBIDAS existe')
        print('    justamente para que se pueda leer entera y creer.')
        print('    Sella de uno en uno, cada uno con su por que.')
        return 1
    for ruta, viejo, nuevo in suben:
        subidas.append('%-58s %d -> %d  %s' % (ruta, viejo, nuevo, motivo))

    # ** Y SE DICE LO QUE SE LLEVA. Sellar borraba en silencio las buenas
    # noticias: el 19-08 `obj/file.rs` y `scene/data.rs` bajaron de 1.000 --al
    # partir `cargando.rs`-- y salieron de la lista sin que nadie lo leyera. Un
    # trinquete que solo cuenta lo que cuesta y nunca lo que se gano acaba
    # pareciendo un peaje.
    vivos = {f.ruta for f in fichas if f.codigo > LIMITE}
    for ruta, viejo in sorted(techos.items()):
        if ruta not in vivos:
            print('    [+] sale de la lista  %s (estaba en %d)' % (ruta, viejo))

    hoy = []
    hoy.append('# LINEA BASE del censo modular -- el techo de cada fichero que')
    hoy.append('# hoy incumple L6a. La regla del trinquete: un fichero de esta')
    hoy.append('# lista solo puede ENCOGER. Si crece, o si aparece uno nuevo por')
    hoy.append('# encima de %d lineas DE CODIGO --ni comentario ni blanco--,' % LIMITE)
    hoy.append('# `censo_modular.py --check` dice NO. Ver `solo_codigo` para el')
    hoy.append('# por que: L6a maneja el riesgo del CODIGO, y un comentario hace')
    hoy.append('# un fichero mas facil de auditar, no mas dificil.')
    hoy.append('#')
    hoy.append('# No se edita a mano: se regenera con `--sellar` cuando un')
    hoy.append('# reparto baja un numero, y el commit ensena cuanto bajo.')
    hoy.append('')
    hoy.append('[TECHOS]')
    for f in fichas:
        if f.codigo > LIMITE and f.ruta not in exentos and not f.generado:
            hoy.append('%6d  %s' % (f.codigo, f.ruta))
    hoy.append('')
    hoy.append('# EXENTOS -- un "no" con motivo escrito, que se puede discutir.')
    hoy.append('# Aqui solo entra lo que NO lo escribio una persona: si la')
    hoy.append('# modularidad de un fichero la decide su fabrica, la regla se le')
    hoy.append('# aplica a la fabrica y no a lo que emite.')
    hoy.append('')
    hoy.append('[EXENTOS]')
    for ruta, m in sorted(exentos.items()):
        hoy.append('%s  %s' % (ruta, m))
    hoy.append('')
    hoy.append('# SUBIDAS -- cada techo que se levanto, con su por que. No se')
    hoy.append('# borran: una lista de excusas que se puede leer entera es lo que')
    hoy.append('# hace caro poner la siguiente.')
    hoy.append('')
    hoy.append('[SUBIDAS]')
    hoy.extend(subidas)
    hoy.append('')
    io.open(BASE, 'w', encoding='utf-8', newline='\n').write('\n'.join(hoy))
    return 0


# == NIETO =====================================================================
# The verdict. The only generation with an opinion: it is the one that knows
# what the limit means, what the baseline promised, and what deserves an exit
# code of 1. It lives at the end because nothing above it may ask what it thinks.

def juicio(fichas, techos, exentos):
    nuevos, crecidos, encogidos, salidos = [], [], [], []
    vistos = set()

    for f in fichas:
        if f.ruta in exentos or f.generado:
            continue
        vistos.add(f.ruta)
        techo = techos.get(f.ruta)
        if techo is None:
            if f.codigo > LIMITE:
                nuevos.append(f)
        elif f.codigo > techo:
            crecidos.append((f, techo))
        elif f.codigo < techo:
            encogidos.append((f, techo))

    for ruta, techo in techos.items():
        if ruta not in vistos:
            salidos.append((ruta, techo))

    return nuevos, crecidos, encogidos, salidos


def informe(fichas, techos, exentos, nuevos, crecidos, encogidos, salidos, subidas):
    # ** LAS DOS COLUMNAS, y la de la izquierda es la que JUZGA. El total va al
    # lado para que nadie tenga que creerse la cuenta del codigo: si las dos se
    # separan mucho, ese fichero es sobre todo explicacion -- y eso no es un
    # problema, es lo que esta casa pide.
    print('%7s %7s %5s %6s  %-52s %s'
          % ('codigo', 'total', 'fns', 'media', 'fichero', 'especie'))
    fuera = []
    for f in fichas:
        if f.ruta in exentos or f.generado:
            fuera.append(f)
            continue
        marca = '!' if f.codigo > LIMITE else ' '
        print('%7d %7d %5s %6s %s %-52s %s' % (
            f.codigo,
            f.lineas,
            f.funciones if f.funciones is not None else '-',
            f.media if f.media is not None else '-',
            marca, f.ruta, especie(f)))

    pasan = [f for f in fichas
             if f.codigo > LIMITE and f.ruta not in exentos and not f.generado]
    cajones = [f for f in pasan if especie(f) == 'CAJON']
    print()
    print('%d ficheros incumplen L6a (>%d lineas de CODIGO). %d son CAJON, o sea que se'
          % (len(pasan), LIMITE, len(cajones)))
    print('parten moviendo texto y el reparto se demuestra con un hash (L6d).')

    # *** Y DE QUE ANILLO SON, que es la mitad que faltaba (2026-08-24).
    #
    # ** Hasta hoy este censo trataba las diecisiete infracciones como la misma
    # cosa. Y no lo son: un fichero de 1.200 lineas en el kernel y uno de 1.200
    # en un compilador del anfitrion **cuestan distinto cuando fallan**.
    #
    # Lo que este bloque anade no es una regla nueva -- es la capacidad de DECIR
    # cuales importan. Y lo primero que dijo, el dia que se escribio, fue que
    # los dos ficheros mas grandes que corren en Ring 0 viven bajo `platform/` y
    # **no parecen Ring 0 desde la carpeta**.
    por_anillo = {}
    for f in pasan:
        n, l = por_anillo.get(f.anillo, (0, 0))
        por_anillo[f.anillo] = (n + 1, l + f.codigo)
    print()
    print('  por anillo, y ahi esta la diferencia:')
    for cual, etiqueta in ((RING0, 'RING 0  un fallo se lleva la MAQUINA'),
                           (RING3, 'Ring 3  un fallo mata la TAREA'),
                           (UTIL, 'util    no corre en la maquina')):
        n, l = por_anillo.get(cual, (0, 0))
        print('    %-38s %2d ficheros, %6d lineas' % (etiqueta, n, l))
    if por_anillo.get(RING0, (0, 0))[0]:
        print('    [!] los de RING 0 son los que hay que partir primero, y desde')
        print('        el 2026-08-24 NO PUEDE ENTRAR NINGUNO MAS: `--sellar` se')
        print('        niega a admitir un fichero de Ring 0 en la linea base.')

    for f in fuera:
        motivo = exentos.get(f.ruta) or 'lo emite una fabrica: dice AUTO-GENERADO'
        print('  [-] fuera del censo  %s (%d) -- %s' % (f.ruta, f.lineas, motivo))
    for s in subidas:
        print('  [^] techo levantado  %s' % s)

    for f, techo in encogidos:
        print('  [+] ENCOGIO   %s: %d -> %d' % (f.ruta, techo, f.lineas))
    for ruta, techo in salidos:
        print('  [+] YA NO ESTA %s (estaba en %d)' % (ruta, techo))
    if encogidos or salidos:
        print('      -> `--sellar` para que el trinquete no permita volver atras.')

    for f in nuevos:
        print('  [X] NUEVO     %s: %d lineas, y no estaba en la linea base' % (f.ruta, f.lineas))
        print('                %s -> %s' % (especie(f), COMO_SE_PARTE[especie(f)]))
    for f, techo in crecidos:
        print('  [X] CRECIO    %s: %d -> %d (+%d)' % (f.ruta, techo, f.lineas, f.lineas - techo))


def main():
    ap = argparse.ArgumentParser(description='El metro de L6a, con trinquete.')
    ap.add_argument('--check', action='store_true',
                    help='sale con 1 si algo es nuevo o crecio')
    ap.add_argument('--sellar', action='store_true',
                    help='vuelve a grabar la linea base tal como esta hoy')
    ap.add_argument('--motivo', default='',
                    help='POR QUE sube un techo. Sin esto, una subida se rechaza')
    ap.add_argument('--raiz', default=None, help='raiz del repo (por defecto, la de este fichero)')
    args = ap.parse_args()

    raiz = args.raiz or os.path.abspath(
        os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..', '..'))

    fichas = censar(raiz)
    techos, exentos, subidas = leer_linea_base()

    if args.sellar:
        if sellar(fichas, exentos, techos, subidas, args.motivo.strip()):
            return 1
        print('linea base sellada: %s' % BASE)
        return 0

    nuevos, crecidos, encogidos, salidos = juicio(fichas, techos, exentos)
    informe(fichas, techos, exentos, nuevos, crecidos, encogidos, salidos, subidas)

    fallo = 0
    if not techos:
        print('\n[!] no hay linea base todavia: `--sellar` la graba.')
    elif nuevos or crecidos:
        print('\nL6a: %d nuevos, %d crecidos.' % (len(nuevos), len(crecidos)))
        # *** UN "NO" QUE NO DICE A QUIEN LE HABLA ES UN "NO" QUE ASUSTA.
        #
        # ** Pregunta de Eddi (2026-08-24): *"no quiero imaginar que cuando los
        # nuevos programadores entren a usar mi BMO-X les choque con el guardian
        # que les limita"*. Y son DOS personas distintas que este mensaje tenia
        # juntas:
        #
        #     escribe una APP para BMO-X   -> este guardian NO LE MIRA NUNCA.
        #                                     Lee `git ls-files` de ESTE repo, y
        #                                     su app no esta aqui. Lo que se le
        #                                     exige son las siete R-APP, y
        #                                     ninguna habla de lineas
        #     contribuye A BMO-X           -> si, y tiene que ser asi: aqui el
        #                                     fichero grande lo mantiene otro
        #
        # Decirlo en el momento del NO --y no en un documento que hay que ir a
        # buscar-- es la diferencia entre una regla y un muro.
        print('\n  [i] esto juzga SOLO los ficheros de este repo (`git ls-files`).')
        print('      Si escribes una APP para BMO-X, este guardian no te mira:')
        print('      lo que se te exige son las siete R-APP de META-APP_HARD.md,')
        print('      y ninguna habla de cuantas lineas tiene tu fichero.')
        r0 = [f for f in nuevos if f.anillo == RING0]
        if r0:
            print('      Y de los nuevos, %d son de RING 0: ahi no hay linea base' % len(r0))
            print('      que valga, porque un fallo se lleva la maquina.')
        fallo = 1
    else:
        print('\nclean: ningun fichero nuevo por encima de %d y ninguno crecio.' % LIMITE)

    # ** UNA PUERTA, DOS PREGUNTAS. L7 la contesta `herencia.py` y vive aparte
    # porque es OTRA pregunta --L6b: el corte se elige por la pregunta que
    # responde el fichero-- pero entra por aqui. Asi `build.ps1` tiene un solo
    # guardian que llamar, que es la misma forma que el sistema entero: una
    # puerta, muchas operaciones detras.
    print('\n-- L7, la herencia --')
    if herencia.revisar(raiz):
        fallo = 1

    return fallo if args.check else 0


if __name__ == '__main__':
    sys.exit(main())

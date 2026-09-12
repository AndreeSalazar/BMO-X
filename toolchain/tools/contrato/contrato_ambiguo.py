# -*- coding: utf-8 -*-
"""R22 -- SI ES SI Y NO ES NO: una puerta no contesta EXITO cuando niega.

Pedida por el dueno el 2026-09-12, con estas palabras:

    *"que el orquestador sea ESTRICTO, que diga si si y no no, porque si es
    ambiguo se rompe TODO"*

Vive en su propio fichero **y no dentro de `contrato.py`**, por la leccion que
`contrato_consumo.py` ya lleva escrita en su cabecera: R21 nacio en el monolito,
lo cruzo por encima de las 1.000 lineas y L6a la echo el mismo dia.

# El fallo que la trajo

El DIRECTOR no ensenaba la ventana de DOOM. El motivo estaba tres capas abajo,
en el brazo de `MEM_OP_OFRECER`:

```rust
   let Some(destino) = scheduler::pid_de(frame.r8 as u32) else {
       return BmoStatus::ok_value(0);          // <- EXITO
   };
   let ok = loan::offer(...);
   BmoStatus::ok_value(ok as u64)              // <- EXITO, niegue o no
```

** La puerta tiene DOS canales y la negativa viajaba por el equivocado.
`bmo_codigo` promete que *"0 es lo unico que significa exito"*; `bmo_valor`
lleva la respuesta. Cuando un NO viaja como VALOR, **es indistinguible de una
respuesta**: el kernel decia que no y contestaba que si.

El resultado fue una app dibujando a 60 fps dentro de memoria que no lee nadie,
sin una sola linea en ningun sitio que dijera por que.

# Por que hace falta una REGLA y no un arreglo

`SILENCIO` ya existe como clase de riesgo en L6f --*"equivocarse no falla:
sigue, y da un dato malo"*-- o sea que **el FUERO sabia nombrar la enfermedad y
no tenia quien la buscara**. Eso es lo que esto cambia.

# [!] Y LO QUE ESTA REGLA NO HACE: ADIVINAR

`contrato_consumo.py` lo dejo escrito y aqui se obedece:

    *"Lo que NO comprueba: que la clase sea la CORRECTA. Deducirla seria
    adivinar, y un guardian que adivina da permiso con autoridad."*

Esto **no juzga si contestar exito esta mal**. A veces esta bien: `TASK_OP_TOMAR`
contesta `ok(0)` cuando nadie ofrece, y eso pasa mil veces por segundo -- no es
un fallo, es la respuesta normal. Lo que hace es un HECHO SINTACTICO: *"aqui se
contesta exito desde una rama `None`, `Err` o `else`"*. Quien decide si esa
concreta esta bien es una persona, y lo escribe en `AMBIGUAS.txt` a mano -- la
misma disciplina que `LINEA_BASE.txt`, cuya nota tambien se escribe a mano
porque una herramienta no puede saberlo.

# El trinquete

La lista **esta para encogerse**. Lo que ya esta se tolera con su motivo al
lado; una puerta NUEVA que conteste exito al negar para el build hasta que
alguien decida cual de las dos cosas es.

La ley esta en L6i de `FUERO/META-KERNEL_HARD.md`.
"""

import os
import re

from contrato_ley import raiz

# [!] LOS NOMBRES DE AQUI SON LARGOS A PROPOSITO, y costo un fallo aprenderlo:
# `contrato.py` hace `from contrato_ambiguo import *`, asi que un `BASE` o un
# `leer_base` de aqui **tapan los suyos**. La primera version los tenia cortos y
# `linea_base_leer` acabo leyendo AMBIGUAS.txt -- reventando con
# `invalid literal for int(): 'mod.rs'`. Es la misma disciplina que ya sigue
# `contrato_consumo.py`, y ahora se sabe por que la sigue.

#: Donde vive el despachador. Una puerta ambigua solo importa donde se cruza.
SYSCALL_DIR = "Ultra_kernel_x86-64/kernel/src/ring0/syscall"

AMBIGUAS_TXT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "AMBIGUAS.txt")

#: Contestar que todo fue bien. Es la unica forma que tiene el kernel de decir
#: que si, asi que es la unica que hay que mirar.
RE_EXITO = re.compile(r"\bok_value\s*\(")

#: Las formas SINTACTICAS de una rama de negativa. No se deduce nada: o el
#: codigo escribe `None =>`, o `Err(..) =>`, o abre un `else {`.
RE_NEGATIVA = re.compile(r"^\s*(?:\}\s*)?(?:None\s*=>|Err\s*\(.*?\)\s*=>)"
                         r"|\belse\s*\{\s*$")

#: De quien es la rama. Tres formas, y la tercera hizo falta el primer dia: el
#: brazo de `MEM_OP_OFRECER` se escribe `cap::KIND_MEMORIA if frame.rsi == ...`,
#: y sin ella el hallazgo salia como `mod.rs invoke`, que no dice nada.
RE_GUARDA = re.compile(
    r"^\s*\S+\s+if\s+.*==\s*(?:\w+::)*([A-Z][A-Z0-9_]{3,})\s*=>")
RE_OPERACION = re.compile(r"^\s*(?:\w+::)*([A-Z][A-Z0-9_]{3,})\s*=>")
#: `pub(super) fn` tambien: sin el parentesis generico, `console_read` salia
#: como `(sin nombre)` -- y un hallazgo sin nombre no se puede apuntar en una
#: lista que se ordena por nombre.
RE_FUNCION = re.compile(
    r"^\s*(?:pub(?:\(\w+\))?\s+)?fn\s+([a-z_][a-z0-9_]*)")


def _sangria(linea):
    return len(linea) - len(linea.lstrip())


def _dentro_de_una_negativa(lineas, i):
    """El exito de la linea `i`, esta DENTRO de una rama de negativa?

    == [!] LA PRIMERA VERSION MIRABA TRES LINEAS ATRAS Y SE EQUIVOCO ==

    `op_consola.rs` salio acusado y era inocente:

    ```rust
       None => crate::ring0::uconsole::write_packed(arg0),
       }
       BmoStatus::ok_value(0)          // <- el exito de la FUNCION
    ```

    Ese `ok_value` no es de la rama: es el final del trabajo, que fue bien.
    Contarlo habria sido exactamente lo que la cabecera promete no hacer --
    **adivinar**. Un guardian con un falso positivo se desactiva en una semana,
    y con razon.

    Asi que la pertenencia se decide por SINTAXIS y no por cercania:

    ```text
       misma linea         `None => BmoStatus::ok_value(0),`          SI
       la negativa ABRE    `... else {` y el exito va MAS ADENTRO     SI
       la negativa CIERRA  acaba en `,` y el exito no esta dentro     NO
    ```
    """
    if RE_NEGATIVA.search(lineas[i]):
        return True
    for j in range(max(0, i - 3), i):
        if not RE_NEGATIVA.search(lineas[j]):
            continue
        # Solo cuenta si aquella linea ABRIO un bloque y este exito va dentro.
        if lineas[j].rstrip().endswith("{"):
            if _sangria(lineas[i]) > _sangria(lineas[j]):
                return True
    return False


def puertas_ambiguas(ficheros):
    """`{(fichero, quien): linea}` de cada exito contestado desde una negativa.

    `ficheros` es `{ruta: texto}`. El nombre de `quien` es la operacion del
    `match` --`TASK_OP_TOMAR`-- y no un numero de linea, que se mueve con el
    primer comentario que alguien escriba encima.
    """
    fuera = {}
    for ruta in sorted(ficheros):
        lineas = ficheros[ruta].split("\n")
        quien = "(sin nombre)"
        for i, linea in enumerate(lineas):
            m = (RE_GUARDA.match(linea) or RE_OPERACION.match(linea)
                 or RE_FUNCION.match(linea))
            if m:
                quien = m.group(1)
            if not RE_EXITO.search(linea):
                continue
            if not _dentro_de_una_negativa(lineas, i):
                continue
            fuera.setdefault((os.path.basename(ruta), quien), i + 1)
    return fuera


def r22_si_es_si(ficheros, base=None):
    """L6i -- **una negativa no contesta EXITO**. Devuelve las quejas.

    Dos deberes, y ninguno de los dos es una opinion sobre el codigo:

      1. toda puerta que conteste exito desde una rama `None`/`Err`/`else` esta
         en `AMBIGUAS.txt` con su motivo escrito A MANO.
      2. es un TRINQUETE: una puerta nueva para el build. Lo que ya estaba se
         tolera -- arreglarlas todas de golpe cambia lo que ve todo el que ya
         llama, y eso se decide puerta por puerta.

    [!] Una entrada de la lista que ya no aparece **no es una queja**: es que
    alguien la arreglo. Se dice en la nota del build para que se vea bajar.
    """
    base = leer_ambiguas() if base is None else base
    quejas = []
    for (fichero, quien) in sorted(puertas_ambiguas(ficheros)):
        if (fichero, quien) not in base:
            quejas.append(
                "%s %s contesta EXITO desde una rama de negativa y no esta en "
                "AMBIGUAS.txt. O niega con un codigo de error, o se apunta ahi "
                "con el motivo (L6i)" % (fichero, quien))
    return quejas


def lo_que_sigue_ambiguo(ficheros, base=None):
    """**La nota de cada build**: cuantas puertas siguen sin decir que no.

    La mitad que no se comprueba sola. Una regla que solo prohibe crecer deja
    la deuda quieta y callada; esta linea la mantiene a la vista, y el dia que
    baje se vera bajar.
    """
    base = leer_ambiguas() if base is None else base
    hoy = puertas_ambiguas(ficheros)
    if not hoy and not base:
        return []
    arregladas = [k for k in base if k not in hoy]
    deudas = sum(1 for k in hoy if base.get(k, "").startswith("DEUDA"))
    lineas = ["R22 L6i: %d puerta(s) contestan EXITO al negar -- %d DEUDA"
              % (len(hoy), deudas)]
    for (fichero, quien) in sorted(hoy):
        clase = (base.get((fichero, quien), "") or "?").split()[0]
        lineas.append("        %-9s %-16s %s" % (clase, fichero, quien))
    if arregladas:
        lineas.append("        [x] %d ya no: %s"
                      % (len(arregladas),
                         ", ".join("%s %s" % k for k in sorted(arregladas))))
    return lineas


def comprobar_ambiguo(ficheros=None):
    """Lo que `comprobar()` necesita de R22 en UNA llamada: `(quejas, notas)`.

    Una sola linea en `contrato.py` y no seis, que es lo que le sobraba para
    volver a cruzar las 1.000 de L6a. Misma forma que `comprobar_consumo`.
    """
    ficheros = ficheros_del_despachador() if ficheros is None else ficheros
    base = leer_ambiguas()
    quejas = [("R22 L6i si es si y no es no", q)
              for q in r22_si_es_si(ficheros, base)]
    nota = lo_que_sigue_ambiguo(ficheros, base)
    return quejas, (["\n".join(nota)] if nota else [])


def ficheros_del_despachador():
    """`{ruta: texto}` de los `.rs` de `ring0/syscall/`.

    Solo el despachador: una funcion interna que devuelva `None` no le miente a
    nadie de fuera. **La ambiguedad solo hace dano donde se cruza la frontera**,
    y la frontera es esta carpeta.
    """
    d = os.path.join(raiz(), SYSCALL_DIR.replace("/", os.sep))
    fuera = {}
    if not os.path.isdir(d):
        return fuera
    for dp, dn, fn in os.walk(d):
        dn[:] = [x for x in dn if x not in ("target", ".git")]
        for n in sorted(fn):
            if not n.endswith(".rs"):
                continue
            ruta = os.path.join(dp, n)
            with open(ruta, "r", encoding="utf-8", errors="replace") as f:
                fuera[os.path.relpath(ruta, raiz()).replace(os.sep, "/")] = f.read()
    return fuera


def leer_ambiguas():
    """`{(fichero, quien): nota}` de `AMBIGUAS.txt`. Vacio si no esta."""
    fuera = {}
    if not os.path.exists(AMBIGUAS_TXT):
        return fuera
    with open(AMBIGUAS_TXT, "r", encoding="utf-8") as f:
        for linea in f:
            linea = linea.strip()
            if not linea or linea.startswith("#"):
                continue
            partes = linea.split(None, 2)
            if len(partes) < 2:
                continue
            fuera[(partes[0], partes[1])] = partes[2] if len(partes) > 2 else ""
    return fuera


def escribir_ambiguas(hoy, antes):
    """Graba la lista de hoy, **conservando los motivos ya escritos**.

    Un motivo se escribe a mano y cuesta pensarlo; `--sellar` no puede borrarlo
    por sellar. Lo que no tenia motivo sale con un hueco marcado, que es mas
    honesto que inventarle uno.
    """
    with open(AMBIGUAS_TXT, "w", encoding="utf-8", newline="\n") as f:
        f.write("# R22 (L6i) -- las puertas que contestan EXITO al NEGAR.\n")
        f.write("#\n")
        f.write("# ** ESTA LISTA ESTA PARA ENCOGERSE. Es un trinquete, como el de\n")
        f.write("# L6a: lo que ya esta se tolera con su motivo al lado; una puerta\n")
        f.write("# NUEVA que conteste exito al negar para el build.\n")
        f.write("#\n")
        f.write("# El motivo se escribe A MANO y empieza por UNA de estas dos:\n")
        f.write("#    CORRECTA  el `None` no es un fallo, es la respuesta normal\n")
        f.write("#    DEUDA     miente, y cambiarlo toca a quien ya llama\n")
        f.write("#\n")
        f.write("# Formato:  fichero  quien  motivo\n")
        f.write("# Se regenera con `contrato.py --sellar`; el motivo, no.\n")
        for k in sorted(hoy):
            nota = antes.get(k) or "[!] SIN MOTIVO -- escribelo"
            f.write("%s %s %s\n" % (k[0], k[1], nota))

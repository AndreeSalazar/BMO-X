# BMO C -- LA FABRICA y LA EXPANSION

> Escrito el **2026-09-11** a partir de una pregunta del dueno: *"que el interior
> de C sea una FABRICA de funcionalidades, y otra carpeta de EXPANSION donde
> vivan DOOM, SQLite y lo que venga"*. Al mirar el arbol resulto que **el reparto
> ya existia y ya era ley** -- solo que sin nombre, y con seis ficheros en el
> lado equivocado.

---

# 1. EL MAPA, tal como es

```text
   LA FABRICA -- lo que C promete y como BMO C lo cumple
   ------------------------------------------------------------------------
   toolchain/lang/c/src/                el compilador: 39 ficheros, y cada uno
                                        dice en que FASE aparece su fallo
   toolchain/lang/c/examples/           los 17 programas de la casa: sondas,
                                        medidas y muestras. Ejercen la fabrica
   toolchain/forge/sem-asm/tables/      LO QUE C PROMETE: <stdio.h>, <stdlib.h>,
                                        <string.h>, <stdint.h>... y REX (bmo/*.h)

   LA EXPANSION -- lo que USA la fabrica y no es de la casa
   ------------------------------------------------------------------------
   ../BMO-externo/doom/                 las fuentes de DOOM (GPL): FUERA del arbol
   ../BMO-externo/doom-port/            la cola del port: unity.py, los stubs de
                                        compatibilidad (<windows.h>, <go32.h>...)
```

** `tables/` no se mueve ni se renombra, y el motivo esta en
[`META-SDK_HARD.md`](../../../FUERO/META-SDK_HARD.md): es **la puerta de los
terceros**, y `$BMO_MODS` la tapa sin bifurcar el repo. Una carpeta mas bonita
seria una segunda puerta.

---

# 2. LA REGLA, en tres frases

1. **La fabrica no sabe de ningun programa.** Nada en `src/` ni en `tables/`
   nombra a DOOM, a SQLite ni a ningun port. Si un port destapa un fallo de C,
   el arreglo va a la fabrica **como regla de C con su fila en el banco**, y el
   port no se menciona mas que en la historia del commit.

2. **La expansion no toca la fabrica: la TAPA.** Un port vive en su carpeta con
   sus fuentes --o con un puntero a donde estan, si la licencia las deja fuera--,
   sus stubs, su guion de build y sus instrumentos. Lo que necesita distinto lo
   pone en su `include/` y `$BMO_MODS` lo hace ganar. Nunca edita `tables/`.

3. *** **Lo que un port necesita de C y no esta, se PIDE a la fabrica.** No se
   escribe en el port. Es la frase que faltaba, y es la que separa las dos
   carpetas de verdad:

   ```text
      <stdint.h>    lo pide TODO programa de C     -> fabrica (tables/)
      <windows.h>   lo pide DOOM por una rama muerta -> expansion (el port)
   ```

  > Una cabecera que el segundo port tendria que COPIAR del primero esta en la
  > carpeta equivocada.

---

# 3. LO QUE SE HIZO EL 2026-09-11

**Seis cabeceras ISO C se mudaron a la fabrica**: `stdint.h`, `limits.h`,
`stdbool.h`, `assert.h`, `errno.h`, `inttypes.h`. Vivian en
`doom-port/include/standards/C/` con la cabecera *"minimal, for probing BMO C
against DOOM"* -- y no son de DOOM: son lo que BMO C promete. DOOM fue solo el
primero en pedirlas.

** Se mudaron con sus lecciones dentro. `stdbool.h` lleva escrito por que
`__bool_true_false_are_defined` no es decoracion (52 ficheros culpando al
compilador, y era la cabecera), e `inttypes.h` por que incluye `stdint.h` (77
errores de *"expected type, got uint8_t"* con un stub vacio). Y `assert.h` dice
lo que es: **un limite declarado**, no comprueba nada, y lo que falta para que
compruebe es del sistema.

Comprobado: `doom.bex` sale **byte a byte identico** con las cabeceras en su
sitio nuevo.

Y `toolchain/lang/c/test/` --cuatro ficheros, 26 lineas, que nadie referenciaba
desde agosto-- se fue.

---

# 4. [!] LO QUE QUEDA POR DECIDIR, y es del dueno

## 4.1 La cola del port NO esta versionada

`BMO-externo/` no es repositorio git. Ahi viven `unity.py`, los stubs, y --lo
que mas cuesta-- **los instrumentos que se le meten a DOOM para cazar fallos**:
el censo de columnas y el de spans del 10 y 11 de septiembre, el RANGECHECK
corregido, `bmo_fila_fuera`. Si esa carpeta se pierde, se pierde el trabajo de
tres semanas de sondas.

Dos salidas, y las dos son legitimas:

```text
   a) `git init` en BMO-externo/doom-port      un repo aparte, con su licencia
   b) mudar la COLA al arbol                   toolchain/lang/c/expansion/doom/
      --unity.py, include/, probe.py--         con un puntero a las fuentes GPL
```

La b) es la que el dueno describio. Lo que la frena es una pregunta de
licencia, no de codigo: `doomgeneric_bmo.c` es original de esta casa pero solo
existe para enlazar con GPL. Los stubs y los guiones no tienen esa duda.

## 4.2 `examples/` se llama en ingles

Los 17 programas de la casa. El nombre lo leen `build/ejemplos.ps1` y
`contrato_rex.py` (R14); renombrarlo es un cambio de dos rutas. No se hizo hoy
porque no aporta mas que el nombre, y se dice para que no parezca olvido.

---

# 5. Lo que NO es esta division

No es *"lo importante y lo demas"*. DOOM ha destapado **siete fallos del
compilador** en un mes --el `signed`, el flotante por puntero, el sufijo del
literal, el `abs` sin conversion...-- y ninguna sonda de la casa los habia
visto. **La expansion es el banco de pruebas mas duro que tiene la fabrica**, y
por eso se le da carpeta y nombre: para que el siguiente port entre por la misma
puerta y con las mismas reglas.

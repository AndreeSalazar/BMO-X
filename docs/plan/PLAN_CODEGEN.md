# PLAN DEL CODEGEN DE BMO C -- el censo, los cortes y el numero que los ordena

> Pedido por el dueno el 2026-09-12: *"ir profundo en C por completo, en el
> codegen, analizar y modular por completo"*.
>
> Y se empieza por el CENSO y no por el codigo, que es la regla principal de la
> casa: **preguntar por el SITIO antes de escribir**.

---

## 0. El numero que ordena todo lo demas

No es una opinion sobre estilo. El mismo dia, midiendo la expansion de DOOM en
el Ryzen:

```text
   64.000 pixeles, 2 escrituras cada uno, a RAM CACHEADA
   2.216 us  ->  34,6 ns/px  ->  a 4.497 MHz = 156 CICLOS POR PIXEL
   para SEIS instrucciones utiles
```

Y 750 KiB en 2.216 us son **346 MB/s**, que es el MISMO numero que el "~300
MB/s" que hasta el 09-09 se le achacaba al framebuffer -- donde tampoco era la
memoria, era `write_volatile`. **La misma firma, una capa mas arriba.**

El sospechoso esta documentado y tiene su cifra: la memoria del
proyecto dicen que este codegen emitia **35 instrucciones por escritura util**,
y que la regla *el emisor no decide* lo bajo a **28**.

> O sea que el trabajo del codegen no es "optimizar": es que **28 bajen**. Y
> antes de tocar 28 instrucciones hay que poder encontrarlas.

---

## 1. EL CENSO, a 2026-09-12

```text
   toolchain/lang/c/src          27.815 lineas en total
     tests/                      58 ficheros   <- el banco: 537 filas
     codegen/                    12 + 4 + 4 ficheros
     parser/                      8
     ast/                         5
```

Los diez mas grandes, y la fase que declara cada uno:

```text
   1.738  codegen/mod.rs              EMISION   <- el monolito
   1.204  parser/preprocessor.rs      SINTAXIS
   1.044  parser/declarations.rs      SINTAXIS
     694  parser/mod.rs               SINTAXIS
     604  codegen/format.rs           EMISION
     542  codegen/frame.rs            EMISION
     538  codegen/indexing.rs         EMISION
     529  parser/expressions.rs       SINTAXIS
     430  parser/inicializador.rs     SINTAXIS
     424  codegen/sintetizadas.rs     EMISION
```

★ **Tres ficheros pasan de 1.000**, que es la linea de L6a: `codegen/mod.rs`,
`parser/preprocessor.rs` y `parser/declarations.rs`. L6a es un TRINQUETE --no
obliga a bajarlos, solo prohibe que crezcan-- asi que llevan ahi tiempo sin que
nada grite.

---

## 2. EL PRIMER CORTE, y por que NO es de tamano -- HECHO

`codegen/mod.rs` declaraba en su cabecera `[fase] EMISION`. Y **360 de sus
1.738 lineas eran IMAGEN**:

```text
   separar_bss           204 lineas   reparte los globales en regiones de BSS
   seccion_de_simbolos    57          arma la tabla de simbolos del BEF
   build_bef              99          arma las SECCIONES y escribe el fichero
```

*** **Y `[fase]` no es decoracion.** El build la cuenta --*"40 de los 40
ficheros de BMO C declaran su fase"*-- y `toolchain/tools/fases` la usa para
contestar **donde APARECE un fallo**. Un fichero que declara EMISION y hace
IMAGEN le miente a ese juez: un fallo de `build_bef` no aparece *dentro* como
promete la cabecera de `mod.rs`; aparece **en el metal**, cuando el cargador lee
el fichero.

O sea que el corte no se justifica por las 1.738 lineas: se justifica porque
**la etiqueta era falsa**, y la unica forma de que deje de serlo es que las dos
fases vivan en dos ficheros -- porque la etiqueta es POR FICHERO.

```text
   [x] codegen/bex.rs          nace con [fase] IMAGEN, [carril] AMARILLO,
                               [cuesta] DATO, [riesgo] AJENO
   [x] codegen/mod.rs          1.738 -> 1.378 lineas (consecuencia, no motivo)
   [x] el guardian lo cuenta   IMAGEN 3 -> 4
   [x] y NO cambio un byte     los 14 `.bex` de C salen identicos, y el banco
                               pasa sus 537 filas
```

** Esa ultima fila es la prueba de que fue un MOVIMIENTO y no una reescritura.
Es la misma disciplina que L6d pide para el kernel: si el reparto no cambia lo
que sale, se compara lo que sale.

---

## 3. LOS CORTES QUE QUEDAN, por el mismo criterio

No por tamano: **por lo que cada trozo cuesta si se equivoca** (L6e, la regla de
corte) y por la fase que de verdad hace.

```text
   [ ] `emit_program` son 357 lineas dentro de `mod.rs`, y hace TRES cosas:
       recorre el programa, reune las cadenas y ORQUESTA los pases. La parte de
       las cadenas --`collect_expr_strings` + `collect_stmt_strings`, 93
       lineas-- es una PASADA QUE NO EMITE UN BYTE: su `[cuesta]` es NADA y el
       resto del fichero es DATO. Dos clases en un fichero es exactamente lo que
       L6e llama mal cortado
       -- `codegen/mod.rs`

   [ ] `emit_stmt` son 175 lineas y un `match` de sentencias. Es hermano de
       `emitir/orden.rs`, que ya existe y ya despacha. Preguntar por que vive
       fuera
       -- `codegen/mod.rs`, `codegen/emitir/orden.rs`

   [ ] `parser/preprocessor.rs` son 1.204 lineas y **es el otro monolito**.
       Un preprocesador son tres trabajos con costes distintos: incluir
       ficheros (DATO: trae bytes de fuera), expandir macros (SILENCIO: una
       macro mal expandida compila y hace otra cosa) y las condicionales
       (NADA: se equivoca y no compila)
       -- `parser/preprocessor.rs`

   [ ] `parser/declarations.rs`, 1.044 lineas. Mismo examen: declarar un tipo,
       declarar una funcion y declarar un inicializador no cuestan lo mismo
       -- `parser/declarations.rs`
```

---

## 4. ⚠ Y LO QUE **NO** HAY QUE HACER, dicho por delante

```text
   [!] NO optimizar todavia. `docs/maestro/OPTIMIZACION_MAESTRO.md` es tajante:
       es LO ULTIMO, y antes hay que decir QUIEN pone el presupuesto. Las 28
       instrucciones se bajan DESPUES de que el codigo se pueda encontrar

   [!] NO tocar `decidir/`. Nacio el 09-09 con una regla que funciona --*el
       emisor no decide*-- y ya bajo de 35 a 28. Lo que falta no es otra regla:
       es MEDIR cuales de las 28 sobran

   [!] Y el banco es el juez, no yo. 537 filas, y el 09-09 cazo una version
       mia con 5 de 500. Cualquier corte que cambie un byte emitido tiene que
       justificarlo con una fila nueva
```

---

## 5. EL INSTRUMENTO QUE FALTA, y es el que desbloquea el resto

Hoy no hay forma de preguntar **cuantas instrucciones cuesta una escritura**
sin compilar un programa entero y contarlas a mano.

```text
   [ ] un banco de COSTE: un punado de fragmentos de C minimos --una escritura
       a un global, una a un array, una por puntero, un `+=`-- y su cuenta de
       instrucciones emitidas, como FILA de prueba. Igual que el banco de
       correccion, pero contando en vez de comparando
       -- `toolchain/lang/c/src/tests/`
```

★ Con eso, "28 instrucciones por escritura util" deja de ser una frase de un
documento y pasa a ser **un numero que sube o baja en cada commit** -- que es la
unica forma en que esta casa acepta una cifra.

# El plan largo: de "BMO COBOL compila" a "BMO COBOL lleva un banco"

> Escrito el 2026-08-03, el día que entró `COMP-3`.
> **Revisión 2 — verificado contra el código, línea por línea, y reordenado por
> lo que se midió.** Tres de las dependencias de la primera versión eran falsas
> y una faltaba. Ver *[Lo que cambió en la revisión
> 2](#lo-que-cambió-en-la-revisión-2-y-por-qué)*.
> **Revisión 3 — la decisión `1.0` está TOMADA (camino B) y `2.6 ROUNDED`
> hecho.** Con `1.0` decidida, el camino a *leer lo que ya existe* se queda sin
> candados y `0.5` pasa a ser lo siguiente.
> **Revisión 4 — la FASE 3 estaba mal medida, y a favor.** Sus tres primeras
> tareas (S + M + M) resultan ser **una sola**, `3.0`: el cursor ya existe en el
> kernel y el fichero entero ya vive en RAM. Lo único que falta de verdad es que
> FAT32 sepa **reemplazar**, y de paso eso arregla un fallo que ya está en el
> disco — ver *[FASE 3](#fase-3--el-sistema-debajo)*.
>
> Es la lista de tareas de [`BANCA_REAL.md`](BANCA_REAL.md): aquel documento dice
> **qué falta y por qué**; éste dice **en qué orden, qué bloquea a qué, y cómo se
> sabe que una está hecha**.
>
> Está hecho para avanzar **poco a poco**: cada casilla es una pieza que se
> puede entregar sola, con su prueba, sin dejar el compilador roto entre medias.

## Cómo se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuánto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO, y por QUÉ — comprobado en el código, no supuesto
⚠    tiene una decisión dentro que hay que tomar antes de escribir código
```

Y el tamaño, para poder repartir: **S** una sesión · **M** dos o tres · **L** una
semana de trabajo de verdad · **XL** la pieza grande de su fase.

## La regla que no se negocia

**Nada entra sin su fila en `cobol_feature_matrix_runs_correctly`**, que
EJECUTA el programa en `bmo_lower::emu` en vez de mirar sus bytes. Y cuando la
característica cambia **cómo se guarda** un dato —no qué se hace con él— hace
falta además una prueba que **sólo pueda pasar si el almacenamiento es real**.
La de `COMP-3` es el patrón a copiar: el mismo `12345` en un `PIC 9(3) COMP-3`
sale `345` y en un `PIC 9(3)` sale `12345`. Se comprueba mutando la
característica a no-operación y viendo **cuántos tests caen**. Si no cae
ninguno, la prueba no probaba nada.

## Y la regla de este documento

**Un ⛔ se gana midiendo.** La revisión 2 existe porque tres tareas estaban
marcadas como bloqueadas por razonamiento y no por lectura del código, y las
tres resultaron ser falsas. Antes de poner un ⛔ hay que abrir el fichero y
citarlo.

---

# ★ LA ESTRATEGIA: primero todo lo que no depende del sistema

**Decidido el 2026-08-03 por Eddi.** Está aquí arriba y no al final porque es lo
que decide qué se toca en cada sesión.

## Las dos listas, separadas

Toda tarea de este plan cae en una de dos, y hay que saber en cuál antes de
empezarla:

| | |
|---|---|
| **SIN candado** — sólo `toolchain/lang/cobol` y `toolchain/forge` | `0.5` records · `1.1` registro binario · `1.2` campos posicionales · `1.3` MOVE de grupo · `0.7` texto · `1.7` FILE STATUS · `1.6` EBCDIC · `2.2` PERFORM VARYING · `2.3` STRING · `2.4` INSPECT · `2.5` INITIALIZE · `2.6b` ON SIZE ERROR · `2.7` SEARCH · `2.8` COPY · `2.9` intrínsecas · `0.6` GO TO · `0.2` parser de tokens · `5.1` SORT |
| **CON candado** — pide kernel, ESTRATOS o una decisión de arquitectura | `3.1` EXTEND · `3.2` I-O · `3.3` posicionar · `3.4` ESTRATOS escribe · toda la **fase 4** (VSAM) · toda la **fase 7** (despachador) · `6.1` el enlazador y con él `6.2`–`6.6` |

## La regla, y por qué

> **Se hace primero TODO lo de la columna izquierda.**

1. **Ahí está el salto más grande que queda, y no tiene candado.** Leer
   **registros binarios de verdad** —campos en su sitio, importes empaquetados—
   es lo que separa *"COBOL nuevo"* de *"COBOL que abre los datos que ya
   tienes"*, y se comprobó que **no necesita seek**: `ARCH_OP_LEER` ya saca
   bytes crudos y está en el kernel y en el emulador. Es puro compilador.
2. **El trabajo de sistema no se pone más difícil por esperar.** Las tres
   operaciones que faltan son pequeñas y están descritas; el orden entre ellas y
   COBOL no cambia lo que cuestan.
3. **Cada sesión de COBOL entrega algo que corre.** Una de kernel no: hay que
   cambiar la superficie, el kernel y el emulador antes de que un `.cob` note
   nada.

## ⚠ Y el TECHO, dicho antes de que nadie lo suponga

**Haciendo sólo la columna izquierda se llega hasta el BATCH y no más.** Leer un
fichero, calcular, escribir otro — que es exactamente lo que un banco hace de
noche, y no es poco: es el 80 % del COBOL que hay escrito en el mundo.

Lo que **no** se alcanza sin la columna derecha:

- **Buscar una cuenta sin leer el fichero entero.** *"Dame la 4471-9982"* con
  cuatro millones de registros. Eso es el índice, y el índice pide `3.2` y `3.3`.
- **Modificar un registro en su sitio.** `REWRITE` y `DELETE` necesitan un
  handle que lea y escriba, y hoy el modo se fija al abrir.
- **Transacciones y varios usuarios a la vez.** Fase 7.

**Tres operaciones de kernel bloquean la pieza más grande del proyecto.** No son
una montaña — pero no se pueden saltar, y por eso están escritas aquí y no
escondidas en la fase 3.

---

# FASE 0 — El suelo

- [x] **0.1 · `VALUE` inicializa de verdad** — ✅ 2026-08-03
      Se emite al principio, después de repartir la pila y antes de la primera
      sentencia, **pasando por `store_var`** — así un `COMP-3` se inicializa
      empaquetado sin que el emisor de valores sepa que existen los nibbles.
      Sobre una tabla llena todas las casillas. Figurativas `ZERO`/`ZEROS`/
      `ZEROES`. Se rechaza con motivo: `VALUE` de texto, `VALUE` que no es un
      número, y `VALUE` sin PIC.

- [x] **0.3 · `OR` en las condiciones** — ✅ 2026-08-03
      La condición dejó de ser una `Vec` y es un **árbol** `Simple/Y/O`, porque
      `AND` liga más fuerte que `OR` y una lista plana no puede representar esa
      diferencia. Con **cortocircuito**, que no es una optimización: un operando
      puede ser un elemento de tabla y ahí la evaluación lleva guarda de rango.
      Cayeron con él los dos rechazos que dependían de él: **`88 … VALUE 1 THRU
      5`** y **`88 … VALUE 6, 7`**.

- [x] **0.4 · Párrafos y `PERFORM <párrafo>`** — ✅ 2026-08-03
      Un nombre de párrafo es una palabra sola con punto. Compilan
      `PERFORM <p>`, `PERFORM <p> THRU <q>`, `PERFORM <p> <n> TIMES` y
      `PERFORM <p> UNTIL <cond>`, más `EXIT`/`CONTINUE`.
      ★ El retorno se decide **en ejecución** (una ranura de pila con "en qué
      párrafo hay que volver"), no con un `ret` fijo: el mismo párrafo puede ser
      el final de un rango en una línea y estar en medio de otro en la de abajo.
      ★ De paso salió que **`STOP RUN` no emitía nada** — colaba por ser siempre
      la última línea, y ya estaba mal antes: un `STOP RUN` dentro de un `IF`
      se ignoraba en silencio.

- [ ] **0.2 · El parser sobre TOKENS como principal** — L ⚠
      **DEGRADADO A DEUDA, NO A BLOQUEO** (revisión 2). `tparser.rs` ya parsea un
      programa entero, pero `compile_source` sigue usando `parser.rs`, el
      analizador por líneas.
      Lo que la revisión 1 decía —que la fase 2 entera depende de esto— **es
      falso**: `parser.rs` ya consume varias líneas para `IF … END-IF` y
      `PERFORM … END-PERFORM`, y `EVALUATE … WHEN … END-EVALUATE` tiene
      exactamente esa forma. Los verbos de la fase 2 se pueden hacer hoy.
      Sigue mereciendo la pena, pero por calidad: **una gramática y no dos**.
      ⚠ La decisión sigue en pie: jubilar `parser.rs` de golpe (mover las 136
      pruebas a la vez) o convivir.

- [x] **0.5 · Records anidados con posiciones fijas** — ✅ 2026-08-03
      Grupos `01`/`05`/`10` con **cada campo en su byte, sin relleno** — porque
      la disposición de un registro *es el formato del fichero*, y un byte de
      padding es un byte que aparece en el disco. Vive en `registro.rs` y **NO
      reutiliza `bmo_abi::types::disposicion`** a propósito: aquélla alinea, que
      es lo correcto para C y veneno aquí. Es la excepción que confirma la regla
      de la casa — se comparte la REGLA cuando es la misma, y ésta no lo es.
      ★ **El ÁREA DE REGISTRO del camino B ya funciona**: un grupo tiene sus
      ranuras de trabajo *y* su área de bytes, y la traducción vive sólo donde el
      registro cruza. La otra mitad entró con esto: `bmo_lower::zoned`, donde un
      `DISPLAY` es **un byte por dígito con el signo sobrepunzado en el último**
      — que es por lo que un `PIC S9(5)` mide cinco bytes y no seis.
      ★ La prueba que no se puede fingir: un grupo con un `PIC 9(6)` movido a
      otro con dos `PIC 9(3)` da `123` y `456`. Campo a campo eso es imposible.

- [x] **0.6 · `GO TO` dentro de un párrafo** — ✅ 2026-08-03
      Salió faltando al escribir el ejemplo del nivel 8, y ese ejemplo ya lo usa:
      el descarte dentro de un rango `PERFORM … THRU` pasa de un interruptor y
      un `IF` a una línea.
      ★ Se emite como un `jmp rel32` al MISMO símbolo al que `PERFORM` hace
      `call`, y lo parchea la misma tabla — los dos son rel32 contra la
      instrucción siguiente, así que el parcheador no distingue ni tiene por qué.
      ★ Y lo que pasa después **sale gratis**: el párrafo al que se salta corre y
      su epílogo pregunta si es ahí donde había que volver. Si el `GO TO` fue a
      la salida del rango, vuelve; si no, sigue cayendo hasta encontrarla. Es lo
      que dice el estándar y no hizo falta escribir nada para ello.
      ⚠ Se rechaza desde el **cuerpo principal**: aquí un párrafo es una
      subrutina a la que se entra por `call`, y saltar dentro sin haber entrado
      por su `PERFORM` deja el `ret` del final sin dueño.
      ⛔ `GO TO … DEPENDING ON` todavía no.

- [x] **0.7 · Texto de verdad: `PIC X(n)` con contenido** — ✅ 2026-08-03
      ★ **Sin el límite de 8 caracteres que temía la revisión 2.** El texto no
      pasa por `rax` como todo lo demás: tiene su propio camino, que trabaja con
      **dirección y largo**. Así el ancho es el que diga la PICTURE y no el de un
      registro.
      Compilan: `VALUE "TEXTO"` (con espacios dentro), `MOVE` de literal y de
      campo a campo, `=` y `NOT =` contra literal o contra otro campo, y
      `DISPLAY`.
      ★ Todo **DESENROLLADO** cuando el otro lado es un literal: mover `"00"` a
      un campo son dos `mov` de inmediato, no un bucle. El texto viaja dentro de
      las instrucciones, como `console::write_const`.
      ★ **El relleno con espacios no es cosmético**: el campo se llena ENTERO
      cada vez que se escribe, así que un `MOVE` corto detrás de uno largo no
      deja cola. Hay un test para eso — un `FILE STATUS` que arrastra la letra de
      la operación anterior es peor que uno vacío.
      Se rechaza con motivo: comparar cadenas por ORDEN (`>`, `<`) porque depende
      del juego de caracteres, y mover texto a un campo numérico (eso es
      `FUNCTION NUMVAL`).

---

# FASE 1 — Leer los datos que ya existen

La segunda mitad de `COMP-3`. Un campo empaquetado vive bien **en memoria**, pero
el fichero sigue siendo **texto, un número por línea**. Un banco no da eso: da
registros de longitud fija con campos empaquetados dentro.

**Sin esta fase, BMO COBOL escribe programas nuevos y no puede leer nada de lo
que ya existe.**

- [x] **1.0 ★ LA DECISIÓN: dónde vive un campo de un registro** — ✅ **TOMADA
      el 2026-08-03 por Eddi: CAMINO B.**

      > **El `FD` tiene un ÁREA DE REGISTRO —un buffer de bytes del largo del
      > registro— y cada campo conserva su ranura de trabajo de 64 bits *y*
      > apunta a su posición dentro del buffer. `READ` llena el buffer y
      > DESEMPAQUETA cada campo a su ranura; `WRITE` EMPAQUETA al revés.**

      Los motivos, para que no haya que reconstruirlos dentro de seis meses:

      1. **Es lo que dice COBOL, no un rodeo.** El área de registro sólo vale
         entre un `READ` y el siguiente; el estándar lo dice con esas palabras.
         Empaquetar en la frontera no imita el modelo: *es* el modelo.
      2. **Media pieza ya está hecha.** `bmo_lower::packed` desempaqueta desde
         un puntero desde el 2026-08-03, que es exactamente lo que hace falta
         para un campo `COMP-3` dentro del buffer.
      3. **No toca nada de lo que corre en el Ryzen.** El camino A cambia cómo
         se guarda CADA dato del programa; éste sólo añade una capa en los dos
         sitios donde el registro cruza al disco.
      4. **El truncamiento no se cuela de tapadillo.** Con A, los `DISPLAY` de
         WORKING-STORAGE empezarían a truncar de un día para otro y la salida de
         programas que ya funcionan cambiaría. Con B eso sigue siendo una
         decisión aparte (1.5), tomable el día que se quiera y no como efecto
         secundario de querer leer un fichero.

      **Lo que se paga, dicho:** `REDEFINES` (1.4) sobre un registro **no
      aliasa de verdad** — dos vistas del mismo espacio serían dos juegos de
      ranuras. Cuando llegue 1.4 hay que rechazarlo con motivo o darle su
      propio mecanismo, y no fingir que funciona.

      **El problema, medido**: en `codegen.rs` el reparto de la pila hace
      `let aligned = (size + 7) & !7` **por dato**, y `load_var`/`store_var`
      mueven con `mov rax, [rbp+off]` de 64 bits. Un `PIC 9(3)` contiguo mide
      tres bytes: escribirlo con un `mov` de ocho **se lleva al vecino**.

      **Camino A — zoned decimal de verdad (1.5).** Cada dato pasa a medir lo que
      dice su PICTURE. Es lo correcto y es COBOL. Cuesta caro: toca todo lo que
      corre hoy en el Ryzen, y los `DISPLAY` **empezarán a truncar** como ya hace
      el `COMP-3`.

      **Camino B — área de registro + empaquetado en la frontera.** El `FD` tiene
      un **buffer de bytes** del largo del registro; cada campo conserva su
      ranura de trabajo de 64 bits *y* apunta a su posición dentro del buffer.
      `READ` llena el buffer y **desempaqueta** cada campo a su ranura; `WRITE`
      **empaqueta** al revés. Eso es exactamente lo que dice COBOL: el área de
      registro sólo vale entre un `READ` y el siguiente.
      - **A favor**: no toca nada de lo que ya funciona, y el `COMP-3` ya tiene
        media pieza hecha (`bmo_lower::packed` desempaqueta desde un puntero).
      - **En contra**: `REDEFINES` sobre un registro (1.4) no aliasa de verdad,
        y hay que decidir si se rechaza o se hace aparte.
      - **Coste**: una fracción de A.

      **Elegido: B.** Ver los motivos arriba. `A` queda como 1.5, para el día
      que se quiera truncamiento en WORKING-STORAGE — y ese día será por eso, no
      por poder leer un fichero.

- [x] **1.1 ★ Registro BINARIO de longitud fija** — ✅ 2026-08-03
      La revisión 2 acertó: **no necesitaba seek**. `ARCH_OP_LEER` ya saca bytes
      crudos y el cursor avanza **exactamente lo que devuelve** — comprobado en
      `ring0/obj/archivo.rs`, no supuesto.
      ★ **Pero había un detalle que sólo se ve escribiéndolo**: el paquete son
      SIETE bytes y un registro de banca mide 5, o 16, o 47. La última tirada de
      cada registro trae bytes de más **que son del registro siguiente**, y no se
      pueden devolver porque el cursor es del kernel.
      Por eso el área lleva **16 bytes detrás** con el resto pendiente, y el
      registro de después lo gasta antes de pedir nada. Sin eso, un fichero de
      registros de 5 bytes daría bien el primero y basura todos los demás — el
      fallo que no revienta y descuadra.
      El test que lo caza lee **tres** seguidos y mira el tercero, que es donde
      el error ya se acumuló dos veces.

- [x] **1.2 · Campos posicionales dentro del registro** — ✅ 2026-08-03
      Cada `05` en su offset, mezclando `COMP-3` y `DISPLAY` en el mismo
      registro. Entró **con** `1.1`, porque separados no sirven de nada: leer
      bytes crudos sin campos es un `memcpy`, y campos sin bytes es memoria.
      ★ El `READ`/`WRITE` mira si el `01` del `FD` es un GRUPO: si lo es, el
      fichero **no es texto** y va por el área. El camino de texto —una línea, un
      número— se queda para los ficheros que ya existían. **Son dos cosas
      distintas, no dos modos de la misma.**
      ★ Un registro binario se escribe **sin salto de línea**: mide lo que dice
      su copybook y un separador correría todo lo de detrás un byte.

- [x] **1.3 · `MOVE` de grupo** — ✅ 2026-08-03
      Entró **con** `0.5`, porque es su primer consumidor: sin él la disposición
      sería código sin usuario, que es justo lo que este repo no permite.
      Es una copia de **bytes** con `bmo_lower::memoria::copiar`, no campo a
      campo — que es lo que dice el estándar y lo que permite reinterpretar un
      registro. Se rechaza con motivo mezclar un grupo con un campo suelto: pide
      relleno con espacios, y eso necesita `0.7`.

- [x] **1.3b ★ El COPYBOOK (`--copybook`)** — ✅ 2026-08-03
      El compilador escupe el byte exacto de cada campo de cada registro, con su
      codificación y cómo se lee el signo. En banca ese documento es lo que se
      intercambia para que dos sistemas lean el mismo fichero, y **el que se
      mantiene a mano siempre acaba mintiendo**.
      ★ Éste no puede: sale de **la misma tabla que usa el codegen** para emitir
      el `READ` y el `WRITE`, así que no hay dos sitios donde pueda divergir. Es
      *tablas y no cerebros* aplicado a la documentación — el documento no
      describe el formato, **es** el formato.
      Marca cuáles cruzan de verdad (`[FICHERO]`, los que cuelgan de un `FD`) y
      cuáles son de WORKING-STORAGE, y distingue una PIC de **edición** como lo
      que es: una máscara de presentación, no almacenamiento.

- [x] **1.3c ★ El VISOR de registros (`--ver`)** — ✅ 2026-08-03
      Desde que un `COMP-3` sale al disco, el fichero **deja de poderse mirar**:
      los nibbles no son texto y un `cat` enseña basura. El compilador lo decodifica
      con el copybook de su propio programa, y enseña **el valor y los bytes
      crudos al lado**.
      ★ Lo que ninguna herramienta de fuera puede prometer: **lee con la misma
      regla con la que el programa escribió**. Los decodificadores del anfitrión
      (`packed::desempaquetar_en_rust`, `zoned::leer_en_rust`) están comparados
      contra los EMITIDOS sobre **todos** los patrones de dos bytes — 65 536
      comparaciones cada uno. Si divergieran, el visor enseñaría un importe y el
      programa leería otro, que es peor que no tener visor.
      ★ Si el fichero no es múltiplo del registro, **lo dice y enseña lo que
      sobra**: es el síntoma clásico del copybook equivocado.
      Comprobado con un fichero generado desde **Python**, no desde BMO.

- [ ] **1.4 · `REDEFINES`** — M ⚠ (depende de qué salga en 1.0)
      Dos vistas del mismo espacio. Con el camino B hay que decidir: rechazarlo
      con motivo, o darle su propio mecanismo.

- [ ] **1.5 ⚠ `DISPLAY` como ZONED DECIMAL real** — L ⚠
      El camino A de 1.0. Deja de ser obligatorio si se elige B, pero sigue
      siendo lo correcto a largo plazo y lo único que hace truncar a un
      `DISPLAY` de WORKING-STORAGE.

- [ ] **1.6 · EBCDIC ↔ ASCII al leer** — M
      Los datos de fuera vienen en EBCDIC. Una tabla de 256 entradas → **una
      tabla y no un cerebro**; va en `bmo-lower` junto a `packed`, por el mismo
      motivo: no es semántica de ningún lenguaje.

- [~] **1.7 · `FILE STATUS`** — ✅ 2026-08-03, **con los códigos que se pueden
      dar de verdad y sólo ésos**
      `SELECT … FILE STATUS IS <campo>`, y el código de dos letras se deja
      después de `OPEN`, `READ`, `WRITE` y `CLOSE`.
      ★ Sólo se ponen **`00`, `10`, `30` y `35`**, que son los que la puerta
      permite distinguir: el `OPEN` contesta con un handle o un cero, el `READ`
      con un sí o un no, y el `CLOSE` con un guardó o no guardó. Los demás
      (`37` modo incompatible, `41`/`42` doble apertura o cierre) **no se pueden
      separar todavía** — de un cero no se saca el motivo. Inventarlos mandaría
      a arreglar lo que no está roto. El día que `KIND_ARCHIVO` traiga un
      código, aquí sólo hay que ampliar la tabla; por eso queda `[~]` y no `[x]`.

      ★★ **El `30` del `CLOSE` entró el 2026-08-03, y tapaba un fallo grave.**
      `emit_close` escribía `"00"` **a pelo, sin mirar `rax`**, así que el único
      momento en el que un fichero llega al disco era también el único que no
      se comprobaba: un programa que se había molestado en declarar `FILE
      STATUS` recibía "todo bien" con el fichero sin guardar. Y pasa de verdad —
      hoy `CREAR` no puede reemplazar un fichero existente, o sea que la segunda
      corrida de cualquier programa que escriba su salida caía por ahí. Ver
      `3.0`.
      Para poder probarlo hizo falta que el emulador supiera **fingir un disco
      que dice que no** (`Machine::fallar_al_guardar`): mientras `CERRAR`
      contestó `1` siempre, el camino del fallo era código que ninguna prueba
      podía pisar.
      ★ Se comprueba que el campo **existe y mide dos letras**: si no, el
      programa compararía contra basura y `IF ST = "00"` daría falso siempre —
      un batch que se para cada noche sin motivo.
      El código de dos dígitos (`00` bien, `10` fin de fichero, `23` no
      encontrado, `35` no existe…). **Todo programa de banca lo mira después de
      cada operación.**
      ✅ **El dato ya está ahí**: `archivo::abrir_const` deja el handle en `rax`
      y **cero si no se pudo abrir**, y `leer_linea` deja `rax = 0` al acabarse
      el fichero. No hace falta nada del kernel — falta el campo donde ponerlo,
      y eso es 0.7, porque `FILE STATUS` es un `PIC XX`.

---

# FASE 2 — Los verbos que el código real usa

★ **DESBLOQUEADA EN LA REVISIÓN 2.** La revisión 1 decía "todos dependen de
0.2 (el parser sobre tokens)". **Es falso.** `parser.rs` ya consume varias líneas
para `IF … END-IF` y `PERFORM … END-PERFORM`, y las sentencias de esta fase
tienen la misma forma. Se pueden hacer **hoy**, y son lo que más código real
desbloquea por hora de trabajo.

- [x] **2.1 ★ `EVALUATE`** — ✅ 2026-08-03
      Las dos formas compilan, con `WHEN OTHER`, `WHEN a THRU b` y `WHEN a, b`.
      ★ El `THRU` y la coma **no costaron una línea de gramática nueva**: la
      expansión "¿está este campo en este conjunto?" se sacó a
      `Condicion::de_valores` y la comparten el nivel 88 y el `WHEN`. Y como las
      dos sintaxis llegan al codegen como el MISMO árbol, el emisor son cinco
      líneas y heredan cortocircuito y precedencia gratis.
      Se rechaza con motivo: `EVALUATE FALSE`, varios sujetos (`ALSO`), un `WHEN`
      después del `OTHER` (no se alcanza nunca), sentencias entre el `EVALUATE` y
      el primer `WHEN`, y las sentencias en la misma línea que su `WHEN`.
      Dos formas, las dos corrientes en banca:
      ```cobol
      EVALUATE TIPO-MOV              EVALUATE TRUE
          WHEN 1 …                       WHEN SALDO > 1000.00 …
          WHEN 2 THRU 5 …                WHEN SALDO > 100.00 …
          WHEN 6, 7 …                    WHEN OTHER …
          WHEN OTHER …               END-EVALUATE
      END-EVALUATE
      ```
      La segunda es **la tabla de decisión**, y es como un banco escribe un
      escalado de comisiones. Las dos caen sobre el árbol de condiciones que
      entró con 0.3: `WHEN a THRU b` y `WHEN a, b` son exactamente la expansión
      que ya hace un nivel 88.
      `WHEN … ALSO` (varios sujetos) puede esperar y se rechaza con motivo.

- [x] **2.2 · `PERFORM VARYING` completo** — ✅ 2026-08-03
      `FROM`/`BY`/`UNTIL`, en línea y de párrafo, con **cuantos `AFTER` haga
      falta** (probado con tres).
      ★ El codegen es **recursivo sobre los controles**, y de ahí sale gratis lo
      que de verdad define un `AFTER`: **el de dentro se reinicia cada vez que el
      de fuera avanza**. Escrito como un bucle plano habría que acordarse de
      reiniciar a mano, y olvidarlo recorre la tabla en diagonal — la primera
      fila entera y de las demás sólo la última columna.
      El paso puede ser negativo, y con `WITH TEST BEFORE` un bucle cuya
      condición ya se cumple **no da ni una vuelta**.
      ⚠ Queda dicho en el AST: `UNTIL` dice cuándo **PARAR**, no cuándo seguir.
      Al revés que el `while` de casi todo lo demás, y confundirlo sobre una
      tabla es un subíndice fuera de rango.

- [x] **2.6 ★ `ROUNDED`** — ✅ 2026-08-03
      **Los SEIS modos del estándar**, en las cinco aritméticas, con
      `ROUNDED MODE IS <modo>`. El emisor vive en `bmo_lower::redondeo` por la
      misma razón que `packed` y `fmt`: partir un entero y decidir el último
      dígito es aritmética, no la semántica de un lenguaje.
      ★ Van **todos** y no sólo el clásico porque el redondeo es una **decisión
      legal**: hay jurisdicciones que obligan al del banquero (`NEAREST-EVEN`)
      precisamente porque el clásico tiene sesgo — en una muestra grande los
      empates siempre suben. Hay un test que lo enseña con cuatro empates
      seguidos: el clásico inventa dos céntimos y el del banquero cuadra.
      ★ **Se redondea el RESULTADO, no los operandos**: la operación se hace en
      la escala más alta que aparezca y se baja una sola vez. Con los modos
      asimétricos no es lo mismo — el techo de `-9.995` es `-9.99`, pero
      redondeando el `9.995` primero sale `-10.00`.
      ★ Y hay **dos implementaciones de la misma regla** —la emitida y una en
      Rust para los literales— con un test que las compara valor a valor en
      todo el rango. Dos que tienen que coincidir prueban más que una comparada
      contra una tabla escrita a mano.

- [x] **2.6b · `ON SIZE ERROR`** — ✅ 2026-08-03
      En las cinco aritméticas, con `NOT ON SIZE ERROR` y su `END-<verbo>`.
      ★ **Cuando no cabe, el destino se queda COMO ESTABA.** Ésa es la parte que
      importa y por eso la comprobación va antes del guardado: deja el saldo
      anterior intacto para que el programa lo escriba en un informe de rechazos
      y siga. Guardar el número recortado y avisar después sería avisar de un
      descuadre ya hecho.
      ★ **Dividir entre cero es un desborde, no un fallo del CPU.** Sin eso el
      `idiv` levanta `#DE` y el proceso muere sin decir por qué — en un batch,
      un registro malo se lleva el proceso entero.
      ⚠ La cláusula tiene que **empezar en la línea del verbo**: si no, un
      `ADD A TO B` a secas y uno que sigue abajo se leen igual, y adivinar
      significaría tragarse las sentencias de después.
      ⚠ Y salió una divergencia que queda FIJADA CON TEST: sin la cláusula, BMO
      **no recorta** (guarda `1023` en un `PIC 9(3)`) porque un `DISPLAY` sigue
      siendo un entero de 64 bits — la tarea `1.5`. El día que entre, ese test
      falla, que es exactamente el aviso que hace falta.

- [ ] **2.5 · `INITIALIZE`** — S ⛔ (0.5 para grupos)
      Sobre un dato suelto se puede hoy. `bmo_lower::memoria::rellenar` ya está.

- [ ] **2.7 · `SEARCH` / `SEARCH ALL`** — M
      Búsqueda lineal y binaria en tabla. `SEARCH ALL` pide `ASCENDING KEY`.
      Es el sustituto barato de un índice **dentro de memoria**, y tapa parte de
      lo que la fase 4 no puede dar todavía.

- [~] **2.3 · `STRING`** — ✅ 2026-08-03, **`UNSTRING` no**
      `STRING <fuentes> DELIMITED BY SIZE INTO <destino>`, leído en varias
      líneas —que es como se escribe— y **resuelto entero al compilar**: cada
      fuente tiene un ancho conocido, así que el destino se llena por trozos sin
      un puntero que avance en ejecución.
      El destino se pone a espacios ANTES, para que lo que no se llene no se
      quede con lo del `STRING` de antes.
      ⛔ `DELIMITED BY SPACE` o por un carácter cortan por un largo que sólo se
      sabe en ejecución: es otro emisor y se rechaza con ese motivo. Y
      `UNSTRING` —partir uno en varios— es la mitad que falta.

- [x] **2.4 · `INSPECT`** — ✅ 2026-08-03
      `TALLYING <n> FOR ALL "<c>"` y `REPLACING {ALL|LEADING} "<a>" BY "<b>"`,
      con las figurativas `SPACE` y `ZERO`.
      ★ `ALL` y `LEADING` son **dos formas y no una con una opción**, porque
      sobre un importe dan números distintos: `"  12 34"` con `LEADING " " BY
      "0"` da `"0012 34"` y con `ALL` daría `"0012034"`.
      Los emisores viven en **`bmo_lower::texto`**, hermana de `memoria`:
      aquélla trae los verbos de C (`memcpy`, `memset`) y ésta los que COBOL
      escribe `INSPECT` y Ada `Index`/`Replace_Slice`. Y con la misma frontera —
      **el largo va explícito, aquí no hay NUL que buscar**, que es la
      diferencia entre un campo de COBOL y una cadena de C.
      ⛔ Buscar o sustituir una CADENA es búsqueda de subcadena y se rechaza:
      aceptarlo mirando sólo la primera letra contaría de más.

- [ ] **2.8 · `COPY … REPLACING`** — M
      **Así se comparten los layouts de registro entre programas.** Sin esto,
      cada programa reescribe el `01` del fichero a mano y se descuadran solos.
      No depende de nadie: es inclusión de texto antes de analizar.

- [ ] **2.9 · Las intrínsecas que importan (~15 de 55)** — M
      `NUMVAL`, `NUMVAL-C`, `CURRENT-DATE`, `INTEGER-OF-DATE`,
      `DATE-OF-INTEGER`, `LENGTH`, `MAX`, `MIN`, `MOD`, `REM`, `UPPER-CASE`,
      `LOWER-CASE`, `TRIM`, `ORD`, `WHEN-COMPILED`. La tabla `INTRINSIC[]` ya
      está generada; falta la semántica de cada una.
      ⚠ `CURRENT-DATE` necesita **reloj**, y hoy la superficie sólo da TSC.

---

# FASE 3 — El sistema debajo

⛔ **Nada de esto se arregla en `lang/cobol`.** Comprobado en
`platform/abi/bmo-abi/src/syscalls/surface.rs` y en
`Ultra_kernel_x86-64/kernel/src/ring0/obj/archivo.rs`.

Lo que la puerta **ya da**: `TASK_OP_ARCHIVO_ABRIR` (0x10) y `_CREAR` (0x11);
sobre el handle, `ARCH_OP_LEER` (7 bytes crudos), `ARCH_OP_LEER_LINEA`,
`ARCH_OP_ESCRIBIR`, `ARCH_OP_TAMANO` y `ARCH_OP_CERRAR`.

## ★ Revisión 4 (2026-08-03): las tres primeras son UNA, y no la que se creía

Medido en el código, no supuesto. Las tres tareas de abajo se escribieron
como S + M + M **suponiendo que faltaban tres mecanismos distintos**. Faltan
dos cosas, y sólo una es de verdad:

| Lo que decía el plan | Lo que hay en el código |
|---|---|
| *"No existe ninguna operación de cursor"* | **Existe**: `CURSOR[i]` en `obj/archivo.rs`, uno por ranura, y `leer`/`leer_linea` ya lo mueven. `3.3` es exponerlo con una guarda de rango — decenas de líneas, no una M |
| *"Un handle que lea y escriba"* | El fichero **entero vive ya en RAM** por ranura (marcos contiguos que se doblan al llenarse) y `cerrar` lo vuelca de una vez. Leer-y-escribir no pide modelo nuevo: pide que `abrir` deje `ESCRIBE = true` y que `escribir` respete el cursor en vez de añadir al final |
| *"Añadir al final"* | Cae solo con lo anterior: `CURSOR = LARGO` al abrir |

★ **Lo que falta de verdad es que FAT32 sepa REEMPLAZAR.**
`create_file_in_dir` devuelve `WriteError::Exists` si el nombre ya está, y
`archivo::crear()` **no lo comprueba al abrir**: el fallo aparece en `cerrar()`,
que devuelve `0` y sólo deja un `warn` en la CABINA.

La consecuencia se ve hoy y no es teórica: **un programa que escriba su fichero
sólo es honesto la primera vez que se corre.** El nivel 10 de los ejemplos
escribe tres cuentas, las relee y las imprime; en la segunda corrida no guarda
nada, y como relee el mismo fichero con los mismos valores **la pantalla sale
idéntica**. Es la peor forma de un fallo: la que se parece a funcionar.

Las piezas para arreglarlo están puestas — `free_chain` y `mark_cluster_eoc` ya
viven en el driver. Es liberar la cadena vieja, escribir la nueva y reescribir
el primer cluster y el tamaño en la entrada de directorio.

**Por eso `3.0` va delante de las otras tres y ninguna se puede entregar sin
ella.**

- [ ] **3.0 ★ FAT32: reemplazar un fichero que ya existe** — M ⛔ kernel
      El bloqueante común de `3.1`, `3.2` y `3.3`, y además lo que hace que un
      programa se pueda correr dos veces sin mentir. Su prueba no es que
      compile: es **correr el nivel 10 dos veces con un saldo cambiado y ver el
      nuevo**.

- [ ] **3.1 · `KIND_ARCHIVO`: modo EXTEND** — S ⛔ (3.0)
      Añadir al final. Hoy `OPEN EXTEND` se rechaza a propósito: sólo hay
      `_CREAR`, que crea de cero, así que compilarlo como `OUTPUT` borraría el
      histórico y el programa parecería funcionar hasta que alguien buscara el
      mes pasado.

- [ ] **3.2 ★ `KIND_ARCHIVO`: modo I-O** — S ⛔ (3.0)
      Un handle que **lea y escriba**. Es lo que bloquea `REWRITE` y `DELETE`,
      o sea **lo que hace que un KSDS sea un KSDS y no un listado ordenado**.
      Baja de M a S por la revisión 4: el buffer completo en RAM ya da la
      semántica; lo que falta es el modo y el volcado, que es `3.0`.

- [ ] **3.3 ★ `KIND_ARCHIVO`: posicionar por byte** — S ⛔ (3.0)
      Exponer el `CURSOR` que ya existe. Sin esto no hay acceso **directo** a
      nada. (Pero sí hay acceso **secuencial** binario — ver 1.1.)
      Baja de M a S por la revisión 4.

- [ ] **3.4 ★ ESTRATOS: crear objetos y ESCRIBIR** — XL ⛔ ESTRATOS
      Hoy monta, lee y sabe commitear (`sellar()` en `ring0/fsys/estratos.rs`, y
      la máquina de estados de la transacción probada en el anfitrión). **Falta
      crear.** Sin esto, un índice sólo cabe sobre `KIND_ARCHIVO` — y ahí se
      pierden las tres cosas que hacen que este índice sea mejor que el de z/OS.

---

# FASE 4 — VSAM: de listados a banca

★★ **La fase que decide.** *"Dame la cuenta 4471-9982"* no puede significar leer
cuatro millones de registros. **Sin índice no hay banca, hay listados.**

- [ ] **4.1 · RRDS — registros por número** — M ⛔ (3.3)
- [ ] **4.2 · ESDS — secuencial de sólo añadir** — S ⛔ (3.1)
- [ ] **4.3 ★ KSDS, la LECTURA** — XL ⛔ (3.2, 3.3)
      El B-tree. `RECORD KEY`, `READ … KEY IS`, `START`, `READ NEXT`.
- [ ] **4.4 · KSDS, la ESCRITURA** — L ⛔ (3.2, 4.3)
- [ ] **4.5 · `ALTERNATE RECORD KEY … WITH DUPLICATES`** — L ⛔ (4.4)
      El índice por DNI además del de número de cuenta.
- [ ] **4.6 ★ El índice SOBRE ESTRATOS** — XL ⛔ (3.4, 4.4)
      Copy-on-write, transaccional (**no hay índice a medias**) y auditable.
      **Es lo que un auditor quiere y VSAM sobre z/OS no le da.**

---

# FASE 5 — SORT

- [ ] **5.1 · `SORT` externo** — L
      ✅ **Menos bloqueado de lo que decía la revisión 1.** No hace falta
      `EXTEND`: una mezcla por tramos puede escribir **cada pasada a un fichero
      nuevo** con `TASK_OP_ARCHIVO_CREAR`, que ya existe. Cuesta E/S de más y es
      perfectamente honesto para empezar.
- [ ] **5.2 · `MERGE`** — M ⛔ (5.1)
- [ ] **5.3 · `INPUT PROCEDURE` / `OUTPUT PROCEDURE`** — M ⛔ (5.1)
      Con `RELEASE` y `RETURN`. Los párrafos que necesita ya están (0.4).

---

# FASE 6 — Programas que se llaman, y el batch (JCL sustituido)

- [ ] **6.1 ★⚠ LA DECISIÓN DEL ENLAZADOR** — ⚠ decisión, no código
      Escrita y sin tomar en `toolchain/forge/README.md`, con los dos caminos
      medidos: **A** enlazador de verdad (semanas; `bex-link` produce imágenes ya
      enlazadas a base fija y dos de ésas no se concatenan) y **B** funciones
      sintetizadas (una sesión; el mecanismo ya corre en metal con
      `__bmo_syscall_stub`).
      **Una decisión, tres desbloqueos**: `CALL` de COBOL, la libc de C, y C++
      con unidades separadas.
- [ ] **6.2 · `CALL` estático** — L ⛔ (6.1)
- [ ] **6.3 · `LINKAGE SECTION` / `PROCEDURE DIVISION USING`** — M ⛔ (6.2)
- [ ] **6.4 · `CALL` dinámico y `CANCEL`** — L ⛔ (6.1)
- [ ] **6.5 · `RETURN-CODE`** — S ⛔ superficie
      ⚠ Comprobado: `bmo_lower::task::exit` **no acepta código de salida** y
      `TASK_OP_EXIT` tampoco lo lleva (el kernel hace revoke+reap). Esto toca la
      superficie congelada, no sólo COBOL.
- [ ] **6.6 · El batch declarativo que sustituye a JCL** — M ⛔ (6.5)
      **Sustituir, no clonar**: planificador con dependencias, asignación de
      recursos y códigos de retorno, en un TOML que se lee.

---

# FASE 7 — El despachador (CICS sustituido)

★ CICS pasó cincuenta años atornillando transacciones sobre un sistema de
ficheros que no las tenía. ESTRATOS las tiene en el fondo. **Lo que falta no es
la transaccionalidad — es el despachador.**

- [ ] **7.1 ★ El despachador de transacciones** — L ⛔ (3.4, 6.2)
- [ ] **7.2 · `SYNCPOINT` / `ROLLBACK` desde COBOL** — M ⛔ (7.1)
      El `SYNCPOINT` es el superbloque alterno; el `ROLLBACK` es **no commitear**.
- [ ] **7.3 · Modelo pseudo-conversacional** — L ⛔ (7.1)
      **No hay un proceso vivo por usuario** — eso es lo que deja a un mainframe
      llevar miles de terminales.
- [ ] **7.4 · Seguridad por capabilities (sustituye a RACF)** — M ⛔ (7.1)
- [ ] **7.5 ⚠ Bloqueo de registro / concurrencia** — L ⚠ ⛔ (4.4, 7.1)
      ⚠ Copy-on-write da aislamiento de lectura gratis pero **no resuelve la
      escritura concurrente**: hay que elegir entre bloqueo pesimista y detección
      de conflicto al commitear.

---

# FASE 8 — Lo que hace que sea un BANCO y no un programa

- [ ] **8.1 · Auditoría de verdad** — M ⛔ (4.6)
      La versión anterior sigue ahí; falta **poder preguntarla**.
- [ ] **8.2 · Cierre contable y cuadre** — L ⛔ (5.1, 6.6)
- [ ] **8.3 ★ Un banco pequeño, de punta a punta** — XL
      Alta de cuenta, movimiento, consulta por número **y por DNI**, extracto,
      cierre diario. **En el Ryzen, no en el emulador.**

---

## Lo que cambió en la revisión 2, y por qué

Cuatro correcciones, todas por abrir el fichero en vez de razonar sobre él.

**1. La FASE 2 no estaba bloqueada por el parser de tokens.** `parser.rs` ya
consume varias líneas (`parse_if`, `parse_perform`), y `EVALUATE … WHEN …
END-EVALUATE` tiene la misma forma. Eso mueve la fase entera de "después de una
L" a "se puede empezar hoy", y con ella el verbo que más falta hace.

**2. `1.1` (registro binario) no necesita seek.** `ARCH_OP_LEER` ya saca bytes
crudos sin cortar por el salto de línea, y está en el kernel y en el emulador.
Leer un registro de largo fijo **en secuencia** es repetirlo. El seek hace falta
para el acceso **directo** (fase 4), no para leer un fichero de principio a fin.

**3. Apareció una tarea que no estaba: `0.7`, el texto.** `FILE STATUS` es un
`PIC XX`, y hoy un `PIC X` no guarda caracteres. La revisión 1 daba `1.7` por
barata y sin dependencias; lo es, pero cuelga de que exista el texto. Con ella
cuelgan también `STRING`, `INSPECT` y `EBCDIC`.

**4. `SORT` no necesita `EXTEND`.** Cada pasada de la mezcla puede escribir a un
fichero nuevo con la operación de crear que ya existe.

Y una que sigue en pie de la revisión 1: **`0.5` no se puede hacer con el
reparto de pila de hoy**, pero ahora hay dos caminos y no uno (ver `1.0`).

## El orden corto

```
HECHO   0.1 VALUE · 0.3 OR (+ los 88 con THRU) · 0.4 PARRAFOS · COMP-3

HECHO   2.1 EVALUATE (las dos formas) · 2.6 ROUNDED (los seis modos)
        1.0 LA DECISION ── TOMADA: camino B, area de registro

        ══════ SIN CANDADO: todo esto es COBOL y va PRIMERO ══════

HECHO   0.5 RECORDS + el AREA + 1.3 MOVE de grupo + bmo-lower::zoned

HECHO   1.1 + 1.2 REGISTROS BINARIOS ── LEER LO QUE YA EXISTE
        un fichero de largo fijo, campos en su byte, importes empaquetados

HECHO   0.7 TEXTO ── PIC X con caracteres, sin limite de ancho

HECHO   1.7 FILE STATUS ── 00, 10 y 35: los que se pueden dar de verdad

HECHO   2.4 INSPECT · 2.3 STRING (falta UNSTRING)

HECHO   2.6b ON SIZE ERROR ── y dividir entre cero deja de matar el proceso

HECHO   0.6 GO TO ── y el ejemplo del nivel 8 ya no necesita el interruptor

HECHO   2.2 PERFORM VARYING ── con AFTER, y el reinicio sale de la recursion

AHORA   2.7 SEARCH · 2.8 COPY · 2.9 intrinsecas · UNSTRING · 5.1 SORT

LUEGO   0.7 TEXTO ──→ 1.7 FILE STATUS · 2.3 STRING · 2.4 INSPECT · 1.6 EBCDIC
        2.6b ON SIZE ERROR · 0.6 GO TO · 2.2 PERFORM VARYING · 2.7 SEARCH
        2.8 COPY · 2.9 intrinsecas · 5.1 SORT · 0.2 parser de tokens

        ══════ hasta aqui se llega al BATCH, y ahi esta el TECHO ══════

KERNEL  3.1 EXTEND · 3.2 I-O · 3.3 POSICIONAR · 3.4 ESTRATOS ESCRIBE
                          └──→ FASE 4 (VSAM) ──→ FASE 7 (despachador)
        tres operaciones pequenas, y sin ellas no hay INDICE ni consulta viva

APARTE  6.1 EL ENLAZADOR ──→ CALL, y de paso la libc y C++
```

**Si hay que elegir UNA cosa por sesión**:
~~`0.1`~~ → ~~`0.3`~~ → ~~`0.4`~~ → ~~`2.1`~~ → ~~`2.6`~~ → ~~`1.0`~~ →
~~`0.5`~~ → ~~`1.3`~~ → ~~`1.2`~~ → ~~`1.1`~~ → ~~`0.7`~~ → ~~`1.7`~~ →
~~`2.4`~~ → ~~`2.3`~~ (falta `UNSTRING`) → ~~`2.6b`~~ → ~~`0.6`~~ → ~~`2.2`~~ →
**`2.7`** → `2.8` → `5.1` → `1.6` → `2.9` → `1.5` → `0.2` → **luego el kernel**:
`3.1` → `3.3` → `3.2` → `3.4` → fase 4 …

★ **`0.5` sube a lo siguiente** porque la decisión `1.0` ya está tomada y con
ella deja de tener candados. Es el primer eslabón de *leer lo que ya existe*,
que es lo que separa "COBOL que compila" de "COBOL que sirve para un banco".

★ **Y el kernel va al final de la lista de COBOL, no al principio**, por la
[estrategia de arriba](#-la-estrategia-primero-todo-lo-que-no-depende-del-sistema):
ahí está el salto que queda sin candado, y las tres operaciones que faltan no se
ponen más difíciles por esperar. Pero **están en la lista**, no descartadas:
sin ellas el techo es el batch.

---

## El límite, dicho aquí también

Nada de esta lista convierte a BMO COBOL en un **destino de migración desde
z/OS**. Ese código lleva cuarenta años escrito contra CICS, JCL, VSAM y las
extensiones de IBM *tal cual son*, no contra equivalentes mejores.

Esto es para **sistemas que se escriben ahora**, y pequeños. Lo que esta lista sí
consigue, si se termina, es que un banco pequeño pueda funcionar encima — con
auditoría que z/OS no da, y sin pagar licencia a nadie.

---

## Registro de lo hecho

| Fecha | Qué entró | Dónde |
|---|---|---|
| 2026-08-03 | ★ **`COMP-3` real** — el dato vive en nibbles, del ancho que dice su PICTURE | `bmo-lower::packed` + `codegen.rs` · ejemplo `7-empaquetado/` |
| 2026-08-03 | **0.1 `VALUE`** inicializa de verdad (se parseaba y no se emitía nunca) | `codegen::emit_valores_iniciales` |
| 2026-08-03 | **0.3 `OR`** — la condición es un árbol con cortocircuito; caen los `88` con `THRU` y con varios valores | `ast::Condicion` + `codegen::emit_jump_if_true/false` |
| 2026-08-03 | ★ **0.4 PÁRRAFOS** y las cuatro formas del `PERFORM` fuera de línea; `STOP RUN` termina de verdad | `codegen::emit_parrafos` · ejemplo `8-parrafos/` |
| 2026-08-03 | ★ **2.1 `EVALUATE`** — con sujeto y `EVALUATE TRUE`; `THRU` y listas compartidos con el nivel 88 | `parser::parse_evaluate` + `Condicion::de_valores` |
| 2026-08-03 | ★ **2.6 `ROUNDED`** — los seis modos del estándar en las cinco aritméticas; se redondea el RESULTADO | `bmo-lower::redondeo` + `codegen.rs` |
| 2026-08-03 | **Bug de precisión** que destapó `ROUNDED`: `COMPUTE` recortaba los operandos ANTES de operar | `codegen::emit_compute` (escala de trabajo) |
| 2026-08-03 | ★ **1.0 decidido** — camino B: área de registro con empaquetado en la frontera | este documento, §FASE 1 |
| 2026-08-03 | ★★ **0.5 + 1.3** — grupos con cada campo en su byte, el ÁREA DE REGISTRO funcionando, y `MOVE` de grupo por bytes | `cobol/registro.rs` + `bmo-lower::zoned` + `codegen.rs` |
| 2026-08-03 | ★ **El COPYBOOK** (`--copybook`) — el byte exacto de cada campo, sacado de la MISMA tabla que emite el `READ` | `registro::Disposicion::copybook` |
| 2026-08-03 | ★★★ **1.1 + 1.2 REGISTROS BINARIOS** — largo fijo, campos en su byte, y el resto de siete bytes bien llevado | `bmo-lower::archivo::leer_bytes` + `codegen::emit_read/emit_write` |
| 2026-08-03 | ★ **El VISOR** (`--ver`) — decodifica un fichero binario con el copybook, y sus lectores están atados a los emitidos | `registro::Disposicion::ver` + `*::*_en_rust` |
| 2026-08-03 | ★ **0.7 TEXTO** — `PIC X(n)` con caracteres de verdad, sin límite de ancho; desbloquea FILE STATUS, STRING e INSPECT | `codegen.rs` (camino de dirección+largo) |
| 2026-08-03 | **1.7 `FILE STATUS`** — `00`/`10`/`35`, los que la puerta permite distinguir, y sólo ésos | `codegen::emit_estado*` |
| 2026-08-03 | **2.4 `INSPECT`** (`TALLYING`, `REPLACING ALL`/`LEADING`) y **2.3 `STRING`** | `bmo-lower::texto` + `codegen.rs` |
| 2026-08-03 | **2.6b `ON SIZE ERROR`** — el destino no se toca cuando no cabe, y dividir entre cero deja de matar el proceso | `codegen::emit_guardar_con_desborde` |
| 2026-08-03 | **0.6 `GO TO`** — el descarte dentro de un rango, con el mismo símbolo que usa `PERFORM` | `codegen.rs` · ejemplo `8-parrafos/` actualizado |
| 2026-08-03 | **2.2 `PERFORM VARYING`** con `AFTER` — el reinicio del interior sale de la recursión | `codegen::emit_varying` |

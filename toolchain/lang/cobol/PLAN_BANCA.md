# El plan largo: de "BMO COBOL compila" a "BMO COBOL lleva un banco"

> Escrito el 2026-08-03, el día que entró `COMP-3`.
> **Revisión 2 — verificado contra el código, línea por línea, y reordenado por
> lo que se midió.** Tres de las dependencias de la primera versión eran falsas
> y una faltaba. Ver *[Lo que cambió en la revisión
> 2](#lo-que-cambió-en-la-revisión-2-y-por-qué)*.
> **Revisión 3 — la decisión `1.0` está TOMADA (camino B) y `2.6 ROUNDED`
> hecho.** Con `1.0` decidida, el camino a *leer lo que ya existe* se queda sin
> candados y `0.5` pasa a ser lo siguiente.
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

- [ ] **0.6 · `GO TO` dentro de un párrafo** — S
      Salió faltando al escribir el ejemplo del nivel 8: sin `GO TO`, el descarte
      dentro de un rango `PERFORM … THRU` se escribe con un interruptor. Es
      COBOL legítimo y su destino ya existe (las etiquetas de párrafo).

- [ ] **0.7 · Texto de verdad: `PIC X(n)` con contenido** — M ⚠
      Hoy un `PIC X` reserva sitio pero **se carga y guarda como un entero de 64
      bits**, así que no hay campos de texto: por eso `VALUE "HOLA"` se rechaza.
      Lo piden `FILE STATUS` (que es `PIC XX`), los literales de comparación
      (`IF ST = "00"`), `STRING`/`UNSTRING`/`INSPECT` y cualquier registro con
      un nombre dentro.
      ⚠ Decisión: hasta 8 caracteres caben en la ranura de 64 bits que ya hay
      (el mismo empaquetado que usa `console::write_const`); más de 8 pide un
      buffer aparte. Empezar por ≤ 8 es honesto y cubre `FILE STATUS`, códigos y
      claves cortas — pero hay que **decir dónde está el límite**.

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

- [ ] **1.1 ★ Registro BINARIO de longitud fija** — M
      ✅ **DESBLOQUEADO en la revisión 2.** La revisión 1 decía "⛔ necesita
      posicionar por byte (3.3)" y **es falso**: leer un registro de largo fijo
      **secuencialmente** no necesita seek ninguno.
      `ARCH_OP_LEER` (0x01) ya existe, saca **7 bytes crudos sin cortar por el
      salto de línea**, y está implementado en el kernel (`ring0/obj/archivo.rs`)
      **y** en el emulador. Un registro de 40 bytes son seis llamadas.
      Lo que falta es de esta casa: un `leer_bytes` en `bmo_lower::archivo`
      (hermano de `leer_linea`) y `RECORD CONTAINS n CHARACTERS` en el parser.
      ⛔ Sólo depende de **1.0** y **0.5**.

- [ ] **1.2 · Campos posicionales dentro del registro** — M ⛔ (1.0, 0.5)
      Cada `05` en su offset, mezclando `COMP-3`, `DISPLAY` y `PIC X` en el mismo
      registro. Es lo que convierte 1.1 en algo útil.

- [x] **1.3 · `MOVE` de grupo** — ✅ 2026-08-03
      Entró **con** `0.5`, porque es su primer consumidor: sin él la disposición
      sería código sin usuario, que es justo lo que este repo no permite.
      Es una copia de **bytes** con `bmo_lower::memoria::copiar`, no campo a
      campo — que es lo que dice el estándar y lo que permite reinterpretar un
      registro. Se rechaza con motivo mezclar un grupo con un campo suelto: pide
      relleno con espacios, y eso necesita `0.7`.

- [ ] **1.4 · `REDEFINES`** — M ⚠ (depende de qué salga en 1.0)
      Dos vistas del mismo espacio. Con el camino B hay que decidir: rechazarlo
      con motivo, o darle su propio mecanismo.

- [ ] **1.5 ⚠ `DISPLAY` como ZONED DECIMAL real** — L ⚠
      El camino A de 1.0. Deja de ser obligatorio si se elige B, pero sigue
      siendo lo correcto a largo plazo y lo único que hace truncar a un
      `DISPLAY` de WORKING-STORAGE.

- [ ] **1.6 · EBCDIC ↔ ASCII al leer** — M ⛔ (0.7)
      Los datos de fuera vienen en EBCDIC. Una tabla de 256 entradas → **una
      tabla y no un cerebro**; va en `bmo-lower` junto a `packed`, por el mismo
      motivo: no es semántica de ningún lenguaje.

- [ ] **1.7 · `FILE STATUS`** — S ⛔ (0.7)
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

- [ ] **2.2 · `PERFORM VARYING` completo** — M
      `FROM`/`BY`/`UNTIL` y `AFTER` para recorrer tablas de dos dimensiones.
      Misma forma de línea que el `PERFORM UNTIL` que ya compila.

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

- [ ] **2.6b · `ON SIZE ERROR`** — M
      Se separó de `ROUNDED` al hacerlo: son dos cosas distintas y la segunda
      necesita **a dónde saltar** cuando el resultado no cabe en la PICTURE del
      destino, o sea un cuerpo de sentencias y un `END-ADD`/`END-COMPUTE`.
      Un desbordamiento silencioso en un importe es el fallo que no se puede
      permitir, así que esto no se queda sin hacer — sólo se hace aparte.
      Desbloquea además `ROUNDED MODE IS PROHIBITED`, que hoy se rechaza
      diciendo justo esto.

- [ ] **2.5 · `INITIALIZE`** — S ⛔ (0.5 para grupos)
      Sobre un dato suelto se puede hoy. `bmo_lower::memoria::rellenar` ya está.

- [ ] **2.7 · `SEARCH` / `SEARCH ALL`** — M
      Búsqueda lineal y binaria en tabla. `SEARCH ALL` pide `ASCENDING KEY`.
      Es el sustituto barato de un índice **dentro de memoria**, y tapa parte de
      lo que la fase 4 no puede dar todavía.

- [ ] **2.3 · `STRING` / `UNSTRING`** — M ⛔ (0.7)
- [ ] **2.4 · `INSPECT`** — M ⛔ (0.7)
      Los dos son manejo de texto y necesitan que exista el texto.

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

- [ ] **3.1 · `KIND_ARCHIVO`: modo EXTEND** — S ⛔ kernel
      Añadir al final. Hoy `OPEN EXTEND` se rechaza a propósito: sólo hay
      `_CREAR`, que crea de cero, así que compilarlo como `OUTPUT` borraría el
      histórico y el programa parecería funcionar hasta que alguien buscara el
      mes pasado.

- [ ] **3.2 ★ `KIND_ARCHIVO`: modo I-O** — M ⛔ kernel
      Un handle que **lea y escriba**. Hoy el modo se fija al abrir — son dos
      operaciones distintas, no un argumento. Es lo que bloquea `REWRITE` y
      `DELETE`, o sea **lo que hace que un KSDS sea un KSDS y no un listado
      ordenado**.

- [ ] **3.3 ★ `KIND_ARCHIVO`: posicionar por byte** — M ⛔ kernel
      No existe ninguna operación de cursor: `ARCH_OP_LEER` avanza y ya. Sin
      esto no hay acceso **directo** a nada. (Pero sí hay acceso **secuencial**
      binario — ver 1.1.)

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

AHORA   1.2 CAMPOS POSICIONALES en un FD ──→ 1.1 REGISTRO BINARIO
        con el area hecha, esto es enchufarla al READ y al WRITE

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
~~`0.5`~~ → ~~`1.3`~~ → **`1.2`** → `1.1` → `0.7` → `1.7` → `2.6b` → `0.6` →
`2.2` → `2.7` → `2.8` → `5.1` → `1.6` → `2.9` → `0.2` → **luego el kernel**:
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

# El plan largo: de "BMO COBOL compila" a "BMO COBOL lleva un banco"

> Escrito el 2026-08-03, el dia que entro `COMP-3`.
> **Revision 2 -- verificado contra el codigo, linea por linea, y reordenado por
> lo que se midio.** Tres de las dependencias de la primera version eran falsas
> y una faltaba. Ver *[Lo que cambio en la revision
> 2](#lo-que-cambio-en-la-revision-2-y-por-que)*.
> **Revision 3 -- la decision `1.0` esta TOMADA (camino B) y `2.6 ROUNDED`
> hecho.** Con `1.0` decidida, el camino a *leer lo que ya existe* se queda sin
> candados y `0.5` pasa a ser lo siguiente.
> **Revision 4 -- la FASE 3 estaba mal medida, y a favor.** Sus tres primeras
> tareas (S + M + M) resultan ser **una sola**, `3.0`: el cursor ya existe en el
> kernel y el fichero entero ya vive en RAM. Lo unico que falta de verdad es que
> FAT32 sepa **reemplazar**, y de paso eso arregla un fallo que ya esta en el
> disco -- ver *[FASE 3](#fase-3--el-sistema-debajo)*.
>
> Es la lista de tareas de [`BANCA_REAL.md`](BANCA_REAL.md): aquel documento dice
> **que falta y por que**; este dice **en que orden, que bloquea a que, y como se
> sabe que una esta hecha**.
>
> Esta hecho para avanzar **poco a poco**: cada casilla es una pieza que se
> puede entregar sola, con su prueba, sin dejar el compilador roto entre medias.

## Como se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO, y por QUE -- comprobado en el codigo, no supuesto
⚠    tiene una decision dentro que hay que tomar antes de escribir codigo
```

Y el tamano, para poder repartir: **S** una sesion - **M** dos o tres - **L** una
semana de trabajo de verdad - **XL** la pieza grande de su fase.

## La regla que no se negocia

**Nada entra sin su fila en `cobol_feature_matrix_runs_correctly`**, que
EJECUTA el programa en `bmo_lower::emu` en vez de mirar sus bytes. Y cuando la
caracteristica cambia **como se guarda** un dato --no que se hace con el-- hace
falta ademas una prueba que **solo pueda pasar si el almacenamiento es real**.
La de `COMP-3` es el patron a copiar: el mismo `12345` en un `PIC 9(3) COMP-3`
sale `345` y en un `PIC 9(3)` sale `12345`. Se comprueba mutando la
caracteristica a no-operacion y viendo **cuantos tests caen**. Si no cae
ninguno, la prueba no probaba nada.

## Y la regla de este documento

**Un ⛔ se gana midiendo.** La revision 2 existe porque tres tareas estaban
marcadas como bloqueadas por razonamiento y no por lectura del codigo, y las
tres resultaron ser falsas. Antes de poner un ⛔ hay que abrir el fichero y
citarlo.

---

# ★ LA ESTRATEGIA: primero todo lo que no depende del sistema

**Decidido el 2026-08-03 por Eddi.** Esta aqui arriba y no al final porque es lo
que decide que se toca en cada sesion.

## Las dos listas, separadas

Toda tarea de este plan cae en una de dos, y hay que saber en cual antes de
empezarla:

| | |
|---|---|
| **SIN candado** -- solo `toolchain/lang/cobol` y `toolchain/forge` | `0.5` records - `1.1` registro binario - `1.2` campos posicionales - `1.3` MOVE de grupo - `0.7` texto - `1.7` FILE STATUS - `1.6` EBCDIC - `2.2` PERFORM VARYING - `2.3` STRING - `2.4` INSPECT - `2.5` INITIALIZE - `2.6b` ON SIZE ERROR - `2.7` SEARCH - `2.8` COPY - `2.9` intrinsecas - `0.6` GO TO - `0.2` parser de tokens - `5.1` SORT |
| **CON candado** -- pide kernel, ESTRATOS o una decision de arquitectura | `3.1` EXTEND - `3.2` I-O - `3.3` posicionar - `3.4` ESTRATOS escribe - toda la **fase 4** (VSAM) - toda la **fase 7** (despachador) - `6.1` el enlazador y con el `6.2`-`6.6` |

## La regla, y por que

> **Se hace primero TODO lo de la columna izquierda.**

1. **Ahi esta el salto mas grande que queda, y no tiene candado.** Leer
   **registros binarios de verdad** --campos en su sitio, importes empaquetados--
   es lo que separa *"COBOL nuevo"* de *"COBOL que abre los datos que ya
   tienes"*, y se comprobo que **no necesita seek**: `ARCH_OP_LEER` ya saca
   bytes crudos y esta en el kernel y en el emulador. Es puro compilador.
2. **El trabajo de sistema no se pone mas dificil por esperar.** Las tres
   operaciones que faltan son pequenas y estan descritas; el orden entre ellas y
   COBOL no cambia lo que cuestan.
3. **Cada sesion de COBOL entrega algo que corre.** Una de kernel no: hay que
   cambiar la superficie, el kernel y el emulador antes de que un `.cob` note
   nada.

## ⚠ Y el TECHO, dicho antes de que nadie lo suponga

**Haciendo solo la columna izquierda se llega hasta el BATCH y no mas.** Leer un
fichero, calcular, escribir otro -- que es exactamente lo que un banco hace de
noche, y no es poco: es el 80 % del COBOL que hay escrito en el mundo.

Lo que **no** se alcanza sin la columna derecha:

- **Buscar una cuenta sin leer el fichero entero.** *"Dame la 4471-9982"* con
  cuatro millones de registros. Eso es el indice, y el indice pide `3.2` y `3.3`.
- **Modificar un registro en su sitio.** `REWRITE` y `DELETE` necesitan un
  handle que lea y escriba, y hoy el modo se fija al abrir.
- **Transacciones y varios usuarios a la vez.** Fase 7.

**Tres operaciones de kernel bloquean la pieza mas grande del proyecto.** No son
una montana -- pero no se pueden saltar, y por eso estan escritas aqui y no
escondidas en la fase 3.

---

# FASE 0 -- El suelo

- [x] **0.1 - `VALUE` inicializa de verdad** -- ✅ 2026-08-03
      Se emite al principio, despues de repartir la pila y antes de la primera
      sentencia, **pasando por `store_var`** -- asi un `COMP-3` se inicializa
      empaquetado sin que el emisor de valores sepa que existen los nibbles.
      Sobre una tabla llena todas las casillas. Figurativas `ZERO`/`ZEROS`/
      `ZEROES`. Se rechaza con motivo: `VALUE` de texto, `VALUE` que no es un
      numero, y `VALUE` sin PIC.

- [x] **0.3 - `OR` en las condiciones** -- ✅ 2026-08-03
      La condicion dejo de ser una `Vec` y es un **arbol** `Simple/Y/O`, porque
      `AND` liga mas fuerte que `OR` y una lista plana no puede representar esa
      diferencia. Con **cortocircuito**, que no es una optimizacion: un operando
      puede ser un elemento de tabla y ahi la evaluacion lleva guarda de rango.
      Cayeron con el los dos rechazos que dependian de el: **`88 ... VALUE 1 THRU
      5`** y **`88 ... VALUE 6, 7`**.

- [x] **0.4 - Parrafos y `PERFORM <parrafo>`** -- ✅ 2026-08-03
      Un nombre de parrafo es una palabra sola con punto. Compilan
      `PERFORM <p>`, `PERFORM <p> THRU <q>`, `PERFORM <p> <n> TIMES` y
      `PERFORM <p> UNTIL <cond>`, mas `EXIT`/`CONTINUE`.
      ★ El retorno se decide **en ejecucion** (una ranura de pila con "en que
      parrafo hay que volver"), no con un `ret` fijo: el mismo parrafo puede ser
      el final de un rango en una linea y estar en medio de otro en la de abajo.
      ★ De paso salio que **`STOP RUN` no emitia nada** -- colaba por ser siempre
      la ultima linea, y ya estaba mal antes: un `STOP RUN` dentro de un `IF`
      se ignoraba en silencio.

- [ ] **0.2 - El parser sobre TOKENS como principal** -- L ⚠
      **DEGRADADO A DEUDA, NO A BLOQUEO** (revision 2). `tparser.rs` ya parsea un
      programa entero, pero `compile_source` sigue usando `parser.rs`, el
      analizador por lineas.
      Lo que la revision 1 decia --que la fase 2 entera depende de esto-- **es
      falso**: `parser.rs` ya consume varias lineas para `IF ... END-IF` y
      `PERFORM ... END-PERFORM`, y `EVALUATE ... WHEN ... END-EVALUATE` tiene
      exactamente esa forma. Los verbos de la fase 2 se pueden hacer hoy.
      Sigue mereciendo la pena, pero por calidad: **una gramatica y no dos**.
      ⚠ La decision sigue en pie: jubilar `parser.rs` de golpe (mover las 136
      pruebas a la vez) o convivir.

- [x] **0.5 - Records anidados con posiciones fijas** -- ✅ 2026-08-03
      Grupos `01`/`05`/`10` con **cada campo en su byte, sin relleno** -- porque
      la disposicion de un registro *es el formato del fichero*, y un byte de
      padding es un byte que aparece en el disco. Vive en `registro.rs` y **NO
      reutiliza `bmo_abi::types::disposicion`** a proposito: aquella alinea, que
      es lo correcto para C y veneno aqui. Es la excepcion que confirma la regla
      de la casa -- se comparte la REGLA cuando es la misma, y esta no lo es.
      ★ **El AREA DE REGISTRO del camino B ya funciona**: un grupo tiene sus
      ranuras de trabajo *y* su area de bytes, y la traduccion vive solo donde el
      registro cruza. La otra mitad entro con esto: `bmo_lower::zoned`, donde un
      `DISPLAY` es **un byte por digito con el signo sobrepunzado en el ultimo**
      -- que es por lo que un `PIC S9(5)` mide cinco bytes y no seis.
      ★ La prueba que no se puede fingir: un grupo con un `PIC 9(6)` movido a
      otro con dos `PIC 9(3)` da `123` y `456`. Campo a campo eso es imposible.

- [x] **0.6 - `GO TO` dentro de un parrafo** -- ✅ 2026-08-03
      Salio faltando al escribir el ejemplo del nivel 8, y ese ejemplo ya lo usa:
      el descarte dentro de un rango `PERFORM ... THRU` pasa de un interruptor y
      un `IF` a una linea.
      ★ Se emite como un `jmp rel32` al MISMO simbolo al que `PERFORM` hace
      `call`, y lo parchea la misma tabla -- los dos son rel32 contra la
      instruccion siguiente, asi que el parcheador no distingue ni tiene por que.
      ★ Y lo que pasa despues **sale gratis**: el parrafo al que se salta corre y
      su epilogo pregunta si es ahi donde habia que volver. Si el `GO TO` fue a
      la salida del rango, vuelve; si no, sigue cayendo hasta encontrarla. Es lo
      que dice el estandar y no hizo falta escribir nada para ello.
      ⚠ Se rechaza desde el **cuerpo principal**: aqui un parrafo es una
      subrutina a la que se entra por `call`, y saltar dentro sin haber entrado
      por su `PERFORM` deja el `ret` del final sin dueno.
      ⛔ `GO TO ... DEPENDING ON` todavia no.

- [x] **0.7 - Texto de verdad: `PIC X(n)` con contenido** -- ✅ 2026-08-03
      ★ **Sin el limite de 8 caracteres que temia la revision 2.** El texto no
      pasa por `rax` como todo lo demas: tiene su propio camino, que trabaja con
      **direccion y largo**. Asi el ancho es el que diga la PICTURE y no el de un
      registro.
      Compilan: `VALUE "TEXTO"` (con espacios dentro), `MOVE` de literal y de
      campo a campo, `=` y `NOT =` contra literal o contra otro campo, y
      `DISPLAY`.
      ★ Todo **DESENROLLADO** cuando el otro lado es un literal: mover `"00"` a
      un campo son dos `mov` de inmediato, no un bucle. El texto viaja dentro de
      las instrucciones, como `console::write_const`.
      ★ **El relleno con espacios no es cosmetico**: el campo se llena ENTERO
      cada vez que se escribe, asi que un `MOVE` corto detras de uno largo no
      deja cola. Hay un test para eso -- un `FILE STATUS` que arrastra la letra de
      la operacion anterior es peor que uno vacio.
      Se rechaza con motivo: comparar cadenas por ORDEN (`>`, `<`) porque depende
      del juego de caracteres, y mover texto a un campo numerico (eso es
      `FUNCTION NUMVAL`).

---

# FASE 1 -- Leer los datos que ya existen

La segunda mitad de `COMP-3`. Un campo empaquetado vive bien **en memoria**, pero
el fichero sigue siendo **texto, un numero por linea**. Un banco no da eso: da
registros de longitud fija con campos empaquetados dentro.

**Sin esta fase, BMO COBOL escribe programas nuevos y no puede leer nada de lo
que ya existe.**

- [x] **1.0 ★ LA DECISION: donde vive un campo de un registro** -- ✅ **TOMADA
      el 2026-08-03 por Eddi: CAMINO B.**

      > **El `FD` tiene un AREA DE REGISTRO --un buffer de bytes del largo del
      > registro-- y cada campo conserva su ranura de trabajo de 64 bits *y*
      > apunta a su posicion dentro del buffer. `READ` llena el buffer y
      > DESEMPAQUETA cada campo a su ranura; `WRITE` EMPAQUETA al reves.**

      Los motivos, para que no haya que reconstruirlos dentro de seis meses:

      1. **Es lo que dice COBOL, no un rodeo.** El area de registro solo vale
         entre un `READ` y el siguiente; el estandar lo dice con esas palabras.
         Empaquetar en la frontera no imita el modelo: *es* el modelo.
      2. **Media pieza ya esta hecha.** `bmo_lower::packed` desempaqueta desde
         un puntero desde el 2026-08-03, que es exactamente lo que hace falta
         para un campo `COMP-3` dentro del buffer.
      3. **No toca nada de lo que corre en el Ryzen.** El camino A cambia como
         se guarda CADA dato del programa; este solo anade una capa en los dos
         sitios donde el registro cruza al disco.
      4. **El truncamiento no se cuela de tapadillo.** Con A, los `DISPLAY` de
         WORKING-STORAGE empezarian a truncar de un dia para otro y la salida de
         programas que ya funcionan cambiaria. Con B eso sigue siendo una
         decision aparte (1.5), tomable el dia que se quiera y no como efecto
         secundario de querer leer un fichero.

      **Lo que se paga, dicho:** `REDEFINES` (1.4) sobre un registro **no
      aliasa de verdad** -- dos vistas del mismo espacio serian dos juegos de
      ranuras. Cuando llegue 1.4 hay que rechazarlo con motivo o darle su
      propio mecanismo, y no fingir que funciona.

      **El problema, medido**: en `codegen.rs` el reparto de la pila hace
      `let aligned = (size + 7) & !7` **por dato**, y `load_var`/`store_var`
      mueven con `mov rax, [rbp+off]` de 64 bits. Un `PIC 9(3)` contiguo mide
      tres bytes: escribirlo con un `mov` de ocho **se lleva al vecino**.

      **Camino A -- zoned decimal de verdad (1.5).** Cada dato pasa a medir lo que
      dice su PICTURE. Es lo correcto y es COBOL. Cuesta caro: toca todo lo que
      corre hoy en el Ryzen, y los `DISPLAY` **empezaran a truncar** como ya hace
      el `COMP-3`.

      **Camino B -- area de registro + empaquetado en la frontera.** El `FD` tiene
      un **buffer de bytes** del largo del registro; cada campo conserva su
      ranura de trabajo de 64 bits *y* apunta a su posicion dentro del buffer.
      `READ` llena el buffer y **desempaqueta** cada campo a su ranura; `WRITE`
      **empaqueta** al reves. Eso es exactamente lo que dice COBOL: el area de
      registro solo vale entre un `READ` y el siguiente.
      - **A favor**: no toca nada de lo que ya funciona, y el `COMP-3` ya tiene
        media pieza hecha (`bmo_lower::packed` desempaqueta desde un puntero).
      - **En contra**: `REDEFINES` sobre un registro (1.4) no aliasa de verdad,
        y hay que decidir si se rechaza o se hace aparte.
      - **Coste**: una fraccion de A.

      **Elegido: B.** Ver los motivos arriba. `A` queda como 1.5, para el dia
      que se quiera truncamiento en WORKING-STORAGE -- y ese dia sera por eso, no
      por poder leer un fichero.

- [x] **1.1 ★ Registro BINARIO de longitud fija** -- ✅ 2026-08-03
      La revision 2 acerto: **no necesitaba seek**. `ARCH_OP_LEER` ya saca bytes
      crudos y el cursor avanza **exactamente lo que devuelve** -- comprobado en
      `ring0/obj/archivo.rs`, no supuesto.
      ★ **Pero habia un detalle que solo se ve escribiendolo**: el paquete son
      SIETE bytes y un registro de banca mide 5, o 16, o 47. La ultima tirada de
      cada registro trae bytes de mas **que son del registro siguiente**, y no se
      pueden devolver porque el cursor es del kernel.
      Por eso el area lleva **16 bytes detras** con el resto pendiente, y el
      registro de despues lo gasta antes de pedir nada. Sin eso, un fichero de
      registros de 5 bytes daria bien el primero y basura todos los demas -- el
      fallo que no revienta y descuadra.
      El test que lo caza lee **tres** seguidos y mira el tercero, que es donde
      el error ya se acumulo dos veces.

- [x] **1.2 - Campos posicionales dentro del registro** -- ✅ 2026-08-03
      Cada `05` en su offset, mezclando `COMP-3` y `DISPLAY` en el mismo
      registro. Entro **con** `1.1`, porque separados no sirven de nada: leer
      bytes crudos sin campos es un `memcpy`, y campos sin bytes es memoria.
      ★ El `READ`/`WRITE` mira si el `01` del `FD` es un GRUPO: si lo es, el
      fichero **no es texto** y va por el area. El camino de texto --una linea, un
      numero-- se queda para los ficheros que ya existian. **Son dos cosas
      distintas, no dos modos de la misma.**
      ★ Un registro binario se escribe **sin salto de linea**: mide lo que dice
      su copybook y un separador correria todo lo de detras un byte.

- [x] **1.3 - `MOVE` de grupo** -- ✅ 2026-08-03
      Entro **con** `0.5`, porque es su primer consumidor: sin el la disposicion
      seria codigo sin usuario, que es justo lo que este repo no permite.
      Es una copia de **bytes** con `bmo_lower::memoria::copiar`, no campo a
      campo -- que es lo que dice el estandar y lo que permite reinterpretar un
      registro. Se rechaza con motivo mezclar un grupo con un campo suelto: pide
      relleno con espacios, y eso necesita `0.7`.

- [x] **1.3b ★ El COPYBOOK (`--copybook`)** -- ✅ 2026-08-03
      El compilador escupe el byte exacto de cada campo de cada registro, con su
      codificacion y como se lee el signo. En banca ese documento es lo que se
      intercambia para que dos sistemas lean el mismo fichero, y **el que se
      mantiene a mano siempre acaba mintiendo**.
      ★ Este no puede: sale de **la misma tabla que usa el codegen** para emitir
      el `READ` y el `WRITE`, asi que no hay dos sitios donde pueda divergir. Es
      *tablas y no cerebros* aplicado a la documentacion -- el documento no
      describe el formato, **es** el formato.
      Marca cuales cruzan de verdad (`[FICHERO]`, los que cuelgan de un `FD`) y
      cuales son de WORKING-STORAGE, y distingue una PIC de **edicion** como lo
      que es: una mascara de presentacion, no almacenamiento.

- [x] **1.3c ★ El VISOR de registros (`--ver`)** -- ✅ 2026-08-03
      Desde que un `COMP-3` sale al disco, el fichero **deja de poderse mirar**:
      los nibbles no son texto y un `cat` ensena basura. El compilador lo decodifica
      con el copybook de su propio programa, y ensena **el valor y los bytes
      crudos al lado**.
      ★ Lo que ninguna herramienta de fuera puede prometer: **lee con la misma
      regla con la que el programa escribio**. Los decodificadores del anfitrion
      (`packed::desempaquetar_en_rust`, `zoned::leer_en_rust`) estan comparados
      contra los EMITIDOS sobre **todos** los patrones de dos bytes -- 65 536
      comparaciones cada uno. Si divergieran, el visor ensenaria un importe y el
      programa leeria otro, que es peor que no tener visor.
      ★ Si el fichero no es multiplo del registro, **lo dice y ensena lo que
      sobra**: es el sintoma clasico del copybook equivocado.
      Comprobado con un fichero generado desde **Python**, no desde BMO.

- [ ] **1.4 - `REDEFINES`** -- M ⚠ (depende de que salga en 1.0)
      Dos vistas del mismo espacio. Con el camino B hay que decidir: rechazarlo
      con motivo, o darle su propio mecanismo.

- [ ] **1.5 ⚠ `DISPLAY` como ZONED DECIMAL real** -- L ⚠
      El camino A de 1.0. Deja de ser obligatorio si se elige B, pero sigue
      siendo lo correcto a largo plazo y lo unico que hace truncar a un
      `DISPLAY` de WORKING-STORAGE.

- [ ] **1.6 - EBCDIC <-> ASCII al leer** -- M
      Los datos de fuera vienen en EBCDIC. Una tabla de 256 entradas -> **una
      tabla y no un cerebro**; va en `bmo-lower` junto a `packed`, por el mismo
      motivo: no es semantica de ningun lenguaje.

- [~] **1.7 - `FILE STATUS`** -- ✅ 2026-08-03, **con los codigos que se pueden
      dar de verdad y solo esos**
      `SELECT ... FILE STATUS IS <campo>`, y el codigo de dos letras se deja
      despues de `OPEN`, `READ`, `WRITE` y `CLOSE`.
      ★ Solo se ponen **`00`, `10`, `30` y `35`**, que son los que la puerta
      permite distinguir: el `OPEN` contesta con un handle o un cero, el `READ`
      con un si o un no, y el `CLOSE` con un guardo o no guardo. Los demas
      (`37` modo incompatible, `41`/`42` doble apertura o cierre) **no se pueden
      separar todavia** -- de un cero no se saca el motivo. Inventarlos mandaria
      a arreglar lo que no esta roto. El dia que `KIND_ARCHIVO` traiga un
      codigo, aqui solo hay que ampliar la tabla; por eso queda `[~]` y no `[x]`.

      ★★ **El `30` del `CLOSE` entro el 2026-08-03, y tapaba un fallo grave.**
      `emit_close` escribia `"00"` **a pelo, sin mirar `rax`**, asi que el unico
      momento en el que un fichero llega al disco era tambien el unico que no
      se comprobaba: un programa que se habia molestado en declarar `FILE
      STATUS` recibia "todo bien" con el fichero sin guardar. Y pasa de verdad --
      hoy `CREAR` no puede reemplazar un fichero existente, o sea que la segunda
      corrida de cualquier programa que escriba su salida caia por ahi. Ver
      `3.0`.
      Para poder probarlo hizo falta que el emulador supiera **fingir un disco
      que dice que no** (`Machine::fallar_al_guardar`): mientras `CERRAR`
      contesto `1` siempre, el camino del fallo era codigo que ninguna prueba
      podia pisar.
      ★ Se comprueba que el campo **existe y mide dos letras**: si no, el
      programa compararia contra basura y `IF ST = "00"` daria falso siempre --
      un batch que se para cada noche sin motivo.
      El codigo de dos digitos (`00` bien, `10` fin de fichero, `23` no
      encontrado, `35` no existe...). **Todo programa de banca lo mira despues de
      cada operacion.**
      ✅ **El dato ya esta ahi**: `archivo::abrir_const` deja el handle en `rax`
      y **cero si no se pudo abrir**, y `leer_linea` deja `rax = 0` al acabarse
      el fichero. No hace falta nada del kernel -- falta el campo donde ponerlo,
      y eso es 0.7, porque `FILE STATUS` es un `PIC XX`.

---

# FASE 2 -- Los verbos que el codigo real usa

★ **DESBLOQUEADA EN LA REVISION 2.** La revision 1 decia "todos dependen de
0.2 (el parser sobre tokens)". **Es falso.** `parser.rs` ya consume varias lineas
para `IF ... END-IF` y `PERFORM ... END-PERFORM`, y las sentencias de esta fase
tienen la misma forma. Se pueden hacer **hoy**, y son lo que mas codigo real
desbloquea por hora de trabajo.

- [x] **2.1 ★ `EVALUATE`** -- ✅ 2026-08-03
      Las dos formas compilan, con `WHEN OTHER`, `WHEN a THRU b` y `WHEN a, b`.
      ★ El `THRU` y la coma **no costaron una linea de gramatica nueva**: la
      expansion "esta este campo en este conjunto?" se saco a
      `Condicion::de_valores` y la comparten el nivel 88 y el `WHEN`. Y como las
      dos sintaxis llegan al codegen como el MISMO arbol, el emisor son cinco
      lineas y heredan cortocircuito y precedencia gratis.
      Se rechaza con motivo: `EVALUATE FALSE`, varios sujetos (`ALSO`), un `WHEN`
      despues del `OTHER` (no se alcanza nunca), sentencias entre el `EVALUATE` y
      el primer `WHEN`, y las sentencias en la misma linea que su `WHEN`.
      Dos formas, las dos corrientes en banca:
      ```cobol
      EVALUATE TIPO-MOV              EVALUATE TRUE
          WHEN 1 ...                       WHEN SALDO > 1000.00 ...
          WHEN 2 THRU 5 ...                WHEN SALDO > 100.00 ...
          WHEN 6, 7 ...                    WHEN OTHER ...
          WHEN OTHER ...               END-EVALUATE
      END-EVALUATE
      ```
      La segunda es **la tabla de decision**, y es como un banco escribe un
      escalado de comisiones. Las dos caen sobre el arbol de condiciones que
      entro con 0.3: `WHEN a THRU b` y `WHEN a, b` son exactamente la expansion
      que ya hace un nivel 88.
      `WHEN ... ALSO` (varios sujetos) puede esperar y se rechaza con motivo.

- [x] **2.2 - `PERFORM VARYING` completo** -- ✅ 2026-08-03
      `FROM`/`BY`/`UNTIL`, en linea y de parrafo, con **cuantos `AFTER` haga
      falta** (probado con tres).
      ★ El codegen es **recursivo sobre los controles**, y de ahi sale gratis lo
      que de verdad define un `AFTER`: **el de dentro se reinicia cada vez que el
      de fuera avanza**. Escrito como un bucle plano habria que acordarse de
      reiniciar a mano, y olvidarlo recorre la tabla en diagonal -- la primera
      fila entera y de las demas solo la ultima columna.
      El paso puede ser negativo, y con `WITH TEST BEFORE` un bucle cuya
      condicion ya se cumple **no da ni una vuelta**.
      ⚠ Queda dicho en el AST: `UNTIL` dice cuando **PARAR**, no cuando seguir.
      Al reves que el `while` de casi todo lo demas, y confundirlo sobre una
      tabla es un subindice fuera de rango.

- [x] **2.6 ★ `ROUNDED`** -- ✅ 2026-08-03
      **Los SEIS modos del estandar**, en las cinco aritmeticas, con
      `ROUNDED MODE IS <modo>`. El emisor vive en `bmo_lower::redondeo` por la
      misma razon que `packed` y `fmt`: partir un entero y decidir el ultimo
      digito es aritmetica, no la semantica de un lenguaje.
      ★ Van **todos** y no solo el clasico porque el redondeo es una **decision
      legal**: hay jurisdicciones que obligan al del banquero (`NEAREST-EVEN`)
      precisamente porque el clasico tiene sesgo -- en una muestra grande los
      empates siempre suben. Hay un test que lo ensena con cuatro empates
      seguidos: el clasico inventa dos centimos y el del banquero cuadra.
      ★ **Se redondea el RESULTADO, no los operandos**: la operacion se hace en
      la escala mas alta que aparezca y se baja una sola vez. Con los modos
      asimetricos no es lo mismo -- el techo de `-9.995` es `-9.99`, pero
      redondeando el `9.995` primero sale `-10.00`.
      ★ Y hay **dos implementaciones de la misma regla** --la emitida y una en
      Rust para los literales-- con un test que las compara valor a valor en
      todo el rango. Dos que tienen que coincidir prueban mas que una comparada
      contra una tabla escrita a mano.

- [x] **2.6b - `ON SIZE ERROR`** -- ✅ 2026-08-03
      En las cinco aritmeticas, con `NOT ON SIZE ERROR` y su `END-<verbo>`.
      ★ **Cuando no cabe, el destino se queda COMO ESTABA.** Esa es la parte que
      importa y por eso la comprobacion va antes del guardado: deja el saldo
      anterior intacto para que el programa lo escriba en un informe de rechazos
      y siga. Guardar el numero recortado y avisar despues seria avisar de un
      descuadre ya hecho.
      ★ **Dividir entre cero es un desborde, no un fallo del CPU.** Sin eso el
      `idiv` levanta `#DE` y el proceso muere sin decir por que -- en un batch,
      un registro malo se lleva el proceso entero.
      ⚠ La clausula tiene que **empezar en la linea del verbo**: si no, un
      `ADD A TO B` a secas y uno que sigue abajo se leen igual, y adivinar
      significaria tragarse las sentencias de despues.
      ⚠ Y salio una divergencia que queda FIJADA CON TEST: sin la clausula, BMO
      **no recorta** (guarda `1023` en un `PIC 9(3)`) porque un `DISPLAY` sigue
      siendo un entero de 64 bits -- la tarea `1.5`. El dia que entre, ese test
      falla, que es exactamente el aviso que hace falta.

- [ ] **2.5 - `INITIALIZE`** -- S ⛔ (0.5 para grupos)
      Sobre un dato suelto se puede hoy. `bmo_lower::memoria::rellenar` ya esta.

- [ ] **2.7 - `SEARCH` / `SEARCH ALL`** -- M
      Busqueda lineal y binaria en tabla. `SEARCH ALL` pide `ASCENDING KEY`.
      Es el sustituto barato de un indice **dentro de memoria**, y tapa parte de
      lo que la fase 4 no puede dar todavia.

- [~] **2.3 - `STRING`** -- ✅ 2026-08-03, **`UNSTRING` no**
      `STRING <fuentes> DELIMITED BY SIZE INTO <destino>`, leido en varias
      lineas --que es como se escribe-- y **resuelto entero al compilar**: cada
      fuente tiene un ancho conocido, asi que el destino se llena por trozos sin
      un puntero que avance en ejecucion.
      El destino se pone a espacios ANTES, para que lo que no se llene no se
      quede con lo del `STRING` de antes.
      ⛔ `DELIMITED BY SPACE` o por un caracter cortan por un largo que solo se
      sabe en ejecucion: es otro emisor y se rechaza con ese motivo. Y
      `UNSTRING` --partir uno en varios-- es la mitad que falta.

- [x] **2.4 - `INSPECT`** -- ✅ 2026-08-03
      `TALLYING <n> FOR ALL "<c>"` y `REPLACING {ALL|LEADING} "<a>" BY "<b>"`,
      con las figurativas `SPACE` y `ZERO`.
      ★ `ALL` y `LEADING` son **dos formas y no una con una opcion**, porque
      sobre un importe dan numeros distintos: `"  12 34"` con `LEADING " " BY
      "0"` da `"0012 34"` y con `ALL` daria `"0012034"`.
      Los emisores viven en **`bmo_lower::texto`**, hermana de `memoria`:
      aquella trae los verbos de C (`memcpy`, `memset`) y esta los que COBOL
      escribe `INSPECT` y Ada `Index`/`Replace_Slice`. Y con la misma frontera --
      **el largo va explicito, aqui no hay NUL que buscar**, que es la
      diferencia entre un campo de COBOL y una cadena de C.
      ⛔ Buscar o sustituir una CADENA es busqueda de subcadena y se rechaza:
      aceptarlo mirando solo la primera letra contaria de mas.

- [ ] **2.8 - `COPY ... REPLACING`** -- M
      **Asi se comparten los layouts de registro entre programas.** Sin esto,
      cada programa reescribe el `01` del fichero a mano y se descuadran solos.
      No depende de nadie: es inclusion de texto antes de analizar.

- [ ] **2.9 - Las intrinsecas que importan (~15 de 55)** -- M
      `NUMVAL`, `NUMVAL-C`, `CURRENT-DATE`, `INTEGER-OF-DATE`,
      `DATE-OF-INTEGER`, `LENGTH`, `MAX`, `MIN`, `MOD`, `REM`, `UPPER-CASE`,
      `LOWER-CASE`, `TRIM`, `ORD`, `WHEN-COMPILED`. La tabla `INTRINSIC[]` ya
      esta generada; falta la semantica de cada una.
      ⚠ `CURRENT-DATE` necesita **reloj**, y hoy la superficie solo da TSC.

---

# FASE 3 -- El sistema debajo

⛔ **Nada de esto se arregla en `lang/cobol`.** Comprobado en
`platform/abi/bmo-abi/src/syscalls/surface.rs` y en
`Ultra_kernel_x86-64/kernel/src/ring0/obj/archivo.rs`.

Lo que la puerta **ya da**: `TASK_OP_ARCHIVO_ABRIR` (0x10) y `_CREAR` (0x11);
sobre el handle, `ARCH_OP_LEER` (7 bytes crudos), `ARCH_OP_LEER_LINEA`,
`ARCH_OP_ESCRIBIR`, `ARCH_OP_TAMANO` y `ARCH_OP_CERRAR`.

## ★ Revision 4 (2026-08-03): las tres primeras son UNA, y no la que se creia

Medido en el codigo, no supuesto. Las tres tareas de abajo se escribieron
como S + M + M **suponiendo que faltaban tres mecanismos distintos**. Faltan
dos cosas, y solo una es de verdad:

| Lo que decia el plan | Lo que hay en el codigo |
|---|---|
| *"No existe ninguna operacion de cursor"* | **Existe**: `CURSOR[i]` en `obj/archivo.rs`, uno por ranura, y `leer`/`leer_linea` ya lo mueven. `3.3` es exponerlo con una guarda de rango -- decenas de lineas, no una M |
| *"Un handle que lea y escriba"* | El fichero **entero vive ya en RAM** por ranura (marcos contiguos que se doblan al llenarse) y `cerrar` lo vuelca de una vez. Leer-y-escribir no pide modelo nuevo: pide que `abrir` deje `ESCRIBE = true` y que `escribir` respete el cursor en vez de anadir al final |
| *"Anadir al final"* | Cae solo con lo anterior: `CURSOR = LARGO` al abrir |

★ **Lo que falta de verdad es que FAT32 sepa REEMPLAZAR.**
`create_file_in_dir` devuelve `WriteError::Exists` si el nombre ya esta, y
`archivo::crear()` **no lo comprueba al abrir**: el fallo aparece en `cerrar()`,
que devuelve `0` y solo deja un `warn` en la CABINA.

La consecuencia se ve hoy y no es teorica: **un programa que escriba su fichero
solo es honesto la primera vez que se corre.** El nivel 10 de los ejemplos
escribe tres cuentas, las relee y las imprime; en la segunda corrida no guarda
nada, y como relee el mismo fichero con los mismos valores **la pantalla sale
identica**. Es la peor forma de un fallo: la que se parece a funcionar.

Las piezas para arreglarlo estan puestas -- `free_chain` y `mark_cluster_eoc` ya
viven en el driver. Es liberar la cadena vieja, escribir la nueva y reescribir
el primer cluster y el tamano en la entrada de directorio.

**Por eso `3.0` va delante de las otras tres y ninguna se puede entregar sin
ella.**

- [~] **3.0 ★ FAT32: reemplazar un fichero que ya existe** -- ✅ 2026-08-03 en
      el codigo, **⏳ sin verificar en el Ryzen**
      `save_file_in_dir` en el driver (crea si no esta, reemplaza si esta),
      `guardar_en` en `fs.rs`, y `archivo::cerrar` pasa por ahi. `create_file_in_dir`
      **sigue rechazando con `Exists`**: pisar hay que pedirlo, no heredarlo de
      un flag al final de la lista de argumentos.
      ★ El orden es lo unico que importa: cadena nueva entera -> apuntar la
      entrada (un solo sector) -> soltar la vieja. Asi un corte de corriente deja
      una fuga de clusters, nunca un archivo perdido ni un nombre apuntando a
      datos a medias. Cuesta aguantar **las dos copias a la vez** durante la
      escritura, y se paga a gusto.
      ★★ Y trajo lo que faltaba desde el principio: **el driver de FAT32 no
      tenia UNA sola prueba**. Era el unico codigo de BMO que escribe en un
      disco de verdad y se verificaba flasheando. Ahora son 9, sobre un volumen
      de mentira en RAM, con detector de fugas de clusters incluido. Mutado
      --quitando el `free_chain`-- cae la que toca.
      Queda `[~]` hasta la prueba de verdad: **correr el nivel 10, cambiar un
      saldo en el fuente, recompilar y volver a correrlo en el Ryzen**. Tiene
      que salir el nuevo.

- [ ] **3.1 - `KIND_ARCHIVO`: modo EXTEND** -- S ⛔ (3.0)
      Anadir al final. Hoy `OPEN EXTEND` se rechaza a proposito: solo hay
      `_CREAR`, que crea de cero, asi que compilarlo como `OUTPUT` borraria el
      historico y el programa pareceria funcionar hasta que alguien buscara el
      mes pasado.

- [ ] **3.2 ★ `KIND_ARCHIVO`: modo I-O** -- S ⛔ (3.0)
      Un handle que **lea y escriba**. Es lo que bloquea `REWRITE` y `DELETE`,
      o sea **lo que hace que un KSDS sea un KSDS y no un listado ordenado**.
      Baja de M a S por la revision 4: el buffer completo en RAM ya da la
      semantica; lo que falta es el modo y el volcado, que es `3.0`.

- [ ] **3.3 ★ `KIND_ARCHIVO`: posicionar por byte** -- S ⛔ (3.0)
      Exponer el `CURSOR` que ya existe. Sin esto no hay acceso **directo** a
      nada. (Pero si hay acceso **secuencial** binario -- ver 1.1.)
      Baja de M a S por la revision 4.

- [ ] **3.4 ★ ESTRATOS: crear objetos y ESCRIBIR** -- XL ⛔ ESTRATOS
      Hoy monta, lee y sabe commitear (`sellar()` en `ring0/fsys/estratos.rs`, y
      la maquina de estados de la transaccion probada en el anfitrion). **Falta
      crear.** Sin esto, un indice solo cabe sobre `KIND_ARCHIVO` -- y ahi se
      pierden las tres cosas que hacen que este indice sea mejor que el de z/OS.

---

# FASE 4 -- VSAM: de listados a banca

★★ **La fase que decide.** *"Dame la cuenta 4471-9982"* no puede significar leer
cuatro millones de registros. **Sin indice no hay banca, hay listados.**

- [ ] **4.1 - RRDS -- registros por numero** -- M ⛔ (3.3)
- [ ] **4.2 - ESDS -- secuencial de solo anadir** -- S ⛔ (3.1)
- [ ] **4.3 ★ KSDS, la LECTURA** -- XL ⛔ (3.2, 3.3)
      El B-tree. `RECORD KEY`, `READ ... KEY IS`, `START`, `READ NEXT`.
- [ ] **4.4 - KSDS, la ESCRITURA** -- L ⛔ (3.2, 4.3)
- [ ] **4.5 - `ALTERNATE RECORD KEY ... WITH DUPLICATES`** -- L ⛔ (4.4)
      El indice por DNI ademas del de numero de cuenta.
- [ ] **4.6 ★ El indice SOBRE ESTRATOS** -- XL ⛔ (3.4, 4.4)
      Copy-on-write, transaccional (**no hay indice a medias**) y auditable.
      **Es lo que un auditor quiere y VSAM sobre z/OS no le da.**

---

# FASE 5 -- SORT

- [ ] **5.1 - `SORT` externo** -- L
      ✅ **Menos bloqueado de lo que decia la revision 1.** No hace falta
      `EXTEND`: una mezcla por tramos puede escribir **cada pasada a un fichero
      nuevo** con `TASK_OP_ARCHIVO_CREAR`, que ya existe. Cuesta E/S de mas y es
      perfectamente honesto para empezar.
- [ ] **5.2 - `MERGE`** -- M ⛔ (5.1)
- [ ] **5.3 - `INPUT PROCEDURE` / `OUTPUT PROCEDURE`** -- M ⛔ (5.1)
      Con `RELEASE` y `RETURN`. Los parrafos que necesita ya estan (0.4).

---

# FASE 6 -- Programas que se llaman, y el batch (JCL sustituido)

- [ ] **6.1 ★⚠ LA DECISION DEL ENLAZADOR** -- ⚠ decision, no codigo
      Escrita y sin tomar en `toolchain/forge/README.md`, con los dos caminos
      medidos: **A** enlazador de verdad (semanas; `bex-link` produce imagenes ya
      enlazadas a base fija y dos de esas no se concatenan) y **B** funciones
      sintetizadas (una sesion; el mecanismo ya corre en metal con
      `__bmo_syscall_stub`).
      **Una decision, tres desbloqueos**: `CALL` de COBOL, la libc de C, y C++
      con unidades separadas.
- [ ] **6.2 - `CALL` estatico** -- L ⛔ (6.1)
- [ ] **6.3 - `LINKAGE SECTION` / `PROCEDURE DIVISION USING`** -- M ⛔ (6.2)
- [ ] **6.4 - `CALL` dinamico y `CANCEL`** -- L ⛔ (6.1)
- [ ] **6.5 - `RETURN-CODE`** -- S ⛔ superficie
      ⚠ Comprobado: `bmo_lower::task::exit` **no acepta codigo de salida** y
      `TASK_OP_EXIT` tampoco lo lleva (el kernel hace revoke+reap). Esto toca la
      superficie congelada, no solo COBOL.
- [ ] **6.6 - El batch declarativo que sustituye a JCL** -- M ⛔ (6.5)
      **Sustituir, no clonar**: planificador con dependencias, asignacion de
      recursos y codigos de retorno, en un TOML que se lee.

---

# FASE 7 -- El despachador (CICS sustituido)

★ CICS paso cincuenta anos atornillando transacciones sobre un sistema de
ficheros que no las tenia. ESTRATOS las tiene en el fondo. **Lo que falta no es
la transaccionalidad -- es el despachador.**

- [ ] **7.1 ★ El despachador de transacciones** -- L ⛔ (3.4, 6.2)
- [ ] **7.2 - `SYNCPOINT` / `ROLLBACK` desde COBOL** -- M ⛔ (7.1)
      El `SYNCPOINT` es el superbloque alterno; el `ROLLBACK` es **no commitear**.
- [ ] **7.3 - Modelo pseudo-conversacional** -- L ⛔ (7.1)
      **No hay un proceso vivo por usuario** -- eso es lo que deja a un mainframe
      llevar miles de terminales.
- [ ] **7.4 - Seguridad por capabilities (sustituye a RACF)** -- M ⛔ (7.1)
- [ ] **7.5 ⚠ Bloqueo de registro / concurrencia** -- L ⚠ ⛔ (4.4, 7.1)
      ⚠ Copy-on-write da aislamiento de lectura gratis pero **no resuelve la
      escritura concurrente**: hay que elegir entre bloqueo pesimista y deteccion
      de conflicto al commitear.

---

# FASE 8 -- Lo que hace que sea un BANCO y no un programa

- [ ] **8.1 - Auditoria de verdad** -- M ⛔ (4.6)
      La version anterior sigue ahi; falta **poder preguntarla**.
- [ ] **8.2 - Cierre contable y cuadre** -- L ⛔ (5.1, 6.6)
- [ ] **8.3 ★ Un banco pequeno, de punta a punta** -- XL
      Alta de cuenta, movimiento, consulta por numero **y por DNI**, extracto,
      cierre diario. **En el Ryzen, no en el emulador.**

---

## Lo que cambio en la revision 2, y por que

Cuatro correcciones, todas por abrir el fichero en vez de razonar sobre el.

**1. La FASE 2 no estaba bloqueada por el parser de tokens.** `parser.rs` ya
consume varias lineas (`parse_if`, `parse_perform`), y `EVALUATE ... WHEN ...
END-EVALUATE` tiene la misma forma. Eso mueve la fase entera de "despues de una
L" a "se puede empezar hoy", y con ella el verbo que mas falta hace.

**2. `1.1` (registro binario) no necesita seek.** `ARCH_OP_LEER` ya saca bytes
crudos sin cortar por el salto de linea, y esta en el kernel y en el emulador.
Leer un registro de largo fijo **en secuencia** es repetirlo. El seek hace falta
para el acceso **directo** (fase 4), no para leer un fichero de principio a fin.

**3. Aparecio una tarea que no estaba: `0.7`, el texto.** `FILE STATUS` es un
`PIC XX`, y hoy un `PIC X` no guarda caracteres. La revision 1 daba `1.7` por
barata y sin dependencias; lo es, pero cuelga de que exista el texto. Con ella
cuelgan tambien `STRING`, `INSPECT` y `EBCDIC`.

**4. `SORT` no necesita `EXTEND`.** Cada pasada de la mezcla puede escribir a un
fichero nuevo con la operacion de crear que ya existe.

Y una que sigue en pie de la revision 1: **`0.5` no se puede hacer con el
reparto de pila de hoy**, pero ahora hay dos caminos y no uno (ver `1.0`).

## El orden corto

```
HECHO   0.1 VALUE - 0.3 OR (+ los 88 con THRU) - 0.4 PARRAFOS - COMP-3

HECHO   2.1 EVALUATE (las dos formas) - 2.6 ROUNDED (los seis modos)
        1.0 LA DECISION -- TOMADA: camino B, area de registro

        ====== SIN CANDADO: todo esto es COBOL y va PRIMERO ======

HECHO   0.5 RECORDS + el AREA + 1.3 MOVE de grupo + bmo-lower::zoned

HECHO   1.1 + 1.2 REGISTROS BINARIOS -- LEER LO QUE YA EXISTE
        un fichero de largo fijo, campos en su byte, importes empaquetados

HECHO   0.7 TEXTO -- PIC X con caracteres, sin limite de ancho

HECHO   1.7 FILE STATUS -- 00, 10 y 35: los que se pueden dar de verdad

HECHO   2.4 INSPECT - 2.3 STRING (falta UNSTRING)

HECHO   2.6b ON SIZE ERROR -- y dividir entre cero deja de matar el proceso

HECHO   0.6 GO TO -- y el ejemplo del nivel 8 ya no necesita el interruptor

HECHO   2.2 PERFORM VARYING -- con AFTER, y el reinicio sale de la recursion

AHORA   2.7 SEARCH - 2.8 COPY - 2.9 intrinsecas - UNSTRING - 5.1 SORT

LUEGO   0.7 TEXTO ---> 1.7 FILE STATUS - 2.3 STRING - 2.4 INSPECT - 1.6 EBCDIC
        2.6b ON SIZE ERROR - 0.6 GO TO - 2.2 PERFORM VARYING - 2.7 SEARCH
        2.8 COPY - 2.9 intrinsecas - 5.1 SORT - 0.2 parser de tokens

        ====== hasta aqui se llega al BATCH, y ahi esta el TECHO ======

KERNEL  3.1 EXTEND - 3.2 I-O - 3.3 POSICIONAR - 3.4 ESTRATOS ESCRIBE
                          +---> FASE 4 (VSAM) ---> FASE 7 (despachador)
        tres operaciones pequenas, y sin ellas no hay INDICE ni consulta viva

APARTE  6.1 EL ENLAZADOR ---> CALL, y de paso la libc y C++
```

**Si hay que elegir UNA cosa por sesion**:
~~`0.1`~~ -> ~~`0.3`~~ -> ~~`0.4`~~ -> ~~`2.1`~~ -> ~~`2.6`~~ -> ~~`1.0`~~ ->
~~`0.5`~~ -> ~~`1.3`~~ -> ~~`1.2`~~ -> ~~`1.1`~~ -> ~~`0.7`~~ -> ~~`1.7`~~ ->
~~`2.4`~~ -> ~~`2.3`~~ (falta `UNSTRING`) -> ~~`2.6b`~~ -> ~~`0.6`~~ -> ~~`2.2`~~ ->
**`2.7`** -> `2.8` -> `5.1` -> `1.6` -> `2.9` -> `1.5` -> `0.2` -> **luego el kernel**:
`3.1` -> `3.3` -> `3.2` -> `3.4` -> fase 4 ...

★ **`0.5` sube a lo siguiente** porque la decision `1.0` ya esta tomada y con
ella deja de tener candados. Es el primer eslabon de *leer lo que ya existe*,
que es lo que separa "COBOL que compila" de "COBOL que sirve para un banco".

★ **Y el kernel va al final de la lista de COBOL, no al principio**, por la
[estrategia de arriba](#-la-estrategia-primero-todo-lo-que-no-depende-del-sistema):
ahi esta el salto que queda sin candado, y las tres operaciones que faltan no se
ponen mas dificiles por esperar. Pero **estan en la lista**, no descartadas:
sin ellas el techo es el batch.

---

## El limite, dicho aqui tambien

Nada de esta lista convierte a BMO COBOL en un **destino de migracion desde
z/OS**. Ese codigo lleva cuarenta anos escrito contra CICS, JCL, VSAM y las
extensiones de IBM *tal cual son*, no contra equivalentes mejores.

Esto es para **sistemas que se escriben ahora**, y pequenos. Lo que esta lista si
consigue, si se termina, es que un banco pequeno pueda funcionar encima -- con
auditoria que z/OS no da, y sin pagar licencia a nadie.

---

## Registro de lo hecho

| Fecha | Que entro | Donde |
|---|---|---|
| 2026-08-03 | ★ **`COMP-3` real** -- el dato vive en nibbles, del ancho que dice su PICTURE | `bmo-lower::packed` + `codegen.rs` - ejemplo `7-empaquetado/` |
| 2026-08-03 | **0.1 `VALUE`** inicializa de verdad (se parseaba y no se emitia nunca) | `codegen::emit_valores_iniciales` |
| 2026-08-03 | **0.3 `OR`** -- la condicion es un arbol con cortocircuito; caen los `88` con `THRU` y con varios valores | `ast::Condicion` + `codegen::emit_jump_if_true/false` |
| 2026-08-03 | ★ **0.4 PARRAFOS** y las cuatro formas del `PERFORM` fuera de linea; `STOP RUN` termina de verdad | `codegen::emit_parrafos` - ejemplo `8-parrafos/` |
| 2026-08-03 | ★ **2.1 `EVALUATE`** -- con sujeto y `EVALUATE TRUE`; `THRU` y listas compartidos con el nivel 88 | `parser::parse_evaluate` + `Condicion::de_valores` |
| 2026-08-03 | ★ **2.6 `ROUNDED`** -- los seis modos del estandar en las cinco aritmeticas; se redondea el RESULTADO | `bmo-lower::redondeo` + `codegen.rs` |
| 2026-08-03 | **Bug de precision** que destapo `ROUNDED`: `COMPUTE` recortaba los operandos ANTES de operar | `codegen::emit_compute` (escala de trabajo) |
| 2026-08-03 | ★ **1.0 decidido** -- camino B: area de registro con empaquetado en la frontera | este documento, section FASE 1 |
| 2026-08-03 | ★★ **0.5 + 1.3** -- grupos con cada campo en su byte, el AREA DE REGISTRO funcionando, y `MOVE` de grupo por bytes | `cobol/registro.rs` + `bmo-lower::zoned` + `codegen.rs` |
| 2026-08-03 | ★ **El COPYBOOK** (`--copybook`) -- el byte exacto de cada campo, sacado de la MISMA tabla que emite el `READ` | `registro::Disposicion::copybook` |
| 2026-08-03 | ★★★ **1.1 + 1.2 REGISTROS BINARIOS** -- largo fijo, campos en su byte, y el resto de siete bytes bien llevado | `bmo-lower::archivo::leer_bytes` + `codegen::emit_read/emit_write` |
| 2026-08-03 | ★ **El VISOR** (`--ver`) -- decodifica un fichero binario con el copybook, y sus lectores estan atados a los emitidos | `registro::Disposicion::ver` + `*::*_en_rust` |
| 2026-08-03 | ★ **0.7 TEXTO** -- `PIC X(n)` con caracteres de verdad, sin limite de ancho; desbloquea FILE STATUS, STRING e INSPECT | `codegen.rs` (camino de direccion+largo) |
| 2026-08-03 | **1.7 `FILE STATUS`** -- `00`/`10`/`35`, los que la puerta permite distinguir, y solo esos | `codegen::emit_estado*` |
| 2026-08-03 | **2.4 `INSPECT`** (`TALLYING`, `REPLACING ALL`/`LEADING`) y **2.3 `STRING`** | `bmo-lower::texto` + `codegen.rs` |
| 2026-08-03 | **2.6b `ON SIZE ERROR`** -- el destino no se toca cuando no cabe, y dividir entre cero deja de matar el proceso | `codegen::emit_guardar_con_desborde` |
| 2026-08-03 | **0.6 `GO TO`** -- el descarte dentro de un rango, con el mismo simbolo que usa `PERFORM` | `codegen.rs` - ejemplo `8-parrafos/` actualizado |
| 2026-08-03 | **2.2 `PERFORM VARYING`** con `AFTER` -- el reinicio del interior sale de la recursion | `codegen::emit_varying` |

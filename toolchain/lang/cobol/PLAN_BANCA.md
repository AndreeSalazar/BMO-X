# El plan largo: de "BMO COBOL compila" a "BMO COBOL lleva un banco"

> Escrito el 2026-08-03, el día que entró `COMP-3`. Es la lista de tareas de
> [`BANCA_REAL.md`](BANCA_REAL.md): aquel documento dice **qué falta y por qué**;
> éste dice **en qué orden, qué bloquea a qué, y cómo se sabe que una está
> hecha**.
>
> Está hecho para avanzar **poco a poco**: cada casilla es una pieza que se
> puede entregar sola, con su prueba, sin dejar el compilador roto entre medias.
> Ninguna sesión debería tener que abrir dos fases a la vez.

## Cómo se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuánto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO por algo que NO es el compilador
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

---

# FASE 0 — El suelo

Nada grande de las fases siguientes entra limpio sin esto. Son baratas y
desbloquean mucho: hacerlas después significa hacer dos veces lo de en medio.

- [x] **0.1 · `VALUE` inicializa de verdad** — ✅ 2026-08-03
      Se emite al principio, después de repartir la pila y antes de la primera
      sentencia, **pasando por `store_var`** — así un `COMP-3` se inicializa
      empaquetado sin que el emisor de valores sepa que existen los nibbles.
      Sobre una tabla llena todas las casillas. Figurativas `ZERO`/`ZEROS`/
      `ZEROES`. Se rechaza con motivo: `VALUE` de texto, `VALUE` que no es un
      número, y `VALUE` sin PIC.

- [ ] **0.2 ★ El parser sobre TOKENS como principal** — L ⚠
      `tparser.rs` ya parsea un programa entero hasta BEF, pero **`compile_source`
      sigue usando `parser.rs`**, el analizador por líneas que decide con
      `upper.starts_with("MOVE ")`.
      Esto no es limpieza: es el **techo** de la fase 2 entera. `EVALUATE … WHEN
      … ALSO`, `STRING … DELIMITED BY … INTO` e `INSPECT … REPLACING` son
      sentencias multi-cláusula, y meterlas en el analizador de hoy es escribir
      deuda para rehacerla después.
      ⚠ La decisión: se jubila `parser.rs` de golpe o conviven. Conviviendo hay
      dos gramáticas que mantener; de golpe hay que mover **las 105 pruebas** a
      la vez. Escribir la decisión antes de tocar nada.

- [x] **0.3 · `OR` en las condiciones** — ✅ 2026-08-03
      La condición dejó de ser una `Vec` y es un **árbol** `Simple/Y/O`, porque
      `AND` liga más fuerte que `OR` y una lista plana no puede representar esa
      diferencia. Con **cortocircuito**, que no es una optimización: un operando
      puede ser un elemento de tabla y ahí la evaluación lleva guarda de rango.
      Cayeron con él los dos rechazos que dependían de él: **`88 … VALUE 1 THRU
      5`** y **`88 … VALUE 6, 7`**, que se expanden y bajan por el mismo emisor.
      Sigue faltando el `WHEN a, b, c` porque falta `EVALUATE` (2.1).

- [x] **0.4 · Párrafos y `PERFORM <párrafo>`** — ✅ 2026-08-03
      Un nombre de párrafo es una palabra sola con punto. Compilan
      `PERFORM <p>`, `PERFORM <p> THRU <q>`, `PERFORM <p> <n> TIMES` y
      `PERFORM <p> UNTIL <cond>`, más `EXIT`/`CONTINUE`.
      ★ El retorno se decide **en ejecución** (una ranura de pila con "en qué
      párrafo hay que volver"), no con un `ret` fijo: el mismo párrafo puede ser
      el final de un rango en una línea y estar en medio de otro en la de abajo.
      ⚠ **Falta `GO TO`**, y sin él el descarte dentro de un rango se escribe
      con un interruptor. Está dicho en el ejemplo en vez de fingirse.
      ★ De paso salió que **`STOP RUN` no emitía nada** — colaba por ser siempre
      la última línea, y ya estaba mal antes: un `STOP RUN` dentro de un `IF`
      se ignoraba en silencio.

- [ ] **0.5 · Records anidados con posiciones fijas** — M
      ⛔ **MOVIDA A LA FASE 1: depende de 1.5, y eso se descubrió midiendo el
      2026-08-03.** Ver el aviso de abajo. Sigue aquí sólo para que quede el
      rastro de por qué se movió.
      Grupos `01`/`05`/`10` donde cada campo cae en **su offset dentro del
      registro**. Hoy sólo existe el grupo `01` + `05` que usa `OCCURS`, y el
      registro de un fichero es **un solo campo**.

> ## ⚠ Lo que se descubrió al llegar a 0.5, y que reordena el plan
>
> **Un campo no puede caer en su offset mientras cada dato ocupe su propia
> ranura de ocho bytes.** Medido en `codegen.rs`: el reparto de la pila hace
> `let aligned = (size + 7) & !7` **por dato**, y `load_var`/`store_var` mueven
> con `mov rax, [rbp+off]` de 64 bits. Un `PIC 9(3)` contiguo mide tres bytes, y
> escribirlo con un `mov` de ocho **se lleva por delante al vecino**.
>
> O sea que "cada campo en su offset" y "un `DISPLAY` es un entero de 64 bits"
> son incompatibles, y **1.5 va primero**. Eso convierte a 1.5 —que estaba
> aparcada como *la decisión cara*— en la llave de toda la fase 1, no en un
> extra.
>
> El orden bueno pasa a ser: **1.5 → 0.5 → 1.2 → 1.1**.
>
> Y hay una consecuencia que conviene ver antes de empezar: el día que un
> `DISPLAY` mida lo que dice su PICTURE, **empezará a truncar**, igual que ya
> hace el `COMP-3`. Eso cambia el resultado de programas que hoy corren en el
> Ryzen. No es una regresión, es el estándar — pero hay que verlo venir, tener
> el ejemplo del nivel 7 delante (que enseña justo esa diferencia) y decidirlo a
> propósito.

---

# FASE 1 — Leer los datos que ya existen

La segunda mitad de `COMP-3`. Hoy un campo empaquetado vive bien **en memoria**,
pero el fichero sigue siendo **texto, un número por línea**. Un banco no da eso:
da registros de longitud fija con campos empaquetados dentro.

**Sin esta fase, BMO COBOL escribe programas nuevos y no puede leer nada de lo
que ya existe.** Es la fase que decide si esto sirve para migrar datos o sólo
para empezar de cero.

- [ ] **1.1 ★ Registro BINARIO de longitud fija** — L ⛔ (necesita 3.3)
      `RECORD CONTAINS n CHARACTERS`. Leer **n bytes crudos** en vez de una
      línea hasta el `\n`. Hoy `emit_read` llama a `leer_linea` y convierte con
      `parse_decimal_scaled`: eso es un registro de texto y no hay forma de
      pedirle otra cosa.
      ⛔ Pide posicionar por byte, que es 3.3 y es del kernel.

- [ ] **1.2 · Campos posicionales dentro del registro** — M
      Cada `05` en su offset, mezclando `COMP-3`, `DISPLAY` y `PIC X` en el
      mismo registro. Es 0.5 aplicado a un `FD`.

- [ ] **1.3 · `MOVE` de grupo** — S
      Mover un `01` entero a otro. La emisión ya existe: `bmo_lower::memoria`
      tiene `copiar`. En `lang/cobol` sólo falta el nombre.

- [ ] **1.4 · `REDEFINES`** — M
      Dos vistas del mismo espacio. Es como un banco lee un registro cuyo
      formato depende de un campo de tipo — el patrón está en todos lados y hoy
      se rechaza.

- [ ] **1.5 ★⚠ `DISPLAY` como ZONED DECIMAL real — LA LLAVE DE ESTA FASE** — L ⚠
      Hoy un campo `DISPLAY` es un **entero de 64 bits** con la escala de su
      PIC, así que **no trunca al ancho de su PICTURE** — por eso `PIC 9(3)`
      guarda `12345`. En COBOL de verdad es *un byte por dígito*, con el signo
      sobrepunzado en el último.
      ★ **Va la PRIMERA de la fase**, no la quinta: mientras cada dato ocupe su
      ranura de ocho bytes, ningún campo puede caer en su offset dentro de un
      registro (ver el aviso al final de la fase 0). Sin esto no hay 0.5, y sin
      0.5 no hay 1.1 ni 1.2 — o sea, no hay forma de leer un fichero de fuera.
      ⚠ **Es la decisión más cara de la lista.** Toca todo lo que ya funciona y
      ejecuta en el Ryzen, y el día que entre los campos **empezarán a truncar**
      igual que ya hace el `COMP-3`. Eso es el estándar, no una regresión, pero
      cambia la salida de programas que hoy corren. **Escribir la decisión antes
      de tocar una línea**, con el ejemplo del nivel 7 delante.

- [ ] **1.6 · EBCDIC ↔ ASCII al leer** — M
      Los datos de fuera vienen en EBCDIC. Una tabla de 256 entradas, que en
      esta casa significa **una tabla y no un cerebro** — el sitio natural es
      `bmo-lower`, junto a `packed`, por el mismo motivo: no es semántica de
      ningún lenguaje.

- [ ] **1.7 · `FILE STATUS`** — S
      El código de dos dígitos (`00` bien, `10` fin de fichero, `23` no
      encontrado, `35` no existe…). **Todo programa de banca lo mira después de
      cada operación**, y hoy no hay ninguno. Va antes de la fase 4 porque el
      acceso por clave sin `FILE STATUS` no se puede programar.

---

# FASE 2 — Los verbos que el código real usa

Baratos uno a uno, y desbloquean código que hoy **se rechaza con su motivo**
—que está bien, pero no compila—. Todos dependen de 0.2.

- [ ] **2.1 · `EVALUATE`** — M ⛔ (necesita 0.2, y 0.3 para el `WHEN a, b`)
      El `switch` de COBOL, con `WHEN … ALSO` y `WHEN OTHER`. El más usado de
      los que faltan.
- [ ] **2.2 · `PERFORM VARYING` completo** — M ⛔ (0.2)
      Con `FROM`/`BY`/`UNTIL` y `AFTER` para recorrer tablas de dos dimensiones.
- [ ] **2.3 · `STRING` / `UNSTRING`** — M ⛔ (0.2)
      Componer y partir cadenas. `DELIMITED BY`, `POINTER`, `ON OVERFLOW`.
- [ ] **2.4 · `INSPECT`** — M ⛔ (0.2)
      `TALLYING` y `REPLACING`. Es como COBOL limpia un campo antes de usarlo.
- [ ] **2.5 · `INITIALIZE`** — S
      Poner un grupo entero a su valor neutro. Barato una vez esté 1.3.
- [ ] **2.6 ★ `ROUNDED` / `ON SIZE ERROR`** — M
      **No son cláusulas de sintaxis: son de banca.** El redondeo es una
      decisión legal, y un desbordamiento silencioso en un importe es el fallo
      que no se puede permitir. Van con los cinco modos del estándar
      (`NEAREST-AWAY-FROM-ZERO` y compañía), no con uno inventado.
- [ ] **2.7 · `SEARCH` / `SEARCH ALL`** — M
      Búsqueda lineal y binaria en tabla. `SEARCH ALL` pide `ASCENDING KEY`.
- [ ] **2.8 · `COPY … REPLACING`** — M
      **Así se comparten los layouts de registro entre programas.** Sin esto,
      cada programa reescribe el `01` del fichero a mano y se descuadran solos.
- [ ] **2.9 · Las intrínsecas que importan (~15 de 55)** — M
      `NUMVAL`, `NUMVAL-C`, `CURRENT-DATE`, `INTEGER-OF-DATE`,
      `DATE-OF-INTEGER`, `LENGTH`, `MAX`, `MIN`, `MOD`, `REM`, `UPPER-CASE`,
      `LOWER-CASE`, `TRIM`, `ORD`, `WHEN-COMPILED`. Las otras 40 son cola larga.

---

# FASE 3 — El sistema debajo

⛔ **Nada de esto se arregla en `lang/cobol`.** Son cambios de kernel y de
ESTRATOS, y la fase 4 entera está detrás de ellos. Comprobado el 2026-08-03.

- [ ] **3.1 · `KIND_ARCHIVO`: modo EXTEND** — S ⛔ kernel
      Añadir al final. Hoy `OPEN EXTEND` **se rechaza a propósito**: la puerta
      abre creando de cero, así que compilarlo como `OUTPUT` borraría el
      histórico entero y el programa parecería funcionar hasta que alguien
      buscara el mes pasado.
- [ ] **3.2 ★ `KIND_ARCHIVO`: modo I-O** — M ⛔ kernel
      Un handle que **lea y escriba**. Hoy el modo se fija al abrir, y por eso
      `OPEN I-O` se rechaza con ese motivo escrito en `emit_open`.
      Es lo que bloquea `REWRITE` y `DELETE`, o sea **lo que hace que un KSDS
      sea un KSDS y no un listado ordenado**.
- [ ] **3.3 ★ `KIND_ARCHIVO`: posicionar por byte** — M ⛔ kernel
      Sin esto no hay acceso directo a nada: ni RRDS, ni un B-tree, ni un
      registro de longitud fija leído por número.
- [ ] **3.4 ★ ESTRATOS: crear objetos y ESCRIBIR** — XL ⛔ ESTRATOS
      Hoy monta, lee y sabe commitear (`sellar()`), y la máquina de estados de
      la transacción está probada en el anfitrión. **Lo que falta es crear.**
      Mientras no esté, un índice sólo cabe sobre `KIND_ARCHIVO` — y ahí se
      pierden exactamente las tres cosas que hacen que este índice sea mejor que
      el de z/OS: copy-on-write, transaccionalidad y auditoría.

---

# FASE 4 — VSAM: de listados a banca

★★ **La fase que decide.** *"Dame la cuenta 4471-9982"* no puede significar leer
cuatro millones de registros. **Sin índice no hay banca, hay listados.**

- [ ] **4.1 · RRDS — registros por número** — M ⛔ (3.3)
      `ORGANIZATION RELATIVE`, `ACCESS MODE RANDOM`, `RELATIVE KEY`. Es lo más
      barato que ya es acceso directo, y sirve de banco de pruebas de 3.3.
- [ ] **4.2 · ESDS — secuencial de sólo añadir** — S ⛔ (3.1)
      Casi hecho: es el File I/O de hoy más el modo `EXTEND`.
- [ ] **4.3 ★ KSDS, la LECTURA** — XL ⛔ (3.2, 3.3)
      El B-tree. `RECORD KEY`, `READ … KEY IS`, `START`, `READ NEXT`.
      Un KSDS **es** un B-tree, y sobre ESTRATOS hereda tres cosas gratis que
      z/OS no da (ver `BANCA_REAL.md`).
- [ ] **4.4 · KSDS, la ESCRITURA** — L ⛔ (3.2, 4.3)
      Insertar sin destruir el árbol anterior, `REWRITE`, `DELETE`.
- [ ] **4.5 · `ALTERNATE RECORD KEY … WITH DUPLICATES`** — L ⛔ (4.4)
      El índice por DNI además del de número de cuenta. Es lo que un banco
      pregunta todo el rato.
- [ ] **4.6 ★ El índice SOBRE ESTRATOS** — XL ⛔ (3.4, 4.4)
      Copy-on-write (una inserción no destruye el árbol anterior), transaccional
      (el índice nuevo sólo existe al commitear: **no hay índice a medias**) y
      auditable (la versión anterior sigue ahí).
      **Esto es lo que un auditor quiere y VSAM sobre z/OS no le da.**

---

# FASE 5 — SORT

Sin ordenación externa no hay batch de verdad: un cierre ordena antes de
totalizar, y el fichero no cabe en memoria.

- [ ] **5.1 · `SORT` externo** — L ⛔ (3.1)
      Mezcla por tramos sobre ficheros. `ON ASCENDING/DESCENDING KEY`.
- [ ] **5.2 · `MERGE`** — M ⛔ (5.1)
- [ ] **5.3 · `INPUT PROCEDURE` / `OUTPUT PROCEDURE`** — M ⛔ (0.4, 5.1)
      Con `RELEASE` y `RETURN`. Es como un batch filtra mientras ordena, y pide
      párrafos (0.4).

---

# FASE 6 — Programas que se llaman, y el batch (JCL sustituido)

- [ ] **6.1 ★⚠ LA DECISIÓN DEL ENLAZADOR** — ⚠ decisión, no código
      Está **escrita y sin tomar** en `toolchain/forge/README.md`.
      **Una decisión, tres desbloqueos**: `CALL` de COBOL, la libc de C, y C++
      con unidades de traducción separadas. No es casualidad — es la misma
      pregunta las tres veces.
      **Va antes que 6.2, y no se empieza 6.2 sin ella tomada.**
- [ ] **6.2 · `CALL` estático** — L ⛔ (6.1)
      Llamar a otro programa COBOL. Imprescindible: un banco no tiene un
      programa, tiene mil que se llaman.
- [ ] **6.3 · `LINKAGE SECTION` / `PROCEDURE DIVISION USING`** — M ⛔ (6.2)
      Cómo se pasan los datos. Va pegado a 6.2.
- [ ] **6.4 · `CALL` dinámico y `CANCEL`** — L ⛔ (6.1)
      Resolver el nombre en ejecución. Es lo que permite sustituir un programa
      sin recompilar los mil que lo llaman.
- [ ] **6.5 · `RETURN-CODE`** — S ⛔ (6.2)
      El registro especial. Sin él, el planificador de 6.6 no puede decidir.
      ⚠ Hoy `TASK_OP_EXIT` **no acepta código de salida** (el kernel hace
      revoke+reap): esto toca la superficie, no sólo COBOL.
- [ ] **6.6 · El batch declarativo que sustituye a JCL** — M ⛔ (6.5)
      **Sustituir, no clonar.** Debajo de la sintaxis de JCL hay tres cosas
      razonables —planificador con dependencias, asignación de recursos y manejo
      de códigos de retorno— y clonar la forma sería importar sesenta años de
      accidentes para no ganar nada. Un TOML dice lo mismo y se lee.

---

# FASE 7 — El despachador (CICS sustituido)

★ **Aquí BMO-X sale ganando por una razón estructural**: CICS pasó cincuenta
años atornillando transacciones sobre un sistema de ficheros que no las tenía.
ESTRATOS las tiene en el fondo. **Lo que falta no es la transaccionalidad — es
el despachador.**

- [ ] **7.1 ★ El despachador de transacciones** — L ⛔ (3.4, 6.2)
      Recibir una petición, entregarle sus capabilities, ejecutar, y commitear o
      abandonar. Es pequeño; lo caro estaba debajo y ya está.
- [ ] **7.2 · `SYNCPOINT` / `ROLLBACK` desde COBOL** — M ⛔ (7.1)
      El `SYNCPOINT` es el superbloque alterno; el `ROLLBACK` es **no hacer el
      commit**. No hace falta journal de recuperación porque nada se
      sobreescribe.
- [ ] **7.3 · Modelo pseudo-conversacional** — L ⛔ (7.1)
      El programa corre, termina, y su estado se guarda entre pantallas. **No
      hay un proceso vivo por usuario** — que es lo que deja a un mainframe
      llevar miles de terminales.
- [ ] **7.4 · Seguridad por capabilities (sustituye a RACF)** — M ⛔ (7.1)
      Una transacción recibe **las capabilities que necesita y ninguna más**.
      Esto es lo que BMO-X ya sabe hacer y RACF simula con listas.
- [ ] **7.5 ⚠ Bloqueo de registro / concurrencia** — L ⚠ ⛔ (4.4, 7.1)
      Dos transacciones sobre la misma cuenta. ⚠ Copy-on-write da aislamiento de
      lectura gratis pero **no resuelve la escritura concurrente**: hay que
      decidir entre bloqueo pesimista y detección de conflicto al commitear.
      Escribir la decisión antes de escribir código.

---

# FASE 8 — Lo que hace que sea un BANCO y no un programa

- [ ] **8.1 · Auditoría de verdad** — M ⛔ (4.6)
      La versión anterior sigue ahí. ESTRATOS lo da gratis; lo que falta es
      **poder preguntarlo**: cómo estaba esta cuenta el martes.
- [ ] **8.2 · Cierre contable y cuadre** — L ⛔ (5.1, 6.6)
      El proceso nocturno completo, con su descuadre detectado y dicho.
- [ ] **8.3 ★ Un banco pequeño, de punta a punta** — XL
      Alta de cuenta, movimiento, consulta por número **y por DNI**, extracto,
      cierre diario. **En el Ryzen, no en el emulador.**
      Es la única prueba que vale de que las siete fases de arriba se sostienen
      juntas.

---

## El orden corto, para no leer todo

```
 0.1 VALUE ✅
 0.3 OR ✅  ──→ (cayeron con el los 88 con THRU y con varios valores)
 0.4 PARRAFOS ✅ ──→ 5.3, 6.x
 0.2 PARSER ──→ FASE 2 (los verbos)
 1.5 ZONED ──→ 0.5 RECORDS ──→ 1.2 ──→ 1.1 (leer ficheros de fuera)

 3.1/3.2/3.3 KIND_ARCHIVO ──→ 1.1, FASE 4, FASE 5
 3.4 ESTRATOS ESCRIBE ──────→ 4.6, FASE 7

 6.1 EL ENLAZADOR ──────────→ 6.2..6.6, y de paso la libc y C++
```

**Si hay que elegir UNA cosa por sesión**, el orden que menos trabajo tira
(tachado lo hecho el 2026-08-03):

~~`0.1`~~ → ~~`0.3`~~ → ~~`0.4`~~ → **`0.2`** → `1.7` → `2.1` → `2.6` → `1.5` →
`0.5` → `3.1` → `3.3` → `1.2` → `1.1` → `3.2` → `4.1` → `4.3` → …

Cambió respecto de la primera versión por dos hallazgos: **1.5 subió** (sin ella
no hay records con posiciones fijas, y sin eso no hay fase 1), y **1.7 bajó** —
`FILE STATUS` es barato, no depende de nadie y hace falta en cuanto se toque
E/S de verdad.

---

## El límite, dicho aquí también

Nada de esta lista convierte a BMO COBOL en un **destino de migración desde
z/OS**. Ese código lleva cuarenta años escrito contra CICS, JCL, VSAM y las
extensiones de IBM *tal cual son*, no contra equivalentes mejores.

Esto es para **sistemas que se escriben ahora**, y pequeños. Lo que esta lista sí
consigue, si se termina, es que un banco pequeño pueda funcionar encima —
con auditoría que z/OS no da, y sin pagar licencia a nadie.

Ver el README raíz, sección *"And one boundary worth stating before anyone
assumes otherwise"*.

---

## Registro de lo hecho

| Fecha | Qué entró | Dónde |
|---|---|---|
| 2026-08-03 | ★ **`COMP-3` real** — el dato vive en nibbles, del ancho que dice su PICTURE | `bmo-lower::packed` + `codegen.rs` · ejemplo `7-empaquetado/` |
| 2026-08-03 | **0.1 `VALUE`** inicializa de verdad (se parseaba y no se emitía nunca) | `codegen::emit_valores_iniciales` |
| 2026-08-03 | **0.3 `OR`** — la condición es un árbol con cortocircuito; caen los `88` con `THRU` y con varios valores | `ast::Condicion` + `codegen::emit_jump_if_true/false` |
| 2026-08-03 | ★ **0.4 PÁRRAFOS** y las cuatro formas del `PERFORM` fuera de línea; `STOP RUN` termina de verdad | `codegen::emit_parrafos` · ejemplo `8-parrafos/` |

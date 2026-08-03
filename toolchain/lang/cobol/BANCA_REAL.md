# Lo que le falta a BMO COBOL para banca de verdad

> Escrito el 2026-08-02. La pregunta que lo motivó: *"dije COBOL, pero fue muy
> general — ¿qué necesitaría de verdad?"*
>
> Porque "tener COBOL" no es tener el lenguaje. Un sistema bancario de
> mainframe se apoya en **cuatro cosas** además del compilador, y hasta que no
> se nombran una por una, "COBOL" es una palabra que promete de más.

## Resumen, para no leer todo

```
CICS   el monitor de transacciones   -> ★ LO DIFICIL YA ESTA HECHO (ESTRATOS)
JCL    el batch                      -> sustituir, no clonar. Una tarde.
VSAM   ficheros indexados            -> ★★ EL HUECO REAL. B-tree sobre ESTRATOS
EXT.   extensiones de IBM            -> COMP-3 ✅ · CALL, EVALUATE, STRING, SORT
```

> **Lo hecho desde que se escribió esto** — 2026-08-03: **`COMP-3` funciona de
> verdad**. Punto 1 de la lista del final. Ver [§4](#4-las-extensiones-de-ibm--la-lista-larga)
> y el ejemplo `examples/7-empaquetado/cuentas.cob`.

**No se clona nada de esto.** Hace falta la **capacidad**, no la **forma**. Un
`EXEC CICS` con la sintaxis de 1968 no aporta nada; lo que aporta es que la
transacción se aplique entera o nada.

---

## 1. CICS — el monitor de transacciones (1968)

Gestiona miles de terminales concurrentes, cada una ejecutando transacciones
cortas. El COBOL lo invoca con `EXEC CICS ... END-EXEC` incrustado, que un
preprocesador traduce antes de compilar.

**Lo que de verdad aporta:**

- Despacho de transacciones — una unidad de trabajo por interacción
- **Modelo pseudo-conversacional**: el programa corre, termina, y su estado se
  guarda entre pantallas. No hay un proceso vivo por usuario
- **Syncpoint**: o la transacción entera se aplica, o **nada**
- Gestión de pantallas 3270 (BMS)
- Seguridad, vía RACF

### ★ Aquí BMO-X sale ganando, y por una razón estructural

**CICS pasó cincuenta años atornillando transacciones sobre un sistema de
ficheros que no las tenía.** ESTRATOS las tiene en el fondo:

| CICS necesita | ESTRATOS |
|---|---|
| `SYNCPOINT` (aplicar todo o nada) | **escribir ES commitear**: el superbloque alterno de un sector, atómico por el disco |
| `SYNCPOINT ROLLBACK` | no hacer el commit. El volumen sigue siendo el de antes, sin deshacer nada |
| journal de recuperación | no hace falta: **nada se sobreescribe** |
| punto de recuperación tras un corte | el superbloque de la generación anterior |

Lo que falta **no** es la transaccionalidad. Es el **despachador**: recibir una
petición, entregarle sus capabilities, ejecutar, y commitear o abandonar.

Y las pantallas 3270 no se quieren. Son de 1972 y su sustituto ya existe:
`KIND_CONSOLE` en los dos sentidos, que ya funciona en metal.

**Trabajo estimado**: el despachador es pequeño. La transaccionalidad, hecha.

---

## 2. JCL — el lenguaje de trabajos batch (1964)

Describe qué programa correr, qué ficheros asignarle, qué hacer según el código
de retorno, y en qué orden.

```jcl
//STEP1  EXEC PGM=CIERRE
//INPUT    DD DSN=BANCO.MOVIM,DISP=SHR
//OUTPUT   DD DSN=BANCO.CIERRE,DISP=(NEW,CATLG)
//SYSOUT   DD SYSOUT=*
```

Debajo de esa sintaxis hay tres cosas razonables: un **planificador con
dependencias**, una **asignación de recursos** (qué fichero se llama cómo dentro
del programa) y un **manejo de códigos de retorno**.

### Veredicto: sustituir, no clonar

Es lo que BMO-X hace mejor sin esfuerzo, porque **ya piensa en tablas**. Un TOML
declarativo dice lo mismo y se lee:

```toml
[[paso]]
programa = "cobol/cierre.bex"
entrada  = { MOVIM = "datos/movim.txt" }
salida   = { CIERRE = "datos/cierre.txt" }
sigue_si = 0
```

Y encaja con la regla de la casa: **Python/tablas para lo tabular, Rust para la
semántica**. Clonar la sintaxis de JCL sería importar sesenta años de accidentes
para no ganar nada.

**Trabajo estimado**: una tarde, y queda mejor que el original.

---

## 3. VSAM — ★★ EL HUECO REAL

**No es un sistema de ficheros: son métodos de acceso por REGISTRO.**

| Tipo | Qué es | ¿Hace falta? |
|---|---|---|
| **KSDS** (Key-Sequenced) | **indexado por clave, un B-tree**. El caballo de batalla | 🔴 **sí — es EL que falta** |
| **ESDS** (Entry-Sequenced) | secuencial, sólo añadir | 🟡 casi hecho (File I/O secuencial) |
| **RRDS** (Relative Record) | registros fijos, por número | 🟡 fácil sobre lo que hay |
| **LDS** (Linear) | bytes crudos sin estructura | ⚪ no |

En COBOL esto es:

```cobol
SELECT CUENTAS ASSIGN TO "datos/cuentas"
    ORGANIZATION IS INDEXED
    ACCESS MODE IS DYNAMIC
    RECORD KEY IS CTA-NUMERO
    ALTERNATE RECORD KEY IS CTA-DNI WITH DUPLICATES.
```

Y los verbos que lo acompañan: `READ ... KEY IS`, `START`, `READ NEXT`,
`REWRITE`, `DELETE`.

### Por qué éste es el que decide

Hoy BMO COBOL tiene **File I/O secuencial**. Eso sirve para un batch que recorre
todo. **No sirve para banca interactiva**: *"dame la cuenta 4471-9982"* no puede
significar leer cuatro millones de registros.

**Sin índice no hay banca, hay listados.**

### La buena noticia

Un KSDS es **un B-tree**, y ESTRATOS ya es un grafo de objetos con punteros y
sumas BLAKE3. Un índice sobre ESTRATOS hereda tres cosas gratis:

- **Copy-on-write**: una inserción no destruye el árbol anterior
- **Transaccional**: el índice nuevo sólo existe cuando se commitea el
  superbloque. No hay índice a medias
- **Auditable**: la versión anterior del índice sigue ahí

Eso es lo que un VSAM sobre z/OS **no** te da, y es lo que un auditor quiere.

### ⚠️ Dos bloqueos que no son del compilador (comprobados el 2026-08-03)

Antes de escribir una línea de B-tree hay que saber esto, porque ninguna de las
dos cosas se arregla en `lang/cobol`:

1. **`OPEN I-O` es imposible hoy.** `KIND_ARCHIVO` **fija el modo al abrir**, así
   que no existe un handle que lea y escriba (`codegen.rs`, `emit_open`, lo
   rechaza diciéndolo). Y `REWRITE` y `DELETE` —los verbos que hacen que un KSDS
   sea un KSDS y no un listado ordenado— necesitan justamente eso. **Es un
   cambio de kernel, y va primero.**
2. **ESTRATOS todavía no crea objetos.** Monta, lee y sabe commitear (`sellar()`
   en `ring0/fsys/estratos.rs`, y la máquina de estados de la transacción está
   probada en el anfitrión). Un índice encima necesita escritura de verdad.
   Mientras tanto sólo cabe ponerlo sobre `KIND_ARCHIVO` — y ahí se pierden
   exactamente las tres cosas que esta sección vende como gratis: el
   copy-on-write, la transaccionalidad y la auditoría.

**Trabajo estimado**: es la pieza grande de esta lista, y la de mayor valor —
pero el camino empieza por esos dos, no por el árbol.

---

## 4. Las extensiones de IBM — la lista larga

| Extensión | Qué es | Veredicto |
|---|---|---|
| **`COMP-3`** (packed decimal) | 2 dígitos por byte + signo. **El formato en el que están los datos reales de un banco** | ✅ **HECHO** (2026-08-03) — ver abajo |
| **`CALL`** | llamar a otro programa, estático o dinámico | 🔴 imprescindible — **depende de la decisión del enlazador** (ver `forge/README.md`) |
| **`EVALUATE`** | el `switch` de COBOL, con `WHEN ... ALSO` | 🟠 muy usado |
| **`STRING` / `UNSTRING` / `INSPECT`** | manejo de cadenas | 🟠 muy usado |
| **`COPY ... REPLACING`** | inclusión con sustitución de texto | 🟠 muy usado: así se comparten los layouts de registro |
| **`SORT` / `MERGE`** | ordenación externa de ficheros grandes | 🟠 necesario en batch de verdad |
| **`ROUNDED` / `ON SIZE ERROR`** | cláusulas de la aritmética | 🟠 son de banca: el redondeo es una decisión legal |
| `COMP-5` | binario nativo del host | 🟡 fácil |
| `PERFORM VARYING` completo | bucles con índice | 🟡 medio hecho |
| Las 55 intrínsecas | `FUNCTION NUMVAL`, `CURRENT-DATE`… | 🟡 unas quince importan |
| `COMP-1` / `COMP-2` | coma flotante | ⚪ **la banca no lo usa, y con razón** |
| `EXEC SQL` | Db2 incrustado | ⚪ es otra base de datos entera |
| `EXEC CICS` | ver punto 1 | ⚪ no clonar la sintaxis |
| Report Writer | generador de informes declarativo | ⚪ casi nadie lo usa |
| LE (Language Environment) | el runtime de IBM | ⚪ es el suyo; BMO tiene el propio |
| DBCS / `NATIONAL` (UTF-16) | japonés, chino | ⚪ fuera de alcance |

### ✅ `COMP-3` — hecho el 2026-08-03

Un campo declarado `COMP-3` / `COMPUTATIONAL-3` / `PACKED-DECIMAL` **guarda
nibbles**, no un entero de 64 bits con otro nombre:

```cobol
01 SALDO   PIC S9(7)V99 COMP-3.        *> 6 bytes, signo en el último nibble
01 CORTO   PIC 9(3)     COMP-3.        *> 2 bytes, y TRUNCA a 3 dígitos
```

Lo que se decidió, y por qué:

- **La conversión BCD vive en `bmo-lower::packed`, no en COBOL.** Empaquetar es
  una REPRESENTACIÓN, no la semántica de un lenguaje: los mismos nibbles en el
  mismo orden los pide el `Decimal` del Annex F de Ada y el `FIXED DECIMAL` de
  PL/I. Se comparten librerías, nunca cerebros. Lo que se queda en COBOL es
  *quién* es COMP-3, y eso lo dice la PICTURE.
- **Sólo lo miran `load_var` y `store_var`.** La aritmética sigue viendo el
  entero escalado de siempre — el decimal exacto no se entera de la
  representación, que es exactamente el reparto que lo mantiene exacto.
- **Se emite DESENROLLADO.** El ancho se conoce al compilar, así que no hay
  bucle ni salto hacia atrás: un campo de 18 dígitos son diez pasos.
- **El `S` decide el nibble**: `C` positivo, `D` negativo, `F` sin signo. Un
  campo sin `S` guarda el **valor absoluto**, como manda el estándar. Al LEER se
  acepta además `B` como negativo, porque viene en los datos de fuera y tomarlo
  por positivo convierte un cargo en un abono sin que salte nada.
- **`COMP` / `BINARY` / `COMP-5` se RECHAZAN diciendo por qué**, en vez de
  aceptarse y guardar exactamente lo mismo que un `DISPLAY`. `COMP-1` / `COMP-2`
  se rechazan porque son coma flotante y la banca no la usa.

**Lo que esto todavía NO es:** el fichero sigue siendo **texto**, un número por
línea. Leer los bytes empaquetados *tal cual vienen del mainframe* pide un
**registro binario con varios campos**, y eso es otro paso — el que de verdad
cierra "leer los datos que ya existen".

---

## El orden en que yo lo haría

1. ~~**`COMP-3`**~~ — ✅ **hecho el 2026-08-03**. Falta su segunda mitad: el
   **registro binario**, que es lo que permite leer un fichero empaquetado de
   fuera en vez de sólo guardar así en memoria
2. **`EVALUATE`, `STRING`, `INSPECT`** — verbos, baratos, y desbloquean código
   real que hoy no compila. ⚠️ **Piden antes el parser sobre tokens**: son
   sentencias multi-cláusula, y meterlas en el analizador por líneas de hoy
   (`upper.starts_with("MOVE ")`) es construir deuda para rehacerla después
3. **`SORT`** — sin ordenación no hay batch de verdad
4. **KSDS (índice)** — la pieza grande, y la que convierte listados en banca
5. **El despachador de transacciones** sobre ESTRATOS
6. **El batch declarativo** que sustituye a JCL
7. **`CALL`** — cuando la decisión del enlazador esté tomada

## Y un detalle que no es casualidad

**`CALL` depende de la misma decisión que bloquea la libc y que bloquearía a C++
con unidades separadas.** Una decisión, tres desbloqueos — y por eso está
escrita y sin tomar en `toolchain/forge/README.md` en vez de resuelta de paso.

## El límite, dicho aquí también

Nada de esta lista convierte a BMO COBOL en un **destino de migración desde
z/OS**. Ese código lleva cuarenta años escrito contra CICS, JCL, VSAM y las
extensiones de IBM *tal cual son*, no contra equivalentes mejores.

Esto es para **sistemas que se escriben ahora**, y pequeños. Ver el README raíz,
sección *"And one boundary worth stating before anyone assumes otherwise"*.

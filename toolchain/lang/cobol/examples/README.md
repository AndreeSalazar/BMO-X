# Los ejemplos, por NIVEL

No están ordenados por tema sino por **cuánto COBOL hace falta que el
compilador sepa** para que corran. Subir un escalón es una característica nueva
en el codegen, no un ejemplo más largo.

Sirve para tres cosas:

1. Ver de un vistazo **hasta dónde llega BMO COBOL hoy**.
2. Tener un orden por el que romper cosas: **si falla el 10, comprueba primero
   que el 1 sigue vivo**.
3. **Verificar en el Ryzen de uno en uno.** En el volumen de datos cada nivel
   tiene su carpeta, así que se sube la escalera a mano:

```
run cobol/1/hola.bex        y si eso va, subir
run cobol/2/banco.bex       y si eso va, subir
...
run cobol/10/maestro.bex
```

## La escalera

La columna **Ryzen** no es decorativa: un test en el emulador dice que el
codegen es coherente consigo mismo; una foto de la pantalla dice que el CPU
real hizo lo que el fuente promete. No son la misma afirmación.

| Nivel | Carpeta | En el disco | Ryzen | Qué hace falta que el compilador sepa |
|---|---|---|---|---|
| 1 | `1-basico/` | `cobol/1/hola.bex` | ✅ | `DISPLAY` de literal, `STOP RUN` |
| 2 | `2-decimal/` | `cobol/2/banco.bex` `calc.bex` `calcgui.bex` | ✅ | `PIC` con escala, `MOVE`, las cinco operaciones, `IF`/`ELSE`, `PERFORM`, `COMPUTE` con precedencia, `ACCEPT` |
| 3 | `3-presentacion/` | `cobol/3/extracto.bex` | ✅ | `PICTURE` de **edición** emitida como instrucciones |
| 4 | `4-ficheros/` | `cobol/4/batch.bex` | ✅ | `SELECT`/`ASSIGN`, `FD`, `OPEN`/`READ … AT END`/`WRITE`/`CLOSE` |
| 5 | `5-tablas/` | `cobol/5/concep.bex` | ✅ | `OCCURS` con subíndice literal y variable, con guarda de rango |
| 6 | `6-condiciones/` | `cobol/6/carter.bex` | ✅ | Nivel **88**: nombres de condición |
| 7 | `7-empaquetado/` | `cobol/7/cuentas.bex` | — | `USAGE COMP-3`: el dato guardado en **nibbles**, del ancho de su PIC |
| 8 | `8-parrafos/` | `cobol/8/cierre.bex` | — | **Párrafos** y las cuatro formas del `PERFORM` fuera de línea, `VALUE`, `OR` |
| 9 | `9-decision/` | `cobol/9/comisio.bex` | ✅ **2026-08-03** | `EVALUATE TRUE` y **`ROUNDED` con sus modos** |
| 10 | `10-binario/` | `cobol/10/maestro.bex` | ✅ **2026-08-03** | **Registros binarios de largo fijo** con los campos en su byte |

Los diez corren en el emulador y tienen su test.

### Lo que se vio en el Ryzen el 2026-08-03

Los dos escalones de arriba se verificaron **antes** que el 7 y el 8, que
siguen sin estrenar. Se dice así y no "del 1 al 10" porque un hueco declarado
se puede cerrar y uno tapado no.

**Nivel 9 — los dos redondeos divergen en metal.** El clásico dio `0.10` y el
del banquero `0.08` sobre los mismos cuatro empates. Son los números exactos
que el fuente promete en su comentario, y **es la línea que no se puede
fingir**: si `MODE IS NEAREST-EVEN` se hubiera compilado como el redondeo de
siempre, las dos cifras saldrían iguales y nadie lo notaría hasta el cuadre.

**Nivel 10 — el ida y vuelta por el disco cuadra.** Escribió tres registros de
16 bytes a `datos/ctas.bin`, los releyó, y salió:

```
15.234,75  −  890,10  +  3.105,40  =  17.450,05
                    en descubierto: 1
```

Con `890.10CR` impreso con su marca de signo. Eso es COMP-3 empaquetando a
nibbles, cruzando FAT32 hasta el Kingston y desempaquetando **sin perder un
céntimo** — el ciclo entero de E/S de COBOL en hardware real.

> ⚠ **La trampa que estas fotos NO podían enseñar, y que ya está arreglada en
> el código.** Hasta el 2026-08-03, `OPEN OUTPUT` bajaba a
> `TASK_OP_ARCHIVO_CREAR` y el FAT32 del kernel **no sabía reemplazar un
> fichero que ya existía**: a partir de la segunda corrida el `CLOSE` fallaba y
> no guardaba nada. Como el programa vuelve a leer el mismo fichero con los
> mismos valores, **la pantalla salía idéntica**. Por eso no se puede saber si
> estas fotos son de la primera corrida o de la quinta: desde la pantalla no se
> distingue, y ése era el problema.
>
> Arreglado con `save_file_in_dir` en el driver y `guardar_en` en `fs.rs`, y el
> `CLOSE` de COBOL ahora deja `30` en el `FILE STATUS` cuando no se guardó.
> **Pendiente de verificar en el Ryzen**: la prueba es correr el nivel 10, tocar
> un saldo en el fuente, recompilar y volver a correrlo — tiene que salir el
> nuevo.

> ⚠ La carpeta del disco es el **número a secas** y no el nombre largo. No es
> pereza: el driver FAT32 del kernel **se niega a recortar**, y
> `3-presentacion` son trece letras. El nombre del nivel vive aquí, que es
> donde se lee.

## Qué hay en cada escalón

**1 — `hola.cob`.** El `DISPLAY` baja a `bmo-lower` y de ahí al único syscall
que existe. Sin runtime de COBOL y sin libc. Quince líneas.

**2 — el decimal, que es la razón de que COBOL siga vivo.**
- `hola_COBOL.cob` — **el que el kernel EMBEBE** con `include_bytes!`. Cada
  sección prueba algo que antes fingía: `IF` ejecutaba las dos ramas, `PERFORM`
  no repetía, los operandos se tomaban todos como literales. `3 × 19.99 = 59.97`
  exacto, en centavos. **Al tocar el codegen hay que regenerarlo** — su salida
  está fijada por un test.
- `banco.cob` — cuotas y devoluciones sobre un saldo.
- `calc.cob` — `ACCEPT` de dos importes por consola.
- `calcgui.cob` — **lo llama el compositor** cuando pulsas `=` en la
  calculadora. La cara es Rust; la cuenta es COBOL. No se borra: tiene usuario.

**3 — `extracto.cob`.** `$12,345.67`, `*****0.45`, `120.00CR`. La máscara no se
interpreta en ejecución: el recorrido de la plantilla **se emite como
instrucciones**, así que en el `.bex` no queda ni la máscara ni un intérprete.

**4 — `batch.cob`.** El proceso por lotes: leer movimientos, totalizar en
decimal exacto, escribir el cierre. `AT END` es obligatorio y no por rigor — es
lo único que puede parar un `PERFORM UNTIL` sobre un fichero.

**5 — `conceptos.cob`.** Dos ficheros en paralelo y `OCCURS`: cada importe a la
casilla de su concepto. El subíndice **viene del fichero**, así que el rango no
lo decide el programador; si se sale, el programa para diciendo qué tabla.

**6 — `cartera.cob`.** El mismo batch escrito con NOMBRES: `PERFORM UNTIL
SE-ACABO` en vez de `UNTIL FIN = 1`. Un 88 **no reserva ni un byte** — hay un
test que lo comprueba comparando el tamaño del código con y sin ellos.

**7 — `cuentas.cob`.** El primer escalón que cambia **cómo se guarda** el dato y
no qué se hace con él. Un `COMP-3` vive en nibbles y ocupa **lo que dice su
PICTURE**, así que lo que no cabe se pierde por arriba.

Por eso el programa imprime el mismo `12345` dos veces: en un `PIC 9(3) COMP-3`
sale `345` y en un `PIC 9(3)` a secas sale `12345`. Ésa es la línea que **no se
puede fingir**.

**8 — `cierre.cob`.** El primer ejemplo escrito **como se escribe COBOL de
verdad**: un cuerpo principal de cuatro `PERFORM` que se lee en voz alta, y el
trabajo repartido en párrafos con nombre y número.

```cobol
PERFORM 1000-INICIO.
PERFORM 2000-PROCESO UNTIL SE-ACABO.
PERFORM 3000-CIERRE.
STOP RUN.
```

**Ésa es la razón por la que COBOL se lee: no es el inglés, son los párrafos.**

**9 — `comision.cob`.** Lo que un **banco** tiene que decidir, que no es lo
mismo que lo que un compilador tiene que saber.

- `EVALUATE TRUE` es la **tabla de decisión**: un escalado de comisiones donde
  cada rama es una condición entera y la primera que acierta gana.
- Y **el redondeo es una decisión legal**. El programa suma cuatro empates por
  los dos modos: el clásico da `0.10` —dos céntimos de la nada— y el del
  banquero da `0.08`, que cuadra con la suma exacta. Cuatro empates no son nada;
  cuatro millones son dinero.

**10 — `maestro.cob`.** El **fichero de un banco en su formato de verdad**:
registros de largo fijo, campos en su byte, importes empaquetados, sin
separador. Escribe tres cuentas y las vuelve a leer.

Y trae las dos herramientas que van con eso:

```bash
bmo-cobol --copybook maestro.cob        # el byte exacto de cada campo
bmo-cobol --ver datos/ctas.bin maestro.cob   # el fichero DECODIFICADO
```

★ La máscara del informe lleva `CR` **a propósito**: con `$$$,$$9.99` a secas,
un saldo de `-890,10` se imprime como `$890.10` y el extracto dice que la cuenta
está en verde. Un campo editado sin símbolo de signo **no enseña el signo**.

## El escalón que todavía no existe

Esta lista eran **nueve** cosas que el compilador rechazaba con su motivo. El
2026-08-03 entraron cinco, y ninguna trajo carpeta propia: se metieron en los
escalones que ya había, que es lo correcto — un nivel nuevo se gana cuando hace
falta **una forma distinta de escribir**, no cuando entra un verbo más.

| | Estado |
|---|---|
| `GO TO` dentro de un párrafo | ✅ y el nivel 8 dejó de fingirlo |
| `PIC X(n)` con texto de verdad | ✅ sin límite de ancho |
| `STRING` | ✅ — **`UNSTRING` no** |
| `INSPECT` | ✅ |
| `FILE STATUS` | ✅ con los códigos que se pueden dar de verdad |
| `SEARCH` / `SEARCH ALL` | ❌ pendiente |
| `REDEFINES` | ❌ pendiente |
| `SORT` | ❌ pendiente |
| `CALL` | ⛔ **bloqueado por la decisión del enlazador** — no es COBOL, es que BMO no tiene enlazado |

Lo que queda entra igual que entró lo de arriba: con su fila en
`cobol_feature_matrix_runs_correctly`, que **ejecuta** el programa en vez de
mirarle los bytes.

El orden, y qué bloquea a qué, está en [`../PLAN_BANCA.md`](../PLAN_BANCA.md).

## Los `.bex`

**No se guardan en el repositorio.** Los genera el build a `staging\BMO-DATA\`,
en la carpeta de su nivel:

```
Ultra_kernel_x86-64\build.ps1 -Flash -Drive A -Data A
```

Había una carpeta `bex/` con cuatro binarios commiteados; se borró el
2026-08-03. Eran del 30 de julio, o sea **anteriores a COMP-3, a los párrafos, a
`EVALUATE` y a `ROUNDED`** — y un binario viejo al lado de su fuente nueva es
una trampa: el que lo flashea ve el comportamiento de antes y busca el fallo
donde no está.

Para compilar uno suelto a mano:

```bash
cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/<nivel>/<x>.cob -o <destino>.bex
```

Ojo con el nombre de salida: el volumen es FAT32 y el kernel **rechaza** un
nombre de más de ocho letras. Por eso `conceptos.cob` se compila a `concep.bex`.
El compilador ya comprueba esa regla en las rutas del `ASSIGN TO`, y el build la
comprueba en el nombre del `.bex`.

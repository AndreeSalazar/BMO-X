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

| Nivel | Carpeta | En el disco | Qué hace falta que el compilador sepa |
|---|---|---|---|
| 1 | `1-basico/` | `cobol/1/hola.bex` | `DISPLAY` de literal, `STOP RUN` |
| 2 | `2-decimal/` | `cobol/2/banco.bex` `calc.bex` `calcgui.bex` | `PIC` con escala, `MOVE`, las cinco operaciones, `IF`/`ELSE`, `PERFORM`, `COMPUTE` con precedencia, `ACCEPT` |
| 3 | `3-presentacion/` | `cobol/3/extracto.bex` | `PICTURE` de **edición** emitida como instrucciones |
| 4 | `4-ficheros/` | `cobol/4/batch.bex` | `SELECT`/`ASSIGN`, `FD`, `OPEN`/`READ … AT END`/`WRITE`/`CLOSE` |
| 5 | `5-tablas/` | `cobol/5/concep.bex` | `OCCURS` con subíndice literal y variable, con guarda de rango |
| 6 | `6-condiciones/` | `cobol/6/carter.bex` | Nivel **88**: nombres de condición |
| 7 | `7-empaquetado/` | `cobol/7/cuentas.bex` | `USAGE COMP-3`: el dato guardado en **nibbles**, del ancho de su PIC |
| 8 | `8-parrafos/` | `cobol/8/cierre.bex` | **Párrafos** y las cuatro formas del `PERFORM` fuera de línea, `VALUE`, `OR` |
| 9 | `9-decision/` | `cobol/9/comisio.bex` | `EVALUATE TRUE` y **`ROUNDED` con sus modos** |
| 10 | `10-binario/` | `cobol/10/maestro.bex` | **Registros binarios de largo fijo** con los campos en su byte |

Los diez corren en el emulador y tienen su test. Del 1 al 6, además,
**verificados en el Ryzen**.

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

Un "nivel 11" pediría lo que hoy se rechaza **con su motivo**, no en silencio:
`STRING`, `INSPECT`, `SEARCH`, `CALL`, `SORT`, `GO TO`, `REDEFINES`, `PIC X` con
texto de verdad, y `FILE STATUS`. Cuando uno de ésos entre, entra con su carpeta
y con su fila en `cobol_feature_matrix_runs_correctly`.

El orden en que entran, y qué bloquea a qué, está en
[`../PLAN_BANCA.md`](../PLAN_BANCA.md).

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

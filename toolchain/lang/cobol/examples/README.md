# Los ejemplos, por nivel

No están ordenados por tema sino por **cuánto COBOL hace falta que el
compilador sepa** para que corran. Subir un escalón es una característica nueva
en el codegen, no un ejemplo más largo.

Sirve para dos cosas: ver de un vistazo hasta dónde llega BMO COBOL hoy, y
tener un orden por el que romper cosas cuando algo se rompa — si falla el 4,
comprueba primero que el 1 sigue vivo.

| Nivel | Carpeta | Qué hace falta que el compilador sepa | Se ejecuta en |
|---|---|---|---|
| 1 | `1-basico/` | `DISPLAY` de literal, `STOP RUN` | emulador + Ryzen |
| 2 | `2-decimal/` | `PIC` con escala, `MOVE`, las cinco operaciones, `IF`/`ELSE`, `PERFORM`, `COMPUTE` con precedencia, `ACCEPT` | emulador + Ryzen |
| 3 | `3-presentacion/` | `PICTURE` de **edición** emitida como instrucciones | emulador + Ryzen |
| 4 | `4-ficheros/` | `SELECT`/`ASSIGN`, `FD`, `OPEN`/`READ … AT END`/`WRITE`/`CLOSE` | emulador + Ryzen |
| 5 | `5-tablas/` | `OCCURS` con subíndice literal y variable, con guarda de rango | emulador + Ryzen |
| 6 | `6-condiciones/` | Nivel **88**: nombres de condición | emulador |
| 7 | `7-empaquetado/` | `USAGE COMP-3`: el dato guardado en **nibbles**, del ancho que dice su PIC | emulador |
| 8 | `8-parrafos/` | **Párrafos** y las cuatro formas del `PERFORM` fuera de línea, `VALUE`, `OR` | emulador |

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
SE-ACABO` en vez de `UNTIL FIN = 1`, `IF NO-HUBO-NADA` en vez de `IF CUANTOS =
0`. Un 88 **no reserva ni un byte** — hay un test que lo comprueba comparando
el tamaño del código con y sin ellos. Le pone nombre a una comparación, y ése
es todo su trabajo: que quien audite el programa no tenga que acordarse de qué
significaba el 1.

**7 — `cuentas.cob`.** El primer escalón que cambia **cómo se guarda** el dato y
no qué se hace con él. Un `COMP-3` vive en nibbles —dos dígitos por byte, el
signo en el último— y ocupa **lo que dice su PICTURE**, así que lo que no cabe
se pierde por arriba, como manda el estándar.

Y por eso el programa imprime el mismo `12345` dos veces: en un `PIC 9(3)
COMP-3` sale `345` y en un `PIC 9(3)` a secas sale `12345`. Ésa es la línea que
**no se puede fingir** — el día que las dos salgan iguales, el `COMP-3` volvió a
ser un entero con otro nombre. Los bytes exactos se prueban aparte, en
`bmo_lower::packed`.

Lo que este escalón todavía no trae: el fichero sigue siendo **texto**. Leer
bytes empaquetados tal cual vienen de un mainframe pide un registro binario con
varios campos, y eso es el escalón siguiente.

**8 — `cierre.cob`.** El primer ejemplo escrito **como se escribe COBOL de
verdad**: un cuerpo principal de cuatro `PERFORM` que se lee en voz alta, y el
trabajo repartido en párrafos con nombre y número.

```cobol
PERFORM 1000-INICIO.
PERFORM 2000-PROCESO UNTIL SE-ACABO.
PERFORM 3000-CIERRE.
STOP RUN.
```

Eso de arriba es el programa entero. **Esa es la razón por la que COBOL se lee:
no es el inglés, son los párrafos** — quien audite el cierre puede mirar sólo el
paso que le interesa.

Lo que hace falta que el compilador sepa, y que hasta el 2026-08-03 no sabía:
nombres de párrafo, `PERFORM` que llama **y vuelve**, `PERFORM … THRU` sobre un
rango de tres, `PERFORM … UNTIL` con un `88`, `VALUE` que inicializa, `OR` en
las condiciones, y un `STOP RUN` que de verdad termina.

★ El descarte de un movimiento se hace con un **interruptor y no con un
`GO TO`**, porque `GO TO` todavía no existe. Está dicho dentro del ejemplo:
fingirlo con un `PERFORM` del párrafo de salida sería mentir, porque un `PERFORM`
lo ejecuta y **vuelve**.

## El escalón que todavía no existe

Un "nivel 9" pediría lo que hoy se rechaza **con su motivo**, no en silencio:
`EVALUATE`, `PERFORM VARYING`, `STRING`, `INSPECT`, `SEARCH`, `CALL`, `SORT`,
`GO TO`, y los records anidados con campos en posiciones fijas. Cuando uno de
ésos entre, entra con su carpeta y con su fila en
`cobol_feature_matrix_runs_correctly`.

El orden en que entran, y qué bloquea a qué, está en
[`../PLAN_BANCA.md`](../PLAN_BANCA.md).

## Los `.bex`

`bex/` son binarios **generados**, no fuentes. Se regeneran así, y **hay que
regenerarlos y volver a commitearlos cada vez que se toca el codegen** — los
tests fijan la salida exacta que verá el kernel:

```
cargo run -p bmo-cobol-front -- toolchain/lang/cobol/examples/<nivel>/<x>.cob -o <destino>.bex
```

Ojo con el nombre de salida: el volumen es FAT32 y el kernel **rechaza** un
nombre de más de ocho letras. Por eso `conceptos.cob` se compila a
`concep.bex`. El compilador ya comprueba esa regla en las rutas del `ASSIGN TO`,
pero el nombre del `.bex` lo eliges tú.

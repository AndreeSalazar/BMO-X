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

## El escalón que todavía no existe

Un "nivel 7" pediría lo que hoy se rechaza **con su motivo**, no en silencio:
`EVALUATE`, `PERFORM VARYING`, `STRING`, `INSPECT`, `SEARCH`, `CALL`, `SORT`,
`COMP-3` real, los records anidados — y el `OR` en las condiciones, que es lo
que hoy impide un `88` con `THRU` o con varios valores. Cuando uno de ésos
entre, entra con su carpeta y con su fila en
`cobol_feature_matrix_runs_correctly`.

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

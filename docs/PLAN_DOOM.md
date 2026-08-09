# El plan largo: de "BMO C compila 69 de 81" a "DOOM se juega en el Ryzen"

> Escrito el **2026-08-08**, el dia que la sonda paso de 0 a 69 ficheros sueltos
> y el unity build empezo a parsear las 56.465 lineas enteras.
>
> `docs/QUE_DESBLOQUEA.md` dice **que falta y por que**. Este dice **en que
> orden, que bloquea a que, y como se sabe que una casilla esta hecha.** Es el
> mismo trato que `toolchain/lang/cobol/PLAN_BANCA.md` tiene con la banca.
>
> Esta hecho para avanzar **poco a poco**: cada casilla se puede entregar sola,
> con su prueba, sin dejar el compilador roto entre medias.

## Como se lee esto

```
[ ]  pendiente        [~]  a medias, y se dice cuanto        [x]  hecho, con fecha
★    la pieza que decide su fase
⛔   BLOQUEADO, y por QUE -- comprobado en el codigo, no supuesto
⚠    tiene una decision dentro que hay que tomar antes de escribir codigo
```

Tamano: **S** una sesion - **M** dos o tres - **L** una semana de verdad -
**XL** la pieza grande de su fase.

## La regla que no se negocia

**Nada entra sin su fila en el banco de pruebas, y la fila EJECUTA el programa.**
Hoy son 311 en `bmo-c-front`. Las dos cosas que este proyecto encontro el 08-08
--`p->x++` que no hacia nada y `*p` que leia ocho bytes-- **no dan error**: dan
un numero. Solo se ven ejecutando.

---

# FASE 0 -- Donde estamos, medido

No es una fase de trabajo: es la linea de salida, para que las de abajo se lean
contra algo.

```
   ficheros sueltos:  0 -> 7 -> 27 -> 35 -> 41 -> 47 -> 55 -> 61 -> 67 -> 69
   unity build:       PARSEA LAS 56.465 LINEAS y esta dentro del generador
```

| | |
|---|---|
| Lenguaje | 32/32 sondas, y siete tandas de arreglos el 08-08 |
| Relocations | las **tres** caras: cadena, funcion y global |
| Ficheros | `fopen`/`fread`/`fseek`/`fclose`, `ARCH_OP_LEER_EN` |
| Memoria | `KIND_MEMORIA`, y DOOM pide **un solo bloque** de 6 MiB |
| Pantalla | framebuffer con doble bufer, ya probado con `ray.bex` |
| Entrada | teclado USB con su ESC (que hasta el 08-08 **no existia**) |
| Tiempo | `INFO_TICKS` y el TSC medido |

★ Y los dos hallazgos que cambian la estimacion, comprobados en el codigo de
DOOM y no supuestos:

- **El renderer no necesita coma flotante.** El unico `atan()` esta dentro de un
  `#if 0` que dice *"UNUSED - now getting from tables.c"*.
- **El tope de 4 `malloc` por proceso no lo bloquea.** `I_ZoneBase` pide UN
  bloque y `Z_Malloc` reparte desde dentro.

---

# FASE 1 -- Que el unity build llegue al final

Es la fase que decide todo lo demas: mientras no salga un `.bex`, las fases de
abajo no se pueden ni empezar a probar.

| # | Casilla | Tam | Estado |
|---|---|---|---|
| 1.0 | ★ **`printf` con formato en tiempo de ejecucion** | XL | ⛔ es lo que para el unity HOY |
| 1.1 | `sprintf` / `snprintf` sobre el mismo formateador | M | ⛔ por 1.0 |
| 1.2 | `fprintf` -- 64 llamadas en DOOM | S | ⛔ por 1.0 |
| 1.3 | Las ~20 triviales: `toupper` `isspace` `atoi` `strncpy` `strrchr` `strstr` `strdup` `memmove` `strcasecmp` `ftell` `feof` `fwrite` | M | libre |
| 1.4 | `system` `mkdir` `getenv` `remove` `rename` -- apuntaladas con su motivo | S | libre |
| 1.5 | Que el `.bex` quepa en **1 MiB** (`MAX_BEX`) | ? | por medir |

## ★ 1.0 -- El formateador en ejecucion, que es la pieza de verdad

Hoy `printf` se emite **en linea desde un literal**: el compilador lee el
formato al compilar y escribe las llamadas. `I_Error(fmt, ...)` y `M_snprintf`
reciben el formato **como argumento**, y ahi no hay literal que leer.

Lo que hace falta es un `__bmo_vprintf(fmt, args)` que recorra la cadena al
vuelo. Las piezas ya estan casi todas:

- los formateadores sintetizados (`__bmo_fmt_i64`, `_u64_hex`, `_cstr`...),
- `__va_arg(i)`, que da el variadico `i` porque BMO pasa los variadicos por la
  pila detras de los nombrados,
- y `console::write_const` para lo literal.

⚠ **La decision que hay que tomar antes de escribir codigo**: donde vive.

| Camino | A favor | En contra |
|---|---|---|
| **A** -- en C, dentro del unity | se escribe una vez, se lee, se prueba con el resto | hay que interceptar `printf` en el codegen para que NO lo emita en linea |
| **B** -- como funcion sintetizada | no toca el codegen de llamadas | el mecanismo de sintetizadas recibe `&mut Vec<u8>` y **no sabe de etiquetas**: un bucle con saltos no cabe hoy |

Hoy A es mas barato, y B pide primero ensanchar la tabla de sintetizadas para
que acepte emisores con etiquetas -- que es un cambio de la TABLA, no de las
funciones. Ver la cabecera de `codegen/sintetizadas.rs`.

**Como se sabe que 1.0 esta hecha**: `printf(fmt, 42)` con `fmt` en una variable
imprime `42`, y la fila lo EJECUTA.

---

# FASE 2 -- La capa de plataforma: seis funciones

DOOM (doomgeneric) habla con el sistema por **seis funciones**, y las seis ya
tienen con que hacerse. Es `doomgeneric_bmo.c`, y es el fichero que hay que
escribir de cero.

| # | Casilla | Con que se hace | Tam |
|---|---|---|---|
| 2.0 | `DG_Init` | reclamar pantalla + entrada | S |
| 2.1 | ★ `DG_DrawFrame` | `memcpy` del bufer de DOOM al framebuffer | S |
| 2.2 | `DG_GetTicksMs` | `INFO_TICKS` | S |
| 2.3 | `DG_SleepMs` | ceder el turno hasta el tick | S |
| 2.4 | `DG_GetKey` | `INPUT_OP_TECLA` + tabla a `doomkeys.h` | M |
| 2.5 | `DG_SetWindowTitle` | una linea en la barra, o nada | S |

⚠ **2.1 tiene la unica decision de la fase**: DOOM pinta en **paleta de 8 bits**
y el framebuffer es de 32. Hay que expandir cada pixel por su paleta, y eso son
320x200 = 64.000 lookups por fotograma. `ray.bex` ya escribe pixeles a esa
velocidad, asi que se espera que sobre -- **pero se mide, no se supone.**

**Como se sabe que la fase esta hecha**: sale el menu de DOOM en el Ryzen, con
foto.

---

# FASE 3 -- El WAD, que es donde vive el juego

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 3.0 | Leer `doom1.wad` (4.196.020 B) con la cadena de ficheros | S | ya existe `ARCH_OP_LEER_EN` |
| 3.1 | ⚠ Que quepa: el WAD son 4 MiB y `MAX_BEX` es 1 MiB | M | son cosas distintas -- el WAD NO es la imagen |
| 3.2 | `W_CacheLumpName` sobre el zone allocator | S | es codigo de DOOM, no de BMO |

★ **3.1 es la que hay que mirar antes**: el WAD se lee a la memoria que pidio
`I_ZoneBase`, no al bufer de imagenes. Pero el camino de lectura de hoy copia
por un bufer de rebote del kernel, y 4 MiB por ahi son muchas vueltas. Si duele,
lo que lo arregla es DMA directo al bufer del llamante, que ya esta en la hoja
de ruta como palanca de arquitectura.

---

# FASE 4 -- Que se pueda jugar de verdad

| # | Casilla | Tam |
|---|---|---|
| 4.0 | Guardar partida (`fwrite` + modo I-O) | M |
| 4.1 | El menu y los cheats (dependen de `M_snprintf`, o sea de 1.0) | S |
| 4.2 | Medir fotogramas por segundo y decirlo | S |

---

# FASE 5 -- SONIDO

> **Estado real, comprobado**: `platform/drivers/audio/` existe y son **109
> lineas de altavoz de PC** -- `outb` a un puerto y un retardo por TSC. Sirve
> para un pitido, no para DOOM. Y **no lo llama nadie**: es uno de los crates
> huerfanos de la auditoria de deuda tecnica.

O sea que el audio de verdad **empieza de cero**, y por eso va al final: DOOM se
juega entero sin sonido, y ninguna de las fases de arriba lo necesita.

| # | Casilla | Tam | Nota |
|---|---|---|---|
| 5.0 | Decidir el aparato | M | ⚠ HD Audio (Intel/AMD moderno) o AC'97. En este Ryzen es **HDA**. Es la decision de la fase |
| 5.1 | Enumerar el codec y abrir un stream de salida | XL | es un driver entero, con DMA y su anillo de buffers |
| 5.2 | `KIND_AUDIO` como capability | M | un proceso que no la tiene **no hace ruido**, igual que la pantalla |
| 5.3 | Mezclar los canales de DOOM (`i_sound.c`) | L | DOOM mezcla el mismo, solo pide un buffer |
| 5.4 | Musica MUS -> MIDI (`mus2mid.c` ya compila) | XL | y sin sintetizador MIDI no suena: es OTRO proyecto |

★ **La linea honesta**: 5.0 a 5.3 son "DOOM con efectos". 5.4 es "DOOM con
musica", y eso pide un sintetizador. **Se paran en 5.3 y se dice.**

---

# La cuenta, para poder repartir

| Fase | Casillas | Faltan | Bloquea a |
|---|---|---|---|
| 1 -- el unity termina | 6 | 6, y una es XL | todo |
| 2 -- la plataforma | 6 | 6, todas S/M | la foto del menu |
| 3 -- el WAD | 3 | 3 | jugar |
| 4 -- jugable | 3 | 3 | -- |
| 5 -- sonido | 5 | 5, y **empieza de cero** | nada |

**Lo unico que bloquea a todo lo demas es 1.0.** Es una pieza XL y es la unica:
en cuanto exista, la fase 2 son seis funciones cortas y la 3 es codigo de DOOM
llamando a lo que BMO ya tiene.

---

Ver [`QUE_DESBLOQUEA.md`](QUE_DESBLOQUEA.md) para el censo, `AVANCES.md` para el
estado y `BMO-externo/doom-port/` (fuera del repo) para la sonda y el unity.

# INTI -- el lenguaje de BMO-X

> **INTI** -- el sol en quechua. Extension `.inti`.
>
> 🟡 **Estado: F0 -- CONTRATO ESCRITO, CERO CODIGO.** No hay lexer, ni parser,
> ni runtime. Lo que hay es lo que se decidio, escrito de forma que se pueda
> discutir y medir despues.

## Que es, en una frase

**INTI es a BMO-X lo que C fue a Unix**: el lenguaje en el que se escribe el
sistema. No es el quinto frontend del toolchain -- C, COBOL, C++ y Ada estan
ahi porque **son de otros**, y son compatibilidad con el mundo. INTI es el
primero que no le debe nada a nadie.

Tres cosas lo definen, y ninguna es negociable:

| | |
|---|---|
| **Sin comportamiento indefinido** | toda construccion tiene un resultado dicho por escrito; donde no hay resultado sensato, **atrapa**. Comprobar cuesta ~1%; parchear C a posteriori, 29,1% |
| **Facil, con evidencia y no con gusto** | sangria, palabras en vez de simbolos, y **el mensaje de error como interfaz principal** (el 73% de los envios de codigo llevan errores de sintaxis) |
| **Dos perfiles, un lenguaje** | `llano` escribe drivers; `pleno` escribe aplicaciones. Misma gramatica, mismo compilador |

## Los ficheros

| fichero | que es |
|---|---|
| [`GRAMATICA.md`](GRAMATICA.md) | **la sintaxis**, fijada: las diez decisiones visibles, cada construccion con su ejemplo, y la EBNF completa |
| [`REGLAS.md`](REGLAS.md) | **las doce reglas** que sustituyen a los 203 comportamientos indefinidos de C, con su codigo de error |
| [`CENSO.md`](CENSO.md) | **38 sondas** con el veredicto escrito por delante, para que el desacuerdo se vea solo |
| [`palabras.toml`](../../forge/sem-asm/tables/lang/inti/palabras.toml) | las **49 palabras clave**, en una tabla y con la columna inglesa ya escrita |
| `censo/*.inti` | las sondas. Cada una lleva su expectativa en la primera linea |

El **porque** de todo esto -- la investigacion, los numeros y las alternativas
descartadas -- esta en [`docs/maestro/INTI_MAESTRO.md`](../../../docs/maestro/INTI_MAESTRO.md).

## Los dos perfiles, que es lo que hace que esto no sea otro lenguaje de scripts

```text
   INTI LLANO   sin monton, sin contador de referencias, sin recoleccion.
                Tamanos exactos, todo en la pila o estatico, y `crudo` para
                tocar puertos. Corre a la velocidad de C.

   INTI PLENO   lo de arriba mas texto, listas, tablas, `numero` decimal
                exacto, contador de referencias, congelado y tareas.
```

Un solo lenguaje y una sola gramatica: **lo unico que cambia es que biblioteca
de base admite cada perfil, y el compilador lo comprueba**. En `llano`, usar
algo que asigna memoria es un **error de compilacion con nombre y sitio**, no
una sorpresa en ejecucion.

★ La frontera no es nueva: `bmo-rt` ya reparte asi (`crt0`, `syscall`, `string`
de un lado; `heap/` del otro) y REX ya separa `monton.h` del resto. F0 solo la
nombra.

## Como se ve

```text
perfil pleno

registro Alumno
   nombre es texto
   nota   es numero

funcion media(alumnos es lista de Alumno) devuelve numero
   si esta vacia alumnos
      devuelve 0
   cambiante suma = 0
   para cada a en alumnos
      suma = suma + a.nota           # si se pasa de la cuenta, ATRAPA
   devuelve suma / cuenta de alumnos

funcion principal
   notas = [Alumno("ana", 8.1), Alumno("luis", 4.2)]
   escribe "media:", media(notas)    # 6.15 exacto, no 6.149999999999999

   guarda "media.txt", texto(media(notas)) o si no
      escribe "no pude guardar:", motivo
```

y el mismo lenguaje escribiendo sistema:

```text
perfil llano

funcion lee_tecla devuelve natural8
   repite mientras (entrada_puerto(0x64) bits_y 1) = 0
      espera()

   crudo
      devuelve entrada_puerto(0x60)
```

## La puerta no es sintaxis

Ni una palabra clave de INTI habla de `INVOKE`, `WAIT` o capabilities -- y aun
asi se tiene control absoluto sobre ellas. Tres escalones, y **los tres son
tablas**, no gramatica:

```text
   usa superficie / archivo / entrada    REX: lo normal      guarda "x", t
   usa bmo                               la puerta           invoca(cap, op, ...)
   usa metal                             los intrinsecos     entrada_puerto(0x60)
```

★ **`invoca` NO necesita `crudo`; un puerto SI.** Al otro lado de una capability
hay un kernel que comprueba; al otro lado de un `outb` no hay nadie. `crudo` no
marca *"bajo nivel"*: marca **"aqui nadie comprueba por ti"**.

Motivo de fondo: **`read()` nunca fue palabra clave de C**, y por eso Unix pudo
saltar al Interdata. Un lenguaje que reserva los syscalls de su sistema queda
casado con el. Detalle entero en [`GRAMATICA.md`](GRAMATICA.md) sec. 17.

## Lo siguiente

| fase | entregable | estado |
|---|---|---|
| **F0** | gramatica, doce reglas, censo | ✅ **escrito** (2026-08-19) |
| F1 | lexer + parser -> AST, con los mensajes de 4 partes | pendiente |
| F2 | ★ INTI LLANO compilando a `.bex` nativo, por `bmo-verify` | pendiente |
| F3 | las doce reglas con sus sondas en verde | pendiente |
| F4 | ★★ la foto del Ryzen | pendiente |
| F5-F7 | PLENO, congelado y tareas, y el REPL | pendiente |

⚠ **El REPL va el ultimo a proposito.** Un interprete no puede escribir un
driver, y lo primero que INTI tiene que demostrar es que es un lenguaje de
sistema. El `>>>` es la comodidad, no la esencia.

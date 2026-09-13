# INTI -- el lenguaje de BMO-X

![INTI](../../../docs/arte/inti.png)

> **INTI** -- el sol en quechua. Extension `.inti`, y sus binarios `.ibex`.
>
> 🟢 **Corre en el Ryzen** (22-08: `cpu.inti` con las reglas atrapando en
> silicio; 12-09: `pulso.inti` leyendo 4,52 GHz y 58 W en tiempo real).
> **560 pruebas** en `bmo-inti-front` (276) + `bmo-inti-x86-64` (284), medidas
> por `.\bmo.ps1` el 2026-09-12. El estado de las tres
> frases que lo definen, con lo que falta, esta en [`ESTADO.md`](ESTADO.md).

## ⭐ 2026-09-12 -- la primera HERRAMIENTA, y tres fallos del emisor que llevaban ahi desde siempre

**`ejemplos/bico.inti`** convierte `datos/foto.bmp` y `datos/foto.qoi` a
**BICO**, el formato que el escritorio pinta. Es el sitio donde INTI le gana a
C: leer datos que escribio OTRO. Los fallos famosos de libpng/libjpeg son
medidas mentirosas que desbordan un bufer; aqui toda medida del fichero se
compara con lo que llego, la aritmetica atrapa en vez de dar la vuelta, y los
unicos dos bloques `crudo` estan contados. `tests/bico.rs` le da ficheros
buenos **y rotos** (cortados, con un alto de 60.000, comprimidos): un codigo, y
ningun fichero de salida.

Y escribir programas de verdad destapo tres fallos del emisor, **los tres
mudos** --compilaban, corrian, y hacian otra cosa--:

| fallo | desde | lo destapo | prueba |
|---|---|---|---|
| `y` / `o` no emitian nada: `a y b` devolvia `a` | siempre | `pulso.inti`, un cero en blanco | `pruebas/logica.rs` |
| `espera_a` cruzaba por INVOKE: no dormia, giraba | siempre | `pulso.inti` quemando 1,5 W en el Ryzen | `marco::espera_a_cruza_por_wait...` |
| una VARIABLE hacia `r8`/`r9`/`r10` rompia la instruccion | siempre | `bico.inti`, el emulador en `opcode 0xD0` | `tests/argumentos_altos.rs` |

★ Los tres tienen la forma que este proyecto persigue: **lo que no se emite no
se prueba**. `cpu.inti` corrio en metal sin tocar ninguno de los tres caminos.

| programa | que hace | `run` |
|---|---|---|
| [`sondas/cpu.inti`](sondas/cpu.inti) | lo que el CPU le cuenta a Ring 3, y las reglas atrapando | `inti/cpu.ibex` |
| [`sondas/pulso.inti`](sondas/pulso.inti) | ⭐ el perfil del kernel en tiempo real (MHz, mW, obreros) y **el NO del kernel con su motivo** | `inti/pulso.ibex` |
| [`ejemplos/bico.inti`](ejemplos/bico.inti) | ⭐ BMP/QOI -> BICO, contra ficheros rotos | `inti/bico.ibex` |
| [`ejemplos/png.inti`](ejemplos/png.inti) | escribe un PNG valido | -- |

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
| [`comun.toml`](../../forge/sem-asm/tables/lang/inti/comun.toml) | ★ **la biblioteca que esta sin pedirla** -- donde vive la facilidad |
| [`modulos.toml`](../../forge/sem-asm/tables/lang/inti/modulos.toml) | lo que trae cada `usa <modulo>` de REX, incluido ★ `usa binarios` |
| [`arch/x86_64/inti.toml`](../../forge/sem-asm/tables/arch/x86_64/inti.toml) | ★★ **la maquina entera**: nombres, los 16 registros con su rol, el reparto y el perfil |
| `censo/*.inti` | las sondas. Cada una lleva su expectativa en la primera linea |
| [`LINAJE.md`](LINAJE.md) | ★★ **abuelo, padre, hijo, nieto**: que aguanta cada pieza sola, y como se cambia de golpe. Con `tests/linaje.rs`, que lo vigila |
| [`ARQUITECTURA.md`](ARQUITECTURA.md) | **por que el compilador esta partido asi** -- el criterio de corte, la regla de dependencias, y por que la modularidad es lo que mantiene el syscall fuera del lenguaje |
| `src/` | el frontend, **agnostico**: `aviso`, `palabras`, `lexico`, `arbol`, `sintaxis`, `perfil`, `nombres`, `arquitectura`, `ir` |
| [`emisor-x86_64/`](emisor-x86_64/) | ★ **el emisor**, en su propio crate: es el unico que puede nombrar una maquina |

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
   crudo
      repite mientras (entrada_puerto(0x64) bits_y 1) = 0
         espera()

      devuelve entrada_puerto(0x60)
```

## Modo Python: una columna, no una libreria

```python
profile full

def saludar(nombre):
    return "Hola " + nombre
```

Eso **se lee tal cual** cambiando `idioma_por_defecto` a `"py"` en
[`palabras.toml`](../../forge/sem-asm/tables/lang/inti/palabras.toml). Cero
lineas de compilador -- porque las palabras clave estuvieron en una tabla desde
el primer dia.

★★ Y cambia las **palabras**, no las **reglas**: `0.1 + 0.2` sigue dando `0.3`,
desbordar sigue atrapando, `var` sigue haciendo falta. **La sintaxis de Python
sin las quince sorpresas.**

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

## Como se comprueba

```text
   cargo test -p bmo-inti-front
```

170 pruebas. Las que se ganan el sitio: que **el mensaje de error no tiene jerga
de compilador** (hay lista negra), que **el dedo cae en la columna exacta**
contando caracteres y no bytes, que **el mismo lexer lee ingles** sin cambiar
una linea, que una palabra clave **con tilde sigue siendo palabra clave**, y que
**ninguna de las 38 sondas lleva un fallo de escritura escondido**, y que **las
que dicen COMPILA se leen enteras**.

★ Ese ultimo grupo se gano el sitio cuatro veces el primer dia. El corpus y la
gramatica no estaban de acuerdo en: la sangria (tres espacios contra cuatro),
las llamadas sin parentesis (`escribe x`), formas que no existian
(`anade X a Y`), y una funcion anonima en una sonda de un lenguaje que no
tiene funciones anonimas. **Nada de eso se habria visto leyendo.**

## Que falta para poder afirmar lo que se afirma

[`PLAN_DE_PRUEBAS.md`](PLAN_DE_PRUEBAS.md) -- los siete peldanos, en orden de
DEPENDENCIA y no de importancia, cada uno con un criterio de aprobado que se
puede ejecutar.

Y [`sondas/LEEME.md`](sondas/LEEME.md) -- **lo que solo el metal puede
contestar**: los programas que hay que llevar al Ryzen porque el emulador no
puede dar su respuesta.

⚠ El numero incomodo, al dia (2026-09-12): INTI **ya corre en el Ryzen**
(`cpu.inti`, `pulso.inti`), pero **`bico.inti` no ha corrido todavia en metal**
-- solo en el emulador. Y los tres fallos de arriba dicen lo que vale un
emulador verde: nada de lo que un programa no ejercita.

---

## Lo siguiente

| fase | entregable | estado |
|---|---|---|
| **F0** | gramatica, doce reglas, censo | ✅ **escrito** (2026-08-19) |
| **F1a** | **lexico**: palabras, textos, numeros, sangria, parejas, y los avisos de 4 partes | ✅ **verde** (2026-08-19) |
| **F1b** | **arbol + sintaxis**: precedencia de 10 niveles, declaraciones, sentencias, recuperacion de errores | ✅ **92 pruebas en verde** (2026-08-19) |
| **F2a** | **el analisis de perfiles**: `llano` contra `pleno`, `crudo` contado | ✅ **verde** (2026-08-19) |
| **F2b** | **nombres, `cambiante` y la biblioteca comun**: quisiste-decir, alcance por funcion, y la **llamada sin parentesis** | ✅ **142 pruebas en verde** (2026-08-19) |
| **F2c** | **la IR**: instrucciones con temporales, y las comprobaciones anti-UB **hechas instruccion** | ✅ **170 pruebas en verde** (2026-08-19) |
| **F2d** | ★★ **el emisor**: INTI LLANO a bytes que CORREN, y el `.bex` pasa el gate | ✅ **184 pruebas en verde** (2026-08-19) |
| **linaje** | la jerarquia de piezas, **con test que la hace cumplir** | ✅ verde |
| **CABINA** | ★★ INTI **le cuenta al sistema** que fallo Y lo que sabe, en la capa `Lang` | ✅ **202 pruebas en verde** (2026-08-19) |
| **F3** | ★★★ **los temporales viven en REGISTROS** -- recorrido lineal, y solo cambio `marco.rs` | ✅ **206 pruebas en verde** (2026-08-19) |
| F3b | ★★ **las llamadas**: una funcion de INTI llama a otra, incluso declarada mas abajo | ✅ verde (2026-08-19) |
| **F4a** | ★★ **el arranque**: un programa empieza y termina solo, y sale por la puerta | ✅ verde (2026-08-20) |
| **F4b** | **la memoria**: la puerta se abrio en F4a y al otro lado no habia manos | ✅ verde (2026-08-20) |
| **F4c** | **el monton**, en piezas y escrito en el propio INTI | ✅ verde (2026-08-20) |
| **F5a** | los **cuatro anchos**, y el de 32 es el que cabe un pixel | ✅ verde (2026-08-20) |
| **F5b** | ★★ **la disposicion**: `p.x` y `a[i]` dejan de mentir. Nace `medidas.toml` | ✅ verde (2026-08-20) |
| **F5c** | ★★ **la coma flotante**: las cuatro operaciones, las seis comparaciones con el NaN correcto, y la conversion | ✅ verde (2026-08-21) |
| **F5d** | ★★★ **el metal**: `usa x86_64` y `usa binarios` dejan de ser una tabla que nadie emite | ✅ verde (2026-08-21) |
| **sonda 32** | la tesis de la tabla, ejercitada: otra maquina da otra disposicion **sin tocar Rust** | ✅ verde (2026-08-21) |
| **F5e** | ★★ **las reglas que se contaban y no llegaban a bytes**: la 3 y la 12 atrapan y corren. La 2 espera a `lista de T` | ✅ verde (2026-08-21) |
| **F6a** | ★★★ **LOS TIPOS**: `flotante64 + entero64` deja de compilar. Y una condicion es una pregunta, no "algo que no es cero" | ✅ verde (2026-08-21) |
| **F6b** | ★★★ **LA LINEA DE ORDENES**: `inti fichero.inti` produce un `.bex`. Hasta hoy INTI compilaba **solo dentro de sus pruebas** | ✅ verde (2026-08-21) |
| F6c | ★★ la foto del Ryzen -- **lo unico que el emulador no puede dar**, y ahora ya hay fichero que llevar | ⏳ metal |
| F6 | PLENO: texto, lista, tabla, decimal, contador de referencias | pendiente |
| F7 | congelado y tareas, y el REPL | pendiente |

**Hoy: 1.138 pruebas en verde** en INTI y en todo lo que comparte tabla con el
(BMO C, COBOL, C++, Ada y `bmo-lower`).

⚠ Y el numero incomodo, que esta medido y no estimado: de los **61 nombres** de
la tabla de x86-64, **25 se pueden ejecutar en el emulador y 36 solo en metal**.
No es un fallo del emulador: es su regla -- devolver un cero como si fuera el
valor de un registro de control seria inventarse un dato. Los 36 estan
**escritos con nombre** en `SOLO_EN_METAL`, que es la diferencia entre pendiente
y olvidado.

⚠ **El REPL va el ultimo a proposito.** Un interprete no puede escribir un
driver, y lo primero que INTI tiene que demostrar es que es un lenguaje de
sistema. El `>>>` es la comodidad, no la esencia.

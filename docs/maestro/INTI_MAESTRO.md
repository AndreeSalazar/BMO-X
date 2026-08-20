# INTI MAESTRO -- el lenguaje de BMO-X

> **INTI** -- el sol en quechua. Nombre elegido por Eddi el 2026-08-19.
> Extension `.inti`.

Escrito el **2026-08-19**. Empezo como *"un lenguaje inspirado en Python pero
ULTRA MAS facil, sin GIL, y sin la sorpresa de que todo son objetos"*, y en la
segunda vuelta Eddi lo reencuadro entero:

> *"es como lo hizo C pero para Unix, pero BMO-X tendra su INTI. Basicamente el
> lenguaje de programacion, para portabilidad y facilidad, y no tendra UB. Es
> muy estricto para facilitar, pero te ayuda. No es Rust, pero si inspirado en
> Python."*

**Ese reencuadre cambia el documento entero**, y la seccion 1 explica por que.

---

## 0. Que es INTI, y en que se diferencia de `PYTHON_MAESTRO.md`

Hay tres preguntas parecidas y son tres trabajos distintos. Conviene no
mezclarlas nunca mas:

| | pregunta | quien manda | estado |
|---|---|---|---|
| `PYTHON_MAESTRO.md` | como corre **Python** aqui | CPython. Su semantica es un hecho externo | investigado, aparcado |
| primera vuelta de este | un **Python facil** propio | tu, pero solo para apps | superado por lo de abajo |
| **INTI** | **el lenguaje EN EL QUE SE ESCRIBE BMO-X** | tu, y ademas tiene que poder tocar el metal | **este documento** |

★★★ **La consecuencia:** INTI no es el quinto frontend del toolchain. Es el
lenguaje al que aspiran a llegar los otros cuatro. C, COBOL, C++ y Ada estan
ahi porque **son de otros** -- son compatibilidad con el mundo. INTI es el
primero que no le debe nada a nadie.

Lo que se hereda de `PYTHON_MAESTRO.md` y no se vuelve a discutir:

- El runtime **no entra por `INVOKE`**: la puerta cuesta **969 ciclos** medidos
  contra **20** de una llamada. `2+2` no tiene autoridad que arbitrar.
- **Lo que si es contrato es el FORMATO**: secciones BEF `Tipos`, `Bytecode`,
  `Constantes`, con offsets y no punteros.
- **La cabecera de objeto ya esta hecha** (`bmo_abi::dynobj`: 16 bytes, bit 63
  = INMORTAL, 0x70 ranuras numeradas, 14 tests). Es **representacion**, no
  semantica de Python: sirve tal cual.
- **`bex-link` existe**: un crate de Rust `no_std` se convierte en `.bex`.

⚠ Y **una cosa se invierte**: aquel documento decia *"el REPL fuerza un
interprete, y el AOT va detras"*. **Para INTI el orden es al reves**, y la
seccion 12 dice por que.

---

## 1. QUE FUE C PARA UNIX DE VERDAD -- y que tendria que ser INTI

### 1.1 La cronologia, que casi nadie recuerda bien

| ano | que paso |
|---|---|
| 1969 | Unix v1, **en ensamblador de PDP-7**. C no existe |
| 1970 | Thompson escribe **B** (sin tipos, heredado de BCPL). No basta: no sabe de bytes ni de `struct` |
| 1971-72 | Ritchie le anade **tipos, punteros y `struct`** a B. Sale **C**. Se le anaden justo las cosas que el kernel necesitaba y no cabian |
| **1973** | ★ **Se reescribe el kernel de Unix en C.** ~**9.000 lineas** de C y ensamblador |
| 1977-78 | Johnson y Ritchie **portan Unix al Interdata 8/32**. Es la prueba: el sistema se movio de maquina porque estaba escrito en C |

★★ **Dos cosas de esa tabla hay que grabarlas:**

1. **C no existia antes que Unix: crecio PARA Unix.** B no valia y le anadieron
   exactamente lo que faltaba. **El lenguaje se diseno contra un sistema real
   que ya estaba escrito**, no contra una idea de lenguaje.
2. **Ni siquiera despues de 1973 se escribio todo en C.** El arranque y el
   cambio de contexto siguieron en ensamblador **y siguen hoy**. *El lenguaje
   del sistema nunca escribio el 100% del sistema.* Eso quita presion: INTI no
   tiene que tragarse `entry.rs`.

★★★ **Y el paralelismo que da miedo de lo exacto que es:** aquel kernel eran
~9.000 lineas. La estimacion de la seccion 12 para llevar INTI hasta correr en
el Ryzen es **~9.000 lineas**. No es una casualidad profunda -- es que ese es
el tamano de una pieza de sistema que una persona puede sostener en la cabeza.

### 1.2 Las tres cosas que C le dio a Unix

| lo que dio | por que importo |
|---|---|
| **Portabilidad del SISTEMA** | Unix salto del PDP-11 al Interdata porque estaba en un lenguaje, no en instrucciones |
| **Un lenguaje para las dos capas** | el kernel y las herramientas se escribian igual. No habia dos mundos |
| **Cero runtime debajo** | no hay que arrancar nada antes de que corra el primer programa |

La tercera es la que decide la arquitectura de INTI, y la seccion 1.4 la cobra.

### 1.3 ★ Y lo que C le COBRO a Unix: el comportamiento indefinido

Esta es la parte que la frase *"como lo hizo C pero para Unix"* no debe copiar,
y hay que decirlo antes de disenar nada.

**C consiguio su portabilidad VENDIENDO el comportamiento definido.** En 1978
las maquinas eran genuinamente distintas: complemento a dos, complemento a uno
y signo-magnitud convivian; habia palabras de 36 bits; habia EBCDIC. Para que
el mismo `int` corriera en todas, el estandar **no podia decir que pasa** al
desbordar. Asi que dijo: *indefinido*. Y con eso el compilador de cada maquina
generaba lo que su hardware hacia, sin codigo extra.

**Hoy quedan 203 comportamientos indefinidos catalogados** en el Anexo J.2 de
C11 -- y el propio estandar avisa de que **esa lista no es exhaustiva**.

★★★ **Y aqui esta el hallazgo que justifica todo el capitulo 6:** **la razon
original CADUCO**. C23 hizo **obligatorio el complemento a dos** y prohibio las
otras dos representaciones, porque ya no existe hardware que las use. Dicho por
la gente del estandar: *desde que el complemento a dos se impuso, no definir el
desbordamiento ha sido una cuestion de optimizacion, no de representacion.*

O sea: **el UB nacio para portar y hoy sobrevive para optimizar.** Son dos
motivos distintos, y el segundo se puede medir -- la seccion 6.3 lo mide.

### 1.4 ⚠ LA TENSION, y hay que resolverla antes de escribir una linea

Junta las dos frases de Eddi y chocan:

```text
   "como lo hizo C para Unix"     ->  cero runtime, toca el metal, AOT
   "inspirado en Python, facil"   ->  contador de referencias, texto que crece,
                                      tablas, despacho dinamico
```

**Un manejador de interrupciones no puede llamar a un asignador de memoria.**
Esto no es opinion: es la razon por la que Linux no esta escrito en Java. Si
INTI arrastra un runtime, no puede escribir el sistema; si no lo arrastra, no
es facil como Python.

**La salida no hay que inventarla, ya esta en este arbol dos veces:** Ada de
BMO eligio el perfil **ZFP** (*Zero FootPrint*) en vez de Ravenscar, y C vive
sin `<threads.h>` porque *"resuelve problemas que BMO no tiene"*. **La
respuesta es un PERFIL**, y en INTI son dos:

```text
   INTI LLANO   (perfil de sistema)
      sin monton, sin contador de referencias, sin recoleccion.
      Todo en la pila o estatico, tamanos conocidos en compilacion.
      Puede tocar puertos, MMIO y registros.
      -> es lo que hoy se escribe en C o en Rust no_std.

   INTI PLENO   (perfil de aplicacion)
      lo de arriba MAS texto, listas, tablas, contador de referencias,
      congelado y tareas.
      -> es lo que hoy se escribiria en Python.
```

**Un solo lenguaje, una sola gramatica, un solo compilador.** Lo que cambia es
**que biblioteca de base admite**, y el compilador **lo comprueba**: en LLANO,
usar algo que asigna memoria **es un error de compilacion con nombre y sitio**,
no una sorpresa en ejecucion.

★ Y la prueba de que la linea esta bien puesta: `bmo-rt` ya reparte asi
(`crt0`, `syscall`, `string` de un lado; `heap/` del otro), y REX ya separa
`monton.h` de todo lo demas. **La frontera existe, solo hay que nombrarla.**

---

## 2. ABC: alguien ya intento "Python pero ultra mas facil". Y fracaso.

Va antes que el diseno porque **es literalmente el proyecto que planteaste,
hecho por gente muy buena, y salio mal**.

ABC se diseno en el CWI de Amsterdam (Meertens, Pemberton) como *lenguaje para
no-programadores*: indentacion en vez de llaves, tipos de alto nivel dentro del
lenguaje, sin declaraciones, sin gestion de memoria, entorno interactivo con el
prompt `>>>`. **Guido van Rossum trabajo en su implementacion**, y en las
navidades de 1989 se llevo a Python la indentacion, los tipos de alto nivel y
el `>>>` literalmente.

**Por que murio**, en palabras del propio Guido: *un diseno precioso y un
fracaso total*, por ser **un sistema cerrado**:

| lo que fallo | por que mata |
|---|---|
| **No se podia extender** | si el lenguaje no lo trae, no hay forma de anadirlo |
| **No hablaba con el sistema** | sin ficheros ni procesos no se escriben programas de verdad |
| **Su entorno era obligatorio** | habia que vivir *dentro* de ABC |
| **Monolitico** | sin modulos, nadie de fuera podia aportar una pieza |

★★★ **LA LECCION NUMERO 1:** *facil* no fue suficiente. **ABC era mas facil que
Python y perdio.** Lo que gano no fue la facilidad, fue **poder salir del
lenguaje**: modulos en C, acceso al sistema, cooperar con Unix.

Y por eso el reencuadre de Eddi -- *"como C para Unix"* -- **es la correccion
correcta al documento anterior**: un lenguaje que puede escribir su propio
sistema es, por definicion, un lenguaje del que se puede salir. **INTI LLANO no
es un lujo de rendimiento: es la vacuna contra ABC.**

**Regla no negociable, que se aplica linea por linea del diseno:** *ninguna
simplificacion puede costar la capacidad de salir del lenguaje.*

---

## 3. LA ANATOMIA DE PYTHON: siete decisiones, con su precio

Ninguna es tonta; todas compran algo. Lo que hay que ver es el precio.

| # | decision | que te da | que te cuesta | INTI |
|---|---|---|---|---|
| 1 | **Todo es objeto** | uniformidad | **28 bytes** por un entero de 8; una indireccion por operacion; la identidad se vuelve observable (`is`) | **NO**. Tres clases de valor (10.2) |
| 2 | **Conteo de referencias** | memoria liberada al instante | una escritura por uso; no libera ciclos; **es el padre del GIL** | solo en PLENO, y solo dentro de una tarea |
| 3 | **GIL** | el punto 2 sale correcto y gratis | **un nucleo por proceso** | **no existe**: nada compartido y mutable a la vez (sec. 4) |
| 4 | **Todo mutable en ejecucion** | monkey patching | nada se precalcula, se congela ni se comparte | congelado fuera de la funcion |
| 5 | **Duck typing** | escribes rapido | el error sale **en ejecucion y lejos** | igual, mas tipos **opcionales** |
| 6 | **Indentacion** | legibilidad | pelea con tabuladores | **SI**: lo unico de la lista con evidencia empirica (sec. 9) |
| 7 | **Excepciones** | reaccionar | cualquier linea salta a cualquier sitio | errores **como datos** (10.6) |

### ★ El numero que contesta lo de "todo son objetos"

```text
   un entero en C .............   8 bytes, en un registro
   un entero en Python ........  28 bytes en el monton
   una lista de 3 enteros ..... ~200 bytes  (12 en C)
```

Y la pila de la maquina virtual de CPython **solo sabe guardar `PyObject*`**:
el resultado de cada operacion tiene que ser un objeto del monton aunque quepa
en un registro. Por eso `a + b` no puede ser un `add`.

La sorpresa no es filosofica, es de comportamiento:

```python
   a = 256;  b = 256;  a is b     # True
   a = 257;  b = 257;  a is b     # False   <-- la sorpresa
```

Es la cache de enteros pequenos (-5 a 256) asomando. **El lenguaje te obligo a
saber donde vive un valor para poder predecir lo que hace.**

★ Y no hace falta inventar la alternativa: **Swift ya la tiene**. Sus `struct`
son **valores** (se copian, con copia-al-escribir), y el contador de
referencias **solo existe para las clases**. Swift es un lenguaje de sistema,
sin recolector y sin GIL, y se lee casi como Python. Es la prueba de que la
combinacion que pide Eddi **existe y funciona en produccion**.

---

## 4. EL GIL: la cadena entera, y por que INTI no la tiene

No es "Python es lento". Son tres eslabones, y **basta romper uno**:

```text
   1. cada objeto lleva un CONTADOR en su cabecera
   2. cualquier hilo puede tocar cualquier objeto
   3. -> dos hilos escriben el mismo contador -> se libera algo vivo
```

| salida | quien | precio |
|---|---|---|
| **cerrojo global** | CPython | correcto, barato, **un solo nucleo** |
| **contadores sesgados / por objeto** | PEP 703, Python 3.14t | ~4x en multihilo, pero **5-10% mas lento en un hilo**, **15-20% mas memoria**, **rompio la ABI** (hubo que cambiar la cabecera del objeto) y llego tras 13 anos. "GIL apagado por defecto": 2028-2030 |
| **que no haya nada compartido** | Erlang, Pony, **Starlark** | rompe el eslabon 2 y los otros dos sobran |

★★★ **INTI rompe el eslabon 2, y la pieza ya esta implementada aqui.**

Starlark (el dialecto de Python de Bazel) hace esto: **al terminar de
ejecutarse un modulo, todos sus valores de nivel superior quedan CONGELADOS**;
como son inmutables, **el modulo se publica a otros hilos sin ningun cerrojo**.

Y en este arbol ya esta escrito:

```text
   dynobj::header  ->  bit 63 = INMORTAL; "un inmortal nunca cambia su contador"
   MEM_OP_OFRECER / PRESTADO_OP_*   ->  prestar paginas entre procesos
   loan::take  ->  un objeto compartido lleva un INDICE, nunca un puntero
```

★★★ **El "valor congelado" de Starlark y el "objeto inmortal prestado" de
BMO-X son la misma cosa, y la de BMO ya tiene sus tests en verde.** El modelo
de concurrencia del lenguaje coincide con el modelo de memoria del sistema
operativo. **Eso no lo puede tener un lenguaje portable**, y es el argumento de
identidad mas fuerte que va a tener INTI.

**Las tres reglas:**

```text
   R1  una TAREA no ve el monton de otra. Nunca.
   R2  lo que cruza esta CONGELADO, o se copia.
   R3  un congelado no tiene contador que actualizar -> dos nucleos leyendolo
       a la vez no escriben nada -> no hay nada que proteger.
```

⚠ **Lo que se pierde, sin adornos:** no puedes tener una lista grande y mutable
que dos nucleos toquen a la vez. Es la limitacion que acepta Erlang, y es la
razon de que Erlang lleve 40 anos sin un data race.

★ Premio de sistema: si codigo, constantes y runtime estan congelados, **el
kernel los presta en vez de copiarlos**. Arrancar el segundo programa de INTI
no copia el runtime -- el escalon de `LA_RAM.md` cobrado por un lenguaje.

---

## 5. LAS SORPRESAS DE PYTHON, Y QUE HACE INTI CON CADA UNA

Python es **inspiracion, no herencia**: de aqui no se copia nada, se aprende de
lo que le salio caro. Una sola tabla, y cada fila termina en una decision.

| # | la sorpresa | INTI | regla |
|---|---|---|---|
| 1 | `def f(x, lista=[])`: el `[]` se crea una vez y todas las llamadas lo comparten | el valor por defecto se **congela** al declarar | 10.7 |
| 2 | `[lambda: i for i in range(3)]` da `2,2,2` | **no hay closures**: sin captura no hay *late binding* | 10.5 |
| 3 | `a is b` con 256 y con 257 dan cosas distintas | **`es` no existe**: un VALOR no tiene identidad | 10.2 |
| 4 | `b = a` con listas no copia | pasar no permite cambiar (+ copia-al-escribir) | 10.7 |
| 5 | `t[1] += [3]` **anade y ademas falla** | un congelado no se toca, y falla **antes** | 10.2 |
| 6 | `x.sort()` ordena y devuelve `None` | toda operacion **devuelve su resultado** | P3 |
| 7 | `UnboundLocalError` | no existe: un nombre es del bloque donde nacio | 10.8 |
| 8 | `global` / `nonlocal`, que solo existen para tapar el 7 | no existen | 10.8 |
| 9 | `0.1 + 0.2 != 0.3` | **decimal exacto** | 10.3 |
| 10 | `5/2` contra `5//2`, dos simbolos que se parecen | `/` divide, `entre` da cociente entero. **Nombres, no simbolos** | 10.9 |
| 11 | `except:` pelado se traga hasta `Ctrl-C` | los errores son datos: **no hay nada que tragarse** | 10.6 |
| 12 | un generador se agota: la segunda vuelta da vacio | recorrer no consume | P3 |
| 13 | `self`, el unico parametro que escribes siempre y nunca pasas | | |
| 14 | `__init__` no es el constructor: lo es `__new__` | **no hay clases** | 10.5 |
| 15 | el MRO de la herencia multiple es un algoritmo que hay que estudiar aparte | | |

★★★ **LA REGLA QUE LAS GENERA:** las filas 1 a 6 y la 12 son el mismo fallo
dicho de siete maneras:

> **la IDENTIDAD de un valor es observable, y la MUTABILIDAD es el estado por
> defecto.**

Quitando esas dos, **siete de quince desaparecen solas**, sin anadir ninguna
regla nueva. **No se tapan: dejan de poder existir.** Y las ocho restantes caen
de decisiones que ademas compran otra cosa -- ninguna necesita un parche.

⚠ Lo que **no** se hereda de Python, y conviene decirlo aqui para que nadie lo
busque: el modelo de objetos, el `dict` como mecanismo de todo, los metodos
`__dunder__`, las clases, `eval` y la compatibilidad. Ver la seccion 14.

---

## 6. SIN COMPORTAMIENTO INDEFINIDO -- el capitulo que pidio Eddi

### 6.1 Que es, y cuanto hay

Comportamiento indefinido (**UB**) es cuando el estandar dice: *si esto pasa,
no prometo nada*. No es "da un valor raro": es que **el compilador puede
suponer que nunca pasa** y borrar el codigo que lo comprueba.

```c
   int suma(int a, int b) {
       if (a + b < a) return -1;   /* comprobar desbordamiento */
       return a + b;
   }
```

El compilador razona: *desbordar un `int` es UB; el UB no pasa; luego `a+b<a`
es imposible; luego borro el `if`*. **La comprobacion desaparece del binario.**
Eso es lo que hace de esto una fabrica de agujeros de seguridad y no una
curiosidad academica.

**Cuanto hay:** **203 comportamientos indefinidos** catalogados en el Anexo J.2
de C11, **y el estandar avisa de que la lista no es exhaustiva**.

### 6.2 Por que existe -- dos razones, y una ya caduco

| razon | vigencia |
|---|---|
| **Portabilidad**: en 1978 habia complemento a uno, signo-magnitud, palabras de 36 bits | ⛔ **CADUCADA**. C23 hizo obligatorio el complemento a dos porque no queda hardware que use otra cosa |
| **Optimizacion**: sin UB, el compilador no puede suponer que un bucle termina, que un puntero no es nulo, que un indice esta dentro | ✅ vigente... y **medible** |

★★ La segunda es la unica que queda en pie, **y una razon medible es una razon
que se puede pagar a sabiendas**. Eso es 6.3.

### 6.3 ★★★ LO QUE CUESTA QUITARLO -- los numeros, que es lo que decide

| comprobacion | coste medido | fuente |
|---|---|---|
| **desbordamiento de enteros** | **0,8%** de caudal, y sin desviacion medible con lotes grandes | driver de red en Rust vs C (ixy) |
| **limites de array** | **0,881% (+/- 0,009)** en acceso a `vector` de C++ | estudio dedicado |
| **Rust seguro entero vs C**, mismo driver | **>90% del rendimiento de C**; 6-11% mas ciclos por paquete *haciendo mas trabajo* | ixy |
| instrumentar C a posteriori (Checked C) | **29,1%** de media geometrica | Checked C |
| sanitizers (ASan) | **4x-6x** | UBSan/ASan |

★★★ **La lectura, y es la frase que justifica la decision entera:**

> **Comprobar cuesta el 1%. Reparar C a posteriori cuesta el 30%. La diferencia
> no es la comprobacion: es hacerlo desde el diseno o hacerlo con parches.**

Y hay un motivo de silicio: un procesador moderno fuera de orden **predice bien
la comprobacion**, porque en ejecucion normal nunca salta. La comprobacion se
especula y desaparece del camino critico. **El coste de la seguridad se paga en
1978, no en 2026.**

### 6.4 Los tres modelos posibles, y cual coge INTI

| modelo | quien | que hace | veredicto |
|---|---|---|---|
| **Parchear C** | *Friendly C* (Cuoq, Flatt, Regehr, 2014) | convierte 14 categorias de UB en valores no especificados: el desbordamiento con signo da la vuelta, se elimina el alias estricto, leer sin inicializar da un valor cualquiera | ⚠ **deja fuera lo peor** (memoria liberada, fuera de limites siguen indefinidos) y **once anos despues los compiladores solo ofrecen banderas sueltas** (`-fwrapv`, `-fno-strict-aliasing`), no el dialecto. **Parchear no funciono** |
| **Detectar** | **Zig** | lo llama *comportamiento ilegal detectable*: en `Debug` y `ReleaseSafe` da panico; en `ReleaseFast` **vuelve a ser UB** | ⚠ honesto pero a medias: **el binario que entregas es el que no comprueba** |
| **★ Definir** | **WebAssembly**, Java, y en su capa segura Rust | **cada instruccion tiene semantica definida, sin nada indefinido ni dependiente de la implementacion**; lo que no tiene resultado sensato **atrapa** (trap), y atrapar es un resultado | ✅ **es el que coge INTI** |

★★★ **Y WASM es la prueba de que la tesis de C es falsa hoy.** WASM da
**semantica determinista a los casos raros -- desplazamientos fuera de rango,
division por cero, desbordamiento al convertir un flotante, alineacion --
"en todo el hardware y con sobrecarga minima"**, y ademas fija el orden de
bytes y IEEE-754. Es decir: **portabilidad TOTAL y CERO comportamiento
indefinido a la vez.** C dijo que habia que elegir. Ya no.

### 6.5 Las doce reglas de INTI, escritas para poder discutirlas una a una

| # | en C es | en INTI es |
|---|---|---|
| 1 | desbordar un entero con signo: **UB** | **atrapa**: devuelve un error como dato. Si quieres que de la vuelta, se pide con otro nombre (`suma_circular`) |
| 2 | indice fuera del array: **UB** | **atrapa**. Y donde el compilador ve el rango, **ni siquiera compila** |
| 3 | dividir por cero: **UB** | **atrapa** |
| 4 | leer una variable sin inicializar: **UB** | **imposible**: no existe declarar sin valor |
| 5 | puntero colgante / liberado: **UB** | en PLENO no hay punteros crudos; en LLANO, prestamos con vida comprobada en compilacion |
| 6 | orden de evaluacion de argumentos: **no especificado** | **izquierda a derecha, siempre** |
| 7 | desplazar mas bits que el ancho: **UB** | **definido**: da cero, y el compilador avisa si es constante |
| 8 | alias estricto (`int*` y `float*`): **UB** | **no existe**: dos nombres pueden ver los mismos bytes y esta definido |
| 9 | `int` mide "al menos 16 bits" | **tamanos exactos**: `entero8/16/32/64`, y `numero` para lo demas |
| 10 | orden de bytes: de la maquina | **little-endian fijado** en todo lo que se serializa |
| 11 | el compilador puede reasociar flotantes y meter FMA | **IEEE-754 estricto**: el mismo programa da el mismo bit en cualquier maquina |
| 12 | conversion flotante->entero fuera de rango: **UB** | **atrapa** (lo mismo que hace WASM) |

★ Fijate en el patron: **casi todo acaba en "atrapa"**, y atrapar en INTI **no
es un panico ni un aborto: es un error como dato** (10.6). O sea que las doce
reglas y el sistema de errores son la misma decision mirada dos veces.

### 6.6 Lo que NO se puede definir gratis, dicho por delante

Honestidad, que es lo que este proyecto pide:

- **Bucles infinitos**: C permite suponer que todo bucle termina; sin eso se
  pierden optimizaciones reales. INTI **no lo supone**: un bucle que no termina,
  no termina. Se paga.
- **La aritmetica de punteros en LLANO**: si INTI va a escribir un driver, tiene
  que poder escribir una direccion fisica. Ahi **la comprobacion no la puede
  hacer el lenguaje**. Solucion: un bloque `crudo` que hay **que escribir**, que
  el compilador **cuenta y publica**, y que `bmo-verify` puede exigir firmado.
  **Igual que `unsafe` de Rust, y por la misma razon: no se puede eliminar, se
  puede hacer VISIBLE y CONTABLE.**
- **La reproducibilidad de los flotantes cuesta**: prohibir FMA y reasociacion
  deja rendimiento en la mesa en calculo numerico. Se acepta: **el mismo
  resultado en cualquier maquina vale mas aqui**, donde el argumento de venta
  del sistema es que se puede verificar.

---

## 7. PORTABILIDAD: son dos cosas, y hay que decir cual se quiere

Eddi dijo *"para portabilidad y facilidad"*. Portabilidad significa dos cosas
distintas y **INTI necesita las dos, por motivos distintos**:

| | que significa | por que importa aqui |
|---|---|---|
| **A. El PROGRAMA se porta** | un `.inti` da lo mismo en cualquier maquina | es lo que dan las 12 reglas de 6.5. **Sin UB, el mismo programa da el mismo bit** -- que es exactamente lo que hace WASM |
| **B. El SISTEMA se porta** | BMO-X se mueve a ARM o RISC-V porque esta escrito en un lenguaje, no en instrucciones | ★ es **lo que C le dio a Unix en 1977** con el Interdata, y la unica razon historica por la que existio C |

★★ **Y la B ya esta a medio camino en este arbol, aunque nadie lo haya
llamado asi:** `tables/arch/x86_64/intrinsics.toml`. Las instrucciones de la
maquina **ya viven en una tabla**, no en el compilador -- por eso *"anadir una
instruccion = 1 entrada TOML, CERO Rust"*. **La forma de la portabilidad ya
esta elegida: otra arquitectura es otra carpeta de tablas**, no otro compilador.

⚠ Y el limite, dicho: **portable no significa que el `.bex` corra en otra
maquina.** Significa que el **fuente** vuelve a compilar. C nunca prometio mas
que eso, y prometer mas seria WASM -- otro trabajo, y esta descartado (sec. 13).

---

### 7.1 CONTROL NO ES PRIVILEGIO -- y INTI no da root

> Duda de Eddi, 2026-08-19: *"que viva en control... pero suena extrano, control
> total como root, pero neh... no es root, no? Es por motivos, necesito eso para
> portabilidad."*

**No es root.** Y la confusion es tan comun que conviene partirla en **tres ejes
independientes**, porque cada uno lo concede alguien distinto:

| eje | la pregunta | quien lo concede | INTI |
|---|---|---|---|
| **EXPRESION** | *puedo DECIRLO?* -- este byte aqui, este tamano exacto, esta instruccion | **el lenguaje** | ✅ **esto es lo que INTI da** |
| **PERMISO** | *me DEJAN hacerlo?* -- tocar esa pagina, reclamar la pantalla, leer ese disco | el kernel: capabilities, MMU y anillo | ⛔ INTI no lo toca |
| **CONFIANZA** | *alguien FIRMO que esto puede correr?* | `bmo-verify` y la firma del BEF | ⛔ tampoco |

★★★ **La frase que lo cierra: INTI da control sobre TU maquina, no autoridad
sobre la de nadie.** Root es lo segundo. Y ademas son opuestos en algo que este
proyecto ya tiene escrito: **root es AMBIENTAL** --lo tienes por ser quien
eres-- y aqui *el privilegio no es ambiental, se sostiene por capabilities
explicitas*. Una capability se pasa y se revoca; root no se revoca, se tiene.

#### La prueba de que el lenguaje no podria dar privilegio aunque quisiera

Un `.bex` de INTI corre en **Ring 3** y pasa por el gate. Lo unico que hace el
lenguaje es **emitir bytes**; quien decide si esos bytes pueden tocar algo es la
MMU y el anillo. Aunque INTI compilara un `outb` a un puerto en un programa de
usuario, **el hardware contesta con un `#GP`**. El lenguaje no puede escalar
privilegios **porque el privilegio no vive en el lenguaje**.

⚠ Y el caso honesto que hay que decir: el dia que se escriba un driver en INTI
LLANO, ese codigo si tendra privilegio -- **pero por donde lo cargan, no por en
que lenguaje esta escrito**. Es exactamente lo que pasa hoy con el kernel, que
esta en Rust: nadie diria que Rust "da root".

> **Ningun lenguaje da root. El cargador y el anillo dan root.**

#### Y por que el control SI hace falta para la portabilidad, que era tu motivo

Va al grano: la mitad B de la seccion 7 -- *el SISTEMA se porta* -- solo se
cumple si **lo que hoy solo se puede escribir en ensamblador se puede escribir
en INTI**. Disposicion exacta, tamanos exactos, la instruccion concreta cuando
hace falta.

Si INTI no tuviera ese control, habria partes de BMO-X **atadas para siempre al
ensamblador de x86**, y esas partes no se moverian nunca a ARM ni a RISC-V. O
sea: **el control de expresion es el precio de la portabilidad**, y no tiene
nada que ver con el permiso.

★ El resumen en una linea: `crudo` no te da permisos nuevos. Te deja **decir**
lo que ya podias hacer -- y hace que se vea.

---

### 7.2 AGNOSTICO, y vigilado con un test

> Regla de Eddi, 2026-08-19: *"INTI es agnostico, entonces mis lenguajes son
> agnosticos, lo mismo con BMO ABI; no va a representar x86-64 exclusivo, si no
> seria atado. Por eso tenias razon en la palabra CRUDO."*

**La ley:** el frontend de INTI **no puede nombrar una maquina**. Ni registros,
ni opcodes, ni anchos de palabra, ni convenciones de llamada. Lo que si sabe de
la maquina vive en `tables/arch/<arquitectura>/`, que es una carpeta de
**datos**.

#### El reparto, con la frontera dicha

| capa | sabe de la maquina | por que |
|---|---|---|
| `lexico`, `arbol`, `sintaxis` | **nada** | un `si` es un `si` en cualquier procesador |
| analisis (perfiles, nombres, tipos) | **nada** | tampoco |
| la IR | **nada** | por eso hay una IR: si el arbol fuera a bytes, la maquina se colaria en el arbol |
| el **perfil de maquina** | ancho de puntero, alineacion, orden de bytes, registros, convencion | ★ son **datos**, no codigo |
| la seleccion de instrucciones | todo | y vive en `tables/arch/`, como `intrinsics.toml` |

#### ⚠ Y esto NO es una promesa: hay un test

`tests/agnostico.rs` recorre el frontend entero y falla si aparece `rax`,
`x86`, `sysv`, `modrm`... **incluidos los comentarios**, a proposito: un
comentario que explica algo en terminos de `rax` es la senal de que alguien
estaba pensando en x86 mientras escribia una parte que no debia saber de eso.

Una regla asi **se cumple sola el primer dia y se rompe el tercero**, cuando
alguien necesita el tamano de un puntero y escribe un `8`. No se rompe por
descuido: se rompe porque en ese momento es *lo mas facil* -- exactamente igual
que meter el syscall en el compilador. Por eso se vigila como se vigila la
codificacion con `ascii-sweep`.

★ Lo que el test **no** puede probar: que INTI corra en ARM. Eso solo lo prueba
un ARM. Prueba que **nadie ha escrito todavia la linea que lo impediria**, y eso
se puede saber hoy y sin hardware.

#### ★★ Y `crudo` no es solo una valvula: es el MEDIDOR

Aqui es donde la palabra se gana el nombre. Como `crudo` hay que **escribirlo**
y el compilador lo **cuenta**, la pregunta *"cuanto de mi programa esta atado a
esta maquina?"* deja de ser una impresion y pasa a ser **un numero que sale en
el informe del `.bex`**.

```text
   programa de 4.000 lineas, 2 bloques `crudo`  ->  se porta, y se sabe por donde
   programa de 4.000 lineas, 200 bloques        ->  no se porta, y tambien se sabe
```

Ningun otro lenguaje puede decirte eso: en C, lo atado a la maquina esta
repartido en `#ifdef`s que nadie ha contado nunca.

---

## 8. "ESTRICTO PARA FACILITAR, PERO TE AYUDA" -- en que se diferencia de Rust

Eddi lo dijo asi y merece precision, porque **hay dos estricteces muy
distintas** y confundirlas es lo que hace que la gente rebote con Rust:

| | Rust | INTI |
|---|---|---|
| **sobre que es estricto** | sobre la **PROPIEDAD**: quien es dueno de que, cuanto vive, quien lo presta | sobre la **DEFINICION**: que pasa exactamente en cada caso |
| **que te obliga a hacer** | **demostrarle al compilador** que no hay dos referencias mutables. Eso es una disciplina nueva que hay que aprender | **decir que quieres** cuando hay dos respuestas razonables |
| **cuando falla** | te pide que reestructures el programa | te pide que anadas una palabra |
| **coste de aprendizaje** | semanas | minutos |

★★★ **Esa es la frase del documento:** *la estrictez de Rust recae sobre el
programador; la de INTI recae sobre el lenguaje.* INTI no te pide que
demuestres nada: **se compromete el, y a cambio te obliga a no dejar huecos.**

Y "te ayuda" tiene forma concreta, porque hay evidencia de que el mensaje de
error **es la interfaz principal del lenguaje** (9.2). El formato es un contrato
de cuatro partes, disenado contra los cuatro factores que el estudio de CHI 2021
midio -- longitud, jerga, estructura y vocabulario:

```text
   [QUE PASO]     La suma de `total` y `linea` se puede pasar de la cuenta.
   [DONDE]        notas.inti, linea 12:   total = total + linea
   [QUE HABIA]    `total` es entero32 y aqui puede llegar a 2.147.483.700.
   [QUE HACER]    Elige una:
                    total = total + linea            (atrapa si se pasa)
                    total = suma_circular(total, linea)   (da la vuelta)
                    cambia `total` a entero64
```

Reglas del mensaje, y son **verificables con un test**: cero jerga de compilador
(nada de "token", "AST", "unexpected EOF"), **el nombre que escribio el usuario
aparece siempre**, la sugerencia es codigo que se puede pegar, y cada error
tiene un **codigo estable** (`E0042`) para poder buscarlo.

---

## 9. QUE DICE LA EVIDENCIA sobre "facil" -- no opiniones

### 9.1 La sintaxis tipo C no ayuda a un novato. Nada.

Stefik y Siebert (ACM TOCE 2013) midieron novatos en seis lenguajes, incluido
**Randomo**, cuyas palabras clave se eligieron **al azar del ASCII** -- un
placebo. ★★ **Los lenguajes de sintaxis tipo C (Java, Perl) no dieron tasas de
acierto significativamente mejores que las palabras al azar.** Los que si:
**Quorum, Python y Ruby**.

### 9.2 El error de sintaxis es el estado normal

El trabajo de Hermans sobre **Hedy** cita el dato: **el 73% de los envios de
codigo de estudiantes tienen errores de sintaxis; los mejores, el 50%.** Por
tanto el mensaje de error no es un caso de borde: es la interfaz principal. Y
CHI 2021 midio que lo decide: **longitud, jerga, estructura, vocabulario**.

### 9.3 Lo dificil no es la sintaxis: es no tener una maquina en la cabeza

Sorva: lo que separa al que aprende es tener modelo mental de **que hace la
maquina**. Soloway con el *rainfall problem* (leer numeros hasta un centinela y
sacar la media) mostro que lo caro es **componer planes**, no las construcciones
sueltas.

★★★ **Y aqui BMO-X tiene ventaja injusta:** una *maquina nocional* solo se puede
ensenar si es pequena y visible. La de Python no lo es. **La de INTI cabe en una
hoja, y ademas se puede ensenar DE VERDAD porque el sistema es tuyo entero**: un
depurador de INTI puede mostrar la memoria real y no una metafora.

### 9.4 El idioma es una barrera medida

Guo (CHI 2018) y Becker (PPIG 2019) documentan la barrera del ingles. Y hay
resultado positivo: los novatos que programan con **palabras clave y entorno en
su lengua** aprenden conceptos nuevos **mas rapido**. En el mundo hispano el
caso es masivo: **PSeInt** (Pablo Novara, 2003) -- `Proceso`, `Escribir`,
`Leer`, `Si-Entonces` -- es con lo que aprende media Latinoamerica.

### 9.5 Las cuatro palancas, y la que NO existe

1. **Mensajes de error que ensenan** (la interfaz principal, 73%).
2. **Una maquina nocional pequena y visible**.
3. **Sintaxis que se aparta de C** (lo unico que batio al placebo).
4. **El idioma de quien escribe**.

⚠ **Y lo que NO esta medido:** que menos palabras clave, menos teclas o no
escribir tipos ayuden. **La brevedad no es facilidad.** ABC era brevisimo.

---

## 10. EL DISENO DE INTI

### 10.1 Los seis principios

| # | principio | de donde sale |
|---|---|---|
| **P1** | **Nada indefinido.** Toda construccion tiene un resultado dicho por escrito | sec. 6 |
| **P2** | **Si dos cosas se ven iguales, se comportan igual.** Sin identidad observable | sec. 5 |
| **P3** | **Lo que no dices, no pasa.** Ni conversiones, ni copias, ni mutacion a distancia | sec. 5 (4, 5, 6) |
| **P4** | **Un error es un dato con nombre, no un salto** | 9.2 |
| **P5** | **El lenguaje ensena su maquina** | 9.3 |
| **P6** | **Se puede salir del lenguaje: INTI escribe sistema** | sec. 2, la leccion de ABC; sec. 1, la de C |

### 10.2 El modelo de valores: TRES clases, no una

```text
   VALOR       cabe en la mano. Se copia. NO tiene identidad.
               numero, si/no, letra, nada, registro pequeno
               -> comparar es comparar. `es` NO EXISTE.
               -> es el `struct` de Swift, y es todo lo que hay en LLANO.

   COSA        vive en tu monton, con contador de referencias.
               texto, lista, tabla, funcion con captura
               -> SOLO tu tarea la ve. Nadie mas. Nunca.
               -> es la `class` de Swift con ARC. Solo en PLENO.

   CONGELADO   inmortal. Nadie lo cambia, nadie cuenta sus referencias.
               literales, constantes, un modulo cargado, un mensaje enviado
               -> se PRESTA entre tareas y entre procesos.
               -> es el `IMMORTAL` que ya esta en dynobj::header.
```

Y **copia-al-escribir** para las COSA grandes, como Swift: copiar una lista de
mil elementos al pasarla **no copia nada** hasta que alguien escribe. Asi P3
("nada cambia a tu espalda") no cuesta memoria.

**Representacion: ranura de 16 bytes** (etiqueta 8 + carga 8). ⚠ Y hay que
decir por que **no NaN-boxing**, que es lo que usan LuaJIT y JavaScriptCore y
es mas rapido: NaN-boxing mete todo dentro de un `double`, y **solo funciona si
tu numero por defecto es un `double`**. Con decimal exacto (10.3) no cabe. Es un
intercambio consciente: **el doble de memoria por ranura a cambio de que
`0.1 + 0.2` sea `0.3`**.

### 10.3 Un solo `numero`, y exacto por defecto

Python tiene `int`, `float`, `Decimal`, `Fraction` y `complex`: **cinco tipos
numericos, y el que esta por defecto es el que miente**. INTI tiene **uno
visible**, con dos formas por dentro:

```text
   entero    i64                            para contar
   decimal   coeficiente 128b + escala      para dinero y para lo demas
```

y una promesa que se puede poner en la portada:

```text
   escribe 0.1 + 0.2      ->   0.3      y no 0.30000000000000004
```

★★ **No hay que inventarlo: el motor ya esta en el arbol.** COBOL de BMO tiene
`PICTURE` con decimal exacto y `bmo_lower::packed` emite BCD. Es la pieza que el
plan de Ada ya dio por *pagada*. **INTI es el tercer cliente del mismo motor.**

⚠ **Numero incomodo:** una suma decimal de 128 bits cuesta **5-20x** una entera
de 64. En LLANO no aparece (ahi se escribe `entero64`). En PLENO queda tapada
por el despacho. **Se notaria en el AOT numerico**, y por eso `entero` es una
forma aparte que sigue siendo un `add`.

Detalles que son trampa y hay que fijar de una vez: **el separador decimal es el
punto** (la coma ya separa argumentos); `1/3` **redondea a 28 digitos y avisa
una vez**, no falla; y **sin enteros de precision arbitraria** en la primera
version -- si hace falta criptografia, entra como libreria y no como tipo.

### 10.4 Concurrencia

Las tres reglas de la seccion 4 (R1/R2/R3). Sin GIL, sin cerrojos, sin
contadores atomicos, sin el 5-10% ni el 15-20% que le costo a Python 3.14t. Y
encaja con `AXION` cuando aprenda a encender nucleos.

### 10.5 Sin herencia, sin clases, sin `self`

Lo que Python paga: MRO C3, metaclases, `__slots__`, `super()`, descriptores,
`__new__` contra `__init__`, y el `self` explicito. Precedentes de que se vive
sin eso: **Starlark no tiene tipos de usuario** (ni herencia, ni reflexion, ni
excepciones); **Lua tiene una sola estructura**; **Go no tiene herencia**.

```text
   registro Punto
      x es numero
      y es numero

   funcion mover(p es Punto, dx, dy) devuelve Punto
      devuelve Punto(p.x + dx, p.y + dy)
```

Y para los tipos que necesitan comportamiento propio, **una tabla de operaciones
numeradas** -- que es exactamente `dynobj::slots`, ya escrito, con `OP_ADD`,
`OP_COMPARE`, `OP_HASH`, `OP_LEN`, `OP_GETATTR`... **Se indexa por numero
(`call [rax+N]`), sin buscar metodos por nombre en la via caliente.**

⚠ Un `registro` no hereda de otro. La composicion se escribe. Es menos potente
y es a proposito.

### 10.6 Los errores son datos

**No hay excepciones invisibles.** Toda funcion que pueda fallar lo dice:

```text
   resultado = abrir("notas.txt")
   si fallo resultado
      escribe "no pude abrir:", motivo de resultado
      devuelve

   texto = valor de resultado
```

y la forma corta:

```text
   texto = abrir("notas.txt") o si no ""
```

★ Premio tecnico que ya vio `PYTHON_MAESTRO.md`: **un interprete no necesita
tablas de desenrollado** -- propaga una bandera y cada bytecode la mira. Las
excepciones eran la construccion mas cara de C++ y aqui salen casi gratis
**justamente porque no son excepciones**. ★★ Y en el modo AOT es todavia mejor:
un error es un valor de retorno, o sea **un registro**, y BMO C++ pudo
descartar las tablas de desenrollado por esta misma razon.

### 10.7 Mutabilidad: hay que pedirla

```text
   x = 5              # x no cambia
   cambiante y = 5    # y puede cambiar
```

Y una COSA que se pasa a una funcion **no se puede cambiar dentro** salvo que se
diga. Mata las sorpresas 1, 4, 5 y 6 de la seccion 5, **y es lo que hace posible
congelar barato** (sec. 4). Es la decision que mas teclas cuesta y mas compra.

### 10.8 Alcance: uno, lexico, sin `global`

Un nombre pertenece al bloque donde nacio; un bloque interior **puede leer pero
no escribir** un nombre de fuera; las capturas son **por valor al crear la
funcion**. Con eso `global`, `nonlocal` y `UnboundLocalError` **no existen**.

### 10.9 Sintaxis

- **Indentacion** (9.1: es lo unico que batio al placebo).
- **Sin `:` de bloque, sin `;`, sin `{}`, sin parentesis en las condiciones.**
- **Parentesis en las llamadas, si**: `f` y `f()` tienen que verse distintos.
- **Un solo bucle, tres formas** -- porque el rainfall problem dice que lo caro
  es componer, no la palabra:

```text
   para cada x en lista        ...
   repite 10 veces             ...
   repite mientras <condicion> ...
```

- Comentario `#`. Sin comprensiones anidadas, sin ternario, sin `lambda` con
  reglas propias, sin decoradores, sin walrus.

### 10.10 El idioma: espanol en ASCII, y en una TABLA

| pieza | decision | motivo |
|---|---|---|
| palabras clave | **espanol sin tildes**: `funcion`, `si`, `sino`, `mientras`, `para cada`, `devuelve`, `escribe`, `cambiante`, `registro`, `crudo` | ASCII puro; el lexer no necesita unicode |
| la tilde | **alias**: la version con tilde lexa igual | quien escribe con tildes no tropieza |
| identificadores | UTF-8 permitido, con aviso | son sus nombres |
| textos | UTF-8, pasan tal cual | ⚠ la consola del kernel es Latin-1 por diseno: **una tilde en un literal ya inflo un `.bex` de 512 bytes a 492.032**. La conversion la hace `escribe`, no el usuario |
| mensajes de error | espanol | son la interfaz principal |

⚠ **La parte incomoda:** palabras clave en espanol significa que **nadie fuera
de tu idioma va a contribuir**. Es un intercambio real. ★ La salida es barata
**si se toma ahora**: la tabla de palabras clave es **una tabla**, no codigo --
el mismo patron que `intrinsics.toml`. Un fichero mas y INTI habla ingles sin
tocar el compilador. **Hacerlo asi hoy no cuesta nada; convertirlo despues
cuesta el parser entero.**

### 10.11 Tipos: opcionales en PLENO, obligatorios en LLANO

```text
   funcion media(numeros)                                 # PLENO, sin tipos
   funcion media(numeros es lista de numero) devuelve numero
```

★ En **LLANO son obligatorios**, y no por rigor: **sin tipos no hay tamanos, y
sin tamanos no hay perfil sin monton.** La obligacion sale del perfil, no del
gusto.

⚠ **No es tipado gradual sonoro y hay que decirlo:** Typed Racket demostro que
la version sonora puede costar **hasta 100x** por los contratos que envuelven
cada frontera. INTI hace lo que TypeScript: comprueba lo que ve, no garantiza lo
que no ve, **y no envuelve nada en ejecucion**.

---

## 11. LO QUE SOLO SE PUEDE HACER AQUI

La leccion de ABC dice que un lenguaje que no habla con su sistema se muere.
Esto es lo que INTI puede hacer **el primer dia**, todo con piezas en metal:

| lo que se escribe | contra que va | estado |
|---|---|---|
| `escribe "hola"` | `bmo_lower::fmt` | HECHO |
| `pinta rectangulo(...)` | `superficie.h` + DIRECTOR | HECHO |
| `tecla = espera tecla()` | `entrada.h` + teclado USB | HECHO |
| `guarda "notas.txt", texto` | ESTRATOS desde Ring 3 | HECHO (18-08) |
| `abre recurso("logo.png")` | `Resources 0x0B` + `TASK_OP_MI_PAQUETE` | HECHO (09-08) |
| `crudo { escribe_puerto(0x60, x) }` | `intrinsics.toml` (`__outb`) | HECHO |
| `invoca(cap, op, a0, a1, a2)` | la puerta: fila `[syscall]` de `intrinsics.toml` | HECHO |
| `en paralelo: ...` | tareas aisladas + prestamo de congelados | contrato listo, falta cablear |

★★ **Y la que no tiene nadie:** *"se abre, no se carga"*. En Python, `import`
lee, descomprime y copia a RAM. Aqui **el modulo vive en el `.bex` y se lee por
desplazamiento sin traerlo entero** -- lo mismo que le quito el 87,1% a DOOM. Un
programa de INTI es **un fichero**: codigo, modulos, imagenes **y su firma
dentro**. Un `.pyz` no puede llevar su firma.

★★ **Y una precision que hay que tener clara: nada de eso es sintaxis.** La
puerta **no vive en la gramatica** -- `invoca` viene de `usa bmo`, que baja a la
fila `[syscall]` de `intrinsics.toml`. Es la leccion de C otra vez: **`read()`
nunca fue una palabra clave**, y por eso Unix pudo saltar al Interdata. Si INTI
reservara `INVOKE`, quedaria casado con este sistema y se perderia la mitad B de
la portabilidad (sec. 7). Los tres escalones -- REX, la puerta, los intrinsecos
-- son **tablas**, y por tanto se pueden tapar con `bmo-mods` sin bifurcar nada.

★ Y la regla que decide donde hace falta `crudo`: **`invoca` no lo necesita y un
puerto de E/S si**. No es incoherencia -- al otro lado de una capability hay un
kernel que comprueba; al otro lado de un `outb` no hay nadie. `crudo` no marca
*"bajo nivel"*, marca **"aqui nadie comprueba por ti"**.

★★★ **Y la que responde a P5:** el depurador de INTI puede ensenar la memoria
**de verdad**, no un dibujo. Nada de lo que Sorva llama *maquina nocional* tiene
por que ser una metafora aqui. **Eso no lo puede copiar nadie que corra sobre
Linux.**

---

## 12. COMO SE CONSTRUYE

Donde vive: **`toolchain/lang/inti/`**, al lado de `c`, `cobol`, `cpp`, `ada`.
Frontend en Rust como los otros cuatro; **runtime en Rust `no_std`** enlazado
con **`bex-link`** (esto corrige lo que `PYTHON_MAESTRO.md` proponia -- runtime
en C dentro de `forge/` --, escrito cuando `bex-link` no pesaba en la balanza).

⚠ **Y aqui va la inversion que anuncie en la seccion 0.** Aquel documento decia
*"el REPL fuerza un interprete, y el AOT no se puede hacer primero"*. **Para
INTI el orden es el contrario**, y el motivo es la seccion 1: **el lenguaje del
sistema tiene que producir un `.bex` que corra sin nada debajo**. Un interprete
no puede escribir un driver. Asi que:

| fase | entregable | como se sabe que esta | tamano |
|---|---|---|---|
| **F0** | ✅ **HECHO el 2026-08-19**: `toolchain/lang/inti/` -- [`GRAMATICA.md`](../../toolchain/lang/inti/GRAMATICA.md) (sintaxis + EBNF), [`REGLAS.md`](../../toolchain/lang/inti/REGLAS.md) (las doce), [`CENSO.md`](../../toolchain/lang/inti/CENSO.md) + **35 sondas** en `censo/*.inti` | cada sonda lleva su veredicto en la primera linea; el test comparara el informe **entero** contra la constante | ~1.100 lineas |
| **F1** | Lexer + parser -> AST, en el anfitrion | un `.inti` entra, un arbol sale, y **los mensajes ya cumplen el formato de 4 partes** | ~2.000 |
| **F2** | ★ **INTI LLANO compila a `.bex` nativo** | un programa sin monton que `escribe` y suma, **corriendo en el emulador**, y pasando `bmo-verify` | ~2.500 |
| **F3** | Las 12 reglas, **con sus sondas en verde** | atrapa el desbordamiento, atrapa el indice, IEEE-754 estricto | ~1.000 |
| **F4** | ★★ **La foto del Ryzen**: un programa de INTI LLANO en metal | la prueba de aceptacion. **Aqui INTI ya es un lenguaje de verdad** | ~600 |
| **F5** | INTI PLENO: valores, texto, lista, tabla, decimal, contador de referencias | tests de anfitrion + una app que pinta y guarda | ~3.000 |
| **F6** | Congelar + tareas + prestamo | dos tareas, dos nucleos, cero cerrojos | ~1.500 |
| **F7** | El REPL | `>>>`, `2+2`, `4` -- **la comodidad, no la esencia** | ~1.200 |

**Numeros incomodos por delante:**

- **~7.000 lineas hasta la foto del metal (F0-F4)**; ~12.000 con PLENO.
  Referencia: el kernel de Unix de 1973 eran **~9.000**. Un Python propio
  compatible eran 25-40k. MicroPython entero, 100k.
- **INTI LLANO corre a la velocidad de C** (no hay runtime que pagar).
  **INTI PLENO interpretado ira 50-200x mas lento**, y el AOT de PLENO da
  **2-4x, no 50x**, porque `x + y` sigue siendo `call sumar`.
- Las comprobaciones de 6.5 cuestan **~1%** (6.3). El decimal, 5-20x, y solo
  donde se use.

✅ **F0 esta escrito.** Las 35 sondas y las doce reglas viven en
`toolchain/lang/inti/`, y **cinco de las doce reglas no tienen sonda ahi a
proposito**: orden de evaluacion, alias, orden de bytes e IEEE-754 no se pueden
comprobar leyendo un fuente, y nacen en F3. Escribirles una sonda de fuente
habria sido fingir que estan cubiertas.

**Por que fue F0 y no otro paso:** es lo unico que **no se puede meter
despues** -- una regla de 6.5 mal puesta se paga en cada programa que se escriba
nunca --, es texto y sondas (cero riesgo de que algo deje de arrancar), y es lo
que este proyecto ya acordo: **el CONTRATO antes que el codigo**, igual que
`Resources = 0x0B`, declarada y vacia desde que se diseno BEF.

---

## 13. INTI CONTRA EL ENSAMBLADOR -- que se pierde, y quien lo va a decir

> Duda de Eddi, 2026-08-19: *"estoy empezando a sentir extranado INTI vs ASM en
> velocidad, aunque si, ambos tienen su propio lado, aunque INTI......"*

Es la pregunta correcta y llega en el momento correcto. Se contesta en cuatro
partes, y la primera es separar dos preguntas que se confunden todo el rato.

### 13.1 Son DOS preguntas, y solo una tiene debate

| | contra ASM a mano | hay discusion |
|---|---|---|
| **INTI PLENO** interpretado | **50-200x mas lento** | **no.** Es el precio de un interprete, en cualquier sitio, y no lo arregla ningun kernel |
| **INTI PLENO** en modo AOT | ~15-60x | no |
| **INTI LLANO** | **el 90-99% de ASM** | ✅ **aqui esta la pregunta de verdad** |

★★ **Y por eso existen los dos perfiles.** No son una comodidad: son la
respuesta a esta duda **tomada por adelantado**. Un lenguaje con un solo perfil
habria tenido que elegir entre ser facil y ser rapido, y habria acertado la
mitad de las veces.

### 13.2 Los TRES sitios donde INTI LLANO pierde contra ASM escrito a mano

Sin adornos, porque el numero incomodo va delante:

| donde | cuanto | se puede recuperar |
|---|---|---|
| **Las comprobaciones** (desborde, limites) que el ensamblador simplemente no hace | **~1%** medido: desbordamiento 0,8%, limites 0,881% | no, y no se quiere: es lo que compra el "sin comportamiento indefinido" |
| **El prologo y la convencion de llamada** que un humano se salta cuando sabe que no hace falta | unos ciclos por llamada; medible con la fila 2 de `coste` (una llamada son **20 ciclos**) | si, en parte: funciones que se meten en linea |
| **Lo que un humano ve y un compilador no**: elegir registros con la vista puesta en el bucle entero, reutilizar una bandera de la CPU ya calculada, SIMD escrito a mano | esto **si puede ser 2-5x** en un nucleo apretado | ⚠ no del todo. Es real y hay que decirlo |

### 13.3 Los tres sitios donde INTI LLANO **gana** a ASM escrito a mano

Y esto no es consuelo, es el motivo por el que Unix se reescribio:

- **No se equivoca de registro.** El fallo tipico de un ensamblador a mano no es
  ser lento: es olvidarse de restaurar `rbx` en una rama de tres.
- **Se puede reescribir sin miedo.** Un bucle de ASM optimizado a mano es un
  bloque que nadie vuelve a tocar. Uno de INTI se reescribe el martes.
- **Se porta.** Y esa es la mitad B de la seccion 7, la unica razon historica
  por la que existio C.

★★★ **El precedente exacto, y es el mismo miedo:** en 1973, reescribir el kernel
de Unix en C fue *audaz* precisamente porque todo el mundo sabia que se perdia
velocidad frente al ensamblador -- *"en aquel momento la programacion de sistemas
se hacia en ensamblador para sacarle el maximo al hardware"*. **Perdieron algo de
velocidad y ganaron el sistema entero.** Y ni siquiera lo perdieron del todo: el
arranque y el cambio de contexto **siguieron en ensamblador**, igual que aqui.

### 13.4 ★★ Y la parte que deshace la pregunta: INTI no compite con ASM, lo INCLUYE

Aqui el ensamblador **no es el rival de INTI: es su suelo**, y ya esta cableado.

```text
   crudo
       escribe_puerto(0x60, x)      -> una FILA de intrinsics.toml, bytes exactos
```

En BMO C esto ya funciona asi y la propiedad esta escrita en `bmo.h`: *"no es
una caja negra tipo `asm()` que el compilador copia sin leer. El compilador
emite ESOS bytes"*. O sea que la comparacion honesta no es *INTI o ASM*, sino:

> **cuanto de tu programa necesita de verdad la instruccion exacta?**

En DOOM fue ~0%. En un driver, tres lineas. Y esas tres lineas **se escriben en
INTI**, dentro de un `crudo`, con los bytes de una tabla.

### 13.5 Como se va a zanjar: midiendo, no discutiendo

⚠ **Hoy cualquier cifra de INTI seria inventada**, porque no hay generador de
codigo: F1 es texto y piezas. Decir "va a ir al 95% de C" ahora mismo seria
exactamente la clase de promesa que `PROPOSITO.md` manda medir en vez de
respetar.

El banco ya existe y solo hay que copiarlo. `coste_C.bex` zanjo la discusion de
la puerta (2.663 -> 969 ciclos) con un metodo que se reutiliza tal cual:

```text
   fila 1   bucle vacio          la base
   fila 2   una llamada          20 ciclos
   fila 5   rdtsc suelto         ** el instrumento, para RESTARLO
```

★ La fila 5 es la que hace que esto sea una medida y no una impresion: *"un
`rdtsc` suelto son 69 ciclos, no ~25. La fila existia para que esto fuera una
resta y no una suposicion, y menos mal."*

**La sonda de INTI, cuando exista F2**: el mismo bucle escrito tres veces --
en `sem-asm`, en BMO C y en INTI LLANO --, los tres en el mismo `.bex`, los tres
medidos con la misma fila de control, corriendo en el Ryzen.

Y el criterio de aceptacion, escrito ahora para que no se negocie despues.

### CORRECCION del 2026-08-19, el mismo dia: **un solo liston no vale**

Se escribio *"INTI LLANO >= 90% de BMO C"* y **se queda corto**. Comprobado en
`codegen/mod.rs`:

```text
   1399:  self.code.push(0x50);   // push rax -> el valor vive en [rsp]
```

**BMO C evalua expresiones POR PILA**, con acumulador en `rax`. Estar al 90% de
eso mide que el emisor funciona -- **no que el lenguaje este al nivel del
ensamblador**. Hacen falta dos listones y dicen cosas distintas:

```text
   LLANO vs BMO C        >= 90%   -> el emisor funciona (higiene)
   LLANO vs ASM a mano   >= 85%   -> ★ el liston de verdad, en el bucle de
                                     referencia y SIN SIMD
   LLANO vs ASM con SIMD  se pierde, y se recupera escribiendo ESE trozo
                          en `crudo`
```

★★ Y la referencia doble no es indecision: **contra C se mide el emisor, contra
ASM se mide el lenguaje.** Comparar solo contra ASM escrito por un experto mide
al experto; comparar solo contra C deja pasar un emisor mediocre si el de al
lado tambien lo es.

### 13.6 Los TRES mecanismos que valen el 90% de la distancia

No es una lista interminable. Contra el ensamblador, casi todo esta aqui:

| # | mecanismo | lo que vale | lo que cuesta |
|---|---|---|---|
| **1** | **Asignacion de registros** -- que un valor viva en un registro durante todo su tramo de vida en vez de empujarse a la pila | **2-4x** en codigo aritmetico, sin tocar una linea del programa del usuario | ~800-1.500 lineas. ⚠ pide **una IR con temporales**, que es el cambio estructural de verdad |
| **2** | **Meter funciones en linea** | aqui vale mas que en otros lenguajes: **las comprobaciones de INTI son funciones**, y sin inline cada una cuesta una llamada (20 ciclos) en vez de dos instrucciones | ~300 lineas, si ya hay IR |
| **3** | **Borrar la comprobacion que se puede demostrar** | es lo que hace que "sin UB" cueste **1% y no 30%** | ~400 lineas |

★★★ Y una ventaja de INTI que es GRATIS, porque es diseno y no optimizacion:
**`para cada x en lista` no tiene indice.** No hay nada que comprobar porque no
hay nada que pueda salirse. **El bucle idiomatico del lenguaje es justamente el
que no paga**, y por eso el 1% es alcanzable de verdad y no una esperanza.

Lo demas -- sacar invariantes del bucle, `lea` para aritmetica de direcciones,
no recalcular una comparacion que la ALU ya dejo en las banderas, funciones hoja
sin marco -- vale entre el **1% y el 5%** cada una. Son reales y van **despues**:
hacerlas antes de tener registros es pintar una casa sin cimientos.

### 13.7 SIMD: no se autovectoriza, y es una decision

GCC y LLVM llevan veinte anos en la autovectorizacion y **sigue siendo fragil**:
funciona hasta que cambias una linea y deja de funcionar sin decirtelo.

INTI da **SIMD por intrinsecos de tabla**, con el mecanismo que ya existe
(`intrinsics.toml`: bytes exactos, aridad validada, cero Rust por instruccion
nueva). Quien quiere SIMD lo escribe, y no depende de que el compilador adivine.
Es la misma decision que el proyecto ya tomo con `__outb` y con la puerta.

### 13.8 ★★★ EL TRATO: el tiempo se gasta al compilar, no al ejecutar

Idea de Eddi, 2026-08-19, y es la que ordena toda la seccion:

> *"INTI compila pero analiza, se tarda un poco... PERO ES PROCESO. Luego el
> resultado: aqui se libera TODO el potencial, para estar al nivel de ASM, no de
> C."*

**Es correcto, y en BMO-X lo es mas que en cualquier otro sitio.** Tres razones,
y la tercera es propia de este sistema:

1. **Un programa se compila una vez y se ejecuta millones.** El tiempo de
   compilacion es el sitio barato.
2. ★★ **El analisis ya hay que pagarlo por CORRECCION, no por velocidad.** Para
   que INTI no tenga comportamiento indefinido hay que demostrar que un indice
   esta en rango, que un entero no se pasa, que un prestamo no sobrevive. **Y
   saber el rango es exactamente lo que te deja BORRAR la comprobacion.** La
   seguridad y la velocidad piden el mismo trabajo: el mecanismo 3 sale **de
   camino** del que ya era obligatorio.
3. **El `.bex` viaja firmado y no se recompila en el destino.** El tiempo se
   paga UNA vez, en la maquina de quien lo escribe, y todos los que lo ejecutan
   reciben el resultado. En un mundo con JIT esto no seria asi.

Los tres pasos, con su nombre, porque son tres cosas distintas y solo una es
optimizacion:

```text
   1. ANALISIS OBLIGATORIO   perfil, mutabilidad, las doce reglas
                             -> sin esto el lenguaje no cumple lo que promete
   2. LA IR, y lo que ENSENA  rangos, tramos de vida, lo que se puede probar
                             -> el subproducto del paso 1
   3. EMISION                 usar lo aprendido para NO emitir lo que sobra
                             -> aqui se suelta el potencial
```

#### ⚠ La acotacion, y viene del propio `PROPOSITO.md`

*"100% del potencial"* hay que decirlo con cuidado, porque **es literalmente la
promesa que hizo Itanium**: el hardware dejaria de reordenar porque *el
compilador lo haria mejor, en tiempo de compilacion, con toda la informacion del
programa delante*. Los compiladores nunca llegaron, y no por falta de dinero:
**parte de lo que hace falta solo se sabe en ejecucion** -- que rama se toma,
que cache falla.

Asi que el trato de arriba es bueno **y tiene un techo**:

- Mas tiempo de compilacion compra los tres mecanismos de 13.6. Eso es la
  distancia contra C, y se gana entera.
- **No compra lo que solo se sabe corriendo**, ni lo que el experto sabe de sus
  datos (*"este array siempre tiene 16"*).

Por eso el liston es **>= 85% contra ASM sin SIMD** y no *"el 100%"*: el numero
es una medida, no una aspiracion. Y el 15% que falta tiene su valvula escrita
desde el primer dia -- `crudo`.

★★ El corolario que hace innecesaria la carrera: **si el 3% de tu programa
necesita la instruccion exacta, lo escribes en `crudo`; el otro 97% no tiene por
que pagar por ello.** Es exactamente lo que hace C en Linux hoy: nadie escribe
un kernel entero en ensamblador porque el compilador pierda un 8%.

### 13.9 El orden, y lo que NO toca todavia

```text
   F2      emitir codigo CORRECTO, aunque sea lento
   F2.5    la IR con temporales            <- el cambio estructural
   F3      asignacion de registros         <- aqui llega el 2-4x
   F3.5    inline + borrado de comprobaciones
   F4      las decimas, medidas una a una
```

~2.500-3.500 lineas sobre lo que hay. ⚠ **y el orden no se puede cambiar**: si
se intentan los registros antes de que el lenguaje compile algo correcto, un
fallo no se sabra si es del asignador o del parser. Un compilador rapido y
equivocado no sirve para nada.

---

## 14. LO QUE NO ENTRA, con motivo

| fuera | motivo |
|---|---|
| **Herencia y clases de usuario** | MRO, metaclases, `super()`, descriptores: media complejidad de Python, y Starlark demuestra que se vive sin ello |
| **`eval` de cadenas** | rompe el AOT y contradice el gate de BEF, donde **lo que ejecuta paso el control**. Es seguridad, no carencia |
| **Monkey patching** | es lo que impide congelar, y congelar es lo que quita el GIL |
| **Hilos con memoria compartida** | es el eslabon que se rompe a proposito (sec. 4) |
| **Enteros de precision arbitraria** | `i64` + decimal de 128 bits cubre lo que se escribe |
| **Unicode completo** (locale, colacion, normalizacion) | igual que en `BRECHA.md`: una libc de verdad empieza aqui y no acaba nunca. UTF-8 pasa a traves |
| **Operadores definidos por el usuario** | dos librerias definen `<>` distinto y el lenguaje deja de leerse |
| **JIT** | pide memoria ejecutable en ejecucion; contradice el modelo de carga de BEF |
| **Un destino tipo WASM** | portabilidad **del fuente**, no del binario. Un `.bex` es nativo y firmado, y eso es una identidad del sistema |
| **Compatibilidad con Python** | ⚠ **la tentacion mas cara del documento.** El dia que se prometa correr un `.py` de internet vuelven las 15 sorpresas y el trabajo se multiplica por cuatro. *Inspirado en* no es *compatible con* |
| **Recolector de ciclos en la v1** | el contador no libera `a.b = a`. `OP_TRAVERSE` y `OP_CLEAR` **ya estan numeradas** en `dynobj::slots`: el hueco esta reservado y vacio, como debe ser |

---

## 15. LOS RIESGOS

| riesgo | senal temprana | que lo desactiva |
|---|---|---|
| ★★★ **El fantasma de ABC**: precioso y cerrado | que pase un mes sin un programa que pinte, lea el teclado y guarde | F2 y F4 son de sistema, no de REPL. **P6 no es negociable** |
| ★★ **Los dos perfiles se separan** y acaban siendo dos lenguajes | que LLANO necesite su propia sintaxis para algo | una sola gramatica; **lo unico que cambia es que biblioteca admite**, y el compilador lo comprueba |
| **"Lenguaje de juguete"** | que solo sirva para el REPL | el criterio de DOOM: un programa **real** en INTI corriendo en el Ryzen |
| **El 1% resulta ser 30%** | F3 mide y sale caro | ya esta previsto donde: comprobar en el diseno cuesta 1%, parchear despues cuesta 29,1%. Si sale caro, **es que se puso una comprobacion en el sitio equivocado** |
| **Sintaxis por gusto** | discutir palabras clave mas de un dia | manda la seccion 9: sin evidencia, se copia lo que funciona |
| **La deriva a Python** | *"esto seria facil de anadir y Python lo tiene"* | releer la seccion 14 antes de anadir |

---

## 16. UN PROGRAMA ENTERO

```text
# notas.inti -- lee notas, saca la media, la guarda.
# Es el "rainfall problem" de Soloway, que falla mas de media clase de
# universidad escrito en Python.

registro Alumno
   nombre es texto
   nota   es numero

funcion media(alumnos es lista de Alumno) devuelve numero
   si esta vacia alumnos
      devuelve 0

   cambiante suma = 0
   para cada a en alumnos
      suma = suma + a.nota          # si se pasa de la cuenta, ATRAPA.
                                    # No da la vuelta en silencio.
   devuelve suma / cuenta de alumnos

funcion principal
   cambiante alumnos = lista vacia

   repite mientras haya linea en lee()
      partes = parte linea por ","
      si cuenta de partes no es 2
         escribe("salto esta linea, no tiene dos partes:", linea)
         continua

      nota = numero(partes[1]) o si no
         escribe("'", partes[1], "' no es un numero. La salto.")
         continua

      anade Alumno(partes[0], nota) a alumnos

   m = media(alumnos)
   escribe "media:", m                    # 4.15, no 4.1499999999999995

   guarda "media.txt", texto(m) o si no
      escribe("no pude guardar:", motivo)
```

Y el mismo lenguaje, perfil LLANO, escribiendo sistema:

```text
# tecla.inti -- perfil LLANO: sin monton, sin contador, sin runtime.

perfil llano

funcion lee_tecla devuelve entero8
   repite mientras (entrada_puerto(0x64) y 1) es 0
      espera()

   crudo
      devuelve entrada_puerto(0x60)      # el unico sitio que el lenguaje
                                         # no puede comprobar, y se VE
```

Lo que hay que mirar: **`cambiante` aparece y se ve**; no hay `try` ni `except`
y **no hay error sin tratar**; la suma **atrapa** en vez de dar la vuelta; el
decimal es exacto; **`crudo` es la unica ventana sin comprobar y esta escrita en
el codigo**, no escondida; y **`guarda` no hay que instalarlo, y tampoco es
sintaxis**: es la biblioteca base, y la biblioteca base **viaja dentro**. Ver
`toolchain/lang/inti/GRAMATICA.md` sec. 17.

---

## 17. FUENTES

- **C y Unix**: [The Development of the C Language, Ritchie (HOPL II)](https://www.nokia.com/bell-labs/about/dennis-m-ritchie/chist.pdf) --
  [dl.acm.org/doi/10.1145/234286.1057834](https://dl.acm.org/doi/10.1145/234286.1057834) --
  [Origins and History of Unix (ESR)](http://www.catb.org/esr/writings/taoup/html/ch02s01.html) --
  [comentario del kernel de Unix V4 (1973)](https://github.com/unix-v4-commentary/unix-v4-source-commentary)
- **Comportamiento indefinido**: [A Guide to Undefined Behavior in C and C++ (Regehr)](https://blog.regehr.org/archives/213) --
  [Undefined Behavior in 2017 (Regehr)](https://blog.regehr.org/archives/1520) --
  [Proposal for a Friendly Dialect of C](https://blog.regehr.org/archives/1180) --
  [What Every C Programmer Should Know About UB (LLVM)](https://blog.llvm.org/2011/05/what-every-c-programmer-should-know.html) --
  [CERT: CC. Undefined Behavior (Anexo J.2)](https://wiki.sei.cmu.edu/confluence/display/c/CC.+Undefined+Behavior)
- **El complemento a dos obligatorio en C23**: [discusion y contexto](https://news.ycombinator.com/item?id=35437009)
- **Portabilidad sin UB**: [Bringing the Web up to Speed with WebAssembly (CACM)](https://people.mpi-sws.org/~rossberg/papers/Rossberg,%20Titzer,%20Haas,%20Schuff,%20Gohman,%20Wagner,%20Zakai,%20Bastien,%20Holman%20-%20Bringing%20the%20Web%20up%20to%20Speed%20with%20WebAssembly%20[CACM].pdf) --
  [WebAssembly: Portability](https://webassembly.org/docs/portability/) --
  [WebAssembly: Security](https://webassembly.org/docs/security/)
- **Lo que cuesta comprobar**: [Rust vs C, driver de red ixy](https://github.com/ixy-languages/ixy-languages/blob/master/Rust-vs-C-performance.md) --
  [coste de comprobar limites en C++](https://www.ncbi.nlm.nih.gov/pmc/articles/PMC4487316/) --
  [Integer overflow checking cost (Dan Luu)](https://danluu.com/integer-overflow/) --
  [A Formal Model of Checked C](https://arxiv.org/pdf/2201.13394)
- **Detectar en vez de definir**: [Zig: compilation modes / detectable illegal behavior](https://ziglang.org/learn/overview/)
- **Valores y contador de referencias en un lenguaje de sistema**: [Swift: Automatic Reference Counting](https://docs.swift.org/swift-book/documentation/the-swift-programming-language/automaticreferencecounting/)
- **ABC y el origen de Python**: [The Making of Python (Artima)](https://www.artima.com/articles/the-making-of-python) --
  [General Python FAQ](https://docs.python.org/3/faq/general.html)
- **Sintaxis y novatos**: [An Empirical Investigation into Programming Language Syntax (Stefik & Siebert, TOCE 2013)](https://dl.acm.org/doi/10.1145/2534973) --
  [resumen](https://neverworkintheory.org/2014/01/29/stefik-siebert-syntax.html) --
  [Quorum: evidencia](https://quorumlanguage.com/evidence.html)
- **El 73% y el lenguaje gradual**: [Hedy (Hermans)](https://hedy.org/research/Hedy_A_Gradual_Language_for_Programming_Education_2020.pdf)
- **Mensajes de error**: [On Designing Programming Error Messages for Novices (CHI 2021)](https://dl.acm.org/doi/10.1145/3411764.3445696)
- **Maquina nocional y rainfall**: [Sorva, ACM TOCE](https://dl.acm.org/doi/10.1145/2483710.2483713) --
  [Soloway's Rainfall Problem Has Become Harder](https://ieeexplore.ieee.org/document/6542249/)
- **Idioma**: [Non-Native English Speakers Learning Programming (Guo)](https://www.semanticscholar.org/paper/212e10d10dc3abe680ea9db10aae44b966992236) --
  [Becker, PPIG 2019](https://www.ppig.org/files/2019-PPIG-30th-becker.pdf) --
  [PSeInt](https://pseint.sourceforge.net/index.php?page=pseudocodigo.php)
- **GIL**: [PEP 703](https://peps.python.org/pep-0703/) --
  [free threading en 3.14](https://docs.python.org/3/howto/free-threading-python.html)
- **Congelar para paralelizar**: [Starlark: design principles](https://github.com/bazelbuild/starlark/blob/master/design.md) --
  [spec](https://github.com/bazelbuild/starlark/blob/master/spec.md)
- **Un lenguaje pequeno hecho bien**: [The Evolution of Lua (HOPL III)](https://www.lua.org/doc/hopl.pdf)
- **Representacion de valores**: [Crafting Interpreters: Optimization (NaN boxing)](https://craftinginterpreters.com/optimization.html)
- **El coste del boxing**: [Pyjion OPT-16](https://pyjion.readthedocs.io/en/latest/opt/opt-16.html)
- **Tipado gradual**: [Gradual Soundness: Lessons from Static Python](https://arxiv.org/pdf/2206.13831)

---

Ver `PYTHON_MAESTRO.md` (por que portar Python es otro trabajo, y que se hereda
de el), `toolchain/lang/PROPOSITO.md` (para que existe cada lenguaje),
`QUE_DESBLOQUEA.md` (por que la superficie manda sobre el lenguaje),
`LA_RAM.md` ("se abre, no se carga"), `EL_FUERO.md` (lo que el sistema concede y
exige) y `toolchain/forge/sem-asm/tables/bmo/README.md` (REX, la superficie que
INTI usa el primer dia).

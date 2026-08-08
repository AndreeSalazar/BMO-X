# PROPOSITO -- para que existe cada lenguaje, y por que eso decide el alcance

> La regla que este documento fija, dicha por Eddi el 2026-08-02:
>
> *"No voy a poner promesas ni implementar mas cosas porque no significa meter
> todo "por si acaso". Implementare lo que sea normal, lo que C es para que
> existe, y lo que COBOL para que existe, y lo que Ada para que existen."*

Es un criterio mejor que "implementar el estandar", y conviene decir por que.

## El problema de implementar por estandar

Un estandar es un **acuerdo entre implementadores**, no una lista de cosas
utiles. Dentro hay tres clases de cosas mezcladas sin etiqueta:

1. **Lo que el lenguaje ES.** Sin ello, el programa no se puede escribir.
2. **Lo que alguien necesito una vez** y consiguio que entrara.
3. **Promesas al optimizador**: construcciones que **no cambian lo que el
   programa hace** y solo prometen que ira mas rapido.

La tercera clase es la trampa, porque parece trabajo pendiente y no lo es.

### ★ El caso que lo zanja: Itanium

Intel y HP disenaron **IA-64** sobre una promesa concreta: el hardware dejaria
de reordenar instrucciones porque **el compilador lo haria mejor**, en tiempo
de compilacion, con toda la informacion del programa delante. La arquitectura
entera --predicacion, ranuras explicitas, especulacion-- se apoyaba en ese
supuesto.

Los compiladores nunca llegaron. No por falta de dinero ni de talento: buena
parte de lo que hace falta para programar las ranuras bien **solo se sabe en
ejecucion** (que rama se toma, que cache falla), y eso un compilador AOT no lo
puede saber. Itanium se quedo por el camino mientras x86, que reordena en el
silicio y no promete nada, siguio.

**La moraleja para este proyecto**: una construccion que promete rendimiento
sin cambiar el resultado es exactamente la clase de cosa que se puede no
implementar sin mentirle a nadie. `restrict` es una promesa. `register` es una
promesa. `inline` es una promesa. Las tres se aceptan y se tiran, y el programa
hace lo mismo.

Y el corolario incomodo: si Intel no pudo cumplir la suya con una arquitectura
entera detras, **la respuesta correcta a una promesa no es respetarla por
respeto, es medirla**.

---

## Para que existe cada uno

### C -- control sin runtime

C existe para **escribir lo que toca el hardware**: memoria con la disposicion
que tu dices, punteros, un modelo de ejecucion que se puede seguir con el dedo,
y **cero runtime debajo**. Es ensamblador portatil con tipos.

- **Entra**: todo lo que de control sobre memoria, disposicion y flujo.
- **No entra**: lo que solo promete velocidad (`restrict`, `inline`), lo que
  supone un sistema operativo grande debajo (`<signal.h>`, `<locale.h>`), y lo
  que resuelve problemas que BMO no tiene (`_Atomic`, `<threads.h>`).

Prueba de fuego: *esto me deja decir donde esta el byte?* Si si, es C.

### COBOL -- el numero que no se puede redondear

COBOL existe por **una** razon que ningun otro lenguaje resuelve igual de bien:
**decimal exacto y registros de tamano fijo**. `PICTURE` no es formato, es el
tipo. Un banco no puede permitirse que 0,10 + 0,20 no sea 0,30.

- **Entra**: `PICTURE`, edicion, `OCCURS`, ficheros secuenciales, niveles.
- **No entra**: la cola larga del estandar (`SORT`, `STRING`, COMP-3) hasta que
  un caso real la pida. Ya esta cerrado en su alcance declarado.

Prueba de fuego: *esto lo pide un cierre contable?*

### Ada -- el fallo que se caza antes de correr

Ada existe para **lo que no puede fallar**: rangos comprobados, tipos que no se
mezclan por accidente, contratos. Su Annex F trae decimal exacto -- que resulto
ser el mismo `PICTURE` de COBOL, y por eso salio barato.

- **Entra**: ZFP secuencial, Annex F, rangos y subtipos.
- **No entra**: Ravenscar y el tasking -- piden un planificador dentro del
  runtime del lenguaje, y aqui el planificador es del kernel.

Prueba de fuego: *esto convierte un fallo de ejecucion en uno de compilacion?*

### C++ -- abstraccion que no se paga

C++ existe por el **principio de coste cero**: escribir con clases,
destructores y plantillas y que el binario salga como si lo hubieras escrito a
mano. Ni mas lento ni mas grande.

Y aqui hay que ser honesto en dos direcciones a la vez:

**No es "el lenguaje de las apps".** Lo es de los navegadores, los motores de
juego, Office y Photoshop -- sistemas grandes y de rendimiento. La mayoria del
software de negocio de hoy es Java, C#, JavaScript o Python.

**Pero no se puede subestimar**: donde hace falta rendimiento y tamano, no hay
sustituto con la misma adopcion.

#### Por que C++ es mas dificil de lo que parece

★ **Y no tiene NADA que ver con los tres syscalls.** La superficie del sistema
no cambia: un `.bex` de C++ llama a lo mismo que uno de C. La dificultad esta
**dentro del lenguaje**, y son cuatro cosas concretas:

| Pieza | Por que cuesta |
|---|---|
| **Plantillas** | son un compilador dentro del compilador: hay que instanciar codigo nuevo por cada combinacion de tipos, y decidir cual gana |
| **Excepciones** | ★ la mas cara. Piden **tablas de desenrollado** y una rutina de personalidad: al lanzar hay que saber recorrer la pila hacia atras destruyendo lo que haya vivo. Es un subsistema entero |
| **Destructores / RAII** | el orden de destruccion es parte del lenguaje: cada salida de ambito --incluido un `return` en medio-- tiene que deshacer lo suyo, en orden inverso |
| **Nombres y sobrecarga** | `mangling`, resolucion por argumentos, ADL. Es donde se va el tiempo cuando el resto ya funciona |

De ahi la decision ya tomada en la hoja de ruta: **C++ acotado, sin la bola
moderna** --nada de concepts, corrutinas, modulos, ranges ni la STL entera-- y,
sobre todo, **sin excepciones ni RTTI**. No es una rareza: es lo que hace todo
el mundo que escribe C++ para sistemas empotrados (`-fno-exceptions`,
`-fno-rtti`), y por el mismo motivo.

Es el propio Stroustrup el que dio la frase que aplica: *"Remember the Vasa"* --
el barco que se hundio en el puerto por meterle todo lo que pidieron.

Prueba de fuego: *esto me deja abstraer sin pagar?* Si la respuesta pide un
runtime, no es C++ esencial: es la bola.

---

## Como se aplica esto

En [`c-gen`](../tools/c-gen/) cada elemento del censo lleva veredicto
--`ESENCIA` / `UTIL` / `DESCARTAR`-- **con motivo escrito**. De los 91 elementos
de C, 25 estan fuera.

Y la regla que hace esto sostenible:

> **`DESCARTAR` no significa nunca. Significa "no en este alcance, y este es el
> motivo". El dia que el motivo caduque, la fila cambia.**

Un descarte con motivo se puede discutir. Uno sin motivo es un agujero.

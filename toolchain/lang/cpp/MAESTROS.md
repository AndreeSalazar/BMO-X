# MAESTROS -- como lo hacen Cfront, GCC, LLVM, MSVC y EDG, y que se extrae

> Escrito a mano. No lo genera nadie: es el estudio, no el censo. El censo
> (que entra y que no) vive en [`BRECHA.md`](BRECHA.md); el contrato con BMO C
> vive en [`HERENCIA.md`](HERENCIA.md).

## Que significa "extraer" aqui

**El mecanismo, no el codigo.** Igual que `extern/gnucobol-rs` es oraculo de
validacion y no fuente a copiar, aqui se lee como resuelven un problema los que
llevan cuarenta anos resolviendolo, y se decide que se toma.

Hay ademas un motivo legal que conviene decir en voz alta: **`gcc/cp/` es
GPL**. Se lee, se aprende, no se copia. El *Itanium C++ ABI* es otra cosa --es
una **especificacion publicada**, no codigo-- y por eso es lo unico de esta
lista que se puede seguir al pie de la letra.

---

## ★ Cfront: el precedente EXACTO, y por que murio

Esto va primero porque **BMO C++ es Cfront**. Literalmente: la misma
arquitectura, tomada por el mismo motivo. Conviene saber como termino.

Cfront (Bjarne Stroustrup, Bell Labs, 1983-1993) no era un compilador: era un
**traductor de C++ a C**, y el backend era el compilador de C que ya hubiera en
la maquina. Sus decisiones son, una por una, las que BMO va a tomar:

| Cfront | Como lo bajaba |
|---|---|
| clase | `struct` |
| metodo | funcion libre con `this` como **primer parametro** |
| virtual | array de punteros a funcion; el objeto empieza por un puntero a ese array |
| herencia simple | el `struct` derivado **empieza por** el de la base |
| sobrecarga | *name encoding* -- **el mangling se invento aqui**, para poder sobrevivir a un enlazador de C que solo ve nombres |

Funciono. C++ se extendio por el mundo con esto, no con un compilador nativo.

### Y murio por tres cosas concretas

1. **Excepciones.** Para lanzar hay que recorrer la pila hacia atras sabiendo
   que objetos estan vivos en cada marco y destruirlos. Un traductor que emite
   C **no puede expresar eso**: el C generado no controla el marco de pila que
   genera el compilador de C de abajo. Los intentos con `setjmp`/`longjmp`
   eran lentos y se saltaban destructores.
2. **Plantillas.** Una instanciacion puede hacer falta en una unidad de
   traduccion y definirse en otra. Cfront lo resolvio con un **prelinker**:
   enlazar, mirar que simbolos faltan, deducir que instanciaciones generar,
   compilarlas, volver a enlazar -- y repetir hasta que converja. Los tiempos de
   compilacion se hicieron insoportables. (GCC documenta esto como *"el modelo
   Cfront"* frente al *"modelo Borland"*, y su bandera `-frepo` es el fosil.)
3. **El depurador veia el C generado**, no el C++ escrito. Poner un punto de
   ruptura en un metodo era arqueologia.

### ★ Lo que esto significa para BMO, que es el hallazgo entero

**Las tres causas de muerte de Cfront ya estan eliminadas, y ninguna por este
motivo:**

| Lo que mato a Cfront | Estado en BMO | Por que |
|---|---|---|
| Excepciones | **descartadas** con motivo escrito | ya estaba en `BRECHA.md`; aqui resulta que ademas era el bloqueante arquitectonico |
| Prelinker de plantillas | **imposible que ocurra** | BMO compila **una sola unidad de traduccion** y monomorfiza en el frontend. No hay simbolo pendiente que descubrir: si se usa, se instancia; si no, no existe |
| Depurador ciego | **no aplica** | BMO **no emite texto C**. Emite el **AST** de BMO C y lo pasa a su descenso. La linea del `.cpp` viaja en el nodo; nunca hay un fichero `.c` intermedio que confunda a nadie |

Cfront no fracaso por ser un traductor. Fracaso por traducir **a texto** un
lenguaje con **excepciones** y **plantillas entre unidades**. BMO no hace
ninguna de las tres cosas.

Ese es el permiso para decir *meses, no decadas* sin mentir.

---

## El Itanium C++ ABI: lo unico que se sigue al pie de la letra

Es la especificacion que implementan GCC, Clang e ICC -- todos menos MSVC. Es
publica y esta escrita para ser leida. De ahi salen tres formas:

### 1. La disposicion de la vtable

El puntero del objeto (`vptr`) **no apunta al principio de la tabla, apunta al
medio**. Por encima quedan el *offset-to-top* y el puntero de RTTI; por debajo,
los punteros a las funciones virtuales en orden de declaracion.

**Lo que BMO toma**: el orden por declaracion y que la base ocupe las primeras
ranuras (asi el derivado solo anade al final, y un puntero a base sirve tal
cual). **Lo que BMO tira**: la ranura de RTTI (descartado) y el *offset-to-top*
(solo hace falta con herencia multiple, descartada). Con eso el `vptr` puede
apuntar al **principio** de la tabla, que es lo que se escribiria a mano en C.

### 2. ★ Las variantes de constructor y destructor -- y cuantas necesita BMO

El ABI define **C1/C2/C3** para constructores y **D0/D1/D2** para destructores.
Esto asusta hasta que se ve por que existen:

- **C1 (completo) vs C2 (base)** difieren **solo con bases virtuales**: alguien
  tiene que decidir quien construye la base compartida del diamante.
- **D1 (completo) vs D2 (base)**: lo mismo, por el otro extremo.
- **D0 (deleting)** es D1 y ademas llamar a `operator delete` -- hace falta para
  `delete p` a traves de un puntero a base con destructor virtual.

**BMO descarto la herencia virtual.** Luego, mecanicamente:

> **C1 = C2 -> UN constructor por clase. D1 = D2 -> UN destructor.** Y D0 solo
> aparece el dia que existan `new`/`delete`, que esperan a la capability de
> memoria.

De seis variantes a **dos**, y no por recortar: por una decision que ya estaba
tomada por otro motivo. Esto es lo que se gana leyendo el ABI antes de escribir
el codigo en vez de despues.

### 3. El mangling

`int P::doble()` -> `_ZN1P5dobleEv`. Se lee: `_Z` (esto va manglado), `N...E`
(nombre anidado), `1P` (un componente de 1 letra: `P`), `5doble`, `v` (sin
parametros).

**Aqui BMO tiene una libertad que conviene usar a sabiendas**: el mangling de
Itanium existe para que objetos de compiladores distintos se enlacen. **BMO no
enlaza nada de nadie** -- no hay enlazador, no hay `.o` ajenos, no hay carga
dinamica. Luego no necesita compatibilidad, necesita las tres **propiedades**:
determinista, sin colisiones y reversible a ojo.

Y ya hay precedente dentro de casa: BMO C promueve un `static` de funcion a
global llamandola **`funcion.variable`**, porque el punto es ilegal en C y por
tanto no puede chocar. La misma idea sirve aqui. La decision concreta se toma
en el paso 4 del orden, pero la restriccion se escribe hoy: **sea cual sea, va
documentada el mismo dia que se implementa** (ver la leccion de MSVC).

---

## GCC -- donde esta el peso DE VERDAD

`gcc/cp/` es del orden de doscientas mil lineas. Lo util no es el tamano, es el
**reparto**, porque no esta donde uno espera:

| Fichero | Que hace | Tamano relativo |
|---|---|---|
| `pt.cc` | plantillas | **el mas grande, con diferencia** |
| `call.cc` | resolucion de sobrecarga | **el segundo, y sorprende** |
| `class.cc` | disposicion de objetos y construccion de vtables | mediano |
| `search.cc` | recorrer jerarquias de bases | mediano |
| `except.cc`, `rtti.cc` | excepciones y RTTI | subsistemas enteros |
| `mangle.cc` | mangling | **pequeno: es una tabla** |

**Lo que se extrae**: las vtables son baratas --una tabla-- y el mangling tambien.
Las montanas son **plantillas** y **sobrecarga**. Coincide exactamente con lo
que ya dice `PROPOSITO.md` (*"nombres y sobrecarga: es donde se va el tiempo
cuando el resto ya funciona"*), y ahora esta medido en otro sitio.

Y una consecuencia directa para el alcance ya decidido: el censo descarta
**especializacion parcial, plantillas variadicas y SFINAE**. Eso no es recortar
un poco `pt.cc` -- es quitarle justo la parte que lo hace enorme, porque las
tres piden un motor de emparejado de patrones y ordenacion parcial dentro del
compilador. Lo que queda --instanciar una plantilla con tipos conocidos-- es
sustitucion en un AST clonado.

---

## Clang / LLVM -- la prueba de que el ABI es una TABLA

Lo que hay que mirar:

- `lib/AST/ItaniumMangle.cpp` **y** `lib/AST/MicrosoftMangle.cpp`
- `lib/AST/VTableBuilder.cpp` -- construye **las dos** disposiciones
- `lib/Sema/SemaOverload.cpp` -- la montana de la sobrecarga
- `lib/CodeGen/CGClass.cpp` -- emision de constructores y destructores

★ **El mismo frontend habla dos ABIs incompatibles y se cambia con una
bandera.** Eso es *tablas y no cerebros* demostrado a escala por otro. La
lectura para BMO: el ABI de C++ (disposicion, vtable, mangling) va en **un
sitio identificable**, no repartido por el emisor. El dia que haya un segundo
objetivo, se cambia la tabla.

### El otro prestamo: la pila de limpieza

`CGClass.cpp` + `EHScopeStack`: Clang lleva una pila de *cleanups* por ambito y
la ejecuta en cada salida. Con excepciones eso se ramifica en dos caminos
(normal y de desenrollado) y se vuelve caro.

**Sin excepciones colapsa a una lista por ambito que se recorre al reves en
cada salida** -- `return`, `break`, `continue`, y el final de las llaves. Eso es
pequeno, se audita leyendo una funcion, y es exactamente la forma que BMO
necesita para RAII. Es el prestamo mas rentable de toda esta lista.

---

## MSVC -- el contraejemplo, y la leccion que mas vale

MSVC tiene **su propio ABI**: otra disposicion de vtable (una por base), el
`vtordisp` para bases virtuales, y otro mangling -- `int P::doble()` sale como
`?doble@P@@QEAAHXZ`.

Podria ser una anecdota. No lo es, porque **Microsoft nunca publico la
especificacion**. Clang tuvo que **ingenieria-inversarla** para poder interoperar,
y de ahi sale `MicrosoftMangle.cpp`. Anos de un ecosistema partido en dos por un
documento que no se escribio.

> **Leccion, y es una regla, no una observacion: el ABI de C++ de BMO se
> escribe el mismo dia que se implementa.** Disposicion de objeto, vtable,
> mangling y orden de construccion/destruccion.

Es el mismo patron que ya esta anotado en `inicializador.rs` sobre los
inicializadores designados de MSVC: *lo que un frontend deja sin terminar o sin
documentar se lo cobra el ecosistema, no el.*

---

## EDG -- lo que significa que se compre en vez de escribirse

El frontend de **Edison Design Group** lo licencian Intel, NVIDIA y hasta
Microsoft (para IntelliSense). Es decir: **practicamente nadie escribe hoy un
frontend de C++ conforme desde cero.** Los que existen --GCC, Clang, MSVC, EDG--
se cuentan con los dedos de una mano y llevan decadas cada uno.

Esto no desanima el plan: **lo justifica.** BMO no esta escribiendo un frontend
conforme, y esa es precisamente la razon por la que son meses. Un frontend
conforme tiene que soportar SFINAE, especializacion parcial, ADL completo,
`constexpr` evaluando el lenguaje entero y treinta anos de compatibilidad. BMO
descarta **34 de cada 100 elementos**, con motivo escrito uno por uno.

La comparacion honesta no es "BMO C++ contra Clang". Es "BMO C++ contra Cfront
3.0" -- que era del orden de decenas de miles de lineas, cubria **todo** el C++
de 1991, y lo hizo un punado de personas.

---

## Bjarne -- las tres frases que deciden filas

1. **Coste cero**: *no pagas por lo que no usas, y lo que usas no lo podrias
   haber escrito mejor a mano.* Es la pregunta que decide cada fila del censo.
2. **"Remember the Vasa"**: el barco que se hundio en el puerto por meterle
   todo lo que pidieron. Apunta hacia dentro de este proyecto, no hacia fuera.
3. ★ Y la que sostiene el plan entero: **C++ se diseno para poder implementarse
   como una traduccion sobre C.** Por eso `this` es un puntero y no magia, por
   eso una clase es un struct con funciones, por eso una virtual es una tabla.
   No estamos forzando el lenguaje a una forma que no tiene: **estamos usando
   la forma con la que se diseno.**

---

## La tabla de extraccion

| Pieza | Como lo hacen los maestros | Que toma BMO | Que rechaza |
|---|---|---|---|
| clase con metodos | Cfront: struct + `this` como 1er parametro | igual | -- |
| disposicion del objeto | Itanium: base primero, luego miembros, alineado natural | igual (BMO C ya calcula offsets de struct) | empaquetado de bitfields (ya decidido en C) |
| vtable | Itanium: `vptr` al medio, orden de declaracion | orden de declaracion, `vptr` al **principio** | offset-to-top y ranura de RTTI |
| ctor/dtor | Itanium: C1/C2/C3, D0/D1/D2 | **uno y uno** (D0 con `new`/`delete`) | las variantes de bases virtuales |
| orden de destruccion | Clang: pila de limpieza por ambito, al reves en cada salida | igual, **sin la rama de desenrollado** | tablas de desenrollado, rutina de personalidad |
| mangling | Itanium `_ZN1P5dobleEv`; MSVC `?doble@P@@QEAAHXZ` | las **propiedades**, no el formato ajeno | compatibilidad binaria con nadie |
| sobrecarga | GCC `call.cc`: secuencias de conversion con ranking | ranking minimo: exacto > promocion > conversion | ADL completo, plantillas en la resolucion |
| plantillas | GCC `pt.cc`; Cfront: prelinker | **monomorfizacion en el frontend**, una unidad | prelinker, repositorio, instanciacion entre unidades |
| herencia | Itanium: simple, multiple con thunks, virtual con tablas | **simple** | thunks, diamante |
| excepciones | tablas de desenrollado + personalidad | **nada** | todo |
| RTTI | tabla de tipos viva en ejecucion | **nada** | todo |

---

## Meses, no decadas: la aritmetica

Lo que hay que escribir de nuevo, y donde esta el peso real:

| Pieza | Peso | Por que |
|---|---|---|
| lexer + parser de C++ | **alto** | la gramatica es ambigua a proposito; ver abajo |
| tabla de simbolos con ambitos y clases | medio | C++ **no se puede parsear sin ella** |
| disposicion de clases y vtables | **bajo** | offsets y una tabla; BMO C ya hace ambos |
| mangling | **bajo** | es una tabla |
| insercion de ctor/dtor en salidas de ambito | medio | la pila de limpieza de Clang, sin la rama de excepciones |
| resolucion de sobrecarga | **alto** | ★ montana 1 |
| monomorfizacion de plantillas | **alto** | ★ montana 2 |
| descenso a bytes | **CERO** | se hereda de BMO C. Ver `HERENCIA.md` |

Dos montanas, ambas identificadas, ambas acotadas por el censo. Y el backend
--donde se van los anos en un compilador de verdad-- cuesta cero porque ya
existe, tiene 216 tests y esta verificado en el Ryzen.

### ★ Y una advertencia sobre el parser, que es el paso 1

Un parser de C++ **no es el de C mas clases**. Cuatro sitios donde la gramatica
se muerde la cola, y hay que decidirlos antes de escribir la primera linea:

1. **El *most vexing parse***: `T x(y);` declara una variable o declara una
   funcion que devuelve `T`? El estandar dice: **si puede ser declaracion, es
   declaracion.** Hay que implementarlo a proposito.
2. **`>>` en plantillas**: `Vector<Vector<int>>` -- hasta C++11 eso era el
   operador de desplazamiento y habia que escribir un espacio. Se arregla con
   un caso especial en el parser, no en el lexer.
3. ★ **`a<b>(c)`**: es instanciar la plantilla `a` con `b` y llamarla con `c`,
   o es `(a<b)>(c)`, dos comparaciones? **Depende de si `a` es un nombre de
   plantilla** -- es decir, de la tabla de simbolos. **C++ no se puede parsear
   sin resolver nombres a la vez.** Es el mismo problema que el *lexer hack* de
   los `typedef` en C, pero mas grande.
4. **Sentencia-declaracion vs sentencia-expresion**: `T(x);` otra vez las dos
   cosas.

Los cuatro se resuelven con **parser y tabla de simbolos hablandose**, que es
la decision de diseno mas cara de deshacer. Por eso esta escrita aqui y no se
descubre en el paso 3.

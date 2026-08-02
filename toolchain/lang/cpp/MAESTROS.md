# MAESTROS — cómo lo hacen Cfront, GCC, LLVM, MSVC y EDG, y qué se extrae

> Escrito a mano. No lo genera nadie: es el estudio, no el censo. El censo
> (qué entra y qué no) vive en [`BRECHA.md`](BRECHA.md); el contrato con BMO C
> vive en [`HERENCIA.md`](HERENCIA.md).

## Qué significa "extraer" aquí

**El mecanismo, no el código.** Igual que `extern/gnucobol-rs` es oráculo de
validación y no fuente a copiar, aquí se lee cómo resuelven un problema los que
llevan cuarenta años resolviéndolo, y se decide qué se toma.

Hay además un motivo legal que conviene decir en voz alta: **`gcc/cp/` es
GPL**. Se lee, se aprende, no se copia. El *Itanium C++ ABI* es otra cosa —es
una **especificación publicada**, no código— y por eso es lo único de esta
lista que se puede seguir al pie de la letra.

---

## ★ Cfront: el precedente EXACTO, y por qué murió

Esto va primero porque **BMO C++ es Cfront**. Literalmente: la misma
arquitectura, tomada por el mismo motivo. Conviene saber cómo terminó.

Cfront (Bjarne Stroustrup, Bell Labs, 1983–1993) no era un compilador: era un
**traductor de C++ a C**, y el backend era el compilador de C que ya hubiera en
la máquina. Sus decisiones son, una por una, las que BMO va a tomar:

| Cfront | Cómo lo bajaba |
|---|---|
| clase | `struct` |
| método | función libre con `this` como **primer parámetro** |
| virtual | array de punteros a función; el objeto empieza por un puntero a ese array |
| herencia simple | el `struct` derivado **empieza por** el de la base |
| sobrecarga | *name encoding* — **el mangling se inventó aquí**, para poder sobrevivir a un enlazador de C que sólo ve nombres |

Funcionó. C++ se extendió por el mundo con esto, no con un compilador nativo.

### Y murió por tres cosas concretas

1. **Excepciones.** Para lanzar hay que recorrer la pila hacia atrás sabiendo
   qué objetos están vivos en cada marco y destruirlos. Un traductor que emite
   C **no puede expresar eso**: el C generado no controla el marco de pila que
   genera el compilador de C de abajo. Los intentos con `setjmp`/`longjmp`
   eran lentos y se saltaban destructores.
2. **Plantillas.** Una instanciación puede hacer falta en una unidad de
   traducción y definirse en otra. Cfront lo resolvió con un **prelinker**:
   enlazar, mirar qué símbolos faltan, deducir qué instanciaciones generar,
   compilarlas, volver a enlazar — y repetir hasta que converja. Los tiempos de
   compilación se hicieron insoportables. (GCC documenta esto como *"el modelo
   Cfront"* frente al *"modelo Borland"*, y su bandera `-frepo` es el fósil.)
3. **El depurador veía el C generado**, no el C++ escrito. Poner un punto de
   ruptura en un método era arqueología.

### ★ Lo que esto significa para BMO, que es el hallazgo entero

**Las tres causas de muerte de Cfront ya están eliminadas, y ninguna por este
motivo:**

| Lo que mató a Cfront | Estado en BMO | Por qué |
|---|---|---|
| Excepciones | **descartadas** con motivo escrito | ya estaba en `BRECHA.md`; aquí resulta que además era el bloqueante arquitectónico |
| Prelinker de plantillas | **imposible que ocurra** | BMO compila **una sola unidad de traducción** y monomorfiza en el frontend. No hay símbolo pendiente que descubrir: si se usa, se instancia; si no, no existe |
| Depurador ciego | **no aplica** | BMO **no emite texto C**. Emite el **AST** de BMO C y lo pasa a su descenso. La línea del `.cpp` viaja en el nodo; nunca hay un fichero `.c` intermedio que confunda a nadie |

Cfront no fracasó por ser un traductor. Fracasó por traducir **a texto** un
lenguaje con **excepciones** y **plantillas entre unidades**. BMO no hace
ninguna de las tres cosas.

Ése es el permiso para decir *meses, no décadas* sin mentir.

---

## El Itanium C++ ABI: lo único que se sigue al pie de la letra

Es la especificación que implementan GCC, Clang e ICC — todos menos MSVC. Es
pública y está escrita para ser leída. De ahí salen tres formas:

### 1. La disposición de la vtable

El puntero del objeto (`vptr`) **no apunta al principio de la tabla, apunta al
medio**. Por encima quedan el *offset-to-top* y el puntero de RTTI; por debajo,
los punteros a las funciones virtuales en orden de declaración.

**Lo que BMO toma**: el orden por declaración y que la base ocupe las primeras
ranuras (así el derivado sólo añade al final, y un puntero a base sirve tal
cual). **Lo que BMO tira**: la ranura de RTTI (descartado) y el *offset-to-top*
(sólo hace falta con herencia múltiple, descartada). Con eso el `vptr` puede
apuntar al **principio** de la tabla, que es lo que se escribiría a mano en C.

### 2. ★ Las variantes de constructor y destructor — y cuántas necesita BMO

El ABI define **C1/C2/C3** para constructores y **D0/D1/D2** para destructores.
Esto asusta hasta que se ve por qué existen:

- **C1 (completo) vs C2 (base)** difieren **sólo con bases virtuales**: alguien
  tiene que decidir quién construye la base compartida del diamante.
- **D1 (completo) vs D2 (base)**: lo mismo, por el otro extremo.
- **D0 (deleting)** es D1 y además llamar a `operator delete` — hace falta para
  `delete p` a través de un puntero a base con destructor virtual.

**BMO descartó la herencia virtual.** Luego, mecánicamente:

> **C1 = C2 → UN constructor por clase. D1 = D2 → UN destructor.** Y D0 sólo
> aparece el día que existan `new`/`delete`, que esperan a la capability de
> memoria.

De seis variantes a **dos**, y no por recortar: por una decisión que ya estaba
tomada por otro motivo. Esto es lo que se gana leyendo el ABI antes de escribir
el código en vez de después.

### 3. El mangling

`int P::doble()` → `_ZN1P5dobleEv`. Se lee: `_Z` (esto va manglado), `N…E`
(nombre anidado), `1P` (un componente de 1 letra: `P`), `5doble`, `v` (sin
parámetros).

**Aquí BMO tiene una libertad que conviene usar a sabiendas**: el mangling de
Itanium existe para que objetos de compiladores distintos se enlacen. **BMO no
enlaza nada de nadie** — no hay enlazador, no hay `.o` ajenos, no hay carga
dinámica. Luego no necesita compatibilidad, necesita las tres **propiedades**:
determinista, sin colisiones y reversible a ojo.

Y ya hay precedente dentro de casa: BMO C promueve un `static` de función a
global llamándola **`funcion.variable`**, porque el punto es ilegal en C y por
tanto no puede chocar. La misma idea sirve aquí. La decisión concreta se toma
en el paso 4 del orden, pero la restricción se escribe hoy: **sea cual sea, va
documentada el mismo día que se implementa** (ver la lección de MSVC).

---

## GCC — dónde está el peso DE VERDAD

`gcc/cp/` es del orden de doscientas mil líneas. Lo útil no es el tamaño, es el
**reparto**, porque no está donde uno espera:

| Fichero | Qué hace | Tamaño relativo |
|---|---|---|
| `pt.cc` | plantillas | **el más grande, con diferencia** |
| `call.cc` | resolución de sobrecarga | **el segundo, y sorprende** |
| `class.cc` | disposición de objetos y construcción de vtables | mediano |
| `search.cc` | recorrer jerarquías de bases | mediano |
| `except.cc`, `rtti.cc` | excepciones y RTTI | subsistemas enteros |
| `mangle.cc` | mangling | **pequeño: es una tabla** |

**Lo que se extrae**: las vtables son baratas —una tabla— y el mangling también.
Las montañas son **plantillas** y **sobrecarga**. Coincide exactamente con lo
que ya dice `PROPOSITO.md` (*"nombres y sobrecarga: es donde se va el tiempo
cuando el resto ya funciona"*), y ahora está medido en otro sitio.

Y una consecuencia directa para el alcance ya decidido: el censo descarta
**especialización parcial, plantillas variádicas y SFINAE**. Eso no es recortar
un poco `pt.cc` — es quitarle justo la parte que lo hace enorme, porque las
tres piden un motor de emparejado de patrones y ordenación parcial dentro del
compilador. Lo que queda —instanciar una plantilla con tipos conocidos— es
sustitución en un AST clonado.

---

## Clang / LLVM — la prueba de que el ABI es una TABLA

Lo que hay que mirar:

- `lib/AST/ItaniumMangle.cpp` **y** `lib/AST/MicrosoftMangle.cpp`
- `lib/AST/VTableBuilder.cpp` — construye **las dos** disposiciones
- `lib/Sema/SemaOverload.cpp` — la montaña de la sobrecarga
- `lib/CodeGen/CGClass.cpp` — emisión de constructores y destructores

★ **El mismo frontend habla dos ABIs incompatibles y se cambia con una
bandera.** Eso es *tablas y no cerebros* demostrado a escala por otro. La
lectura para BMO: el ABI de C++ (disposición, vtable, mangling) va en **un
sitio identificable**, no repartido por el emisor. El día que haya un segundo
objetivo, se cambia la tabla.

### El otro préstamo: la pila de limpieza

`CGClass.cpp` + `EHScopeStack`: Clang lleva una pila de *cleanups* por ámbito y
la ejecuta en cada salida. Con excepciones eso se ramifica en dos caminos
(normal y de desenrollado) y se vuelve caro.

**Sin excepciones colapsa a una lista por ámbito que se recorre al revés en
cada salida** — `return`, `break`, `continue`, y el final de las llaves. Eso es
pequeño, se audita leyendo una función, y es exactamente la forma que BMO
necesita para RAII. Es el préstamo más rentable de toda esta lista.

---

## MSVC — el contraejemplo, y la lección que más vale

MSVC tiene **su propio ABI**: otra disposición de vtable (una por base), el
`vtordisp` para bases virtuales, y otro mangling — `int P::doble()` sale como
`?doble@P@@QEAAHXZ`.

Podría ser una anécdota. No lo es, porque **Microsoft nunca publicó la
especificación**. Clang tuvo que **ingeniería-inversarla** para poder interoperar,
y de ahí sale `MicrosoftMangle.cpp`. Años de un ecosistema partido en dos por un
documento que no se escribió.

> **Lección, y es una regla, no una observación: el ABI de C++ de BMO se
> escribe el mismo día que se implementa.** Disposición de objeto, vtable,
> mangling y orden de construcción/destrucción.

Es el mismo patrón que ya está anotado en `inicializador.rs` sobre los
inicializadores designados de MSVC: *lo que un frontend deja sin terminar o sin
documentar se lo cobra el ecosistema, no él.*

---

## EDG — lo que significa que se compre en vez de escribirse

El frontend de **Edison Design Group** lo licencian Intel, NVIDIA y hasta
Microsoft (para IntelliSense). Es decir: **prácticamente nadie escribe hoy un
frontend de C++ conforme desde cero.** Los que existen —GCC, Clang, MSVC, EDG—
se cuentan con los dedos de una mano y llevan décadas cada uno.

Esto no desanima el plan: **lo justifica.** BMO no está escribiendo un frontend
conforme, y ésa es precisamente la razón por la que son meses. Un frontend
conforme tiene que soportar SFINAE, especialización parcial, ADL completo,
`constexpr` evaluando el lenguaje entero y treinta años de compatibilidad. BMO
descarta **34 de cada 100 elementos**, con motivo escrito uno por uno.

La comparación honesta no es "BMO C++ contra Clang". Es "BMO C++ contra Cfront
3.0" — que era del orden de decenas de miles de líneas, cubría **todo** el C++
de 1991, y lo hizo un puñado de personas.

---

## Bjarne — las tres frases que deciden filas

1. **Coste cero**: *no pagas por lo que no usas, y lo que usas no lo podrías
   haber escrito mejor a mano.* Es la pregunta que decide cada fila del censo.
2. **"Remember the Vasa"**: el barco que se hundió en el puerto por meterle
   todo lo que pidieron. Apunta hacia dentro de este proyecto, no hacia fuera.
3. ★ Y la que sostiene el plan entero: **C++ se diseñó para poder implementarse
   como una traducción sobre C.** Por eso `this` es un puntero y no magia, por
   eso una clase es un struct con funciones, por eso una virtual es una tabla.
   No estamos forzando el lenguaje a una forma que no tiene: **estamos usando
   la forma con la que se diseñó.**

---

## La tabla de extracción

| Pieza | Cómo lo hacen los maestros | Qué toma BMO | Qué rechaza |
|---|---|---|---|
| clase con métodos | Cfront: struct + `this` como 1er parámetro | igual | — |
| disposición del objeto | Itanium: base primero, luego miembros, alineado natural | igual (BMO C ya calcula offsets de struct) | empaquetado de bitfields (ya decidido en C) |
| vtable | Itanium: `vptr` al medio, orden de declaración | orden de declaración, `vptr` al **principio** | offset-to-top y ranura de RTTI |
| ctor/dtor | Itanium: C1/C2/C3, D0/D1/D2 | **uno y uno** (D0 con `new`/`delete`) | las variantes de bases virtuales |
| orden de destrucción | Clang: pila de limpieza por ámbito, al revés en cada salida | igual, **sin la rama de desenrollado** | tablas de desenrollado, rutina de personalidad |
| mangling | Itanium `_ZN1P5dobleEv`; MSVC `?doble@P@@QEAAHXZ` | las **propiedades**, no el formato ajeno | compatibilidad binaria con nadie |
| sobrecarga | GCC `call.cc`: secuencias de conversión con ranking | ranking mínimo: exacto > promoción > conversión | ADL completo, plantillas en la resolución |
| plantillas | GCC `pt.cc`; Cfront: prelinker | **monomorfización en el frontend**, una unidad | prelinker, repositorio, instanciación entre unidades |
| herencia | Itanium: simple, múltiple con thunks, virtual con tablas | **simple** | thunks, diamante |
| excepciones | tablas de desenrollado + personalidad | **nada** | todo |
| RTTI | tabla de tipos viva en ejecución | **nada** | todo |

---

## Meses, no décadas: la aritmética

Lo que hay que escribir de nuevo, y dónde está el peso real:

| Pieza | Peso | Por qué |
|---|---|---|
| lexer + parser de C++ | **alto** | la gramática es ambigua a propósito; ver abajo |
| tabla de símbolos con ámbitos y clases | medio | C++ **no se puede parsear sin ella** |
| disposición de clases y vtables | **bajo** | offsets y una tabla; BMO C ya hace ambos |
| mangling | **bajo** | es una tabla |
| inserción de ctor/dtor en salidas de ámbito | medio | la pila de limpieza de Clang, sin la rama de excepciones |
| resolución de sobrecarga | **alto** | ★ montaña 1 |
| monomorfización de plantillas | **alto** | ★ montaña 2 |
| descenso a bytes | **CERO** | se hereda de BMO C. Ver `HERENCIA.md` |

Dos montañas, ambas identificadas, ambas acotadas por el censo. Y el backend
—donde se van los años en un compilador de verdad— cuesta cero porque ya
existe, tiene 216 tests y está verificado en el Ryzen.

### ★ Y una advertencia sobre el parser, que es el paso 1

Un parser de C++ **no es el de C más clases**. Cuatro sitios donde la gramática
se muerde la cola, y hay que decidirlos antes de escribir la primera línea:

1. **El *most vexing parse***: `T x(y);` ¿declara una variable o declara una
   función que devuelve `T`? El estándar dice: **si puede ser declaración, es
   declaración.** Hay que implementarlo a propósito.
2. **`>>` en plantillas**: `Vector<Vector<int>>` — hasta C++11 eso era el
   operador de desplazamiento y había que escribir un espacio. Se arregla con
   un caso especial en el parser, no en el lexer.
3. ★ **`a<b>(c)`**: ¿es instanciar la plantilla `a` con `b` y llamarla con `c`,
   o es `(a<b)>(c)`, dos comparaciones? **Depende de si `a` es un nombre de
   plantilla** — es decir, de la tabla de símbolos. **C++ no se puede parsear
   sin resolver nombres a la vez.** Es el mismo problema que el *lexer hack* de
   los `typedef` en C, pero más grande.
4. **Sentencia-declaración vs sentencia-expresión**: `T(x);` otra vez las dos
   cosas.

Los cuatro se resuelven con **parser y tabla de símbolos hablándose**, que es
la decisión de diseño más cara de deshacer. Por eso está escrita aquí y no se
descubre en el paso 3.

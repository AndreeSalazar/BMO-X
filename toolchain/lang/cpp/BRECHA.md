# BRECHA -- el alcance de BMO C++

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate_cpp.py`. No editar a
> mano: se regenera con `py toolchain/tools/c-gen/generate_cpp.py`.

## De donde se parte, dicho sin adornos

★ **Diagnostico corregido el 2026-08-02.** Antes aqui ponia que el frontend
"desborda la pila con una clase con dos metodos", lo que apuntaba al parser.
**No es el parser.** Medido:

```
$ bmo-cpp-front vacio.cpp        # cero bytes de entrada
thread 'main' has overflowed its stack
```

Desborda con un fichero **vacio**, con `class P{};` y con
`int main(){return 1;}`. La causa esta medida y es otra:

| | bytes |
|---|---|
| `IrStmt` | 24 |
| `IrBlock` (256 sentencias en array fijo) | 6 152 |
| `IrFunction` (32 bloques) | 198 480 |
| **`IrModule` (64 funciones)** | **12 711 184 = 12,12 MB** |

`Emitter::new()` construye **12 MB en la pila**, por valor, antes de mirar el
AST. Son arrays de tamano fijo disenados para `no_std` en Ring 0, instanciados
en una herramienta que corre en el anfitrion.

★ **Y arreglarlo no serviria de nada, que es lo grave**: `IrModule` **no tiene
un solo consumidor en todo el repo**. Nada lo convierte en bytes. En C y COBOL
`compile_to_ir` es vestigial --su camino real es `codegen::compile_to_bef_bytes`--
pero en C++ es el **unico** camino. Se ve en el manifiesto: `cpp/Cargo.toml`
depende solo de `bmo-abi`, ni de `bmo-sem-asm` ni de `bmo-lower`. **No hay
emisor porque no se enchufo ninguno.**

Estado real: **1 099 lineas** (C: 10 115), **0 tests** (C: 216), **0 ficheros
`.cpp`** en el repo, **0 bytes emitidos jamas**. El parser ademas es caracter a
caracter sin lexer, sin precedencia de operadores, y `parse_body` **se salta en
silencio** todo lo que no reconoce -- lo que viola de frente la regla de BMO:
*nada que compile y no haga lo que dice*.

Eso no es un defecto que ocultar: es el punto de partida honesto. Y explica por
que este documento **no lleva sondas** todavia, al reves que el de C -- sondear
un frontend que no emite bytes daria una tabla de "NO" sin informacion. Cuando
emita, este guion crece con sondas.

El estudio de como resuelven esto Cfront, GCC, LLVM y MSVC esta en
[`MAESTROS.md`](MAESTROS.md); el contrato con BMO C, en
[`HERENCIA.md`](HERENCIA.md).

## La pregunta que decide cada fila

> **Esto me deja abstraer SIN PAGAR?**

Es el principio de coste cero, y no es una frase bonita: es el motivo por el
que C++ existe en vez de ser "C con clases". Bjarne lo formulo asi -- *no pagas
por lo que no usas, y lo que usas no lo podrias haber escrito mejor a mano*.

★ Y corta en sitios que sorprenden: **las excepciones y los iostreams son C++
y fallan la prueba**. No por gusto, sino por lo que arrastran -- y eso esta
explicado fila a fila.


Escrito el **2026-08-02**.

## El numero

**49 elementos** en el censo de C++:

| Veredicto | Cuantos | Que significa |
|---|---|---|
| **ESENCIA** | 20 | sin esto, C++ no aporta nada sobre C |
| **UTIL** | 12 | aporta de verdad. Entra cuando toque |
| **DESCARTAR** | 17 | existe en C++ y **no entra**, con su motivo |

**34 de cada 100 elementos de C++ se quedan fuera** -- contra 27 de
cada 100 en C. La diferencia no es capricho: C++ acumulo treinta anos de
caracteristicas encima de un lenguaje que ya estaba completo.

### objetos

| Elemento | Veredicto | Motivo |
|---|---|---|
| class / struct con metodos | **ESENCIA** | es el punto de partida; sin esto C++ es C con otro nombre |
| constructor / destructor (RAII) | **ESENCIA** | ★ LA razon de existir de C++. Un recurso atado a un ambito se suelta solo, y eso en un SO sin excepciones sigue siendo la mejor idea del lenguaje |
| constructor de copia y operator= | **ESENCIA** | sin ellos, copiar un objeto con recursos es una doble liberacion esperando |
| referencias (&) | **ESENCIA** | pasar sin copiar y sin sintaxis de puntero |
| metodos const y const-correctness | **ESENCIA** | es comprobacion en compilacion: cuesta cero en ejecucion |
| this | **ESENCIA** | el puntero implicito; es como se emite un metodo |
| miembros static | **ESENCIA** | una global con el nombre dentro de la clase |
| sobrecarga de funciones | **ESENCIA** | y obliga al mangling, que es su coste real |
| sobrecarga de operadores | **ESENCIA** | `v[i]`, `a + b` sobre tipos propios. Es abstraccion que no se paga |
| herencia simple | **ESENCIA** | un objeto que empieza por su base |
| funciones virtuales (vtable) | **ESENCIA** | una tabla de punteros a funcion: exactamente lo que se escribiria a mano en C |
| clases abstractas puras (interfaces) | **ESENCIA** | el caso de las vtables que mas se usa y el mas barato |
| friend | UTIL | escotilla puntual; barata de emitir |
| enum class | UTIL | un enum que no se convierte solo a int |
| herencia MULTIPLE | ~~FUERA~~ | pide ajustar el `this` al llamar (thunks) y un objeto con varias bases a la vez. Bjarne mismo la trata como cara; casi todo el mundo usa interfaces puras en su lugar |
| herencia virtual (el diamante) | ~~FUERA~~ | peor: la base compartida se localiza en ejecucion con una tabla mas. Es el ejemplo canonico de pagar por lo que no usas |

### plantillas

| Elemento | Veredicto | Motivo |
|---|---|---|
| plantillas de funcion | **ESENCIA** | el mismo algoritmo para varios tipos, resuelto en compilacion: coste cero exacto |
| plantillas de clase | **ESENCIA** | un `Vector<T>` sin herencia ni punteros void |
| especializacion explicita | UTIL | el caso raro escrito a mano |
| especializacion PARCIAL | ~~FUERA~~ | obliga a un motor de emparejado de patrones dentro del compilador |
| plantillas variadicas | ~~FUERA~~ | recursion sobre listas de tipos; es donde empieza la metaprogramacion |
| SFINAE / enable_if | ~~FUERA~~ | programar con los ERRORES del compilador. C++20 lo sustituyo por concepts precisamente porque era insostenible |
| concepts (C++20) | ~~FUERA~~ | la bola moderna |

### memoria

| Elemento | Veredicto | Motivo |
|---|---|---|
| new / delete | **ESENCIA** | pide la capability de memoria; encima es constructor + malloc |
| new[] / delete[] | **ESENCIA** | con su cuenta de elementos delante |
| placement new | UTIL | construir en memoria ya reservada. Es lo que hace util una arena -- y DOOM es una arena |
| operator new global reemplazable | ~~FUERA~~ | un gancho global para cambiar el asignador de todo el programa. Aqui el asignador es una capability |
| unique_ptr / shared_ptr | UTIL | no son lenguaje, son clases: salen gratis cuando haya plantillas y RAII. `shared_ptr` ademas lleva contador atomico, o sea que espera a que haya hilos |

### errores

| Elemento | Veredicto | Motivo |
|---|---|---|
| excepciones (try / catch / throw) | ~~FUERA~~ | ★ la mas cara y la que mas se discute. Piden TABLAS DE DESENROLLADO y una rutina de personalidad: al lanzar hay que recorrer la pila hacia atras destruyendo lo vivo. Es un subsistema entero, esta siempre presente aunque nunca lances, y compite con el mecanismo que BMO ya tiene -- aqui un fallo mata la tarea y lo DICE. `-fno-exceptions` es lo que usa todo el mundo en sistemas empotrados, por esto mismo |
| noexcept | ~~FUERA~~ | promesa al optimizador; sin excepciones no dice nada |
| RTTI / dynamic_cast / typeid | ~~FUERA~~ | una tabla de tipos viva en ejecucion para preguntar que es algo. Si hace falta preguntarlo, el diseno ya se torcio |

### moderno

| Elemento | Veredicto | Motivo |
|---|---|---|
| auto | **ESENCIA** | quita ruido y no cuesta nada: el tipo ya se sabe |
| nullptr | **ESENCIA** | un `NULL` que no es un entero disfrazado |
| range-for | UTIL | azucar sobre begin/end; se emite como un `for` normal |
| constexpr (basico) | UTIL | calcular en compilacion es coste cero por definicion |
| lambdas sin captura o por valor | UTIL | una struct con `operator()`. La emision ya se sabe hacer |
| semantica de movimiento (&&) | ~~FUERA~~ | pide el modelo entero de categorias de valor. Sin STL grande, lo que ahorra es poco |
| corrutinas / modulos / ranges | ~~FUERA~~ | la bola moderna, y cada una es un proyecto |

### biblioteca

| Elemento | Veredicto | Motivo |
|---|---|---|
| iostreams (`std::cout`) | ~~FUERA~~ | ★ es C++ y falla la prueba. Arrastra locales, facets, virtuales por caracter y un runtime que pesa mas que muchos programas. `printf` hace lo mismo por dos ordenes de magnitud menos |
| std::string | UTIL | cuando haya plantillas y memoria; es una clase, no lenguaje |
| std::vector | UTIL | idem, y es el 90% del uso real de la STL |
| std::array | UTIL | un array con tamano; casi gratis |
| <algorithm> completo | ~~FUERA~~ | cien algoritmos genericos. Entrarian los tres que se usen, cuando se usen |
| std::thread / mutex / atomic | ~~FUERA~~ | no hay hilos de usuario |
| STL de contenedores (map, set, deque...) | ~~FUERA~~ | es un proyecto del tamano del compilador, y arrastra asignadores y excepciones |

### estructura

| Elemento | Veredicto | Motivo |
|---|---|---|
| namespaces | **ESENCIA** | y su mangling |
| name mangling | **ESENCIA** | no es opcional: en cuanto hay sobrecarga, dos funciones distintas necesitan simbolos distintos |
| inline (C++) | ~~FUERA~~ | en C++ ademas afecta al enlazado, pero con una sola unidad de traduccion no dice nada |
| extern "C" | UTIL | llamar a lo de C sin mangling. Barato y es el puente con el resto de BMO |


## Lo que esto significa en la practica

Un C++ **sin excepciones y sin RTTI** no es una rareza de este proyecto: es
exactamente lo que compila todo el mundo que escribe C++ para sistemas
empotrados (`-fno-exceptions`, `-fno-rtti`), y por el mismo motivo. La
diferencia es que aqui esta escrito por que en vez de ser una opcion heredada
del Makefile de alguien.

Y una honestidad sobre el navegador, que es la razon por la que C++ interesa:

- **Un navegador propio no necesita C++.** NetSurf --motor propio, ~200k
  lineas-- esta escrito en **C**, y C ya esta en 32 de 32 sondas.
- C++ hace falta para **portar** algo que ya existe en C++, y para escribir
  sistemas grandes sin pagar la abstraccion.

O sea que C++ no es el camino al navegador: es el camino a **escribir cosas
grandes sin que se hagan ingobernables**. Que es otra cosa, y tambien vale.

★ El censo completo de **que aplicacion desbloquea que pieza del sistema** --con
la superficie de BMO-X medida, y por que las palancas que mas desbloquean no
piden C++-- esta en [`docs/QUE_DESBLOQUEA.md`](../../../docs/QUE_DESBLOQUEA.md).

## El orden

Empieza en **0**, y el 0 no es el que estaba escrito aqui antes. "Que compile
una clase" no puede ser el primer paso de algo que no emite bytes para un
fichero vacio.

0. ✅ **HECHO -- que emita un byte.** Tirados `ir_emit.rs` y `IrModule`; la
   salida va a `bmo_c_front::ast::Program` -> `codegen::compile_to_bef_bytes`.
   El test que lo sostiene: **el BEF de C++ es byte a byte identico al de BMO
   C** para la misma fuente. Si divergen, o dejo de heredar o se combinaron.
1. ✅ **HECHO (la mitad) -- lexer y parser de verdad.** Tokens con linea real,
   la escalera completa de precedencia, ambitos anidados, y **ninguna rama que
   descarte tokens**. La decision cara quedo tomada: **el parser y la tabla de
   simbolos se hablan** -- sin eso `a<b>(c)` no se desambigua (ver
   `MAESTROS.md`), y el punto de decision ya existe aunque el conjunto de
   plantillas este vacio hasta el paso 6.
   ⏳ **Falta el preprocesador** (`#include`, `#define`): se rechaza con motivo.
2. ✅ **HECHO -- clase con metodos.** El desazucarado de Cfront: clase ->
   `struct`, metodo -> funcion libre con `this` de primer parametro
   (`P.doble(P* this)`, y el punto es ilegal en C, asi que no choca). Corren
   campos publicos y privados, metodos con argumentos, `this` explicito e
   implicito, metodos `const`, acceso por puntero (`p->x`, `p->f()`), un
   metodo llamando a otro, y un campo usado antes de declararse -- que es legal
   en C++ y obliga a parsear la clase en **dos vueltas**.
   ★ Aqui aparecio la primera ambiguedad de verdad: **`P *q` es una
   declaracion o una multiplicacion, y solo la tabla de simbolos lo sabe**. Es
   el hermano pequeno de `a<b>(c)`, y llego sin necesidad de plantillas.
3. ✅ **HECHO -- constructor y destructor (RAII)**, que es la razon de existir
   del lenguaje. Son **funciones normales con `this`**; lo unico especial es
   quien las llama y cuando. La pila de limpieza es la de Clang (`EHScopeStack`)
   **sin la rama de desenrollado**: sin excepciones colapsa a una lista por
   ambito recorrida al reves, y las salidas son cuatro y estan todas a la
   vista -- final de las llaves, `return`, `break` y `continue`.
   ★ El valor del `return` se calcula **antes** de destruir (si no, `return
   p.leer()` devolveria lo que quedara en la pila), y `break` y `continue` no
   destruyen lo mismo: un `switch` para al primero y no al segundo.
   ⏳ Esperan al paso 4: la lista de inicializacion (`P() : x(0)`), varios
   constructores, y el de copia. `new`/`delete` esperan a que haya asignador.
4. ✅ **HECHO -- mangling y sobrecarga.** Esquema propio y **no el de Itanium**:
   este existe para enlazar objetos de compiladores distintos, y BMO no enlaza
   nada de nadie. Se heredan sus **propiedades** --determinista, sin colisiones,
   reversible a ojo-- con `P.doble#i` en vez de `_ZN1P5dobleEv`.
   Corren: sobrecarga por aridad y por tipo, metodos sobrecargados,
   constructores sobrecargados con `P p(1,2)`, y el ranking de tres escalones
   (exacto > promocion > conversion) con el **empate como error**.
   ★ Y el ABI esta escrito **el mismo dia**, en [`CPP_ABI.md`](CPP_ABI.md) --
   que es la leccion de MSVC, cuyo ABI nunca se publico y Clang tuvo que
   ingenieria-inversar.
   ⏳ Fuera del alcance con motivo: ADL, conversiones definidas por el usuario
   y plantillas en la resolucion -- las tres cosas que hacen enorme a
   `gcc/cp/call.cc`.
5. ✅ **HECHO -- herencia simple, virtuales y vtable.** Un derivado empieza por
   la base ENTERA, asi que un `B*` vale como `A*` sin ajustar nada; el `vptr`
   va en el **offset 0** y la vtabla es una global de `n` ranuras que se rellena
   al principio de `main` -- las globales de BMO C solo admiten un entero como
   inicializador, y la direccion de una funcion no se sabe hasta emitir.
   ★ Un `override` **sustituye** su ranura y un virtual nuevo se **anade**: por
   eso las primeras ranuras significan lo mismo en la base y en el derivado.
   Y una llamada a metodo propio **sin `this->`** despacha igual de virtual --
   es el caso que mas se olvida.
   El ABI, actualizado el mismo dia en [`CPP_ABI.md`](CPP_ABI.md).
6. **Plantillas basicas** por monomorfizacion, que es donde C++ deja de ser C
   con azucar.

Y desde el paso 0, **una matriz de conformidad de C++** sobre `bmo_lower::emu`,
con la misma regla que la de C: *al anadir una caracteristica al codegen, se le
anade su fila*. Si no ejecuta lo que dice soportar, no lo soporta.

`new`/`delete` esperan a la **capability de memoria**, igual que `malloc`. Y
devolver objetos por valor espera al `sret`, que es deuda de **C**, no de C++.


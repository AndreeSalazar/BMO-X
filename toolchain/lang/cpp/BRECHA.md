# BRECHA — el alcance de BMO C++

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate_cpp.py`. No editar a
> mano: se regenera con `py toolchain/tools/c-gen/generate_cpp.py`.

## De dónde se parte, dicho sin adornos

★ **Diagnóstico corregido el 2026-08-02.** Antes aquí ponía que el frontend
"desborda la pila con una clase con dos métodos", lo que apuntaba al parser.
**No es el parser.** Medido:

```
$ bmo-cpp-front vacio.cpp        # cero bytes de entrada
thread 'main' has overflowed its stack
```

Desborda con un fichero **vacío**, con `class P{};` y con
`int main(){return 1;}`. La causa está medida y es otra:

| | bytes |
|---|---|
| `IrStmt` | 24 |
| `IrBlock` (256 sentencias en array fijo) | 6 152 |
| `IrFunction` (32 bloques) | 198 480 |
| **`IrModule` (64 funciones)** | **12 711 184 = 12,12 MB** |

`Emitter::new()` construye **12 MB en la pila**, por valor, antes de mirar el
AST. Son arrays de tamaño fijo diseñados para `no_std` en Ring 0, instanciados
en una herramienta que corre en el anfitrión.

★ **Y arreglarlo no serviría de nada, que es lo grave**: `IrModule` **no tiene
un solo consumidor en todo el repo**. Nada lo convierte en bytes. En C y COBOL
`compile_to_ir` es vestigial —su camino real es `codegen::compile_to_bef_bytes`—
pero en C++ es el **único** camino. Se ve en el manifiesto: `cpp/Cargo.toml`
depende sólo de `bmo-abi`, ni de `bmo-sem-asm` ni de `bmo-lower`. **No hay
emisor porque no se enchufó ninguno.**

Estado real: **1 099 líneas** (C: 10 115), **0 tests** (C: 216), **0 ficheros
`.cpp`** en el repo, **0 bytes emitidos jamás**. El parser además es carácter a
carácter sin lexer, sin precedencia de operadores, y `parse_body` **se salta en
silencio** todo lo que no reconoce — lo que viola de frente la regla de BMO:
*nada que compile y no haga lo que dice*.

Eso no es un defecto que ocultar: es el punto de partida honesto. Y explica por
qué este documento **no lleva sondas** todavía, al revés que el de C — sondear
un frontend que no emite bytes daría una tabla de "NO" sin información. Cuando
emita, este guion crece con sondas.

El estudio de cómo resuelven esto Cfront, GCC, LLVM y MSVC está en
[`MAESTROS.md`](MAESTROS.md); el contrato con BMO C, en
[`HERENCIA.md`](HERENCIA.md).

## La pregunta que decide cada fila

> **¿Esto me deja abstraer SIN PAGAR?**

Es el principio de coste cero, y no es una frase bonita: es el motivo por el
que C++ existe en vez de ser "C con clases". Bjarne lo formuló así — *no pagas
por lo que no usas, y lo que usas no lo podrías haber escrito mejor a mano*.

★ Y corta en sitios que sorprenden: **las excepciones y los iostreams son C++
y fallan la prueba**. No por gusto, sino por lo que arrastran — y eso está
explicado fila a fila.


Escrito el **2026-08-02**.

## El número

**49 elementos** en el censo de C++:

| Veredicto | Cuántos | Qué significa |
|---|---|---|
| **ESENCIA** | 20 | sin esto, C++ no aporta nada sobre C |
| **UTIL** | 12 | aporta de verdad. Entra cuando toque |
| **DESCARTAR** | 17 | existe en C++ y **no entra**, con su motivo |

**34 de cada 100 elementos de C++ se quedan fuera** — contra 27 de
cada 100 en C. La diferencia no es capricho: C++ acumuló treinta años de
características encima de un lenguaje que ya estaba completo.

### objetos

| Elemento | Veredicto | Motivo |
|---|---|---|
| class / struct con métodos | **ESENCIA** | es el punto de partida; sin esto C++ es C con otro nombre |
| constructor / destructor (RAII) | **ESENCIA** | ★ LA razón de existir de C++. Un recurso atado a un ámbito se suelta solo, y eso en un SO sin excepciones sigue siendo la mejor idea del lenguaje |
| constructor de copia y operator= | **ESENCIA** | sin ellos, copiar un objeto con recursos es una doble liberación esperando |
| referencias (&) | **ESENCIA** | pasar sin copiar y sin sintaxis de puntero |
| métodos const y const-correctness | **ESENCIA** | es comprobación en compilación: cuesta cero en ejecución |
| this | **ESENCIA** | el puntero implícito; es como se emite un método |
| miembros static | **ESENCIA** | una global con el nombre dentro de la clase |
| sobrecarga de funciones | **ESENCIA** | y obliga al mangling, que es su coste real |
| sobrecarga de operadores | **ESENCIA** | `v[i]`, `a + b` sobre tipos propios. Es abstracción que no se paga |
| herencia simple | **ESENCIA** | un objeto que empieza por su base |
| funciones virtuales (vtable) | **ESENCIA** | una tabla de punteros a función: exactamente lo que se escribiría a mano en C |
| clases abstractas puras (interfaces) | **ESENCIA** | el caso de las vtables que más se usa y el más barato |
| friend | UTIL | escotilla puntual; barata de emitir |
| enum class | UTIL | un enum que no se convierte solo a int |
| herencia MÚLTIPLE | ~~FUERA~~ | pide ajustar el `this` al llamar (thunks) y un objeto con varias bases a la vez. Bjarne mismo la trata como cara; casi todo el mundo usa interfaces puras en su lugar |
| herencia virtual (el diamante) | ~~FUERA~~ | peor: la base compartida se localiza en ejecución con una tabla más. Es el ejemplo canónico de pagar por lo que no usas |

### plantillas

| Elemento | Veredicto | Motivo |
|---|---|---|
| plantillas de función | **ESENCIA** | el mismo algoritmo para varios tipos, resuelto en compilación: coste cero exacto |
| plantillas de clase | **ESENCIA** | un `Vector<T>` sin herencia ni punteros void |
| especialización explícita | UTIL | el caso raro escrito a mano |
| especialización PARCIAL | ~~FUERA~~ | obliga a un motor de emparejado de patrones dentro del compilador |
| plantillas variádicas | ~~FUERA~~ | recursión sobre listas de tipos; es donde empieza la metaprogramación |
| SFINAE / enable_if | ~~FUERA~~ | programar con los ERRORES del compilador. C++20 lo sustituyó por concepts precisamente porque era insostenible |
| concepts (C++20) | ~~FUERA~~ | la bola moderna |

### memoria

| Elemento | Veredicto | Motivo |
|---|---|---|
| new / delete | **ESENCIA** | pide la capability de memoria; encima es constructor + malloc |
| new[] / delete[] | **ESENCIA** | con su cuenta de elementos delante |
| placement new | UTIL | construir en memoria ya reservada. Es lo que hace útil una arena — y DOOM es una arena |
| operator new global reemplazable | ~~FUERA~~ | un gancho global para cambiar el asignador de todo el programa. Aquí el asignador es una capability |
| unique_ptr / shared_ptr | UTIL | no son lenguaje, son clases: salen gratis cuando haya plantillas y RAII. `shared_ptr` además lleva contador atómico, o sea que espera a que haya hilos |

### errores

| Elemento | Veredicto | Motivo |
|---|---|---|
| excepciones (try / catch / throw) | ~~FUERA~~ | ★ la más cara y la que más se discute. Piden TABLAS DE DESENROLLADO y una rutina de personalidad: al lanzar hay que recorrer la pila hacia atrás destruyendo lo vivo. Es un subsistema entero, está siempre presente aunque nunca lances, y compite con el mecanismo que BMO ya tiene — aquí un fallo mata la tarea y lo DICE. `-fno-exceptions` es lo que usa todo el mundo en sistemas empotrados, por esto mismo |
| noexcept | ~~FUERA~~ | promesa al optimizador; sin excepciones no dice nada |
| RTTI / dynamic_cast / typeid | ~~FUERA~~ | una tabla de tipos viva en ejecución para preguntar qué es algo. Si hace falta preguntarlo, el diseño ya se torció |

### moderno

| Elemento | Veredicto | Motivo |
|---|---|---|
| auto | **ESENCIA** | quita ruido y no cuesta nada: el tipo ya se sabe |
| nullptr | **ESENCIA** | un `NULL` que no es un entero disfrazado |
| range-for | UTIL | azúcar sobre begin/end; se emite como un `for` normal |
| constexpr (básico) | UTIL | calcular en compilación es coste cero por definición |
| lambdas sin captura o por valor | UTIL | una struct con `operator()`. La emisión ya se sabe hacer |
| semántica de movimiento (&&) | ~~FUERA~~ | pide el modelo entero de categorías de valor. Sin STL grande, lo que ahorra es poco |
| corrutinas / módulos / ranges | ~~FUERA~~ | la bola moderna, y cada una es un proyecto |

### biblioteca

| Elemento | Veredicto | Motivo |
|---|---|---|
| iostreams (`std::cout`) | ~~FUERA~~ | ★ es C++ y falla la prueba. Arrastra locales, facets, virtuales por carácter y un runtime que pesa más que muchos programas. `printf` hace lo mismo por dos órdenes de magnitud menos |
| std::string | UTIL | cuando haya plantillas y memoria; es una clase, no lenguaje |
| std::vector | UTIL | ídem, y es el 90% del uso real de la STL |
| std::array | UTIL | un array con tamaño; casi gratis |
| <algorithm> completo | ~~FUERA~~ | cien algoritmos genéricos. Entrarían los tres que se usen, cuando se usen |
| std::thread / mutex / atomic | ~~FUERA~~ | no hay hilos de usuario |
| STL de contenedores (map, set, deque…) | ~~FUERA~~ | es un proyecto del tamaño del compilador, y arrastra asignadores y excepciones |

### estructura

| Elemento | Veredicto | Motivo |
|---|---|---|
| namespaces | **ESENCIA** | y su mangling |
| name mangling | **ESENCIA** | no es opcional: en cuanto hay sobrecarga, dos funciones distintas necesitan símbolos distintos |
| inline (C++) | ~~FUERA~~ | en C++ además afecta al enlazado, pero con una sola unidad de traducción no dice nada |
| extern "C" | UTIL | llamar a lo de C sin mangling. Barato y es el puente con el resto de BMO |


## Lo que esto significa en la práctica

Un C++ **sin excepciones y sin RTTI** no es una rareza de este proyecto: es
exactamente lo que compila todo el mundo que escribe C++ para sistemas
empotrados (`-fno-exceptions`, `-fno-rtti`), y por el mismo motivo. La
diferencia es que aquí está escrito por qué en vez de ser una opción heredada
del Makefile de alguien.

Y una honestidad sobre el navegador, que es la razón por la que C++ interesa:

- **Un navegador propio no necesita C++.** NetSurf —motor propio, ~200k
  líneas— está escrito en **C**, y C ya está en 32 de 32 sondas.
- C++ hace falta para **portar** algo que ya existe en C++, y para escribir
  sistemas grandes sin pagar la abstracción.

O sea que C++ no es el camino al navegador: es el camino a **escribir cosas
grandes sin que se hagan ingobernables**. Que es otra cosa, y también vale.

★ El censo completo de **qué aplicación desbloquea qué pieza del sistema** —con
la superficie de BMO-X medida, y por qué las palancas que más desbloquean no
piden C++— está en [`docs/QUE_DESBLOQUEA.md`](../../../docs/QUE_DESBLOQUEA.md).

## El orden

Empieza en **0**, y el 0 no es el que estaba escrito aquí antes. "Que compile
una clase" no puede ser el primer paso de algo que no emite bytes para un
fichero vacío.

0. ✅ **HECHO — que emita un byte.** Tirados `ir_emit.rs` y `IrModule`; la
   salida va a `bmo_c_front::ast::Program` → `codegen::compile_to_bef_bytes`.
   El test que lo sostiene: **el BEF de C++ es byte a byte idéntico al de BMO
   C** para la misma fuente. Si divergen, o dejó de heredar o se combinaron.
1. ✅ **HECHO (la mitad) — lexer y parser de verdad.** Tokens con línea real,
   la escalera completa de precedencia, ámbitos anidados, y **ninguna rama que
   descarte tokens**. La decisión cara quedó tomada: **el parser y la tabla de
   símbolos se hablan** — sin eso `a<b>(c)` no se desambigua (ver
   `MAESTROS.md`), y el punto de decisión ya existe aunque el conjunto de
   plantillas esté vacío hasta el paso 6.
   ⏳ **Falta el preprocesador** (`#include`, `#define`): se rechaza con motivo.
2. ✅ **HECHO — clase con métodos.** El desazucarado de Cfront: clase →
   `struct`, método → función libre con `this` de primer parámetro
   (`P.doble(P* this)`, y el punto es ilegal en C, así que no choca). Corren
   campos públicos y privados, métodos con argumentos, `this` explícito e
   implícito, métodos `const`, acceso por puntero (`p->x`, `p->f()`), un
   método llamando a otro, y un campo usado antes de declararse — que es legal
   en C++ y obliga a parsear la clase en **dos vueltas**.
   ★ Aquí apareció la primera ambigüedad de verdad: **`P *q` es una
   declaración o una multiplicación, y sólo la tabla de símbolos lo sabe**. Es
   el hermano pequeño de `a<b>(c)`, y llegó sin necesidad de plantillas.
3. ✅ **HECHO — constructor y destructor (RAII)**, que es la razón de existir
   del lenguaje. Son **funciones normales con `this`**; lo único especial es
   quién las llama y cuándo. La pila de limpieza es la de Clang (`EHScopeStack`)
   **sin la rama de desenrollado**: sin excepciones colapsa a una lista por
   ámbito recorrida al revés, y las salidas son cuatro y están todas a la
   vista — final de las llaves, `return`, `break` y `continue`.
   ★ El valor del `return` se calcula **antes** de destruir (si no, `return
   p.leer()` devolvería lo que quedara en la pila), y `break` y `continue` no
   destruyen lo mismo: un `switch` para al primero y no al segundo.
   ⏳ Esperan al paso 4: la lista de inicialización (`P() : x(0)`), varios
   constructores, y el de copia. `new`/`delete` esperan a que haya asignador.
4. ✅ **HECHO — mangling y sobrecarga.** Esquema propio y **no el de Itanium**:
   éste existe para enlazar objetos de compiladores distintos, y BMO no enlaza
   nada de nadie. Se heredan sus **propiedades** —determinista, sin colisiones,
   reversible a ojo— con `P.doble#i` en vez de `_ZN1P5dobleEv`.
   Corren: sobrecarga por aridad y por tipo, métodos sobrecargados,
   constructores sobrecargados con `P p(1,2)`, y el ranking de tres escalones
   (exacto > promoción > conversión) con el **empate como error**.
   ★ Y el ABI está escrito **el mismo día**, en [`CPP_ABI.md`](CPP_ABI.md) —
   que es la lección de MSVC, cuyo ABI nunca se publicó y Clang tuvo que
   ingeniería-inversar.
   ⏳ Fuera del alcance con motivo: ADL, conversiones definidas por el usuario
   y plantillas en la resolución — las tres cosas que hacen enorme a
   `gcc/cp/call.cc`.
5. ✅ **HECHO — herencia simple, virtuales y vtable.** Un derivado empieza por
   la base ENTERA, así que un `B*` vale como `A*` sin ajustar nada; el `vptr`
   va en el **offset 0** y la vtabla es una global de `n` ranuras que se rellena
   al principio de `main` — las globales de BMO C sólo admiten un entero como
   inicializador, y la dirección de una función no se sabe hasta emitir.
   ★ Un `override` **sustituye** su ranura y un virtual nuevo se **añade**: por
   eso las primeras ranuras significan lo mismo en la base y en el derivado.
   Y una llamada a método propio **sin `this->`** despacha igual de virtual —
   es el caso que más se olvida.
   El ABI, actualizado el mismo día en [`CPP_ABI.md`](CPP_ABI.md).
6. **Plantillas básicas** por monomorfización, que es donde C++ deja de ser C
   con azúcar.

Y desde el paso 0, **una matriz de conformidad de C++** sobre `bmo_lower::emu`,
con la misma regla que la de C: *al añadir una característica al codegen, se le
añade su fila*. Si no ejecuta lo que dice soportar, no lo soporta.

`new`/`delete` esperan a la **capability de memoria**, igual que `malloc`. Y
devolver objetos por valor espera al `sret`, que es deuda de **C**, no de C++.


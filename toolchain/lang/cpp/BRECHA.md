# BRECHA — el alcance de BMO C++

> **AUTO-GENERADO** por `toolchain/tools/c-gen/generate_cpp.py`. No editar a
> mano: se regenera con `py toolchain/tools/c-gen/generate_cpp.py`.

## De dónde se parte, dicho sin adornos

El frontend de C++ de hoy son **~900 líneas** y **desborda la pila** con esto:

```cpp
class P { public: int x; int doble() { return x * 2; } };
int main() { P p; p.x = 21; return p.doble(); }
```

Eso no es un defecto que ocultar: es el punto de partida. Y explica por qué
este documento **no lleva sondas** todavía, al revés que el de C — sondear un
frontend que no compila una clase daría una tabla de "NO" sin información.
Cuando aguante una clase, este guion crece con sondas.

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

## El orden si algún día se retoma

1. **Que compile una clase con métodos** — hoy desborda la pila. Todo lo demás
   depende de esto.
2. **Constructor y destructor (RAII)**, que es la razón de existir del lenguaje.
3. **Mangling**, en cuanto haya sobrecarga: dos funciones distintas necesitan
   símbolos distintos y no hay forma de esquivarlo.
4. **Virtuales y vtable** — el AST ya las conoce.
5. **Plantillas básicas**, que es donde C++ deja de ser C con azúcar.

`new`/`delete` esperan a la **capability de memoria**, igual que `malloc`.


"""**El censo de C++** — qué entra, qué no, y por qué.

La pregunta que decide cada fila es la de `toolchain/lang/PROPOSITO.md`:

> **¿Esto me deja abstraer SIN PAGAR?**

Ése es el principio de coste cero, y no es una frase bonita: es el motivo por
el que C++ existe en vez de ser C con clases. Bjarne lo formuló así — *"no
pagas por lo que no usas, y lo que usas no lo podrías haber escrito mejor a
mano"*. Todo lo que falla esa prueba está fuera, y el motivo se escribe.

★ Y la prueba corta en sitios que sorprenden: **las excepciones y los
iostreams son C++ y fallan la prueba**. No por gusto — por lo que arrastran.

Veredictos:

    ESENCIA     sin esto, C++ no aporta nada sobre C. Entra.
    UTIL        aporta de verdad. Entra cuando toque.
    DESCARTAR   existe en C++ y NO entra, con el motivo escrito.
"""

# (categoria, elemento, veredicto, motivo)
CENSO_CPP = [
    # ── Objetos: la razón de existir ────────────────────────────────
    ("objetos", "class / struct con métodos", "ESENCIA",
     "es el punto de partida; sin esto C++ es C con otro nombre"),
    ("objetos", "constructor / destructor (RAII)", "ESENCIA",
     "★ LA razón de existir de C++. Un recurso atado a un ámbito se suelta solo, "
     "y eso en un SO sin excepciones sigue siendo la mejor idea del lenguaje"),
    ("objetos", "constructor de copia y operator=", "ESENCIA",
     "sin ellos, copiar un objeto con recursos es una doble liberación esperando"),
    ("objetos", "referencias (&)", "ESENCIA", "pasar sin copiar y sin sintaxis de puntero"),
    ("objetos", "métodos const y const-correctness", "ESENCIA",
     "es comprobación en compilación: cuesta cero en ejecución"),
    ("objetos", "this", "ESENCIA", "el puntero implícito; es como se emite un método"),
    ("objetos", "miembros static", "ESENCIA", "una global con el nombre dentro de la clase"),
    ("objetos", "sobrecarga de funciones", "ESENCIA",
     "y obliga al mangling, que es su coste real"),
    ("objetos", "sobrecarga de operadores", "ESENCIA",
     "`v[i]`, `a + b` sobre tipos propios. Es abstracción que no se paga"),
    ("objetos", "herencia simple", "ESENCIA", "un objeto que empieza por su base"),
    ("objetos", "funciones virtuales (vtable)", "ESENCIA",
     "una tabla de punteros a función: exactamente lo que se escribiría a mano en C"),
    ("objetos", "clases abstractas puras (interfaces)", "ESENCIA",
     "el caso de las vtables que más se usa y el más barato"),
    ("objetos", "friend", "UTIL", "escotilla puntual; barata de emitir"),
    ("objetos", "enum class", "UTIL", "un enum que no se convierte solo a int"),
    ("objetos", "herencia MÚLTIPLE", "DESCARTAR",
     "pide ajustar el `this` al llamar (thunks) y un objeto con varias bases a la vez. "
     "Bjarne mismo la trata como cara; casi todo el mundo usa interfaces puras en su lugar"),
    ("objetos", "herencia virtual (el diamante)", "DESCARTAR",
     "peor: la base compartida se localiza en ejecución con una tabla más. "
     "Es el ejemplo canónico de pagar por lo que no usas"),

    # ── Plantillas ──────────────────────────────────────────────────
    ("plantillas", "plantillas de función", "ESENCIA",
     "el mismo algoritmo para varios tipos, resuelto en compilación: coste cero exacto"),
    ("plantillas", "plantillas de clase", "ESENCIA", "un `Vector<T>` sin herencia ni punteros void"),
    ("plantillas", "especialización explícita", "UTIL", "el caso raro escrito a mano"),
    ("plantillas", "especialización PARCIAL", "DESCARTAR",
     "obliga a un motor de emparejado de patrones dentro del compilador"),
    ("plantillas", "plantillas variádicas", "DESCARTAR",
     "recursión sobre listas de tipos; es donde empieza la metaprogramación"),
    ("plantillas", "SFINAE / enable_if", "DESCARTAR",
     "programar con los ERRORES del compilador. C++20 lo sustituyó por concepts "
     "precisamente porque era insostenible"),
    ("plantillas", "concepts (C++20)", "DESCARTAR", "la bola moderna"),

    # ── Memoria ─────────────────────────────────────────────────────
    ("memoria", "new / delete", "ESENCIA",
     "pide la capability de memoria; encima es constructor + malloc"),
    ("memoria", "new[] / delete[]", "ESENCIA", "con su cuenta de elementos delante"),
    ("memoria", "placement new", "UTIL",
     "construir en memoria ya reservada. Es lo que hace útil una arena — y DOOM es una arena"),
    ("memoria", "operator new global reemplazable", "DESCARTAR",
     "un gancho global para cambiar el asignador de todo el programa. Aquí el asignador es una capability"),
    ("memoria", "unique_ptr / shared_ptr", "UTIL",
     "no son lenguaje, son clases: salen gratis cuando haya plantillas y RAII. "
     "`shared_ptr` además lleva contador atómico, o sea que espera a que haya hilos"),

    # ── Errores ─────────────────────────────────────────────────────
    ("errores", "excepciones (try / catch / throw)", "DESCARTAR",
     "★ la más cara y la que más se discute. Piden TABLAS DE DESENROLLADO y una rutina "
     "de personalidad: al lanzar hay que recorrer la pila hacia atrás destruyendo lo vivo. "
     "Es un subsistema entero, está siempre presente aunque nunca lances, y compite con el "
     "mecanismo que BMO ya tiene — aquí un fallo mata la tarea y lo DICE. "
     "`-fno-exceptions` es lo que usa todo el mundo en sistemas empotrados, por esto mismo"),
    ("errores", "noexcept", "DESCARTAR", "promesa al optimizador; sin excepciones no dice nada"),
    ("errores", "RTTI / dynamic_cast / typeid", "DESCARTAR",
     "una tabla de tipos viva en ejecución para preguntar qué es algo. "
     "Si hace falta preguntarlo, el diseño ya se torció"),

    # ── Lo moderno que SÍ vale ──────────────────────────────────────
    ("moderno", "auto", "ESENCIA", "quita ruido y no cuesta nada: el tipo ya se sabe"),
    ("moderno", "nullptr", "ESENCIA", "un `NULL` que no es un entero disfrazado"),
    ("moderno", "range-for", "UTIL", "azúcar sobre begin/end; se emite como un `for` normal"),
    ("moderno", "constexpr (básico)", "UTIL", "calcular en compilación es coste cero por definición"),
    ("moderno", "lambdas sin captura o por valor", "UTIL",
     "una struct con `operator()`. La emisión ya se sabe hacer"),
    ("moderno", "semántica de movimiento (&&)", "DESCARTAR",
     "pide el modelo entero de categorías de valor. Sin STL grande, lo que ahorra es poco"),
    ("moderno", "corrutinas / módulos / ranges", "DESCARTAR", "la bola moderna, y cada una es un proyecto"),

    # ── Biblioteca ──────────────────────────────────────────────────
    ("biblioteca", "iostreams (`std::cout`)", "DESCARTAR",
     "★ es C++ y falla la prueba. Arrastra locales, facets, virtuales por carácter y un "
     "runtime que pesa más que muchos programas. `printf` hace lo mismo por dos órdenes de magnitud menos"),
    ("biblioteca", "std::string", "UTIL", "cuando haya plantillas y memoria; es una clase, no lenguaje"),
    ("biblioteca", "std::vector", "UTIL", "ídem, y es el 90% del uso real de la STL"),
    ("biblioteca", "std::array", "UTIL", "un array con tamaño; casi gratis"),
    ("biblioteca", "<algorithm> completo", "DESCARTAR",
     "cien algoritmos genéricos. Entrarían los tres que se usen, cuando se usen"),
    ("biblioteca", "std::thread / mutex / atomic", "DESCARTAR", "no hay hilos de usuario"),
    ("biblioteca", "STL de contenedores (map, set, deque…)", "DESCARTAR",
     "es un proyecto del tamaño del compilador, y arrastra asignadores y excepciones"),

    # ── Estructura ──────────────────────────────────────────────────
    ("estructura", "namespaces", "ESENCIA", "y su mangling"),
    ("estructura", "name mangling", "ESENCIA",
     "no es opcional: en cuanto hay sobrecarga, dos funciones distintas necesitan símbolos distintos"),
    ("estructura", "inline (C++)", "DESCARTAR",
     "en C++ además afecta al enlazado, pero con una sola unidad de traducción no dice nada"),
    ("estructura", "extern \"C\"", "UTIL",
     "llamar a lo de C sin mangling. Barato y es el puente con el resto de BMO"),
]


def por_veredicto(v):
    return [f for f in CENSO_CPP if f[2] == v]


def categorias():
    vistas = []
    for c, *_ in CENSO_CPP:
        if c not in vistas:
            vistas.append(c)
    return vistas

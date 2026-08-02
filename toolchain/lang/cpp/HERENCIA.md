# HERENCIA — qué toma BMO C++ de BMO C, y la línea que no se cruza

> La regla que este documento fija, dicha por Eddi el 2026-08-02:
>
> *"Vamos a tomar el BMO C que herede para que BMO C++ pueda enfocar en lo que
> ya importa para funcionar, pero ojo: **no se combinan**."*

Las dos mitades de esa frase tiran en direcciones opuestas y las dos son
correctas. Este documento es dónde se cortan.

---

## La frase que lo resuelve

> **BMO C++ hereda el DESCENSO de BMO C. No hereda su frontend.**

- **Descenso** = del AST a los bytes del BEF. Eso se comparte.
- **Frontend** = lexer, parser, preprocesador, la sintaxis de C. Eso **no se
  toca, no se extiende y no se comparte.** C++ escribe el suyo entero.

Y el punto exacto donde se dan la mano es **un tipo de datos**, no una función
que decide cosas:

```
fuente .cpp
    │
    ▼  [ lexer, parser, tabla de símbolos, clases,        ]
       [ mangling, vtables, plantillas, ctor/dtor  → CPP  ]
    │
    ▼
bmo_c_front::ast::Program        ◄── LA FRONTERA. Es un formato.
    │
    ▼  [ codegen::compile_to_bef_bytes  → C, sin enterarse ]
    │
    ▼
bytes del BEF
```

*Contratos y formatos, nunca cerebros.* `Program` es un formato: structs,
funciones, sentencias y expresiones con offsets ya resueltos. Exactamente el
mismo papel que juega `Vec<Escritura>` entre el parser y el codegen de C, o
`Plantilla` entre el PICTURE de COBOL y su emisor.

**El codegen de C nunca sabrá que existe una clase.** Recibe funciones libres
con nombres raros y punteros a función en tablas. Eso ya lo sabe emitir.

---

## Lo que se hereda (la lista completa, y es corta)

```rust
use bmo_c_front::ast::{Program, Function, TypeSpec, Param, Expr, Stmt, Escritura,
                       GlobalDecl, StructMember};
use bmo_c_front::codegen::compile_to_bef_bytes;
```

Nada más. Ni el parser (`pub(crate)`, y así se queda), ni el lexer (privado),
ni el preprocesador, ni `standard.rs`, ni `module.rs`.

Con eso, todo lo que el censo marca **ESENCIA** tiene dónde aterrizar:

| C++ ESENCIA | Baja a | Nodo de BMO C que ya existe |
|---|---|---|
| clase con métodos | struct + función con `this` de 1er parámetro | `GlobalDecl::Struct`, `Function` |
| miembros | campos con offset | `Expr::Field` / `Arrow` (llevan tipo y offset) |
| `this` | un parámetro más | `Param` de tipo `Ptr(StructRef)` |
| ctor / dtor (RAII) | llamadas insertadas en entrada y en **cada** salida | `Stmt::Expr(Expr::Call)` |
| referencias `&` | puntero, con la indirección puesta por el frontend | `AddrOf` / `Deref` |
| métodos `const` | comprobación en el frontend; **cero** en la emisión | — |
| miembros `static` | global con nombre compuesto | `GlobalDecl::Var` — el precedente es `funcion.variable` |
| sobrecarga | nombres distintos tras el mangling | `Function.name` |
| sobrecarga de operadores | azúcar a llamada | `Expr::Call` |
| herencia simple | el struct derivado empieza por el de la base | `GlobalDecl::Struct` |
| virtuales / vtable | tabla de punteros a función + `vptr` en el offset 0 | `Expr::CallPtr`, punteros a función |
| clases abstractas puras | vtable con la ranura obligatoria | ídem |
| namespaces | mangling | `Function.name` |
| `auto` | inferencia en el frontend | — |
| `nullptr` | `0` | `Expr::Int(0)` |
| plantillas | **monomorfización**: un `Function` por instancia | `Program.functions` |

**Cero nodos nuevos.** Esa es la comprobación de que la frontera está bien
puesta: si un elemento ESENCIA no cabe en el `Program` que ya existe, es que
había que pensarlo más.

---

## Las cuatro reglas de "no se combinan"

### 1. La flecha apunta en un solo sentido, y hay una prueba

`toolchain/lang/cpp/Cargo.toml` depende de `bmo-c-front`.
`toolchain/lang/c/Cargo.toml` **no menciona C++ jamás.**

> **Prueba:** la suite de BMO C tiene que pasar entera con el crate de C++
> sacado del workspace. Si algún día no pasa, se combinaron.

### 2. C no aprende qué es una clase

Ni una variante nueva en `ast::Expr` o `ast::Stmt` que sólo use C++. Si C++
necesita expresar algo, lo **desazucara** a los nodos que ya hay — que es su
trabajo, no el de C.

Es la misma decisión que ya está tomada dentro de C con los inicializadores: el
codegen no sabe que existe `.x = 1`, recibe *(offset, tipo, valor)*. Aquí el
codegen no sabe que existe `p.doble()`, recibe una llamada con un puntero.

### 3. Lo que a C le falta entra **COMO C**, o no entra

Éste es el punto por donde se cuela la contaminación, así que va con criterio
explícito:

> Si lo que C++ necesita **es C**, entra en C con su test de C y su fila en la
> matriz de conformidad de C. Si **no es C**, no entra nunca: C++ lo desazucara
> o está en `DESCARTAR`.

| Lo que C++ va a pedir | ¿Es C? | Veredicto |
|---|---|---|
| **devolver structs por valor (`sret`)** | **sí** — C89 lo tiene, y BMO C lo rechaza con motivo | entra **como C**, y le toca su fila en la matriz de C |
| `bool` | sí (C99 `_Bool`) | entra como C si falta |
| thunks de ajuste de `this` | no | nunca. Además la herencia múltiple está descartada |
| tablas de desenrollado | no | nunca |
| tabla de tipos en ejecución (RTTI) | no | nunca |
| mangling con `::` | no | **no toca a C**: el mangling produce un nombre que ya es legal, y quien lo genera es C++ |

El `sret` es el único hueco real hoy, y conviene decirlo antes de empezar:
RAII lo esquiva casi siempre (un constructor es `void ctor(T* this)`), pero
`operator+` devolviendo un valor lo pide de frente. **Es deuda de C, no de
C++** — estaba en su lista de "falta" antes de esta conversación.

### 4. Los tests no se mezclan

Los 216 de C siguen siendo de C. C++ tiene **su propia matriz de conformidad**
sobre el mismo emulador (`bmo_lower::emu`), con la misma regla: *al añadir una
característica al codegen, se le añade su fila*. Una matriz de C++ en verde con
la de C en rojo es información; las dos revueltas en un fichero no son nada.

---

## Por qué no se mueve el descenso a `forge/` (todavía)

La pregunta es legítima: `toolchain/README.md` dice que `lang/` es privado y
que lo compartido vive en `forge/`. Lo limpio "de libro" sería sacar
`codegen/` de C a `forge/` y que los dos lo enlacen.

**No ahora, y el motivo es el mismo que está escrito en la evaluación de
"BMO C 2.0":** mover 2 412 líneas de codegen que hoy tienen 216 tests en verde
y comportamiento confirmado en el Ryzen, para arreglar un problema que aún no
ha dolido, es el segundo sistema de Brooks. *Remember the Vasa*, apuntando hacia
dentro.

**Cuándo sí**, escrito por adelantado para que sea una condición y no una
opinión:

> El descenso se muda a `forge/` cuando C++ tenga su matriz en verde **y** haya
> aparecido un cambio que C++ necesita y a C no le sirve. Ese día hay dos
> consumidores de verdad y la mudanza se hace con las dos matrices como red.

Mientras tanto la dependencia se declara con un comentario en el `Cargo.toml`
que diga esto mismo, igual que C declara por qué elige `sem-asm` y `bmo-lower`.

---

## Lo que esto le ahorra a C++, en números

| | BMO C | Lo que C++ tendría que escribir sin heredar |
|---|---|---|
| codegen a x86-64 | 2 412 líneas | 2 412 |
| ABI de agregados | 165 | 165 |
| entrada (`getchar`/`scanf`) | 199 | 199 |
| `printf` en línea, memoria, la puerta | en `bmo-lower` | otra vez |
| tests que lo respaldan | **216** | 0 |
| verificado en hardware real | sí | no |

C++ empieza con el backend **hecho y probado**, y puede gastar el cien por cien
del esfuerzo en las dos montañas que `MAESTROS.md` identifica: **resolución de
sobrecarga** y **monomorfización de plantillas**.

Eso es lo que quiere decir "que se enfoque en lo que ya importa para funcionar".

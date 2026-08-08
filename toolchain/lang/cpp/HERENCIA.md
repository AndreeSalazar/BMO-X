# HERENCIA -- que toma BMO C++ de BMO C, y la linea que no se cruza

> La regla que este documento fija, dicha por Eddi el 2026-08-02:
>
> *"Vamos a tomar el BMO C que herede para que BMO C++ pueda enfocar en lo que
> ya importa para funcionar, pero ojo: **no se combinan**."*

Las dos mitades de esa frase tiran en direcciones opuestas y las dos son
correctas. Este documento es donde se cortan.

---

## La frase que lo resuelve

> **BMO C++ hereda el DESCENSO de BMO C. No hereda su frontend.**

- **Descenso** = del AST a los bytes del BEF. Eso se comparte.
- **Frontend** = lexer, parser, preprocesador, la sintaxis de C. Eso **no se
  toca, no se extiende y no se comparte.** C++ escribe el suyo entero.

Y el punto exacto donde se dan la mano es **un tipo de datos**, no una funcion
que decide cosas:

```
fuente .cpp
    |
    ▼  [ lexer, parser, tabla de simbolos, clases,        ]
       [ mangling, vtables, plantillas, ctor/dtor  -> CPP  ]
    |
    ▼
bmo_c_front::ast::Program        ◄-- LA FRONTERA. Es un formato.
    |
    ▼  [ codegen::compile_to_bef_bytes  -> C, sin enterarse ]
    |
    ▼
bytes del BEF
```

*Contratos y formatos, nunca cerebros.* `Program` es un formato: structs,
funciones, sentencias y expresiones con offsets ya resueltos. Exactamente el
mismo papel que juega `Vec<Escritura>` entre el parser y el codegen de C, o
`Plantilla` entre el PICTURE de COBOL y su emisor.

**El codegen de C nunca sabra que existe una clase.** Recibe funciones libres
con nombres raros y punteros a funcion en tablas. Eso ya lo sabe emitir.

---

## Lo que se hereda (la lista completa, y es corta)

```rust
use bmo_c_front::ast::{Program, Function, TypeSpec, Param, Expr, Stmt, Escritura,
                       GlobalDecl, StructMember};
use bmo_c_front::codegen::compile_to_bef_bytes;
```

Nada mas. Ni el parser (`pub(crate)`, y asi se queda), ni el lexer (privado),
ni el preprocesador, ni `standard.rs`, ni `module.rs`.

Con eso, todo lo que el censo marca **ESENCIA** tiene donde aterrizar:

| C++ ESENCIA | Baja a | Nodo de BMO C que ya existe |
|---|---|---|
| clase con metodos | struct + funcion con `this` de 1er parametro | `GlobalDecl::Struct`, `Function` |
| miembros | campos con offset | `Expr::Field` / `Arrow` (llevan tipo y offset) |
| `this` | un parametro mas | `Param` de tipo `Ptr(StructRef)` |
| ctor / dtor (RAII) | llamadas insertadas en entrada y en **cada** salida | `Stmt::Expr(Expr::Call)` |
| referencias `&` | puntero, con la indireccion puesta por el frontend | `AddrOf` / `Deref` |
| metodos `const` | comprobacion en el frontend; **cero** en la emision | -- |
| miembros `static` | global con nombre compuesto | `GlobalDecl::Var` -- el precedente es `funcion.variable` |
| sobrecarga | nombres distintos tras el mangling | `Function.name` |
| sobrecarga de operadores | azucar a llamada | `Expr::Call` |
| herencia simple | el struct derivado empieza por el de la base | `GlobalDecl::Struct` |
| virtuales / vtable | tabla de punteros a funcion + `vptr` en el offset 0 | `Expr::CallPtr`, punteros a funcion |
| clases abstractas puras | vtable con la ranura obligatoria | idem |
| namespaces | mangling | `Function.name` |
| `auto` | inferencia en el frontend | -- |
| `nullptr` | `0` | `Expr::Int(0)` |
| plantillas | **monomorfizacion**: un `Function` por instancia | `Program.functions` |

**Cero nodos nuevos.** Esa es la comprobacion de que la frontera esta bien
puesta: si un elemento ESENCIA no cabe en el `Program` que ya existe, es que
habia que pensarlo mas.

---

## Las cuatro reglas de "no se combinan"

### 1. La flecha apunta en un solo sentido, y hay una prueba

`toolchain/lang/cpp/Cargo.toml` depende de `bmo-c-front`.
`toolchain/lang/c/Cargo.toml` **no menciona C++ jamas.**

> **Prueba:** la suite de BMO C tiene que pasar entera con el crate de C++
> sacado del workspace. Si algun dia no pasa, se combinaron.

### 2. C no aprende que es una clase

Ni una variante nueva en `ast::Expr` o `ast::Stmt` que solo use C++. Si C++
necesita expresar algo, lo **desazucara** a los nodos que ya hay -- que es su
trabajo, no el de C.

Es la misma decision que ya esta tomada dentro de C con los inicializadores: el
codegen no sabe que existe `.x = 1`, recibe *(offset, tipo, valor)*. Aqui el
codegen no sabe que existe `p.doble()`, recibe una llamada con un puntero.

### 3. Lo que a C le falta entra **COMO C**, o no entra

Este es el punto por donde se cuela la contaminacion, asi que va con criterio
explicito:

> Si lo que C++ necesita **es C**, entra en C con su test de C y su fila en la
> matriz de conformidad de C. Si **no es C**, no entra nunca: C++ lo desazucara
> o esta en `DESCARTAR`.

| Lo que C++ va a pedir | Es C? | Veredicto |
|---|---|---|
| **devolver structs por valor (`sret`)** | **si** -- C89 lo tiene, y BMO C lo rechaza con motivo | entra **como C**, y le toca su fila en la matriz de C |
| `bool` | si (C99 `_Bool`) | entra como C si falta |
| thunks de ajuste de `this` | no | nunca. Ademas la herencia multiple esta descartada |
| tablas de desenrollado | no | nunca |
| tabla de tipos en ejecucion (RTTI) | no | nunca |
| mangling con `::` | no | **no toca a C**: el mangling produce un nombre que ya es legal, y quien lo genera es C++ |

El `sret` es el unico hueco real hoy, y conviene decirlo antes de empezar:
RAII lo esquiva casi siempre (un constructor es `void ctor(T* this)`), pero
`operator+` devolviendo un valor lo pide de frente. **Es deuda de C, no de
C++** -- estaba en su lista de "falta" antes de esta conversacion.

### 4. Los tests no se mezclan

Los 216 de C siguen siendo de C. C++ tiene **su propia matriz de conformidad**
sobre el mismo emulador (`bmo_lower::emu`), con la misma regla: *al anadir una
caracteristica al codegen, se le anade su fila*. Una matriz de C++ en verde con
la de C en rojo es informacion; las dos revueltas en un fichero no son nada.

---

## Por que no se mueve el descenso a `forge/` (todavia)

La pregunta es legitima: `toolchain/README.md` dice que `lang/` es privado y
que lo compartido vive en `forge/`. Lo limpio "de libro" seria sacar
`codegen/` de C a `forge/` y que los dos lo enlacen.

**No ahora, y el motivo es el mismo que esta escrito en la evaluacion de
"BMO C 2.0":** mover 2 412 lineas de codegen que hoy tienen 216 tests en verde
y comportamiento confirmado en el Ryzen, para arreglar un problema que aun no
ha dolido, es el segundo sistema de Brooks. *Remember the Vasa*, apuntando hacia
dentro.

**Cuando si**, escrito por adelantado para que sea una condicion y no una
opinion:

> El descenso se muda a `forge/` cuando C++ tenga su matriz en verde **y** haya
> aparecido un cambio que C++ necesita y a C no le sirve. Ese dia hay dos
> consumidores de verdad y la mudanza se hace con las dos matrices como red.

Mientras tanto la dependencia se declara con un comentario en el `Cargo.toml`
que diga esto mismo, igual que C declara por que elige `sem-asm` y `bmo-lower`.

---

## Lo que esto le ahorra a C++, en numeros

| | BMO C | Lo que C++ tendria que escribir sin heredar |
|---|---|---|
| codegen a x86-64 | 2 412 lineas | 2 412 |
| ABI de agregados | 165 | 165 |
| entrada (`getchar`/`scanf`) | 199 | 199 |
| `printf` en linea, memoria, la puerta | en `bmo-lower` | otra vez |
| tests que lo respaldan | **216** | 0 |
| verificado en hardware real | si | no |

C++ empieza con el backend **hecho y probado**, y puede gastar el cien por cien
del esfuerzo en las dos montanas que `MAESTROS.md` identifica: **resolucion de
sobrecarga** y **monomorfizacion de plantillas**.

Eso es lo que quiere decir "que se enfoque en lo que ya importa para funcionar".

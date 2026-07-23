# forge/ — la fragua de BMO-X

Las **librerías compartidas del pipeline de compilación**, y el gate de
verificación. Aquí es donde la salida cruda de cada lenguaje se **forja** en
bytes válidos para el BMO ABI.

> **Ley (ver `../lang/cobol/cobol.md`)**: se comparten CONTRATOS y LIBRERÍAS,
> nunca CEREBROS. Nada de esto es un embudo obligatorio — cada frontend
> (`../lang/*`) **elige** qué enlazar. La esencia de cada lenguaje (parser,
> AST, su descenso propio) vive en su crate, jamás aquí.

## Lo que FUNCIONA hoy (regla: nada de stubs)

| Crate | Rol | Estado |
|-------|-----|--------|
| **`sem-asm/`** (`bmo-sem-asm`) | **Codificación**: motor que lee las tablas TOML (`tables/`) y encodea instrucciones + intrínsecos → bytes. Lo usa `lang/c/codegen.rs`. | ✅ funciona (7 tests) |
| **`bmo-verify/`** | **Gate de verificación**: valida el BEF (header, secciones, imports/relocs, firma, flags) antes del ABI. Delega en el validador REAL de `bmo-abi::bef::validator`. Habilita SIPs (Singularity). | ✅ funciona (4 tests + 15 en bmo-abi) |

## Fases futuras (se crearán CON código real, no como stubs)

Cuando arranquen, estas librerías nacerán con lógica de verdad — no se
dejan andamios vacíos en el árbol:

- **`bmo-lower`** — Descenso al ABI: centralizar la emisión de `INVOKE`/
  subsyscalls hoy duplicada entre `lang/c` y `lang/cobol`.
- **`bmo-opt`** — Optimización genérica a dial: const-fold → DCE →
  strength reduction → register allocation lineal (el que importa para
  loops COBOL).

## Layout

```
toolchain/
  lang/           ← frontends (ESENCIA, individual): c, cobol, cpp, base
  forge/          ← ESTA carpeta: librerías compartidas del pipeline
    sem-asm/
      src/        (motor Rust que lee las tablas)
      tables/     (arch/, standards/, stdlib/ — las TOML)
    bmo-verify/   (gate: delega en bmo-abi::bef::validator)
  tools/          ← generadores: linker, bef-bootstrap, hello-bex, fontgen
```

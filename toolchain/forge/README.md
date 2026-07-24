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
| **`bmo-lower/`** | **L1 — el descenso al ABI**: la puerta. Emite `INVOKE`/subsyscalls (`console::write_const`, `console::write_buffer`, `task::exit`). No sabe qué lenguaje la llamó. Lo usan `lang/c` y `lang/cobol`. | ✅ funciona (7 tests, incluye emulador x86-64) |

### La regla de L1 (`bmo-lower`)

> **L1 solo contiene lo expresable en la superficie congelada por valor.
> Todo lo que tenga semántica de lenguaje —formato `%d`, edición PIC,
> `operator<<`— se queda en L2 (el frontend).**

Es lo que impide que la puerta degenere en un embudo de mínimo común
denominador. `printf("%d", x)` formatea a bytes *dentro* del programa C;
`DISPLAY saldo` aplica la PIC en el suyo; ambos llaman a la misma puerta con
bytes crudos. Cuando entre un cuarto lenguaje, aquí no se toca nada.

Los tests de `bmo-lower` no comparan bytes contra bytes escritos a mano —eso
solo repite el error del autor—: **ejecutan** el código emitido en un
emulador x86-64 mínimo (`src/emu.rs`) que modela la puerta del kernel
(8 bytes LE, NUL-stop) y verifica que el texto reconstruido sea el original.
Un test compara además byte a byte con la secuencia de `tools/hello-bex`, la
única que sabemos con certeza que corrió en el Ryzen real.

## Fases futuras (se crearán CON código real, no como stubs)

Cuando arranquen, estas librerías nacerán con lógica de verdad — no se
dejan andamios vacíos en el árbol:

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
    bmo-lower/    (L1: la puerta INVOKE — console::*, task::*)
  tools/          ← generadores: linker, bef-bootstrap, hello-bex, fontgen
```

# forge/ — la fragua de BMO-X

Las **librerías compartidas del pipeline de compilación**, y el gate de
verificación. Aquí es donde la salida cruda de cada lenguaje se **forja** en
bytes válidos para el BMO ABI.

> **Ley (ver `../lang/cobol/cobol.md`)**: se comparten CONTRATOS y LIBRERÍAS,
> nunca CEREBROS. Nada de esto es un embudo obligatorio — cada frontend
> (`../lang/*`) **elige** qué enlazar. La esencia de cada lenguaje (parser,
> AST, su descenso propio) vive en su crate, jamás aquí.

## Las 3 librerías (opcionales) + el gate

| Crate | Rol | Estado |
|-------|-----|--------|
| **`sem-asm/`** (`bmo-sem-asm`) | **Codificación**: motor que lee las tablas TOML (`tables/`) y encodea instrucciones → bytes. Reemplaza el hardcodeo de bytes duplicado en cada `codegen.rs`. | motor por construir; tablas migradas |
| **`bmo-lower/`** | **Descenso al ABI**: helpers que traducen las *exigencias* del lenguaje (print, I/O) a operaciones del BMO ABI (`INVOKE`/subsyscalls). | esqueleto |
| **`bmo-opt/`** | **Optimización genérica**: pasadas clásicas (register allocation, constant folding, dead-code elimination, strength reduction). | esqueleto (dial: empezar vacío) |
| **`bmo-verify/`** | **Gate de verificación**: valida el BEF (capabilities, memory-safety, firma) antes del ABI. Crece desde `platform/abi/bmo-abi/src/bef/{validator,signing,blake3}.rs`. Habilita SIPs (Singularity). | esqueleto |

## Layout

```
toolchain/
  lang/           ← frontends (ESENCIA, individual): c, cobol, cpp
  forge/          ← ESTA carpeta: librerías compartidas del pipeline
    sem-asm/
      src/        (motor Rust que lee las tablas)
      tables/     (arch/, standards/, stdlib/ — las TOML)
    bmo-lower/
    bmo-opt/
    bmo-verify/
  tools/          ← (futuro) generadores: linker, bef-bootstrap, hello-bex, fontgen
```

## Migración (en curso)

1. ✅ Mover tablas sem-asm a `forge/sem-asm/tables`.
2. ⬜ Motor sem-asm: leer `tables/arch/x86_64/instructions.toml` → encodear.
3. ⬜ Migrar `lang/c/codegen.rs` y `lang/cobol/codegen.rs` a usar el motor
   (borrar el hardcodeo de bytes duplicado; arreglar las rutas muertas
   `X:\FastOS\...\Semantic_ASM`).
4. ⬜ Extraer `bmo-lower` de la emisión de INVOKE duplicada.
5. ⬜ `bmo-opt`: empezar con const-fold + DCE.
6. ⬜ `bmo-verify`: crecer del validator existente.

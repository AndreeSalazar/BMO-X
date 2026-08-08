# toolchain/ -- de codigo fuente a BEX

El pipeline de BMO-X: lenguajes -> BEF -> verificacion -> BMO ABI. Organizado en
**tres carpetas con un rol claro** (antes estaba todo mezclado en la raiz).

```
toolchain/
+-- lang/     <- FRONTENDS (la esencia, individual por lenguaje)
|   +-- c/        bmo-c-front
|   +-- cobol/    bmo-cobol-front   (todos los dialectos)
|   +-- cpp/      bmo-cpp-front
|   +-- base/     stdlib base (bmo/core, pci, lib/printf) -- datos, no crate
|
+-- forge/    <- PIPELINE compartido (librerias OPCIONALES, nunca embudos)
|   +-- sem-asm/    bmo-sem-asm   codificacion: tablas TOML -> bytes  ✅ vivo (lo usa C)
|   +-- bmo-verify/ gate del BEF: delega en bmo-abi::validator  ✅ vivo
|   #  bmo-lower (descenso ABI) y bmo-opt (optimizacion) se crearan
|   #  CON codigo real al empezar su fase -- sin stubs vacios en el arbol.
|
+-- tools/    <- GENERADORES build-time
    +-- bef-bootstrap/  primer payload BEF auditable
    +-- hello-bex/      genera el init_hello.bex embebido del kernel
    +-- fontgen/        genera font16_data.rs (tabla de glifos)
    +-- bmo-linker/     extrae simbolos de .elf -> BMO_SYMBOLS.toml
```

## Reglas de organizacion

- **`lang/` = esencia.** Cada lenguaje es un pipeline COMPLETO y privado
  (parser, AST, su propio descenso). Nadie lo toca. La duplicacion de trabajo
  mecanico se compensa con las librerias de `forge/`, que el frontend
  **elige** enlazar.
- **`forge/` = contratos y librerias, NUNCA cerebros.** Nada es un embudo
  obligatorio. Ver `forge/README.md` y `lang/cobol/cobol.md` para la teoria.
- **`tools/` = generadores.** Se invocan con `cargo run -p <nombre>`
  (independiente de la ruta) y sus salidas se commitean.

## Flujo

```
fuente (C/COBOL/C++) -> [frontend en lang/] -> BEF
                              |  (usa forge/ a voluntad)
                              ▼
                        [bmo-verify] -> BMO ABI (3 syscalls) -> corre en Ring 3
```

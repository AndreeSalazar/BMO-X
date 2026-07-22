# toolchain/ — de código fuente a BEX

El pipeline de BMO-X: lenguajes → BEF → verificación → BMO ABI. Organizado en
**tres carpetas con un rol claro** (antes estaba todo mezclado en la raíz).

```
toolchain/
├── lang/     ← FRONTENDS (la esencia, individual por lenguaje)
│   ├── c/        bmo-c-front
│   ├── cobol/    bmo-cobol-front   (todos los dialectos)
│   ├── cpp/      bmo-cpp-front
│   └── base/     stdlib base (bmo/core, pci, lib/printf) — datos, no crate
│
├── forge/    ← PIPELINE compartido (librerías OPCIONALES, nunca embudos)
│   ├── sem-asm/    bmo-sem-asm   codificación: tablas TOML → bytes  ✅ motor vivo
│   ├── bmo-lower/  descenso al BMO ABI (DISPLAY/printf → INVOKE)
│   ├── bmo-opt/    optimización a dial (const-fold, DCE, regalloc)
│   └── bmo-verify/ gate de verificación del BEF (habilita SIPs)
│
└── tools/    ← GENERADORES build-time
    ├── bef-bootstrap/  primer payload BEF auditable
    ├── hello-bex/      genera el init_hello.bex embebido del kernel
    ├── fontgen/        genera font16_data.rs (tabla de glifos)
    └── bmo-linker/     extrae símbolos de .elf → BMO_SYMBOLS.toml
```

## Reglas de organización

- **`lang/` = esencia.** Cada lenguaje es un pipeline COMPLETO y privado
  (parser, AST, su propio descenso). Nadie lo toca. La duplicación de trabajo
  mecánico se compensa con las librerías de `forge/`, que el frontend
  **elige** enlazar.
- **`forge/` = contratos y librerías, NUNCA cerebros.** Nada es un embudo
  obligatorio. Ver `forge/README.md` y `lang/cobol/cobol.md` para la teoría.
- **`tools/` = generadores.** Se invocan con `cargo run -p <nombre>`
  (independiente de la ruta) y sus salidas se commitean.

## Flujo

```
fuente (C/COBOL/C++) → [frontend en lang/] → BEF
                              │  (usa forge/ a voluntad)
                              ▼
                        [bmo-verify] → BMO ABI (3 syscalls) → corre en Ring 3
```

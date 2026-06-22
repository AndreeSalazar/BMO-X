# `lang/` — Language Subsystems

This directory contains the complete language toolchain for BMO/FastOS.
Every language in this kernel compiles to **ÑEXO AST** as the common
IR, then goes through the BMOasm backend, which emits native machine
code for x86_64, aarch64, and riscv64.

## Quick links

- **[Complete language reference](docs/README.md)** ← full details on every language
- **[ÑEXO](nexo/)** — native BMO language
- **[BMOasm](bmoasm/)** — the single IR
- **[Language frontends](nexo/plugins/languages/)** — C, C++, Java, Python, Rust, Go

## Three-layer architecture

```
┌─────────────────────────────────────────────────────────────┐
│  Source code (C, C++, Java, Python, ÑEXO)                   │
└───────────────────────┬─────────────────────────────────────┘
                        │ Lexer → Parser → Translator
                        ▼
┌─────────────────────────────────────────────────────────────┐
│  ÑEXO AST (common IR for all languages)                     │
└───────────────────────┬─────────────────────────────────────┘
                        │ Sema → Codegen → BMOasm AST
                        ▼
┌─────────────────────────────────────────────────────────────┐
│  BMOasm AST (v0.4.0) with structs, fields, vtables          │
└───────────────────────┬─────────────────────────────────────┘
                        │ Sema → Opt (6 passes) → Traductor
                        ▼
┌─────────────────────────────────────────────────────────────┐
│  Native machine code (x86_64 / aarch64 / riscv64)           │
└─────────────────────────────────────────────────────────────┘
```

## Status at a glance

| Language | Frontend | Codegen | Tests | Status |
|----------|----------|---------|-------|--------|
| **ÑEXO** (native) | ✅ full | ✅ | 22 | Production |
| **C** | ✅ full | ✅ | 8 | Production |
| **BMOasm** (IR) | ✅ | ✅ | 41 | Production |
| **C++** | lexer+AST | stub | 0 | v0.1.0 |
| **Java** | lexer+parser+AST | stub | 0 | v0.1.0 |
| **Python** | lexer+parser+AST | stub | 0 | v0.1.0 |
| **Rust** | stub | n/a | 0 | stub |
| **Go** | stub | n/a | 0 | stub |

See **[docs/README.md](docs/README.md)** for the complete reference.

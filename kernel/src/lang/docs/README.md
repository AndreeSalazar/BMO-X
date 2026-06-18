# BMO/FastOS Language Subsystems

`lang/` contains the complete language toolchain for BMO/FastOS.
Each language produces **ÑEXO AST** as the common IR, which then
goes through the BMOasm backend and emits native x86_64/aarch64/
riscv64 bytes.

## Layout

```
lang/
├── mod.rs              ← doc-comment with overview
├── nexo/               ← ÑEXO: native BMO language
│   ├── lexer.rs        ← 32 keywords, hex/bin/oct, strings
│   ├── parser.rs       ← fn, let, if, while, for, struct, enum, impl
│   ├── sema.rs         ← type checking, scopes
│   ├── codegen.rs      ← ÑEXO AST → BMOasm AST
│   ├── modules.rs      ← `use`, `pub`, `mod`
│   ├── runtime/        ← memory, io, proc, time, fs, error
│   ├── stdlib/         ← sys, io, mem, str, fs, math, time, gfx
│   ├── pm/             ← package manager
│   ├── plugins/        ← GC, GIL, FFI, ABI, languages
│   └── tests.rs        ← 22 end-to-end tests
├── bmoasm/             ← IR intermedio (backend único)
│   ├── parser/         ← AST (v0.4.0) + lexer + parser
│   ├── sema/           ← scope, typeck, fold, dce, opt (6 passes)
│   ├── emit/           ← backends x86_64 / aarch64 / riscv64
│   ├── builtin/        ← cpu flags, memory ordering, intrinsics
│   ├── sample/         ← examples
│   ├── runtime/        ← aloc, libre, atomico, volatil, barr
│   ├── traductor/      ← AST → bytes (with type layouts, field offsets)
│   ├── cache/          ← LRU de blobs traducidos
│   └── tests.rs        ← 41 BMOasm tests
├── docs/               ← documentation
│   ├── C_SUBSET.md
│   ├── CPP_SUBSET.md
│   ├── PYTHON_SUBSET.md
│   ├── JAVA_SUBSET.md
│   └── IR_BRIDGE.md
└── plugins/languages/  ← language frontends (each in own dir)
    ├── c/              ← C language (full frontend)
    ├── cpp/            ← C++ essential (class, virtual, new/delete)
    ├── java/           ← Java essential (class, interface, try/catch)
    ├── python/         ← Python essential (def, class, async)
    ├── rust/           ← Rust (stub)
    └── go/             ← Go (stub)
```

## Status by language

| Language | Frontend | Translator | Codegen | Tests | Status |
|----------|----------|------------|---------|-------|--------|
| **ÑEXO** (native) | ✅ | ✅ | ✅ | 22 | Production |
| **C** | ✅ full | ✅ full | ✅ | 8 | Production |
| **BMOasm** (IR) | ✅ | n/a | ✅ | 41 | Production |
| **C++** | lexer | stub | stub | 0 | v0.1.0 |
| **Java** | lexer+parser | stub | stub | 0 | v0.1.0 |
| **Python** | lexer+parser | stub | stub | 0 | v0.1.0 |
| **Rust** | stub | stub | stub | 0 | stub |
| **Go** | stub | stub | stub | 0 | stub |

## What each language gets you

- **ÑEXO**: native BMO apps, the most ergonomic for this kernel
- **C**: real-world apps, libraries, system tools (the devora-ADN sweet spot)
- **C++**: class-based OOP, virtual dispatch, `new`/`delete`
- **Java**: OOP, interfaces, exception handling
- **Python**: scripting, REPL, dynamic typing
- **Rust**: kernel-level apps (the kernel itself is Rust)
- **Go**: services, networking (not yet implemented)

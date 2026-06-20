# Estructura de BMO Core + BMO GPU (v1.7.9)

> BMO Core es el **kernel de Ring 3**: gestiona software (lenguajes,
> windowing, FS, audio). BMO GPU es el **bridge entre Ring 3 y la
> GPU real** (validación de BSF, shims PE/Windows).

## Estructura final

```
kernel/src/
├── ring0/                        — Hardware (Ryzen 5 5600X)
├── ring3/                        — Apps (preparación)
│
├── bmo_core/                     — Kernel de Ring 3
│   ├── mod.rs
│   ├── api/                      — 256 syscalls 0x100..0x1FF
│   ├── runtime/                  — BMO Runtime placeholder
│   ├── lang/                     — Compilador (BMO, antes Nexo)
│   │   ├── mod.rs
│   │   ├── bmoasm/               — (legacy, será eliminado v2.0)
│   │   └── nexo/                 — Lexer, parser, AST, sema, codegen
│   ├── bmo_abi/                  — BMO ABI: tipos primitivos, status, handle
│   │   ├── fundamentals/         — primitives, status, handle, memory, sync
│   │   ├── values/               — string, time, reflect
│   │   └── runtime/              — BmoRuntimePlaceholder (v1.7.9 stub)
│   ├── bmo_api/                  — Windowing (window, draw, input, event, wm)
│   ├── bef/                      — BMO Executable Format (loader ELF/PE/native)
│   ├── fs/                       — BMO-FS, FAT32, ramdisk
│   ├── ui/                       — Framebuffer wrapper, font, console
│   ├── diag/                     — Logger, telemetry, overlay
│   ├── desktop/                  — Window manager built-in
│   └── audio/                    — Sintetizador (FM, PCM, tracks)
│
└── bmo_gpu/                      — Bridge Ring 3 ↔ GPU
    ├── mod.rs                    — BAREX_VERSION, BxError, BSF_MAGIC
    ├── shims/                    — Compatibilidad con apps externas
    │   ├── pe_imports.rs         — PE import resolver
    │   └── pe_thunks.rs          — Win32 API → BMO API dispatcher
    ├── shader/                   — BSF (BareX Shader Format) loader
    ├── compositor/               — Ring 0 ↔ Ring 3 GPU composition
    └── commands/                 — GPU command buffers (ring submission)
```

## ¿Qué cambió en v1.7.9?

### Eliminado (no aplica a un OS nativo)

| Carpeta | LOC eliminado | Por qué |
|---|---|---|
| `bmo_core/barex/` | 374 | Movido a `bmo_gpu/` |
| `bmo_core/sandbox/` | 45 | Vacío, un solo stub |
| `bmo_core/bmo_abi/interop/` | 1,000+ | Win32/PE shims — no aplica |
| `bmo_core/bmo_abi/machinery/` | 1,180 | type_system, vtable, exception, etc. — confunde con Ring 0 |
| **Total eliminado** | **~2,600 LOC** | De 32,000 → 28,878 LOC en bmo_core |

### Reorganizado

- **BareX → bmo_gpu/** — el bridge GPU vive aparte del kernel de Ring 3
- **BMO ABI simplificado** — sólo essentials (primitives, status, handle, sync, memory)
- **bmo_abi::runtime** — placeholder único (`BmoRuntimePlaceholder`)

### Pendiente (v2.0)

- **Eliminar `lang/bmoasm/`** — sólo BMO (antes Nexo)
- **Reorganizar `lang/nexo/plugins/languages/{c,cpp,java,python}`** → `lang/bmo/plugins/`
- **Implementar `BmoRuntime` real** (types, vtables, lang_bridge)

## BMO ABI: diseño modular (v1.7.9)

```
bmo_abi/
├── fundamentals/        — Tipos que TODO código usa
│   ├── primitives/      — bx_u8..u64, bx_i*, bx_f16/32/64
│   ├── status/          — BmoStatus, BxError (sustituye HRESULT/errno)
│   ├── handle/          — BmoHandle 64-bit con generación
│   ├── option/          — BmoOption<T> FFI-safe
│   ├── result/          — BmoResult<T> FFI-safe
│   ├── memory/          — slice, range, align
│   └── sync/            — BmoAtomicU64, MemOrder
├── values/              — Tipos valor
│   ├── string/          — bx_str, ascii
│   ├── time/            — Instant, Duration
│   └── reflect/         — Mirror (stub v1.7.9)
└── runtime/             — BmoRuntimePlaceholder (v1.7.9 stub)
```

Cada carpeta es **autocontenida**. Apps pueden importar sólo lo que
necesitan:

```rust
use crate::bmo_core::bmo_abi::fundamentals::primitives::bx_u64;
use crate::bmo_core::bmo_abi::fundamentals::status::{BmoStatus, BxError};
```

## BMO GPU: nueva estructura

```
bmo_gpu/
├── mod.rs                — entry point, BAREX_VERSION, BxError
├── shims/                — Compatibilidad con apps externas
│   ├── pe_imports.rs     — PE import resolver
│   └── pe_thunks.rs      — Win32 → BMO dispatcher
├── shader/               — BSF (BareX Shader Format) loader
├── compositor/           — Ring 0 ↔ Ring 3 GPU composition
└── commands/             — GPU command buffers
```

### Flujo de un shader BSF

```
App Ring 3 (BMO)
  │
  │  syscall 0x140 GPU_SUBMIT_SHADER
  ▼
bmo_core/api/syscall.rs   ← dispatch 0x100..0x1FF
  │
  │  GPU syscall
  ▼
bmo_gpu/shader/bsf.rs    ← valida magic, version, BLAKE3
  │
  │  GPU real
  ▼
ring0/dev/amdgpu.rs       ← driver real (futuro v1.8)
```

### Flujo de una app Windows (.exe PE)

```
App Windows (.exe PE)
  │
  │  BEF loader carga el PE
  ▼
bmo_gpu/shims/pe_imports.rs   ← resuelve imports
  │
  │  redirige a:
  ▼
bmo_gpu/shims/pe_thunks.rs    ← ntdll/kernel32/etc → BMO API
  │
  │  si la app dibuja:
  ▼
bmo_gpu/shader/bsf.rs         ← valida BLAKE3
```

## BMO Core: módulos

| Módulo | LOC | Función |
|---|---|---|
| `api/` | 2,390 | 256 syscalls 0x100..0x1FF |
| `lang/nexo/` | 9,736 | Compilador del lenguaje BMO (antes Nexo) |
| `lang/nexo/plugins/languages/` | 3,633 | C, C++, Java, Python como plugins |
| `lang/bmoasm/` | 5,667 | Assembler BM (legacy, a eliminar) |
| `bmo_abi/` | 1,279 | Tipos primitivos BMO ABI |
| `bef/` | 3,064 | BMO Executable Format (loader) |
| `desktop/` | 2,044 | Window manager built-in |
| `diag/` | 1,180 | Logger, telemetry, overlay |
| `ui/` | 1,058 | Framebuffer, font, console |
| `fs/` | 902 | BFS, FAT32, ramdisk |
| `gustos/` | 431 | Sintetizador de audio |

## Pendiente para v2.0

1. **Eliminar `lang/bmoasm/`** (5,667 LOC) — un solo compilador
2. **Reorganizar plugins de lenguajes** → `lang/bmo/plugins/{c,cpp,java,python}/`
3. **Implementar `BmoRuntime` real** con types/vtables/lang_bridge
4. **Implementar `bmo_gpu::compositor`** con surfaces y queues
5. **AMDGPU driver stub** en `ring0/dev/amdgpu.rs`
6. **Test con un driver real** AMDGPU RX 580

## Cómo añadir un nuevo módulo

```rust
// 1. Crear el archivo
// bmo_core/foo/mod.rs
//! Foo subsystem description

#![allow(dead_code)]
// ... implementation

// 2. Registrar en bmo_core/mod.rs
pub mod foo;

// 3. Si expone API a Ring 3:
//    bmo_core/api/syscall.rs → agregar dispatch
// 4. Si toca hardware:
//    bmo_gpu/compositor/ → agregar command
```

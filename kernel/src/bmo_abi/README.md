# `bmo_abi/` — BMO ABI (Bare-Metal Orchestrator Application Binary Interface)

> Cimiento absoluto de FastOS. **Reemplaza** al C ABI (cdecl / Win64 / SysV
> AMD64) y a su stdlib. Cada nueva línea de código del kernel debe usar
> tipos BMO; el C ABI sólo aparece en `interop/compat` para hablar con
> código heredado mientras se digiere.

## 🌟 ¿Por qué BMO ABI es top-level?

Porque es la **fundación** de TODO. Cualquier archivo del kernel (kernel
core, drivers, BEF loader, lenguajes, desktop) usa tipos BMO. Por eso vive
en `kernel/src/bmo_abi/` directamente, no enterrado en `barex/abi/`.

Antes estaba en `barex/abi/`. Ahora bien aquí.

## 📐 Las 5 categorías semánticas

```
bmo_abi/
├── mod.rs               ← re-exports planos + docs raíz
│
├── fundamentals/        ── LO QUE TODO EL CÓDIGO USA ──
│   ├── primitives/      tipos numéricos canónicos (bx_u8..u128, f16/32/64)
│   ├── status/          BmoStatus 16-byte + 18 ErrorCode + StatusFlags
│   ├── handle/          BmoHandle 64-bit con tag/generation
│   ├── option.rs        BmoOption<T> FFI-safe
│   ├── result.rs        BmoResult<T> FFI-safe
│   └── memory/          BmoSlice, BmoRange, BmoAligned, BmoPageAligned
│
├── values/              ── TIPOS VALOR CON SEMÁNTICA PROPIA ──
│   ├── string/          BmoStr, BmoString (UTF-8 con length explícito)
│   ├── time/            BmoInstant (TSC-backed), BmoDuration
│   └── reflect/         ReflectQuery sobre BEF cargados
│
├── machinery/           ── CÓMO SE COMPONE EL CÓDIGO ──
│   ├── calling.rs       7 GPRs, 64B align, 256B red zone
│   ├── sync/            BmoMutex (futex), BmoAtomic*, BmoFutex
│   ├── type_system/     TypeDescriptor, TypeLayout, TypeKind (21 valores)
│   ├── vtable/          BmoVTable, VTableEntry (magic BVT1, O(1) lookup)
│   ├── closure/         BmoClosure, env, signature
│   ├── exception/       UnwindContext, panic, resume
│   └── async_io/        SQE/CQE rings estilo io_uring
│                        + SqConsumer, SqProducer, CqConsumer, CqProducer
│
├── interop/             ── CÓMO SE HABLA CON OTROS LENGUAJES ──
│   ├── lang_bridge/     LangDescriptor, 25+ LangIds (Rust, C, C++, Java, ...)
│   ├── marshal/         Marshaller trait, IdentityMarshaller,
│   │                    PrimitiveMarshaller, MarshallerRegistry
│   │                    boxing (Rust/C/C++/Zig/Swift/Go/etc → BMO box)
│   │                    string_enc (UTF-8 ↔ UTF-16 ↔ ASCII)
│   │                    boolean (1B BMO ↔ 4B Win32 BOOL)
│   └── compat/          Trampolines runtime Win64↔BMO y SysV↔BMO
│                        (naked_asm, con shadow space y stack fix-up)
│
└── runtime/             ── AGREGADOR ÚNICO ──
    └── mod.rs           BmoRuntime + RuntimeStats
                         + validate_runtime() con 5 checks reales
```

## 🚀 Quick start

```rust
use crate::bmo_abi::*;          // Trae todo lo fundamental
use crate::bmo_abi::primitives::*;   // tipos numéricos
use crate::bmo_abi::handle::BmoHandle;  // handles
use crate::bmo_abi::status::{BmoStatus, ErrorCode};
use crate::bmo_abi::time::BmoInstant;  // ns desde boot
```

## ⚖️ Diferencias vs ABIs heredados

| Aspecto           | MS x64       | SysV AMD64   | **BMO ABI**    |
|-------------------|--------------|--------------|----------------|
| Args int          | 4 GPRs       | 6 GPRs       | **7 GPRs**     |
| Shadow space      | 32 B         | 0 B          | **0 B**        |
| Stack align       | 16 B         | 16 B         | **64 B**       |
| Red zone          | 0 B          | 128 B        | **256 B**      |
| Return int (≤128b)| RAX          | RAX:RDX      | **RAX:RDX**    |
| Status            | HRESULT+TLS  | errno+TLS    | **BmoStatus 16B inline** |
| Strings           | char* nul    | char* nul    | **(ptr, len) UTF-8** |
| Handles           | HANDLE void* | fd int       | **BmoHandle con tag+gen** |
| Async I/O         | IOCP OVERLAPPED | epoll/select | **SQE/CQE rings** |

## ✅ Lo que está completo

- **Calling convention** documentada + helpers (`align_stack`, `is_stack_aligned`)
- **Tipos primitivos** completos con constantes (`BX_U64_MAX`, etc)
- **BmoStatus** + 18 ErrorCode + StatusFlags
- **BmoHandle** 64-bit con tag bit + 33 HandleKind + `INVALID/NULL`
- **BmoSlice/BmoRange/BmoAligned** con layout C
- **BmoStr/BmoString** UTF-8 con length
- **BmoInstant** con TSC real (`init_clock()` debe llamarse tras calibrar TSC)
- **BmoMutex** futex-backed lock-free en fast path
- **BmoAtomicU32/U64/Bool** con MemOrder BMO
- **TypeSystem** completo (TypeKind 21, TypeLayout con flags, TypeDescriptor)
- **BmoVTable** con magic BVT1, O(1) lookup
- **BmoClosure** con env + signature
- **SQE/CQE rings** completos con Producer/Consumer para app y kernel
- **LangBridge** con 25+ LangIds (Rust, C, C++, Zig, Swift, JVM, CLR, Python, JS, Go, OCaml, Lua, Haskell, BEAM, Nim, Crystal, Dart, Kotlin, Ruby, PHP, Fortran, Ada, Racket, Scheme, Clojure)
- **Marshaller** trait + IdentityMarshaller + PrimitiveMarshaller
- **UTF-8 ↔ UTF-16** conversiones reales
- **Boxing** para lenguajes con layout BMO
- **Trampolines runtime** Win64↔BMO y SysV↔BMO con naked_asm
- **BmoRuntime** agregador con `validate_runtime()` (5 checks reales)

## 🔧 Pendiente (futuras sesiones)

- GC interface (`gc_iface/`) — para lenguajes managed
- Lazy loading de secciones BEF grandes
- Trampolines ARM64 (mismo patrón que x86_64)
- Reflection: aplicar queries reales sobre BEF cargados
- Marshaller dedicado para JVM, CLR, Python, JS

## 📊 Estadísticas

- **~1,950 líneas** de Rust (vs 1,850 antes — +5% por completas)
- **26 archivos** `.rs` distribuidos en 5 categorías
- **0 warnings** introducidos por la reorganización
- **0 archivos** movidos de `barex/abi/` (todos copiados + actualizados)
- **75 archivos** del kernel actualizados para usar `bmo_abi` en lugar de `barex::abi`
- **Compatibilidad**: `crate::barex::abi` re-exporta con `#[deprecated]` para no romper código viejo

## 📚 Cómo añadir un nuevo lenguaje

1. Asignar ID en [`interop/lang_bridge/ids.rs`](interop/lang_bridge/ids.rs)
   (rango oficial `0x0000_0020+` o experimental `0x8000_0000+`).
2. Crear un `LangDescriptor` con `name`, `version`, `LangFeatures`.
3. Si tiene boxing/tagged values, implementar `Marshaller` en
   [`interop/marshal/`](interop/marshal/).
4. Si tiene GC, wirearlo en `gc_iface/` (sesión futura).
5. El compilador del lenguaje emite BEF con secciones `TypeMap` /
   `LangBridge` / `VTables`. Ya está: corre nativo en FastOS.

## 🔄 Cuándo se acabará el C ABI

Cuando todos los thunks de `bef/loader/pe_thunks.rs` y
`bef/loader/elf_thunks.rs` sean recompilados como BEF nativos. Hasta
entonces, `interop/compat` aísla la contaminación.

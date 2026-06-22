# `bmo_abi/` — BMO ABI (Bare-Metal Orchestrator Application Binary Interface)

> Cimiento absoluto de FastOS. Reemplaza al C ABI (cdecl / Win64 / SysV
> AMD64) y a su stdlib. Toda nueva línea de código del kernel usa
> tipos BMO. El C ABI sólo aparece en `bef/loader/*_thunks.rs` para
> hablar con código heredado mientras se digiere.

## 📐 Especificación

Lee **[`SPEC.md`](./SPEC.md)** — es la fuente de verdad del ABI:
- Calling convention
- Layout de tipos
- Syscall numbers (0x100..0x1FF)
- Patrones de integración con lenguajes
- Tabla rápida de funciones del ABI

## 🏗️ Estructura

```
bmo_abi/
├── mod.rs               ← re-exports planos + docs raíz
├── SPEC.md              ← **FUENTE DE VERDAD** — leer primero
│
├── fundamentals/        ── LO QUE TODO EL CÓDIGO USA ──
│   ├── primitives/      tipos numéricos (bx_u8..u64, bx_i*, bx_f*)
│   ├── status/          BmoStatus 16-byte + 18 ErrorCode + StatusFlags
│   ├── handle/          BmoHandle 64-bit con tag/generation
│   ├── option.rs        BmoOption<T> FFI-safe
│   ├── result.rs        BmoResult<T> FFI-safe
│   ├── error.rs         BmoError unificado
│   ├── convert.rs       BmoError↔BmoStatus↔ErrorCode
│   ├── fmt.rs           BmoFormatter (stack-allocated, sin heap)
│   ├── io.rs            BmoFileHandle, BmoPipe, Read/Write/Seek
│   ├── memory/          BmoSlice, BmoRange, BmoAligned
│   └── sync/            BmoAtomic*, MemOrder, BmoSpinLock
│
├── values/              ── TIPOS VALOR CON SEMÁNTICA PROPIA ──
│   ├── string/          BmoStr, BmoString (UTF-8 con length)
│   ├── time/            BmoInstant (TSC-backed), BmoDuration
│   ├── reflect/         ReflectQuery sobre BEF cargados
│   ├── net/             IPv4/IPv6, SocketAddr, Protocol
│   ├── math/            sqrt, sin, cos, pow
│   └── hash/            FNV-1a, CRC32
│
└── runtime/             ── AGREGADOR ÚNICO ──
    ├── mod.rs           BmoRuntime + validate_runtime()
    ├── types.rs         TypeRegistry (256 slots)
    ├── vtable.rs        VTableStore (64 slots)
    └── lang_bridge.rs   LangBridge (8 slots)
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
| Syscall range     | 0x1000+      | 0x0001..     | **0x100..0x1FF** |

## 📚 Cómo añadir un nuevo lenguaje

1. Asigna un ID en el enum `Language` de
   `crate::bmo_core::lang::bmo::plugins::traits`.
2. Implementa `LanguageAdapter` en
   `crate::bmo_core::lang::bmo::plugins::languages::<lang>`.
3. Compila el lenguaje a BMO AST (o directo a x86-64 nativo).
4. **Todas las llamadas al kernel deben ir por el BMO ABI**
   (syscalls 0x100..0x1FF). El BEF loader rechaza cualquier BEF
   que llame a otro número.

## ✅ Lo que está completo

- Calling convention documentada + helpers (`align_stack`, `is_stack_aligned`)
- Tipos primitivos completos con constantes (`BX_U64_MAX`, etc)
- BmoStatus + 18 ErrorCode + StatusFlags
- BmoHandle 64-bit con tag bit + 33 HandleKind
- BmoOption/BmoResult FFI-safe
- BmoStr/BmoString UTF-8 con length
- BmoInstant con TSC real
- BmoAtomicU32/U64/Bool + MemOrder
- BmoSpinLock TTAS
- BmoFormatter stack-allocated
- BmoRuntime agregador con `validate_runtime()`

## 🔧 Pendiente (futuras sesiones)

- GC interface — para lenguajes managed (Java/Python)
- Lazy loading de secciones BEF grandes
- Trampolines ARM64
- Marshaller dedicado para JVM, CLR, Python

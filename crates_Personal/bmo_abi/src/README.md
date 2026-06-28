# `bmo_abi` — BMO Application Binary Interface

> The standard ABI for FastOS. Replaces C ABI (cdecl / Win64 / SysV AMD64)
> and its standard library. Every kernel-to-userspace boundary uses BMO types.
> The C ABI only appears in `bef/loader/*_thunks.rs` for legacy interop.

## Specification

Read **[`SPEC.md`](./SPEC.md)** — the single source of truth:

- Calling convention (7 GPRs, 256 B red zone, 64 B stack alignment)
- Type layouts and alignment rules
- Syscall numbering (0x100..0x1FF)
- BEF integration patterns

## Module Map

```
bmo_abi/
├── mod.rs              Re-exports + root docs
├── SPEC.md             Canonical specification
│
├── fundamentals/       Types used by EVERY BMO module
│   ├── primitives/     bx_u8..u64, bx_i*, bx_f*, bx_bool
│   ├── status/         BmoStatus (16 B inline), ErrorCode, StatusFlags
│   ├── handle/         BmoHandle (64-bit with tag + generation)
│   └── sync/           BmoAtomicU32/U64/Bool, MemOrder, BmoSpinLock
│
├── values/             Value types with own semantics
│   └── time/           BmoInstant (TSC-backed monotonic), BmoDuration
│
├── windowing/          Window contract: class, create info, events
├── fs/                 File system: handles, open flags, stat, permissions
├── surface/            Pixel formats (ARGB8, etc.), surface descriptors
├── error_code/         21 extended error codes
├── bef/                BEF format: header, sections, manifest, signing, relocs
├── syscalls/           Syscall number table (0x100..0x1CF)
└── profile/            BmoLanguageProfile + ALL_PROFILES registry
```

## Comparison with Legacy ABIs

| Aspect            | MS x64        | SysV AMD64    | **BMO ABI**        |
|-------------------|---------------|---------------|--------------------|
| Integer args      | 4 GPRs        | 6 GPRs        | **7 GPRs**         |
| Shadow space      | 32 B          | 0 B           | **0 B**            |
| Stack alignment   | 16 B          | 16 B          | **64 B**           |
| Red zone          | 0 B           | 128 B         | **256 B**          |
| Return (≤128 bit) | RAX           | RAX:RDX       | **RAX:RDX**        |
| Error reporting   | `HRESULT`+TLS | `errno`+TLS   | **BmoStatus 16 B inline** |
| Strings           | `char*` nul   | `char*` nul   | **(ptr, len) UTF-8** |
| Handles           | `HANDLE void*`| `int fd`      | **BmoHandle with tag+generation** |
| Syscall range     | 0x1000+       | 0x0001..      | **0x100..0x1FF**   |

## What C ABI Lacks — What BMO ABI Provides

| C ABI problem                    | BMO ABI solution                      |
|----------------------------------|---------------------------------------|
| `int`/`long` size varies by arch | `bx_u*` fixed-width, explicit         |
| `errno` global + TLS overhead    | `BmoStatus` 16 B inline return value  |
| `int fd` — no type safety        | `BmoHandle` 64-bit tag + generation   |
| `char*` — null terminated        | `(ptr, len)` UTF-8 (planned)          |
| Scattered syscall numbers        | Compact range 0x100..0x1FF            |
| Platform-specific calling conv   | Single ABI: `bmo_call`                |

## What Exists Today

- Calling convention helpers (`align_stack`, `is_stack_aligned`)
- Fixed-width primitives with constants (`BX_U64_MAX`, ...)
- `BmoStatus` + 21 `ErrorCode` + `StatusFlags`
- `BmoHandle` 64-bit with tag bit + `HandleKind`
- `BmoInstant` backed by real TSC, `BmoDuration`
- `BmoAtomicU32/U64/Bool` + `MemOrder` + `BmoSpinLock` (TTAS)
- Syscall number table (`NR_*` constants for 0x100..0x1CF)
- BEF format: header, sections, manifest, signing, relocations, imports/exports
- `BmoFormat` pixel types (ARGB8, ...)
- `Capabilities`, `BmoFileType`, `BmoPerms`
- `BmoLanguageProfile` + `ALL_PROFILES`

## Future (when Ring 3 ships)

- String type: `BmoStr`/`BmoString` (ptr + len UTF-8, already drafted)
- GC interface for managed languages
- ARM64 trampolines
- Dedicated marshaller for JVM, CLR, Python

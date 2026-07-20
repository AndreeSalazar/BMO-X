# `bmo_abi` — BMO Application Binary Interface

> The native ABI for BMO. It defines BMO's binary contract instead of inheriting
> the C ABI (cdecl / Win64 / SysV AMD64). Every Ring 0 to Ring 3 boundary uses
> BMO types.

## Specification

Read **[`SPEC.md`](./SPEC.md)** — canonical specification including:

- Calling convention (7 GPRs, 256 B red zone, 64 B stack alignment)
- CPU contract: x86-64 Zen 3 (`Ryzen 5 5600X` default; Zen 3 EPYC profile)
- Type layouts with compile-time `static_assert!` (34 assertions)
- Syscall numbering (0x100..0x1FF) + `syscall0`–`syscall6` wrappers
- BEF format: header (48 B), sections, relocs, imports/exports, TLS, signing
- Reflect system wired to `TypeRegistry`

## Module Map (56 files, ~5 KLOC)

```
bmo_abi/
├── fundamentals/       Types used by EVERY BMO module (15 sub-modules)
│   ├── primitives/     bx_u8..u128, bx_i8..i128, bx_f32/64, bx_f16, bx_bool
│   ├── status/         BmoStatus (16 B), StatusFlags
│   ├── handle/         BmoHandle (64-bit tag+gen+idx), 34 HandleKind variants
│   ├── capability/     BmoCap + BmoCapSet (64-bit bitset)
│   ├── option/         BmoOption<T> FFI-safe repr(C)
│   ├── result/         BmoResult<T,E> FFI-safe repr(C)
│   ├── error/          BmoError (16 B unified error type)
│   ├── convert/        BmoStatus ↔ BmoError ↔ ErrorCode
│   ├── string/         BmoStr (borrowed 16 B), BmoString (owned 24 B)
│   ├── memory/         BmoSlice, BmoRange, BmoAligned
│   ├── buffer/         BmoBuffer (32 B shared memory descriptor)
│   ├── allocator/      BmoAllocator trait + Global wrapper
│   ├── io/             BmoRead, BmoWrite, BmoSeek, BmoPipe
│   ├── fmt/            BmoFormatter stack-allocated (256 B)
│   └── sync/           BmoAtomicU32/U64/Bool, BmoSpinLock
│
├── values/             Value types with own semantics (8 sub-modules)
│   ├── time/           BmoInstant (RDTSC monotonic), BmoDuration
│   ├── clock/          BmoClockId, sleep, sleep_until
│   ├── uuid/           BmoUuid 128-bit (RFC 4122)
│   ├── version/        BmoVersion semver (12 B)
│   ├── math/           sqrt, sin, cos, pow (Newton/Taylor, no_std)
│   ├── hash/           FNV-1a 32/64, CRC32c (SSE4.2), CRC32
│   ├── net/            BmoIpv4Addr, BmoIpv6Addr, BmoSocketAddr
│   └── reflect/        BmoTypeInfo, TypeKind, ReflectQuery + TypeRegistry
│
├── runtime/            TypeRegistry (256 slots), VTableStore, LangBridge
├── windowing/          BmoWindowClass, create info, 6 event types
├── fs/                 BmoFileHandle, BmoOpenFlags, BmoStat (72 B), BmoDirEntry
├── surface/            BmoFormat (22 pixel formats), BmoSurfaceInfo
├── error_code/         BmoErrorCode enum (21 codes), severity, flags
├── bef/                BEF format — complete toolchain
│   ├── header/         BefHeader 48 B, BefMagic::detect() (PE/ELF/BEF)
│   ├── sections/       SectionKind (10 types), SectionEntry 48 B
│   ├── writer/         BefBuilder + BefSection — produce valid BEF
│   ├── validator/      validate() — structural integrity check
│   ├── loader/         load() — zero-copy parser + import callback
│   ├── blake3/         Full BLAKE3 (294 L, no_std, no deps)
│   ├── relocations/    3 types: Abs64, Rel32, Got64
│   ├── imports/        ImportEntry 24 B, ImportTable
│   ├── exports/        ExportEntry 32 B, ExportTable
│   ├── symbols/        Symbol 32 B, SymbolTable
│   ├── manifest/       Provenance (Native/PeDevoured/ElfDevoured)
│   ├── tls/            TlsTemplate 24 B, TLS setup/teardown
│   └── signing/        SectionHash 40 B, SignatureHeader 8 B
│
├── syscalls/           Syscall number table (0x100..0x1FF) + syscall0–syscall6
└── profile/            BmoLanguageProfile + ALL_PROFILES
```

## Key Features

| Feature | Status |
|---------|--------|
| 34 `static_assert!` for repr(C) type sizes | ✅ |
| BEF writer/validator/loader | ✅ |
| BLAKE3 hashing (no_std) | ✅ |
| Syscall wrappers (syscall0–6) with inline asm | ✅ |
| Reflect system wired to TypeRegistry | ✅ |
| 42 unit tests + 7 integration tests passing | ✅ |
| PE/ELF detection (`BefMagic::detect`) | ✅ |
| Ed25519 signature infrastructure | ✅ |
| PE/ELF devourers | 🔜 |

## Comparison with Legacy ABIs

| Aspect            | MS x64        | SysV AMD64    | **BMO ABI**        |
|-------------------|---------------|---------------|--------------------|
| Integer args      | 4 GPRs        | 6 GPRs        | **7 GPRs**         |
| Shadow space      | 32 B          | 0 B           | **0 B**            |
| Stack alignment   | 16 B          | 16 B          | **64 B**           |
| Red zone          | 0 B           | 128 B         | **256 B**          |
| Return (≤128 bit) | RAX           | RAX:RDX       | **RAX:RDX**        |
| Error reporting   | `HRESULT`+TLS | `errno`+TLS   | **BmoStatus 16 B** |
| Strings           | `char*` nul   | `char*` nul   | **(ptr, len) UTF-8** |
| Handles           | `HANDLE void*`| `int fd`      | **BmoHandle 64-bit with tag+generation** |
| Syscall range     | 0x1000+       | 0x0001..      | **0x100..0x1FF**   |

## CPU profiles

BMO v1 is not a generic desktop target. Its native CPU contract is x86-64,
little-endian, 64-bit pointers, 4 KiB pages and the Zen 3 feature baseline
(`SSE4.2`, `AVX2`, `FMA`, `BMI1/2`, `AES`, `PCLMULQDQ`, `RDTSCP`, invariant
TSC). `cpu_profiles/` is the single ABI-facing source of those requirements.

Cargo selects the deployment profile:

```toml
bmo-abi = { path = "...", default-features = false, features = ["cpu-epyc-zen3"] }
```

`Ryzen 5 5600X` remains the default profile. A future ARM or RISC-V profile
will keep the BMO data model and BEF semantics, while supplying its own
register and CPU-feature contract.

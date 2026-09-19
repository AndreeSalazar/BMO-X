# `bmo_abi` -- the BMO-X contract, x86-64 only

> What two parts of BMO-X have to agree on: the two kernel doors, the calling
> convention, the BEF format and the types that cross the Ring 0 / Ring 3
> boundary. It is not a libc (that is `toolchain/lang/base` for C and
> `bmo-rt` for Rust) and it is not portable: BMO-X is x86-64 and nothing else
> (`toolchain/tools/isa`).

Canonical text: **[`SPEC.md`](./SPEC.md)**.

## What is here (2026-09-19) -- and every module has a live user

```
bmo_abi/
+-- fundamentals/
|   +-- primitives/     bx_u8..u64, bx_i*, bx_f*, bx_bool
|   +-- status/         BmoStatus: code | flags << 32 in RAX, value in RDX
|   +-- handle/         BmoHandle (tag 63, kind 62..56, gen 55..40, index 39..0)
|   +-- sync/           BmoSpinLock + atomics (used by bmo-rt's heap)
+-- types/              calling convention (IMPORTED by the C and INTI emitters)
|                       + aggregate layout rule (C, C++, COBOL, INTI)
+-- syscalls/           INVOKE (0x00), WAIT (0x02), syscall0..syscall6
+-- bef/                header, sections, relocations, symbols, signing (BLAKE3),
|                       requisitos, recursos, paquete, katanas, objeto (.bo),
|                       writer (BefBuilder), validator
+-- bex.rs              BEX = an executable BEF
+-- dynobj/             text, list, table: INTI's runtime objects
```

| Fact | Value | Where it is decided |
|---|---|---|
| Kernel doors | 2: INVOKE 0x00, WAIT 0x02 (0x01 reserved) | `syscalls/surface/puertas.rs` |
| Door arguments | RDI, RSI, RDX, R10, R8, R9 -> RAX (code), RDX (value) | `types/convention.rs` |
| Call arguments | RDI, RSI, RDX, RCX, R8, R9; 7th+ on the stack | `types/convention.rs::ARGUMENTOS` |
| Return | RAX | `types/convention.rs::RETORNO` |
| Preserved | RBX, RBP, R12-R15 | `types/convention.rs::PRESERVADOS` |
| Red zone | none | `types/convention.rs::RED_ZONE_BYTES` |
| Stack at `call` | 8 B guaranteed | `types/convention.rs::STACK_ALIGN_BYTES` |
| TLS | none (no one programs FS_BASE for Ring 3) | -- |
| ABI versions accepted | 2.x only | `supports_abi` + `bmo-bex-gate` (tied by a test) |
| Linking | static | `bef/objeto.rs`, `bmo-enlazar` |
| Machine | Ryzen 5 5600X, MEASURED | `PERFIL/CPU.txt`, kernel `cpu_vendor/` |

## What left on 2026-09-19, and why

About 8.500 lines with no live user: the v1 syscall table (0x100..0x1FF, which
the kernel answers with "unsupported" -- a `malloc` built on it wrote to
address 0xA), ABI 1.0 acceptance, a 7-register calling convention no emitter
ever used, `values/`, `runtime/`, `ir/`, `fs/`, `windowing/`, `surface/`,
`error_code/`, ten of the fourteen `fundamentals/`, the v1 loader, TLS setup
(a `wrmsr` from a contract crate), `cpu_profiles/` (a CPU chosen by a Cargo
feature) and `profile/`. The 25 `#![allow(dead_code)]` went with them.

The history is in git and in `SPEC.md`.

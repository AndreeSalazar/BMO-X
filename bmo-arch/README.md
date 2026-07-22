# bmo-platform

**The agnostic intermediary between Ring 0 and Ring 3.**

This is the only crate in BMO that knows which CPU the system is running
on. Every other Ring 3 crate (services, drivers, apps) talks to the
platform through this crate's traits and helpers, and never uses
`cfg(target_arch = ...)` directly.

## Why this crate exists

BMO is designed so that the same userland code runs on any CPU the
silicon happens to support. Today that is `x86_64` (AMD Ryzen 5 5600X
on the test bench). Tomorrow it could be `aarch64` (server / phone),
`riscv64` (embedded / OpenTitan), or a custom ASIC. The work to port
should be:

1. Add a new file under `src/arch/<arch>.rs` implementing the `Arch` trait.
2. Add a `#[cfg(target_arch = "...")]` arm in `src/arch/mod.rs`.
3. Add `arch-<arch>` to the `features` list in `Cargo.toml`.

**Nothing else changes.** `bmo-channel`, `bmo-abi`, the userland
services, the drivers, the apps, the BEF format, the BMO syscalls,
the language frontends — all stay the same.

## What lives here

| Module | Purpose |
|---|---|
| `arch` | The `Arch` trait + the per-CPU implementation. Today: `x86_64`. |
| `channel` | "Estuaries" — typed views over `bmo-channel` pages. |
| `runtime` | The Ring 3 boot path. Called once at process spawn. |

## The three pillars

### 1. `arch` — the CPU-aware layer

The `Arch` trait is the **only** thing that differs between CPU
ports. Every method is expressed in CPU-agnostic terms:

- `idle()` says "park the core until the next interrupt", not `hlt`.
- `syscall(nr, args)` issues a syscall, not `syscall` instruction.
- `monotonic_ns()` reads the monotonic clock, not `rdtsc`.

The implementation behind each method is what's CPU-specific.

### 2. `channel` — estuaries (typed IPC)

An "estuary" is a typed view over a `bmo-channel` page. The four
standard estuaries are:

| ID | Name | Direction | Used by |
|---|---|---|---|
| 1 | `Input` | Ring 0 → Ring 3 | `bmo-driver-keyboard`, `bmo-driver-mouse` |
| 2 | `Framebuffer` | Ring 3 → Ring 0 | `bmo-service-gui` (compositor) |
| 3 | `Syscall` | Ring 3 → Ring 0 | `bmo-rt` (for async syscalls) |
| 4 | `Log` | Ring 0 → Ring 3 | `bmo-service-cabina` (diagnostics) |

Custom estuaries (IDs 16+) are user-defined.

Each estuary is a 4096-byte shared page mapped into both Ring 0 and
Ring 3 address spaces. The protocol on each page is statically typed:
`InputEstuary` carries `InputEvent`s, `FramebufferEstuary` carries
`DrawCmd`s, etc. The wire format is `(opcode, arg0, arg1, arg2)`
inside a lock-free ring.

### 3. `runtime` — Ring 3 boot

Called once at process spawn. Reads the kernel's `BootContext`,
builds a CPU-agnostic `BootContextV1`, installs the active `Arch`
impl, and hands the userland four ready-to-use estuaries.

After `runtime::boot` returns, the rest of the process can use the
`bmo_platform` API as if it were running on any CPU.

## How a userland app uses it

```rust
#![no_std]
#![no_main]

use bmo_platform::prelude::*;
use bmo_platform::runtime::{boot, PlatformInfo};

#[no_mangle]
pub extern "C" fn _start(ctx_ptr: *const ()) -> ! {
    // 1. Boot the platform. After this, the four standard
    //    estuaries are ready and `arch::current()` works.
    let (info, mut estuaries) = unsafe {
        boot(ctx_ptr as *const boot_context::BootContext)
    };

    // 2. Identify the platform. No `cfg(target_arch = ...)` needed.
    serial_write(info.arch);     // "x86_64"
    serial_write(info.vendor);   // "AuthenticAMD"
    serial_write(info.brand);    // "AMD Ryzen 5 5600X 6-Core Processor"

    // 3. Send a draw command to the compositor.
    estuaries.framebuffer.send(DrawCmd::FillRect {
        x: 100, y: 100, w: 200, h: 50, color: 0x00FF_8040,
    });
    estuaries.framebuffer.send(DrawCmd::Present);

    // 4. Poll the input estuary.
    estuaries.input.poll(|ev| match ev {
        InputEvent::KeyDown { scancode } => { /* ... */ }
        InputEvent::MouseMove { dx, dy } => { /* ... */ }
        _ => {}
    });

    // 5. Issue a synchronous syscall (e.g. get time).
    let now_ns = arch::current().monotonic_ns(info.clock_freq_hz);

    // 6. Park until the next interrupt.
    arch::current().idle();
}
```

## How the kernel uses it

The kernel-side `bmo-channel` is independent of this crate — it
just allocates the 4096-byte pages and writes the addresses into
`BootContext.channel_pages[]`. The userland side uses
`bmo-platform` to discover and wrap those pages.

When a port to a new CPU is added, the kernel side changes to use
the new CPU's syscall mechanism, but `bmo-platform`'s `channel`
and `runtime` modules are unchanged.

## Layout

```
bmo-platform/
├── Cargo.toml              workspace + no_std lib, features: arch-x86_64 (default)
├── README.md               this file
└── src/
    ├── lib.rs              public API surface, three pillars
    ├── arch/
    │   ├── mod.rs          Arch trait + cfg dispatch
    │   └── x86_64.rs       X86_64 impl (the only arch today)
    ├── channel/
    │   └── mod.rs          Estuary<T>, 4 standard estuaries, Encode/Decode
    └── runtime/
        ├── mod.rs
        └── boot.rs         boot(ctx_ptr) → (PlatformInfo, Estuaries)
```

## Status

- v0.1.0: design complete, code written, not yet integrated into
  the kernel's `BootContext` (which needs a `channel_pages: [u64; 16]`
  field added).
- x86_64 is the only active arch.
- aarch64 / riscv64 are stub-features that will be filled in.

//! # BMO Platform — the agnostic intermediary between Ring 0 and Ring 3
//!
//! ## What this crate is
//!
//! `bmo-platform` is the **only** crate that knows which CPU architecture
//! the system is running on. Every other Ring 3 crate (services, drivers,
//! apps) talks to the platform through this crate's traits and helpers,
//! and **never uses `cfg(target_arch = ...)` directly**.
//!
//! ```text
//!   ┌─────────────────────────── bmo-platform ────────────────────────────┐
//!   │                                                                     │
//!   │  arch::Arch  ──>  current CPU's impl (x86_64 today)                 │
//!   │                    * syscall entry / trampoline                     │
//!   │                    * port I/O, MMIO, TSC                            │
//!   │                    * cache / barrier / fence                        │
//!   │                    * idle (`hlt`, `wfi`, …)                         │
//!   │                                                                     │
//!   │  channel::Estuary  ──>  typed wrapper over bmo-channel              │
//!   │                          * Input / Framebuffer / Syscall / Log      │
//!   │                          * each estuary = one shared page           │
//!   │                                                                     │
//!   │  runtime::Boot  ──>  Ring 3 process boot from BootContext           │
//!   │                       * discovers channel pages from ctx            │
//!   │                       * initializes the platform for this arch      │
//!   │                       * hands off to userland main                  │
//!   │                                                                     │
//!   └─────────────────────────────────────────────────────────────────────┘
//!                            ▲                              ▲
//!                            │                              │
//!                ┌───────────┴────────────┐    ┌────────────┴───────────┐
//!                │  Ring 3 userland       │    │  Ring 0 kernel         │
//!                │  (CPU-agnostic)        │    │  (CPU-specific)        │
//!                │  services, drivers,    │    │  kernel, faggin, uefi  │
//!                │  apps                  │    │                        │
//!                └────────────────────────┘    └────────────────────────┘
//! ```
//!
//! ## Why "agnostic" is the design goal
//!
//! Today the system runs on x86-64. Tomorrow it might run on aarch64
//! (servers, phones), riscv64 (embedded, OpenTitan), or even a custom
//! ASIC. The work to port should be:
//!
//! 1. Add a new file under `src/arch/<arch>.rs`.
//! 2. Wire a new `#[cfg(target_arch = "...")]` arm in `src/arch/mod.rs`.
//! 3. Add `arch-<arch>` to the `features` list in `Cargo.toml`.
//!
//! **Nothing else changes.** `bmo-channel`, `bmo-abi`, the userland
//! services, the drivers, the apps, the BEF format, the BMO syscalls,
//! the language frontends — all stay the same. They only see
//! `bmo_platform::*`, which on aarch64 resolves to a different `Arch`
//! impl, but with the same trait surface.
//!
//! ## What lives where
//!
//! - **`bmo-abi`**: pure data types, syscall numbers, BEF format. No CPU.
//! - **`bmo-channel`**: the lock-free ring buffer primitive. No CPU.
//! - **`bmo-platform`** *(this crate)*: the **CPU-aware** layer that
//!   glues `bmo-abi` and `bmo-channel` to whatever CPU the silicon runs.
//! - **`boot-context`**: the handoff struct (per-arch fields for now).
//! - **`bmo-rt`**: the Ring 3 userspace runtime. Talks to the kernel via
//!   `bmo-platform`, never via raw syscalls.
//!
//! ## Estuaries
//!
//! An "estuary" is a typed view over a `bmo-channel` page. Where
//! `bmo-channel` gives you a raw `(opcode, arg0, arg1, arg2)` ring,
//! an `Estuary<T>` gives you a typed channel of `T` messages — where
//! `T` is whatever the protocol on that estuary speaks.
//!
//! ```text
//!   bmo_channel::Channel   <─ one 4096-byte page, raw opcodes
//!           │
//!           ▼
//!   bmo_platform::Estuary<T>  <─ typed, protocol-aware wrapper
//!           │
//!           ├── InputEstuary      (KeyEvent, MouseEvent)
//!           ├── FramebufferEstuary (DrawRect, DrawText, Present)
//!           ├── SyscallEstuary     (BmoSyscall::Exit, BmoSyscall::Open, …)
//!           ├── LogEstuary         (LogLine, BmoCrashRecord)
//!           └── CustomEstuary<T>   (user-defined T)
//! ```
//!
//! Each estuary maps to a 4096-byte shared page. The kernel hands out
//! the physical addresses of those pages in `BootContext.channel_pages[]`
//! (a future extension). Ring 0 and Ring 3 both map the page into their
//! respective address spaces and communicate through it.
//!
//! ## Versioning
//!
//! This crate is at v0.1.0 and **its surface can change**. The contract
//! that Ring 0 and Ring 3 agree on is encoded in the
//! `bmo_platform::runtime::BootContextV1` struct (mirror of the
//! kernel's `BootContext` minus the x86-64-specific fields, plus
//! the channel-page addresses that this layer adds).

#![no_std]
#![allow(missing_docs)] // v0.1 — docs are added incrementally

// ── Public modules ────────────────────────────────────────────────
//
// The three pillars of the platform layer. Ring 3 code uses these;
// Ring 0 code also uses these to publish its end of the estuaries.

pub mod arch;
pub mod channel;
pub mod runtime;

// ── Re-exports for convenience ───────────────────────────────────
//
// The most common path: a userland app wants to do input, draw to
// the framebuffer, and log a line. The prelude brings those types
// in scope with a single `use bmo_platform::prelude::*;`.

pub mod prelude {
    pub use crate::arch::Arch;
    pub use crate::channel::{
        CustomEstuary, Estuary, EstuaryId, FramebufferEstuary,
        InputEstuary, LogEstuary, SyscallEstuary,
    };
    pub use crate::runtime::{boot, BootContextV1, PlatformInfo};
}

/// Platform version constant. Bumped whenever a backward-incompatible
/// change is made to the BootContextV1 layout or the Estuary opcodes.
pub const PLATFORM_VERSION: u32 = 1;

/// Total number of estuaries reserved by the platform spec.
/// Ring 0 must allocate at least this many shared pages in
/// `BootContext.channel_pages[]`. Matches `boot_context::MAX_CHANNEL_PAGES`.
pub const NUM_ESTUARIES: usize = 16;

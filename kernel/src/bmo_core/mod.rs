//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! BMO Core â€” Windowing API + UI + FS + Desktop + BEF Loader.
//!
//! v1.8.8: BMO Core es el **kernel lÃ³gico de Ring 3** en la arquitectura
//! Opus. DespuÃ©s de que Ring 0 termina el boot, le entrega el control.
//!
//! # Arquitectura limpia (v1.8.8)
//!
//! ```text
//! â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
//! â”‚ bmo_abi/   â† contrato: tipos puros, syscall numbers, BEF     â”‚
//! â”‚               (NO lÃ³gica, NO implementaciÃ³n)                  â”‚
//! â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â–²â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
//!               â”‚ usa
//! â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
//! â”‚ bmo_core/   â† este mÃ³dulo: implementa los handlers, mantiene â”‚
//! â”‚               el estado de ventanas, dispatch, etc.          â”‚
//! â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
//!               â–²
//!               â”‚ usa
//! â”Œâ”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”´â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”
//! â”‚ bmo_gpu/    â† bridge a GPU: shims PE/ELF, BSF shaders       â”‚
//! â””â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”˜
//! ```
//!
//! # Regla de oro
//!
//! - `bmo_core` **importa** de `bmo_abi` (contrato).
//! - `bmo_core` **NO redefine** tipos que ya estÃ¡n en `bmo_abi`.
//! - Si algo de `bmo_core` debe ser visible para Ring 3, vive primero
//!   en `bmo_abi` y `bmo_core` solo lo implementa.
//!
//! # SubmÃ³dulos
//!
//! - `bmo_api`   â€” dispatcher 0x100..0x1FF + WindowManager + Paint
//!                  Compositor. **Importa** syscall numbers de `bmo_abi`.
//! - `desktop`   â€” welcome screen + desktop shell (Ring 0 supervisor).
//! - `ui`        â€” framebuffer primitives + 8x16 font.
//! - `diag`      â€” diagnostic overlay + events + telemetry.
//! - `gustos`    â€” audio system (FM synth + chimes + procedural).
//! - `bef`       â€” BEF binary format + loaders (PE/ELF/native).
//!                  **Es la fuente Ãºnica de verdad** del formato BEF
//!                  (re-exportado en `bmo_abi::bef`).
//! - `fs`        â€” filesystems (exFAT, ramdisk) + VFS.
//!
//! # RelaciÃ³n con BMO ABI (v1.8.8)
//!
//! ```text
//! Ring 3 app
//!     â”‚  syscall (con nÃºmero de bmo_abi::syscalls::NR_*)
//!     â–¼
//! arch::syscall_entry â†’ bmo_core::bmo_api::dispatch_syscall
//!     â”‚  (bmo_api re-exporta syscall numbers de bmo_abi)
//!     â–¼
//! bmo_api::syscall::sys_*  (implementaciones reales)
//!     â”‚
//!     â–¼
//! bmo_api::BmoState (ventanas, handles, surfaces, timers)
//! ```
//!
//! # RelaciÃ³n con BMO GPU
//!
//! ```text
//! bmo_core::bmo_api::syscall (sys_xxx)
//!     â”‚ (cuando el syscall es de GPU)
//!     â–¼
//! bmo_gpu::* (shims PE, BSF shaders, compositor)
//!     â”‚
//!     â–¼ (futuro)
//! ring0::dev::amdgpu (driver real MMIO)
//! ```

#![allow(dead_code)]
#![allow(static_mut_refs)]

#[path = "bmo_core.rs"]
pub mod coord;

pub mod bmo_api;
pub mod desktop;
pub mod ui;
pub mod bef;
pub mod fs;
pub mod desktop3;
pub mod proc;




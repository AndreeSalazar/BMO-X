//! FastOS/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! BMO Core — Windowing API + UI + FS + Desktop + BEF Loader.
//!
//! v1.8.8: BMO Core es el **kernel lógico de Ring 3** en la arquitectura
//! Opus. Después de que Ring 0 termina el boot, le entrega el control.
//!
//! # Arquitectura limpia (v1.8.8)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │ bmo_abi/   ← contrato: tipos puros, syscall numbers, BEF     │
//! │               (NO lógica, NO implementación)                  │
//! └─────────────▲───────────────────────────────────────────────┘
//!               │ usa
//! ┌─────────────┴───────────────────────────────────────────────┐
//! │ bmo_core/   ← este módulo: implementa los handlers, mantiene │
//! │               el estado de ventanas, dispatch, etc.          │
//! └─────────────────────────────────────────────────────────────┘
//!               ▲
//!               │ usa
//! ┌─────────────┴───────────────────────────────────────────────┐
//! │ bmo_gpu/    ← bridge a GPU: shims PE/ELF, BSF shaders       │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! # Regla de oro
//!
//! - `bmo_core` **importa** de `bmo_abi` (contrato).
//! - `bmo_core` **NO redefine** tipos que ya están en `bmo_abi`.
//! - Si algo de `bmo_core` debe ser visible para Ring 3, vive primero
//!   en `bmo_abi` y `bmo_core` solo lo implementa.
//!
//! # Submódulos
//!
//! - `bmo_api`   — dispatcher 0x100..0x1FF + WindowManager + Paint
//!                  Compositor. **Importa** syscall numbers de `bmo_abi`.
//! - `desktop`   — welcome screen + desktop shell (Ring 0 supervisor).
//! - `ui`        — framebuffer primitives + 8x16 font.
//! - `diag`      — diagnostic overlay + events + telemetry.
//! - `gustos`    — audio system (FM synth + chimes + procedural).
//! - `bef`       — BEF binary format + loaders (PE/ELF/native).
//!                  **Es la fuente única de verdad** del formato BEF
//!                  (re-exportado en `bmo_abi::bef`).
//! - `fs`        — filesystems (exFAT, ramdisk) + VFS.
//!
//! # Relación con BMO ABI (v1.8.8)
//!
//! ```text
//! Ring 3 app
//!     │  syscall (con número de bmo_abi::syscalls::NR_*)
//!     ▼
//! arch::syscall_entry → bmo_core::bmo_api::dispatch_syscall
//!     │  (bmo_api re-exporta syscall numbers de bmo_abi)
//!     ▼
//! bmo_api::syscall::sys_*  (implementaciones reales)
//!     │
//!     ▼
//! bmo_api::BmoState (ventanas, handles, surfaces, timers)
//! ```
//!
//! # Relación con BMO GPU
//!
//! ```text
//! bmo_core::bmo_api::syscall (sys_xxx)
//!     │ (cuando el syscall es de GPU)
//!     ▼
//! bmo_gpu::* (shims PE, BSF shaders, compositor)
//!     │
//!     ▼ (futuro)
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


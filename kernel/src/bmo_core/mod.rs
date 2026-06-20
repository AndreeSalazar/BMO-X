//! BMO Core — Windowing API + UI + Lang + FS + Desktop.
//!
//! BMO Core es el "intermediate layer" entre Ring 0 (kernel privileged)
//! y Ring 3 (userland apps). Aquí vive toda la API de windowing, el
//! desktop GUI, los lenguajes (ÑEXO, BMOasm), el sistema de archivos,
//! y los diagnósticos.
//!
//! A diferencia de Ring 0, BMO Core no requiere privilegios especiales
//! para su lógica (la mayoría corre con Ring 0 implícito al estar en
//! el kernel image). Sin embargo, su estado se aísla lógicamente:
//! Ring 3 sólo puede acceder a BMO Core vía los 256 syscalls
//! 0x100..0x1FF (BMO API v2.0).
//!
//! Submódulos:
//!   bmo_api       — BMO API v2.0: 256 syscalls, window manager, paint compositor
//!   desktop       — Welcome + desktop Ring 0 supervisor
//!   ui            — Framebuffer primitives + 8x16 font
//!   diag          — Diagnostic overlay + events + telemetry
//!   barex         — BareX compatibility + shader loader
//!   gustos        — Audio system (FM synth, chimes, procedural tracks)
//!   bmo_abi       — BMO ABI primitives (handles, status, types)
//!   lang          — Languages: BMOasm (compiler) + ÑEXO (CLI + runtime)
//!   bef           — BEF binary devourer (PE/ELF/native)
//!   fs            — Filesystems: FAT32 + BMO-FS + ramdisk
//!   sandbox       — Application sandbox (capabilities)
//!
//! Contrato con Ring 0:
//!   - BMO Core puede llamar a `crate::*` libremente (mismo image).
//!   - Ring 0 expone `arch::cpu::rdtsc`, `arch::cpu::busy_wait_ms` y
//!     syscalls legacy que BMO Core usa para timing.
//!
//! Contrato con Ring 3 (ver ../ring3/mod.rs):
//!   - BMO Core expone 256 syscalls 0x100..0x1FF.
//!   - Ring 3 sólo ve tipos #[repr(C)] y fna signatures estables; nada
//!     más de la API interna.

#![allow(dead_code)]
#![allow(static_mut_refs)]

// ── Coordinator (orquesta init + enter) ──────────────────────────────
// El módulo `coord` apunta a `bmo_core.rs` al lado de este archivo.
#[path = "bmo_core.rs"]
pub mod coord;

pub mod bmo_api;
pub mod desktop;
pub mod ui;
pub mod diag;
pub mod barex;
pub mod gustos;
pub mod bmo_abi;
pub mod lang;
pub mod bef;
pub mod fs;
pub mod sandbox;

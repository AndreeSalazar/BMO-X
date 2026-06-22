//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! v1.8.8 (Phase 2): arquitectura reorganizada. El código del 5600X
//! vive en `vendor/amd/cpu/zen3/`. Ver `Rutas.md` y los comentarios
//! en `vendor/` para detalles.
//!
//! ## Compatibilidad
//!
//! - `crate::AMD` (solo docs) — para referencias, sin código.
//! - `crate::vendor::amd::cpu::zen3::foo` — alias legacy, sigue funcionando.
//! - `crate::vendor::amd::cpu::zen3::foo` — path nuevo, recomendado.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Hardware APIs (5 capas principales) ──────────────────────────────
pub mod arch;
pub mod mem;
pub mod dev;
pub mod proc;
pub mod cpu;

// ── Soporte de Ring 0 ───────────────────────────────────────────────
pub mod boot;

// ── CPU-specific (AMD Ryzen 5 5600X / Zen 3) ───────────────────────
pub mod vendor;

// ── Top-level Ring 0 ────────────────────────────────────────────────
mod panic;
pub mod coordinator;

// ── Módulos hermanos (vía path attribute) ───────────────────────────
#[path = "../bmo_core/mod.rs"]
pub mod bmo_core;
#[path = "../bmo_gpu/mod.rs"]
pub mod bmo_gpu;

// NOTA v1.8.7: `ring3` se desactiva temporalmente. No tiene consumidores.
// #[path = "../ring3/mod.rs"]
// pub mod ring3;

// ── AMD/ — Solo documentación (.md) ────────────────────────────────
#[path = "AMD/mod.rs"]
pub mod amd_docs;

// ── Nueva arquitectura: profile/bus/gpu/syscall ────────────────────
pub mod profile;
pub mod bus;
pub mod gpu;
pub mod syscall;

// Re-exports principales (BootInfo shared from bootloader).
pub use boot::info::{
    BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE,
};

// ── Entry point ─────────────────────────────────────────────────────

use core::arch::naked_asm;

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        "test rdi, rdi",
        "jz 2f",
        "mov rbx, rdi",
        "and rsp, -16",
        "mov rdi, rbx",
        "call kernel_main_real",
        "2: hlt",
        "jmp 2b",
    );
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    coordinator::main(boot_info_ptr);
}

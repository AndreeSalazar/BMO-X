//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! v1.8.15 (Phase 3): reordenado de inicialización. init_fastos_cpu()
//! y init_msrs() corren ANTES de las fases 1-4 para garantizar que
//! MTRR/PAT estén configurados antes de tocar el framebuffer.
//!
//! ## Orden de boot
//!
//!   0. Phase 0 (arch):  GDT + IDT + SYSCALL
//!   1. init_fastos_cpu: CPUID, MTRR/PAT, TSC, erratas
//!   2. init_msrs:       EFER, STAR, LSTAR, FMASK, PAT, TSC_AUX
//!   3. init_acpi:       ACPI tables
//!   4. Phase 1 (mem):   frame allocator + heap
//!   5. Phase 2 (dev):   ACPI/PCI discovery
//!   6. Phase 3 (display): GOP framebuffer (con MTRR/PAT correctos)
//!   7. Phase 4 (sched): scheduler + APIC timer + interrupts
//!   8. bmo_core::init:  cabina + defense + timeback + bmo_api + desktop
//!   9. welcome::run:    event loop (no retorna)

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
#[path = "../bmo_abi/mod.rs"]
pub mod bmo_abi;
#[path = "../cabina/mod.rs"]
pub mod cabina;
#[path = "../defense/mod.rs"]
pub mod defense;
#[path = "../timeback/mod.rs"]
pub mod timeback;
#[path = "../lang/mod.rs"]
pub mod lang;
#[path = "../userland/mod.rs"]
pub mod userland;

// ── AMD/ — Solo documentación (.md) — vive en kernel/src/AMD/ ─────
// No se carga como módulo Rust (solo .md files). Ver
// `kernel/src/AMD/ryzen_5_5600x.md` para la documentación.

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
        // ── Zero-init BSS (defensivo). El bootloader UEFI ya lo hace,
        //    pero algunas páginas (PAGE_SIZE-aligned) pueden quedar
        //    con basura de RAM si la UEFI las marcó como "BootServices"
        //    y el kernel las reclama sin zero-init. Esto causa #GP en
        //    load de static mut con valores random.
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 1f",                          // si BSS vacío, saltar
        "mov rdi, rax",
        "xor eax, eax",
        "rep stosb",
        "1:",
        // ── Entry normal
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

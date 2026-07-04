//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! Ring 0 is the base layer that prepares ALL hardware for the system.
//! It initializes CPU, memory, devices, and display, then stays in a
//! visible GOP-safe idle screen until a higher layer is connected.
//!
//! Boot order:
//!   1. _start: BSS zero, save boot_info_ptr
//!   2. kernel_main_real: early NVRAM breadcrumb
//!   3. phase_1_RING_0::main: full hardware init
//!   4. Ring 0 ready screen + heartbeat loop

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Core Ring 0 modules ─────────────────────────────────────────────
pub mod arch;
pub mod mm;
pub mod dev;
pub mod proc;
pub mod cpu;

// ── CABINA: daemon + panels (omniscient diagnostic infrastructure) ──
pub use cabina_core;
pub use cabina_daemon;
pub use cabina_panels;
#[path = "../cabina/mod.rs"]
pub mod cabina;

// ── BMO support dependencies ───────────────────────────────────────
#[path = "../bmo_gpu/mod.rs"]
pub mod bmo_gpu;


// ── Boot infrastructure (moved from boot/) ──────────────────────────
pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod visual;
pub mod font;
pub mod log;

// ── BMO Core: logical Ring 3 kernel (process mgmt, syscalls, UI, FS) ──
#[path = "../bmo_core/mod.rs"]
pub mod bmo_core;

// ── Main coordinator ────────────────────────────────────────────────
pub mod phase_1_RING_0;

// ── CPU-specific (AMD Ryzen 5 5600X / Zen 3) ───────────────────────
pub mod vendor;

// ── Omniscient infrastructure ───────────────────────────────────────
pub mod omni;

// ── Devour: PE/ELF → BEF translation ─────────────────────────────────
pub mod devour;

// ── Trilogía subsystems (defense + timeback + userland) ────────────
#[path = "../defense/mod.rs"]
pub mod defense;
#[path = "../timeback/mod.rs"]
pub mod timeback;
#[path = "../userland/mod.rs"]
pub mod userland;

// ── Other Ring 0 modules ────────────────────────────────────────────
mod panic;
pub mod profile;

pub use bmo_abi;

// Re-exports (BootInfo shared from bootloader)
pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

// ── Entry point ─────────────────────────────────────────────────────

use core::arch::naked_asm;

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // RDI = boot_info_ptr from bootloader. Save in R12 (preserved) BEFORE BSS zero.
        "mov r12, rdi",
        // Zero-init BSS (defensive).
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 1f",
        "mov rdi, rax",
        "xor eax, eax",
        "rep stosb",
        "1:",
        // Restore RDI and call kernel_main_real.
        "mov rdi, r12",
        "call kernel_main_real",
        "2: hlt",
        "jmp 2b",
    );
}

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    // ── SAFETY ORDER ────────────────────────────────────────────────
    // NVRAM writes FIRST, serial LAST. COM1 can hang if no serial
    // hardware responds (LSR=0x00 → infinite loop in serial_byte).
    // NVRAM is the ONLY reliable diagnostic when serial is dead.

    // 1. RAM marker — no deps, always works, survives warm reset
    unsafe {
        core::ptr::write_volatile(0x9_0000 as *mut u32, 0x464F_5343u32); // "FOSC"
        core::ptr::write_volatile(0x9_0004 as *mut u32, 0u32);
    }

    // 2. Register serial sink (enables cabina-daemon serial output)
    crate::serial::register_cabina_sink();

    // 3. CABINA daemon init (ring buffer + telemetry)
    cabina_daemon::init();

    // 4. NVRAM init + EARLIEST write — BEFORE any serial output
    if !boot_info_ptr.is_null() {
        let uefi_st = unsafe { (*boot_info_ptr).uefi_system_table };
        if uefi_st != 0 {
            crate::uefi_rt::init(uefi_st);
            crate::uefi_rt::write_boot_stage("kmain_early");
        }
    }

    // 5. NOW serial + ring buffer are active
    cabina_daemon::info("ring0", "kernel_main_real entered");
    cabina_daemon::info("ring0", "nvram init + kmain_early written");

    // Enter Ring 0 main coordinator. Returning means Ring 0 completed
    // successfully.
    let ctx = phase_1_RING_0::main(boot_info_ptr);

    // Transition to BMO Core (logical Ring 3 kernel). This initializes
    // all core subsystems and enters the welcome screen (never returns).
    crate::bmo_core::coord::init();
    crate::bmo_core::coord::enter(&ctx, 0, 0);
}

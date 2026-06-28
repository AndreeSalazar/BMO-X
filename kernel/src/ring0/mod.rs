//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! Ring 0 is the base layer that prepares ALL hardware for the system.
//! It initializes CPU, memory, devices, and display before handing off
//! to the next phase (bmo_core / desktop).
//!
//! Boot order:
//!   1. _start: BSS zero, save boot_info_ptr
//!   2. kernel_main_real: early NVRAM breadcrumb
//!   3. phase_1_RING_0::main: full hardware init
//!   4. Return to caller (next phase)

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


// ── Boot infrastructure (moved from boot/) ──────────────────────────
pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod visual;
pub mod font;
pub mod log;

// ── Main coordinator ────────────────────────────────────────────────
pub mod phase_1_RING_0;

// ── CPU-specific (AMD Ryzen 5 5600X / Zen 3) ───────────────────────
pub mod vendor;

// ── Omniscient infrastructure ───────────────────────────────────────
pub mod omni;

// ── Devour: PE/ELF → BEF translation ─────────────────────────────────
pub mod devour;

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

    // Enter Ring 0 main coordinator.
    phase_1_RING_0::main(boot_info_ptr);

    cabina_daemon::fault("ring0", "phase_1_RING_0 returned unexpectedly");
    // Force serial dump before halt
    cabina_daemon::info("ring0", "=== dumping cabina ring buffer ===");
    let cur = cabina_daemon::ring_buffer::next_seq();
    let start = if cur > 64 { cur - 64 } else { 1 };
    for seq in start..cur {
        if let Some(ev) = cabina_daemon::ring_buffer::event_by_seq(seq) {
            crate::dev::console::serial_write(&alloc::format!(
                "#{} {} {}: {}\n", ev.seq, ev.severity.name(), ev.module_str(), ev.msg_str()
            ));
        }
    }
    crate::dev::console::serial_write("=== end cabina dump ===\n");
    loop {
        unsafe { core::arch::asm!("hlt"); }
    }
}

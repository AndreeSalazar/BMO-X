//! Ring 0 â€” Hardware Abstraction Layer.
//!
//! Boot order (desde main.rs):
//!   1. _start: BSS zero, save boot_info_ptr
//!   2. kernel_main_real: early NVRAM breadcrumb
//!   3. phase_1_RING_0::main: full hardware init
//!   4. Ring 0 ready screen + heartbeat loop

// â”€â”€ Core Ring 0 modules â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod arch;
pub mod mm;
pub mod dev;
pub mod proc;
pub mod cpu;

// â”€â”€ CABINA: daemon + panels (omniscient diagnostic infrastructure) â”€â”€
pub use cabina_core;
pub use cabina_daemon;
pub use cabina_panels;
#[path = "../cabina/mod.rs"]
pub mod cabina;

// â”€â”€ BMO support dependencies â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#[path = "../bmo_gpu/mod.rs"]
pub mod bmo_gpu;


// â”€â”€ Boot infrastructure (moved from boot/) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod visual_ring_0;
pub mod visual_ring_3;
pub mod visual;
pub mod font;
pub mod log;

// â”€â”€ BMO Core: logical Ring 3 kernel (process mgmt, syscalls, UI, FS) â”€â”€
#[path = "../bmo_core/mod.rs"]
pub mod bmo_core;

// â”€â”€ Main coordinator â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod phase_1_RING_0;

// â”€â”€ CPU-specific (AMD Ryzen 5 5600X / Zen 3) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod vendor;

// â”€â”€ Omniscient infrastructure â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod omni;

// â”€â”€ Devour: PE/ELF â†’ BEF translation â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
pub mod devour;

// â”€â”€ TrilogÃ­a subsystems (defense + timeback + userland) â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
#[path = "../defense/mod.rs"]
pub mod defense;
#[path = "../timeback/mod.rs"]
pub mod timeback;
#[path = "../userland/mod.rs"]
pub mod userland;

// â”€â”€ Other Ring 0 modules â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
mod panic;
pub mod profile;

pub use bmo_abi;

// Re-exports (BootInfo shared from bootloader)
pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

// â”€â”€ Entry point â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€

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
extern "C" fn kernel_main_real(boot_info_ptr: *const bmo_boot_protocol::BootInfo) -> ! {
    // â”€â”€ SAFETY ORDER â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
    // NVRAM writes FIRST, serial LAST. COM1 can hang if no serial
    // hardware responds (LSR=0x00 â†’ infinite loop in serial_byte).
    // NVRAM is the ONLY reliable diagnostic when serial is dead.

    // 1. RAM marker â€” no deps, always works, survives warm reset
    unsafe {
        core::ptr::write_volatile(0x9_0000 as *mut u32, 0x464F_5343u32); // "FOSC"
        core::ptr::write_volatile(0x9_0004 as *mut u32, 0u32);
    }

    // 2. Register serial sink (enables cabina-daemon serial output)
    crate::serial::register_cabina_sink();

    // 3. CABINA daemon init (ring buffer + telemetry)
    cabina_daemon::init();

    // 4. NVRAM init + EARLIEST write â€” BEFORE any serial output
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

    // Transition to BMO Core (logical Ring 3 kernel) subsystems.
    crate::bmo_core::coord::init();

    // Real CPU-level transition to Ring 3 (CPL=3).
    // Draws a purple border to visually confirm Ring 3 execution.
    self::visual_ring_3::jump_to_ring3();
}

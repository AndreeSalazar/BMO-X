//! Kernel entry point — minimal BSS zero-init and early boot breadcrumbs.
//!
//! Contains the raw `_start` entry (first code executed) and `kernel_main_real`
//! which sequences early init before delegating to the Ring 0 boot phases.

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
    // ── SAFETY ORDER ──────────────────────────────────────────
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
    let ctx = super::boot_phase::main(boot_info_ptr);

    // Transition to BMO Core (logical Ring 3 kernel) subsystems.
    crate::dev::console::serial_write("[entry] boot_phase complete, starting coord::init\n");
    crate::ring0::boot_phase::write_crash_marker(6);
    crate::uefi_rt::write_boot_stage("coord_init");
    bmo_core::coord::init();
    crate::dev::console::serial_write("[entry] coord::init complete, entering desktop\n");
    crate::ring0::boot_phase::write_crash_marker(8);
    crate::uefi_rt::write_boot_stage("welcome_dispatch");

    // Enter the real desktop with Mac-like compositor (alpha, blur, shadows, dock).
    bmo_core::desktop::commands::enter_desktop();
}

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

    // 2. NVRAM init + EARLIEST write — BEFORE any serial output
    if !boot_info_ptr.is_null() {
        let uefi_st = unsafe { (*boot_info_ptr).uefi_system_table };
        if uefi_st != 0 {
            crate::uefi_rt::init(uefi_st);
            crate::uefi_rt::write_boot_stage("kmain_early");
        }
    }

    // 3. Serial output now possible
    crate::dev::console::serial_write("[entry] kernel_main_real entered\n");

    // Enter Ring 0 main coordinator. Returning means Ring 0 completed successfully.
    super::boot_phase::main(boot_info_ptr);

    // Instead of calling bmo_core::coord::init() directly (which is now in a separate
    // module binary), load the desktop module from S: and hand over control.
    crate::dev::console::serial_write("[entry] boot_phase complete, loading desktop module\n");
    crate::ring0::boot_phase::write_crash_marker(6);
    crate::uefi_rt::write_boot_stage("module_load");

    unsafe {
        let hal = crate::ring0::hal_init::HAL_SERVICES;
        if !hal.is_null() {
            let hal_ref = &*hal;
            crate::ring0::mod_loader::load_bmo_core(hal_ref, boot_info_ptr)
        } else {
            crate::dev::console::serial_write("[entry] FATAL: HalServices is null\n");
            loop { unsafe { core::arch::asm!("hlt"); } }
        }
    }
}

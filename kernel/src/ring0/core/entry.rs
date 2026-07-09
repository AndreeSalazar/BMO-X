//! Kernel entry point — raw `_start` + early boot sequence.
//!
//! ```text
//! _start (naked asm)
//!   ├── Save RDI → R12
//!   ├── Zero BSS (stosq + stosb, ~2ms for 16 MB)
//!   ├── Restore RDI ← R12
//!   └── call kernel_main_real ─┐
//!                               ├─ 1. RAM marker ("FOSC")
//!                               ├─ 2. NVRAM (before serial — COM1 can hang)
//!                               ├─ 3. Serial output
//!                               ├─ 4. boot_phase::main() — phases 0..4
//!                               ├─ 5. HAL build + module load
//!                               └─ 6. load_bmo_core → Ring 3 desktop
//! ```

use core::arch::naked_asm;

// ── _start: first code executed ─────────────────────────────────────────

#[unsafe(no_mangle)]
#[link_section = ".text._start"]
#[unsafe(naked)]
unsafe extern "C" fn _start() -> ! {
    naked_asm!(
        // RDI = boot_info_ptr from bootloader. Save in R12 (callee-saved).
        "mov r12, rdi",

        // ── Zero BSS (16 MB typical, ~2ms with stosq) ──────────
        "lea rax, [rip + __bss_start]",
        "lea rcx, [rip + __bss_end]",
        "sub rcx, rax",
        "jz 2f",
        "mov rdi, rax",
        "xor eax, eax",
        // Qword fast path
        "mov rdx, rcx",
        "shr rcx, 3",
        "jz 1f",
        "rep stosq",
        // Byte remainder
        "1: and rdx, 7",
        "mov rcx, rdx",
        "jz 2f",
        "rep stosb",

        // ── Enter kernel ───────────────────────────────────────
        "2: mov rdi, r12",
        "call kernel_main_real",

        // Halt if kernel_main_real ever returns
        "3: hlt",
        "jmp 3b",
    );
}

// ── kernel_main_real: early boot sequence ───────────────────────────────

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const bmo_boot_protocol::BootInfo) -> ! {
    // ═══ STAGE 1: RAM marker (no deps, survives warm reset) ═══
    unsafe {
        core::ptr::write_volatile(0x9_0000 as *mut u32, 0x464F_5343u32); // "FOSC"
        core::ptr::write_volatile(0x9_0004 as *mut u32, 0u32);
    }

    // ═══ STAGE 2: NVRAM init (BEFORE serial — COM1 can hang on LSR=0x00) ═══
    if !boot_info_ptr.is_null() {
        let uefi_st = unsafe { (*boot_info_ptr).uefi_system_table };
        if uefi_st != 0 {
            crate::uefi_rt::init(uefi_st);
            crate::uefi_rt::write_boot_stage("kmain_early");
        }
    }

    // ═══ STAGE 3: Serial output now possible ═══
    crate::dev::console::serial_write("[entry] kernel_main_real entered\n");

    // ═══ STAGE 4: Boot phases 0..4 (arch, mm, dev, display, sched) ═══
    super::phase::main(boot_info_ptr);

    // ═══ STAGE 5: Module load — hand control to Ring 3 desktop ═══
    crate::dev::console::serial_write("[entry] boot_phase complete, loading desktop module\n");
    crate::boot_phase::write_crash_marker(6);
    crate::uefi_rt::write_boot_stage("module_load");

    unsafe {
        let hal = crate::hal_init::HAL_SERVICES;
        if hal.is_null() {
            crate::dev::console::serial_write("[entry] FATAL: HalServices is null\n");
            loop { core::arch::asm!("hlt"); }
        }
        crate::mod_loader::load_bmo_core(&*hal, boot_info_ptr)
    }
}

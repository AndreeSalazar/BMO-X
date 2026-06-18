//! FastOS / BMO Kernel — Entry Point.
//!
//! Boot path (phased, timed, zero-duplication):
//!
//!   Phase 0: CPU init (FPU/SSE/AVX/MTRR/PAT/perf counters)
//!   Phase 1: Memory (page allocator, heap validation)
//!   Phase 2: Devices (ACPI/PCI enumeration)
//!   Phase 3: Display (GOP framebuffer)
//!   Phase 4: Scheduler (APIC timer, interrupts)
//!   Phase 5: Desktop (welcome screen → shell)
//!
//! Each phase lives in `crate::boot::phases`. The entry point below is
//! intentionally tiny: it just validates the boot protocol handoff, stores
//! the boot info globals, paints the first visual checkpoint, and dispatches
//! to each phase in order. All logging flows through `crate::boot::log`.

#![no_std]
#![no_main]

extern crate alloc;

mod allocator;
mod arch;
mod boot_info;
mod ui;
mod diag;
mod desktop;
mod drivers;
mod fs;
mod panic;
mod memory;

mod bmo_abi;

mod barex;
mod bef;
mod sched;
mod syscall;
mod sandbox;

mod lang;
mod security;
mod windows_compat;

mod boot;

use core::arch::naked_asm;

// ── Entry points ───────────────────────────────────────────────────

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

// ── Kernel main — phased boot ──────────────────────────────────────

#[unsafe(no_mangle)]
#[inline(never)]
extern "C" fn kernel_main_real(boot_info_ptr: *const fastos_boot_protocol::BootInfo) -> ! {
    drivers::serial::init_serial();
    boot::log::info("boot", "FastOS BMO Kernel v0.9.0 starting");

    if boot_info_ptr.is_null() {
        boot::log::fault("boot", "boot_info_ptr is NULL");
    }
    let bi = unsafe { &*boot_info_ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        boot::log::fault("boot", "BootInfo magic mismatch");
    }

    unsafe {
        boot_info::BOOT_INFO         = boot_info_ptr;
        boot_info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
        boot_info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
        boot_info::FB_ADDR  = bi.fb_addr;
        boot_info::FB_WIDTH = bi.fb_width;
        boot_info::FB_HEIGHT = bi.fb_height;
        boot_info::FB_STRIDE = bi.fb_stride;
    }

    boot::visual::clear();
    boot::visual::log("boot", "K0 BootInfo received; framebuffer direct writer online",
        boot::visual::color::OK);

    diag::init();

    let boot_start = arch::cpu::rdtsc();

    let (cpu, prev) = boot::phases::phase0_cpu::run(boot_start);
    let _ = cpu; let _ = prev; // ensure value used (compiler hint)

    boot::phases::ring3_tests::run_all_tests();

    let (mem, prev) = boot::phases::phase1_memory::run(bi, prev);
    boot::phases::ring3_tests::run_codegen_tests();

    let prev = boot::phases::phase2_devices::run(bi, prev);
    let prev = boot::phases::phase3_display::run(bi, prev);
    let prev = boot::phases::phase4_scheduler::run(prev);
    boot::phases::phase5_desktop::run(bi, &cpu, &mem, boot_start, prev);
}

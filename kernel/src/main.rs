//! FastOS / BMO Kernel — Entry Point.
//!
//! Pure orchestrator. Validates the boot protocol handoff, then dispatches
//! each phase in order. Every phase lives in `crate::boot::phases` and
//! implements `boot::phases::Phase`, which provides both `run` (normal
//! boot) and `self_test` (isolated, non-destructive).
//!
//! Phases are black boxes to main.rs. To add a new phase, add a module
//! under `boot::phases`, implement the `Phase` trait, and add one line
//! to the dispatch below.

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
mod gustos;

mod boot;

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
    drivers::serial::init_serial();
    boot::log::info("boot", "FastOS BMO Kernel v1.1.0 starting");

    let bi_ref = match validate_boot_info(boot_info_ptr) {
        Ok(bi) => bi,
        Err(msg) => boot::log::fault("boot", msg),
    };
    unsafe { store_boot_info(bi_ref); }

    boot::visual::clear();
    boot::visual::log("boot", "K0 BootInfo received", boot::visual::color::OK);

    boot::visual::log("boot", "K0a diag::init...", boot::visual::color::OK);
    diag::init();
    boot::visual::log("boot", "K0b diag::init DONE", boot::visual::color::OK);

    // v1.1.0: Build a single BootContext and pass it through every phase.
    boot::visual::log("boot", "K0c ctx::new...", boot::visual::color::OK);
    // v1.5.1: pass *const BootInfo to avoid 4.2 KB stack copy
    let mut ctx = boot::BootContext::new(bi_ref as *const fastos_boot_protocol::BootInfo);
    boot::visual::log("boot", "K0d ctx::new DONE", boot::visual::color::OK);

    let t0 = arch::cpu::rdtsc();

    // Run phases. Each `Phase::run` returns the TSC tick at which it ended.
    boot::visual::log("boot", "K0e phase0_cpu::run...", boot::visual::color::OK);
    let (cpu, out0) = boot::phases::phase0_cpu::run(&mut ctx, t0);
    boot::visual::log("boot", "K0f phase0_cpu::run DONE", boot::visual::color::OK);

    // v1.5.1: pass `&ctx` instead of `&bi_copy`. The BootContext holds
    // a *const BootInfo (no stack copy needed).

    boot::visual::log("boot", "K1 phase1_memory::run...", boot::visual::color::OK);
    let (mem, out1) = boot::phases::phase1_memory::run(&mut ctx, out0.prev_end);
    boot::visual::log("boot", "K1 phase1_memory::run DONE", boot::visual::color::OK);

    boot::visual::log("boot", "K2 phase2_devices::run...", boot::visual::color::OK);
    let out2 = boot::phases::phase2_devices::run(&mut ctx, out1.prev_end);
    boot::visual::log("boot", "K2 phase2_devices::run DONE", boot::visual::color::OK);

    boot::visual::log("boot", "K3 phase3_display::run...", boot::visual::color::OK);
    let out3 = boot::phases::phase3_display::run(&ctx, out2.prev_end);
    boot::visual::log("boot", "K3 phase3_display::run DONE", boot::visual::color::OK);

    boot::visual::log("boot", "K4 phase4_scheduler::run...", boot::visual::color::OK);
    let out4 = boot::phases::phase4_scheduler::run(out3.prev_end);
    boot::visual::log("boot", "K4 phase4_scheduler::run DONE", boot::visual::color::OK);

    // Full diag sinks are safe only after the boot-critical path is complete.
    diag::mark_boot_ready();

    // Phase 5 consumes the full boot aggregate; it does not return.
    boot::visual::log("boot", "K5 phase5_desktop::run...", boot::visual::color::OK);
    boot::phases::phase5_desktop::run(&ctx, &cpu, &mem, t0, out4.prev_end);
}

fn validate_boot_info(ptr: *const fastos_boot_protocol::BootInfo)
    -> Result<&'static fastos_boot_protocol::BootInfo, &'static str>
{
    if ptr.is_null() {
        return Err("boot_info_ptr is NULL");
    }
    let bi = unsafe { &*ptr };
    if bi.magic != fastos_boot_protocol::BOOT_MAGIC {
        return Err("BootInfo magic mismatch");
    }
    Ok(bi)
}

unsafe fn store_boot_info(bi: &fastos_boot_protocol::BootInfo) {
    boot_info::BOOT_INFO         = bi as *const _;
    boot_info::RESERVED_PAYLOAD_ADDR = bi.gsp_addr;
    boot_info::RESERVED_PAYLOAD_SIZE = bi.gsp_size;
    boot_info::FB_ADDR  = bi.fb_addr;
    boot_info::FB_WIDTH = bi.fb_width;
    boot_info::FB_HEIGHT = bi.fb_height;
    boot_info::FB_STRIDE = bi.fb_stride;
}

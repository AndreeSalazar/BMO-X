//! s1_cpu -- AMD Ryzen 5 5600X (Zen 3) optimized UEFI handoff + CPU init.
//!
//! This stage is the ONE place that knows the target CPU. Everything
//! CPU-specific lives here, not in the kernel, because:
//!   - It's a one-time setup (boot time only)
//!   - It's CPU-specific (not portable -- each CPU has its quirks)
//!   - The kernel should be generic (portable to AArch64/RISC-V later)
//!   - CPU optimizations like Zen 3 mitigations need to be applied early
//!
//! Zen 3 (Ryzen 5 5600X) specific features enabled here:
//!   - CPUID topology extension (0x80000026): 6C/12T, 1 CCD, 1 CCX
//!   - L1 32KB, L2 512KB, L3 32MB cache hierarchy
//!   - SYSCALL/SYSRET via AMD K8 ABI (different from Intel's)
//!   - Spectre v1 mitigations (SSBD, RSB fill)
//!   - SME/SEV support detection
//!   - Boost clock awareness (4.6 GHz max)
//!   - TSC calibration (3.7 GHz base, 4.6 GHz boost)
//!   - WBNOINVD, RDPID, RDTSCP, INVLPGB, CLZERO Zen 3+ features
//!   - AMD topology: cores per CCX, threads per core, CCD count

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![allow(static_mut_refs)]
#![allow(unsafe_op_in_unsafe_fn)]

use core::panic::PanicInfo;
pub use core::arch::{asm, naked_asm};
pub use boot_context::{BootContext, MemoryEntry, MAX_MEMORY_ENTRIES, KERNEL_STAGE_INDEX, MAX_STAGES};

// == THE STAGE, IN PIECES ============================================
//
// `main.rs` was 1.665 lines: UEFI structs, a serial port, descriptor tables,
// CPU detection, vendor MSRs, SMP bring-up and the entry point, in one file.
//
// The cut that matters is `cpu/`. The kernel already has a CPU-profile
// contract (`ring0/cpu_vendor/profile.rs`) whose header says a new CPU should
// be a profile swap and never a kernel edit -- and this stage, the one that
// runs FIRST, was the one with Zen 3 spread through it.

/// THE FIRMWARE SIDE: UEFI types, protocol GUIDs, and the four stages that end
/// at `ExitBootServices` -- after which there is no firmware left to ask.
mod uefi;
/// MSRs AND `CPUID`: the vocabulary, with no policy in it.
mod msr;
/// COM1: the only output that exists before there is a screen.
mod serial;
/// GDT, TSS AND IDT: the tables, and the code that loads them, together.
mod descriptors;
/// THE CPU: generic bring-up, detection, and the Zen 3 profile. A second CPU is
/// a new file in there, not a search through this stage.
mod cpu;
/// THE OTHER CORES: an AP starts in 16-bit real mode and has to be walked to 64.
mod smp;

pub use uefi::*;
pub use msr::*;
pub use serial::*;
pub use descriptors::*;
pub use cpu::*;
pub use smp::*;



// ===================================================================

static mut CTX: BootContext = unsafe { core::mem::zeroed() };

/// Unified-shim handoff: where the kernel image lives until the post-EBS
/// copy to 0x400000 (0 = not preloaded; the ESP loader placed it).
static mut PRELOAD_KERNEL_SRC: u64 = 0;
static mut PRELOAD_KERNEL_SIZE: u64 = 0;

/// Scratch buffer for GOP/connect-all handle enumeration.
static mut ALL_GOP: [EfiHandle; 256] = [core::ptr::null_mut(); 256];

// ===================================================================
//  ENTRY POINT
// ===================================================================

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text._start")]
pub extern "efiapi" fn s1_entry(
    image_handle: EfiHandle,
    system_table: *mut core::ffi::c_void,
    preload: *const boot_context::PreloadInfo,
) -> ! {
    serial_init();
    ser_print!("\n[s1_cpu] === BMO BOOT START (Zen 3) ===\n");

    // 1. Setup BootContext
    let ctx_ptr: *mut BootContext = core::ptr::addr_of_mut!(CTX);
    let ctx = unsafe { &mut *ctx_ptr };
    ctx.magic = boot_context::MAGIC;
    // Must match boot_context::VERSION exactly -- the kernel's is_valid()
    // rejects any other value. Use the shared constant so the two never
    // drift again (this hardcoded 2-vs-3 mismatch halted the kernel right
    // after entry on real hardware).
    ctx.version = boot_context::VERSION;
    ser_print!("[s1_cpu] magic=0x"); ser_hex!(ctx.magic);
    ser_print!(" version="); ser_dec!(ctx.version as usize); ser_print!("\n");

    unsafe { con_mark(system_table as *mut EfiSystemTable, "s1:enter "); }
    // 2. Memory map
    let mut mem_buf = [0u8; 32768];
    let ec = unsafe { fill_memory_map(ctx, &mut mem_buf, system_table as *mut EfiSystemTable) };
    ser_print!("[s1_cpu] memory map: "); ser_dec!(ec); ser_print!(" entries\n");

    unsafe { con_mark(system_table as *mut EfiSystemTable, "mm "); }
    // 3. GOP framebuffer
    unsafe { fill_gop(ctx, system_table as *mut EfiSystemTable); }

    unsafe {
        if ctx.fb_addr != 0 { con_mark(system_table as *mut EfiSystemTable, "gop+ "); }
        else { con_mark(system_table as *mut EfiSystemTable, "gop- "); }
    }
    // 4. Load s2_mem + kernel. Preferred path: the unified shim already
    // copied both to their fixed addresses and handed off sizes in r8 --
    // no ESP access needed (some firmwares never expose SimpleFS). The
    // ESP loader remains as fallback for legacy/QEMU boots.
    let preloaded = !preload.is_null()
        && unsafe { (*preload).magic } == boot_context::PRELOAD_MAGIC;
    if preloaded {
        let p = unsafe { &*preload };
        ctx.stage_base[0] = 0x100000;
        ctx.stage_entry[0] = 0x100000;
        ctx.stage_base[1] = 0x200000;
        ctx.stage_size[1] = p.s2_size;
        ctx.stage_entry[1] = 0x200000;
        ctx.stage_base[KERNEL_STAGE_INDEX] = 0x400000;
        ctx.stage_size[KERNEL_STAGE_INDEX] = p.kernel_size;
        ctx.stage_entry[KERNEL_STAGE_INDEX] = 0x400000;
        // The kernel bytes stay in the shim image until the post-EBS copy
        // (see exit_boot_services_and_jump).
        unsafe {
            PRELOAD_KERNEL_SRC = p.kernel_src;
            PRELOAD_KERNEL_SIZE = p.kernel_size;
        }
        ser_print!("[s1_cpu] stages preloaded by shim: s2=");
        ser_dec!(p.s2_size as usize);
        ser_print!(" kernel=");
        ser_dec!(p.kernel_size as usize);
        ser_print!(" bytes\n");
    } else if !unsafe { load_from_esp(ctx, system_table as *mut EfiSystemTable, image_handle) } {
        ser_print!("[s1_cpu] FATAL: load failed\n");
        loop { unsafe { asm!("hlt"); } }
    }

    unsafe { con_mark(system_table as *mut EfiSystemTable, "pre "); }
    // 5. CPU detection (AMD Ryzen 5 5600X specific)
    ser_print!("\n[s1_cpu] === AMD ZEN 3 DETECTION ===\n");
    unsafe { detect_cpu(); }

    unsafe { con_mark(system_table as *mut EfiSystemTable, "cpu "); }

    // -- Disable interrupts before swapping the descriptor tables ------
    // The firmware handed off with interrupts ENABLED. If a device IRQ
    // (timer, keyboard, SATA...) fires between `lgdt` and our IDT being
    // installed, the CPU dispatches it through an inconsistent GDT/IDT
    // state and triple-faults -- a reset, not a fault we can catch. This
    // is the classic reason a kernel boots under QEMU (no live IRQs at
    // boot) but resets on real hardware. Do what Linux does at its first
    // boot steps: mask everything now, re-enable only once the kernel has
    // its own IDT + LAPIC. `cli` covers the CPU; masking the legacy 8259
    // PIC covers any IRQ the firmware/other drivers left armed.
    unsafe {
        asm!("cli", options(nomem, nostack));
        outb(0x21, 0xFF);
        outb(0xA1, 0xFF);
    }

    // 6. GDT + IDT (universal x86-64)
    ser_print!("\n[s1_cpu] === UNIVERSAL CPU INIT ===\n");
    unsafe { init_gdt(); }
    ser_print!("[s1_cpu] GDT + TSS loaded\n");
    unsafe { init_idt(); }
    ser_print!("[s1_cpu] IDT loaded\n");

    unsafe { con_mark(system_table as *mut EfiSystemTable, "gdt "); }
    // 7. CR0/CR4/XCR0
    unsafe { init_cr0_cr4(); }

    // 8. FPU
    unsafe { init_fpu(); }

    // 9. TSC
    unsafe { init_tsc(); }

    unsafe { con_mark(system_table as *mut EfiSystemTable, "crs "); }
    // 10. AMD MSRs (Zen 3 specific)
    ser_print!("\n[s1_cpu] === AMD ZEN 3 MSR INIT ===\n");
    unsafe { init_amd_msrs(); }
    unsafe { con_mark(system_table as *mut EfiSystemTable, "am "); }

    // 11. Zen 3 performance configuration
    unsafe { init_zen3_perf(); }
    unsafe { con_mark(system_table as *mut EfiSystemTable, "pf "); }

    // 12. SYSCALL (AMD K8 ABI)
    unsafe { init_syscall(); }

    // SMP remains disabled until its real-mode trampoline and low-memory page
    // tables are reserved and built correctly.  Boot the BSP reliably first.

    unsafe { con_mark(system_table as *mut EfiSystemTable, "msr "); }
    // 13. Publish CPU profile to BootContext
    ctx.gdt_ptr = core::ptr::addr_of!(GDT) as u64;
    ctx.tss_ptr = core::ptr::addr_of!(TSS) as u64;
    ctx.idt_ptr = core::ptr::addr_of!(IDT) as u64;
    ctx.kernel_stack_top = core::ptr::addr_of!(KSTK) as u64 + KSTACK_SIZE as u64;
    ctx.tsc_freq = unsafe { CPU.tsc_freq };
    ctx.syscall_entry = syscall_entry_stub as *const () as u64;

    ser_print!("\n[s1_cpu] === ALL ZEN 3 INIT DONE ===\n");
    ser_print!("[s1_cpu] Ryzen 5 5600X: 6C/12T, 3.7GHz base, 4.6GHz boost\n");
    ser_print!("[s1_cpu] Cache: L1 32K, L2 512K, L3 32M\n");

    unsafe { con_mark(system_table as *mut EfiSystemTable, "cpuOK ebs...\n"); }
    // 14. ExitBootServices + jump to s2_mem. Post-EBS the firmware console
    // is gone; the only output is the framebuffer, which the GOP GUID fix
    // above should now provide. s1/s2/kernel each paint a colored bar
    // (green/cyan/magenta) so the boot is visible without a serial cable.
    unsafe { exit_boot_services_and_jump(ctx_ptr, system_table as *mut EfiSystemTable, image_handle, S2_ADDR); }
}

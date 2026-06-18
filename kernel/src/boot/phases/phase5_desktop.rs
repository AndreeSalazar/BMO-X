//! Phase 5 — Desktop.
//!
//! Prints the kernel boot banner on the console, initialises the desktop
//! subsystem, and launches the welcome screen. This phase does not return —
//! the welcome screen blocks until the user types `Run`.

use crate::{allocator, boot::log, desktop, sched, ui};
use super::phase0_cpu::CpuState;
use super::phase1_memory::MemState;
use fastos_boot_protocol;

pub fn run(
    bi: &fastos_boot_protocol::BootInfo,
    cpu: &CpuState,
    mem: &MemState,
    boot_start: u64,
    phase4_end: u64,
) -> ! {
    log::info("phase5", "=== Phase 5: Desktop ===");
    crate::boot::visual::log("phase5", "=== Phase 5: Desktop ===",
        crate::boot::visual::color::HEADER);

    let boot_end = crate::arch::cpu::rdtsc();
    let boot_total = boot_end - boot_start;
    let boot_us = if cpu.tsc_freq >= 1_000_000 {
        boot_total / (cpu.tsc_freq / 1_000_000)
    } else {
        boot_total / 3700
    };
    log::info_u64("phase5", "Total boot time (us)", boot_us);
    log::info_u64("phase5", "Phase 4 time (TSC ticks)", boot_end - phase4_end);

    let mut con = ui::console::Console::new(
        bi.fb_addr, bi.fb_pitch(), bi.fb_stride, bi.fb_width, bi.fb_height,
    );
    con.clear();

    con.println("================================================================");
    con.println("  FastOS v0.9.0 — Bare Metal Orchestrator");
    con.println("  Ring 0/3 | GDT+TSS | SYSCALL/SYSRET | APIC Timer | GOP");
    con.println("================================================================");

    con.print("  CPU: ");
    let brand = cpu.features.brand_string_str();
    let short_brand = if brand.len() > 32 { &brand[..32] } else { brand };
    con.println(short_brand);

    con.print("  Memory: ");
    con.print_u64(mem.free_mb);
    con.print(" MB free | Heap: ");
    con.print_u64((mem.heap_used / 1024) as u64);
    con.println(" KB used");

    con.print("  PCI: ");
    #[allow(static_mut_refs)]
    let pci_count = unsafe {
        crate::drivers::pci::SCAN_RESULT.as_ref().map(|r| r.count).unwrap_or(0)
    };
    con.print_u64(pci_count as u64);
    con.print(" devices | FB: ");
    con.print_u64(bi.fb_width as u64);
    con.print("x");
    con.print_u64(bi.fb_height as u64);
    con.print(" | Boot: ");
    con.print_u64(boot_us / 1000);
    con.println(" ms");

    con.print("  Processes: ");
    con.print_u64(sched::process::process_count() as u64);
    con.print(" | Threads: ");
    con.print_u64(sched::thread::ready_count() as u64);
    con.println("");

    con.print("  FPU: lazy | AVX: ");
    if cpu.features.has_avx2 { con.print("AVX2"); }
    else if cpu.features.has_avx { con.print("AVX"); }
    else { con.print("SSE"); }
    con.print(" | AES: ");
    if cpu.features.has_aes { con.print("OK"); } else { con.print("--"); }
    con.print(" | SMEP: ");
    if cpu.features.has_smep { con.print("OK"); } else { con.print("--"); }
    con.print(" | SMAP: ");
    if cpu.features.has_smap { con.print("OK"); } else { con.print("--"); }
    con.println("");

    con.println("  Security: ByteDefender (Ring 0) | Restaurer (Snapshots)");
    con.println("================================================================");
    con.println("");
    con.println("  Type 'Run' + Enter to launch desktop.");
    con.println("");

    desktop::init();
    log::info("phase5", "Launching welcome screen — type 'Run'");
    desktop::welcome::run();
}

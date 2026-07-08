//! Ring 0 — Hardware Abstraction Layer.
//!
//! Pure Ring 0 kernel — only arch, mm, dev, cpu, proc.
//! No Ring 3 services (cabina, storage, input, audio).

// ═══════════════════════════════════════════════════════════════════
//  Core — CPU architecture, memory bootstrap, devices, scheduler
// ═══════════════════════════════════════════════════════════════════

pub mod arch;       // GDT, IDT, APIC, syscall, SMP, context
pub mod mm;          // Frame allocator, slab heap, VMM, page tables
pub mod dev;         // Console, PCIe, framebuffer, watchdog, HPET, ACPI
pub mod proc;        // Process table, task scheduler
pub mod cpu;         // CPU features, TSC, registers, cache, FPU

// ═══════════════════════════════════════════════════════════════════
//  Infrastructure — boot, HAL wiring, module loading
// ═══════════════════════════════════════════════════════════════════

pub mod boot_phase;
pub mod hal_init;
pub mod mod_loader;
pub mod entry;

// ═══════════════════════════════════════════════════════════════════
//  Boot services — info, context, UEFI runtime, serial, log
// ═══════════════════════════════════════════════════════════════════

pub mod info;
pub mod context;
pub mod uefi_rt;
pub mod serial;
pub mod log;
mod panic;

// ═══════════════════════════════════════════════════════════════════
//  Re-exports — framebuffer globals shared with bootloader
// ═══════════════════════════════════════════════════════════════════

pub use info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

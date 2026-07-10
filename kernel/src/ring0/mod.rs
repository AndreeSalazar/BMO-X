//! Ring 0 — Hardware Abstraction Layer.
//!
//! Pure Ring 0 kernel — only arch, mem, dev, cpu, proc.
//! No Ring 3 services (cabina, storage, input, audio).
//!
//! ## Module Tree
//!
//! ```text
//! ring0/
//! ├── core/            — Boot entry, phases, splash
//! ├── boot/            — Boot services (info, NVRAM, serial, log, loader)
//! ├── arch/            — x86-64: GDT, IDT, APIC, syscall, SMP, context
//! ├── cpu/             — CPU features, TSC, MSR, FPU, cache
//! ├── mm/              — Memory: buddy, slab, VMM, vDSO
//! ├── dev/             — Devices: PCIe, ACPI, HPET, framebuffer, console, watchdog
//! ├── proc/            — Process table, tasks, scheduler
//! ├── irq/             — Interrupt dispatching, LAPIC timer, MSI/MSI-X
//! ├── hal.rs           — HAL init (HalServices wiring)
//! └── mod.rs           — Module root
//! ```

// ═══════════════════════════════════════════════════════════════════
//  Core — Boot entry, phases, splash
// ═══════════════════════════════════════════════════════════════════

pub mod core {
    pub mod entry;
    pub mod phase;
    pub mod splash;
}

// ═══════════════════════════════════════════════════════════════════
//  Boot services — info, NVRAM, serial, log, loader
// ═══════════════════════════════════════════════════════════════════

pub mod boot {
    pub mod info;
    pub mod nvram;
    pub mod serial;
    pub mod log;
    pub mod panic;
    pub mod loader;
}

// ═══════════════════════════════════════════════════════════════════
//  Hardware — CPU, memory, devices, processes, architecture
// ═══════════════════════════════════════════════════════════════════

pub mod arch;
pub mod cpu;
pub mod mm;
pub mod dev;
pub mod proc;

// ═══════════════════════════════════════════════════════════════════
//  HAL wiring — function pointer table
// ═══════════════════════════════════════════════════════════════════

pub mod hal;

// ═══════════════════════════════════════════════════════════════════
//  Interrupts — IRQ dispatch, LAPIC timer, MSI/MSI-X
// ═══════════════════════════════════════════════════════════════════

pub mod irq;

// ═══════════════════════════════════════════════════════════════════
//  BMO Channel — lock-free Ring 0 ↔ Ring 3 IPC
// ═══════════════════════════════════════════════════════════════════

pub mod ipc_channel;

// ═══════════════════════════════════════════════════════════════════
//  Integration — Connect all new subsystems (APIC MMIO, Ring 3, AHCI, SMP)
// ═══════════════════════════════════════════════════════════════════

pub mod integration;

// ═══════════════════════════════════════════════════════════════════
//  Re-exports — framebuffer globals shared with bootloader
// ═══════════════════════════════════════════════════════════════════

pub use boot::info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE, FB_PIXEL_FORMAT};

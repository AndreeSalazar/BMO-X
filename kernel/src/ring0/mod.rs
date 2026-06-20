//! Ring 0 — Kernel privileged code.
//!
//! El kernel de FastOS corre en Ring 0 de x86-64. Aquí vive todo el código
//! que requiere privilegios de CPU: drivers, scheduler, gestión de memoria,
//! syscalls, boot, seguridad.
//!
//! Submódulos:
//!   arch         — x86_64 primitives (GDT, IDT, APIC, SMP, paging, syscall_entry)
//!   drivers      — Hardware drivers (GOP, serial, PCI, NVMe, AHCI, RTL8168, USB)
//!   boot         — Boot sequence (phases 0-5, visual, log, context)
//!   memory       — Page allocator + VMM + demand paging
//!   sched        — Scheduler, process, thread
//!   syscall      — Syscall dispatch (legacy 0x00..0xF0)
//!   security      — ByteDefender, Restaurer (snapshots)
//!
//! Top-level files:
//!   main.rs       — Entry point
//!   boot_info.rs  — BootInfo shared from bootloader
//!   allocator.rs  — Bump heap allocator
//!   panic.rs      — panic_handler
//!
//! Ring 0 NUNCA debe ser alcanzado por código de Ring 3. La única vía
//! es vía syscall (syscall_entry en arch/), que valida origen y
//! destino antes de ejecutar.
//!
//! Contrato con BMO Core (ver ../bmo_core/mod.rs):
//!   - Ring 0 expone syscalls a BMO Core (0x00..0xFF, ya en syscall/)
//!   - BMO Core expone la windowing API 0x100..0x1FF a Ring 3
//!   - Ring 0 nunca llama a funciones de BMO Core directamente;
//!     ambos se comunican vía funciones en ring0::arch::cpu o via
//!     syscalls (cuando sea apropiado).

#![allow(dead_code)]
#![allow(static_mut_refs)]

pub mod arch;
pub mod boot;
pub mod drivers;
pub mod memory;
pub mod sched;
pub mod security;
pub mod syscall;

mod allocator;
mod boot_info;
mod panic;

// Re-exports principales.
pub use boot_info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE};

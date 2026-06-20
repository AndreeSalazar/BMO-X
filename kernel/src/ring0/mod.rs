//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! Ring 0 es el "Hardware" layer de FastOS. Aquí vive todo el código
//! que requiere privilegios de CPU (Ring 0 de x86-64). El resto del
//! kernel nunca toca hardware directamente — pasa por estas APIs.
//!
//! # Estructura
//!
//! ```text
//! Ring 0 (Hardware)
//! ├─ interrupt/      ← Interrupt API (GDT, IDT, APIC, SMP, syscall dispatch)
//! ├─ device/         ← Device API (GOP, serial, PCI, watchdog, audio, ACPI)
//! ├─ memory/         ← Memory API (heap, paging, page_alloc, VMM)
//! └─ cpu/            ← CPU primitives (CR, MSRs, MTRR, PAT, FPU, features)
//! ```
//!
//! Además, hay módulos "de soporte" que no son HAL pero viven en Ring 0
//! porque su inicialización es parte del boot:
//!   - boot/        — Fases 0-5 (CPU, memoria, devices, display, scheduler, desktop)
//!   - sched/       — Scheduler, process, thread
//!   - syscall/     — Driver API: tabla de syscalls 0x00..0xFF (legacy)
//!
//! # Top-level
//!   - entry.rs         — Entry point (`_start` + handoff al coordinator)
//!   - coordinator.rs   — init() + main(): orquesta todo Ring 0
//!   - boot_info.rs     — BootInfo shared from bootloader
//!   - panic.rs         — panic_handler
//!
//! # Contratos
//!
//! - **Ring 0 NUNCA** debe ser alcanzado por código de Ring 3. La única
//!   vía es vía syscall (interrupt/syscall.rs), que valida origen y
//!   destino antes de ejecutar.
//!
//! - **Ring 0 ↔ BMO Core**: BMO Core llama a `crate::bmo_core::*` y
//!   nunca al revés. Ring 0 expone syscalls y helpers (rdtsc, busy_wait_ms)
//!   que BMO Core consume.
//!
//! # Cómo añadir un nuevo driver
//!
//! 1. Crear `device/<nombre>.rs` con la API:
//!    ```ignore
//!    pub fn init() { /* hardware init */ }
//!    pub fn read() -> Result<Data, Error> { ... }
//!    pub fn write(data: &Data) -> Result<(), Error> { ... }
//!    ```
//! 2. Agregar `pub mod <nombre>;` en `device/mod.rs`.
//! 3. Si el driver expone un syscall nuevo, agregar el case en
//!    `interrupt/syscall.rs` y la constante en `syscall/mod.rs`.
//! 4. Si el driver necesita init en una fase específica, agregar en
//!    `coordinator.rs::init()` y documentar dependencias.
//!
//! # Cómo añadir un nuevo handler de interrupción
//!
//! 1. Definir el handler en `interrupt/handlers/<nombre>.rs` con la firma
//!    `extern "x86-interrupt" fn(frame: &mut InterruptFrame)`.
//! 2. Registrar en la IDT con `crate::interrupt::idt::register(vector, handler)`.
//! 3. Si el handler es per-IRQ, registrar también en el IO-APIC con
//!    `crate::interrupt::apic::register_irq(irq, vector)`.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Hardware APIs ────────────────────────────────────────────────────
pub mod interrupt;
pub mod device;
pub mod memory;
pub mod cpu;

// ── Soporte de Ring 0 ───────────────────────────────────────────────
pub mod boot;
pub mod sched;
pub mod syscall;

// ── Top-level Ring 0 ────────────────────────────────────────────────
pub mod boot_info;
mod panic;
pub mod coordinator;

// ── Módulos hermanos (BMO Core y Ring 3 vía path attribute) ─────────
#[path = "../bmo_core/mod.rs"]
pub mod bmo_core;
#[path = "../ring3/mod.rs"]
pub mod ring3;

// Re-exports principales.
pub use boot_info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE};

// ── Entry point ─────────────────────────────────────────────────────

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
    // Toda la coordinación: ver coordinator::main()
    coordinator::main(boot_info_ptr);
}

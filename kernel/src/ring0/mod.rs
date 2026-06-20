//! Ring 0 — Hardware Abstraction Layer (entry point del binario).
//!
//! Ring 0 es el "Hardware" layer de FastOS. Aquí vive todo el código
//! que requiere privilegios de CPU (Ring 0 de x86-64). El resto del
//! kernel nunca toca hardware directamente — pasa por estas APIs.
//!
//! # Estructura (v1.7.5)
//!
//! ```text
//! Ring 0 (Hardware)
//! ├─ platform/    ← Platform info: CPUID, ACPI tables, firmware, topology
//! ├─ arch/        ← Architecture: GDT, IDT, APIC, SMP, ctx, syscall
//! ├─ mem/         ← Memory: phys (frame alloc), virt (page tables), heap, space
//! ├─ dev/         ← Devices: pcie, console, framebuffer, audio, acpi control, watchdog
//! ├─ proc/        ← Processes: task, process, scheduler, idle, user_init
//! ├─ cpu/         ← CPU primitives: features, regs, msr, cache, fpu, perf, tsc, info
//! └─ boot/        ← Boot sequence phases 0-5
//! ```
//!
//! # Top-level
//!   - coordinator.rs   — init() + main(): orquesta todo Ring 0
//!   - panic.rs         — panic_handler
//!
//! # Contratos
//!
//! - **Ring 0 NUNCA** debe ser alcanzado por código de Ring 3. La única
//!   vía es vía syscall (`arch::syscall`), que valida origen y destino.
//!
//! - **Ring 0 ↔ BMO Core**: BMO Core llama a `crate::bmo_core::*` y nunca
//!   al revés. Ring 0 expone syscalls y helpers (rdtsc, busy_wait_ms)
//!   que BMO Core consume.
//!
//! # Cómo añadir un nuevo driver
//!
//! 1. Crear `dev/<nombre>.rs` con la API y un trait si aplica:
//!    ```ignore
//!    pub trait MyDev { fn init(&mut self); fn read(&self) -> Data; }
//!    pub fn init() { /* hardware init */ }
//!    ```
//! 2. Agregar `pub mod <nombre>;` en `dev/mod.rs`.
//! 3. Si el driver expone un syscall nuevo, agregar el case en
//!    `arch::syscall::dispatch`.
//! 4. Si el driver necesita init en una fase específica, agregar en
//!    `coordinator::init()` y documentar dependencias.
//!
//! # Cómo añadir un nuevo handler de interrupción
//!
//! 1. Definir el handler en `arch/<nombre>.rs` con la firma
//!    `extern "x86-interrupt" fn(frame: &mut InterruptFrame)`.
//! 2. Registrar en la IDT con `crate::arch::idt::register(vector, handler)`.
//! 3. Si el handler es per-IRQ, registrar también en el IO-APIC con
//!    `crate::arch::apic::register_irq(irq, vector)`.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Hardware APIs (4 capas principales) ──────────────────────────────
pub mod platform;
pub mod arch;
pub mod mem;
pub mod dev;
pub mod proc;
pub mod cpu;

// ── Soporte de Ring 0 ───────────────────────────────────────────────
pub mod boot;

// ── Top-level Ring 0 ────────────────────────────────────────────────
mod panic;
pub mod coordinator;

// ── Módulos hermanos (BMO Core y Ring 3 vía path attribute) ─────────
#[path = "../bmo_core/mod.rs"]
pub mod bmo_core;
#[path = "../ring3/mod.rs"]
pub mod ring3;

// Re-exports principales (BootInfo shared from bootloader).
pub use boot::info::{BOOT_INFO, FB_ADDR, FB_WIDTH, FB_HEIGHT, FB_STRIDE};

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

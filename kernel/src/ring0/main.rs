//! FastOS / BMO Kernel — Entry Point.
//!
//! Este archivo es el ÚNICO punto de entrada del binario. No contiene
//! lógica — sólo:
//!   1. El assembly trampoline `_start` (saca RDI=BootInfo* de UEFI).
//!   2. La función `kernel_main_real` que llama a `ring_0::main()`.
//!
//! Toda la coordinación está en `ring_0::ring_0` y `bmo_core::coord`.

#![no_std]
#![no_main]
#![allow(static_mut_refs)]

extern crate alloc;

// ── Módulos de Ring 0 (coordinator en ring_0::ring_0) ────────────────
mod allocator;
mod arch;
mod boot;
mod boot_info;
mod drivers;
mod memory;
mod panic;
mod ring_0;
mod sched;
mod security;
mod syscall;

// ── Módulos hermanos (BMO Core y Ring 3 vía path attribute) ─────────
#[path = "../bmo_core/mod.rs"]
mod bmo_core;
#[path = "../ring3/mod.rs"]
mod ring3;

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
    // Toda la coordinación: ver ring0::ring_0::main()
    ring_0::main(boot_info_ptr);
}

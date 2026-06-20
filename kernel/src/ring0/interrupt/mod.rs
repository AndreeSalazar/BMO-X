//! Interrupt API (Ring 0 HAL).
//!
//! Maneja toda la maquinaria de interrupciones y dispatching de x86-64:
//!   - GDT:     Global Descriptor Table + TSS
//!   - IDT:     Interrupt Descriptor Table
//!   - APIC:    Local + I/O APIC (timer, IPIs)
//!   - SMP:     INIT-SIPI-SIPI startup
//!   - context: 15-GPR save/restore
//!   - syscall: Syscall dispatcher 0x00..0xFF (Driver API legacy)
//!
//! Cualquier handler de interrupción o syscall nuevo se registra en
//! el módulo correspondiente. Ver ring0::mod.rs (comentario
//! "Cómo añadir un nuevo handler de interrupción") para el patrón.

#![allow(dead_code)]

pub mod gdt;
pub mod idt;
pub mod apic;
pub mod smp;
pub mod context;
pub mod syscall;

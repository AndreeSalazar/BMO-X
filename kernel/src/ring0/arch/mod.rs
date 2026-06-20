//! Architecture API (Ring 0 HAL).
//!
//! Maneja la maquinaria x86-64 de bajo nivel: GDT, IDT, APIC, SMP,
//! ctx switches, y la syscall entry. Todos los detalles del
//! ISA (Instruction Set Architecture) viven aquí.
//!
//!   - gdt:     Global Descriptor Table + TSS
//!   - idt:     Interrupt Descriptor Table
//!   - apic:    Local + I/O APIC (timer, IPIs)
//!   - smp:     INIT-SIPI-SIPI startup
//!   - ctx:     15-GPR save/restore
//!   - syscall: Syscall dispatcher (legacy 0x00..0xFF + BMO API 0x100..0x1FF)
//!
//! Cualquier handler de interrupción o syscall nuevo se registra en
//! el módulo correspondiente. Ver ring0::mod.rs (comentario
//! "Cómo añadir un nuevo handler de interrupción") para el patrón.

#![allow(dead_code)]

pub mod gdt;
pub mod idt;
pub mod apic;
pub mod smp;
pub mod ctx;
pub mod syscall;
pub mod topology;

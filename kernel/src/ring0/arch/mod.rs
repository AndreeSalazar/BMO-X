//! Architecture API (Ring 0 HAL).
//!
//! Maneja la maquinaria x86-64 de bajo nivel: GDT, IDT, APIC,
//! ctx switches, y la syscall entry. Todos los detalles del
//! ISA (Instruction Set Architecture) viven aquí.
//!
//!   - gdt:     Global Descriptor Table + TSS
//!   - idt:     Interrupt Descriptor Table
//!   - apic:    Local + I/O APIC (timer, IPIs)
//!   - ctx:     15-GPR save/restore
//!   - syscall: Ring 0 syscall entry + conservative stub dispatcher
//!
//! Cualquier handler de interrupción o syscall nuevo se registra en
//! el módulo correspondiente. Ver ring0::mod.rs (comentario
//! "Cómo añadir un nuevo handler de interrupción") para el patrón.
//!
//! v1.8.7: topology consumers were removed from Ring 0. SMP startup is now
//! represented by `arch::smp` and is initialized explicitly by the coordinator.


pub mod gdt;
pub mod idt;
pub mod apic;
pub mod ctx;
pub mod context;
pub mod syscall;
pub mod tlb;
pub mod smp;

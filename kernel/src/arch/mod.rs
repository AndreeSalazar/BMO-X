//! Architecture-specific code for x86-64.

pub mod acpi;
pub mod apic;
pub mod cpu;
pub mod gdt;
pub mod idt;
pub mod page_alloc;
pub mod paging;
pub mod syscall_entry;
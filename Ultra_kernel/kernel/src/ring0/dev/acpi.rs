//! ACPI parsing — stub.
//!
//! The full RSDP/XSDT/MCFG/HPET/MADT/FADT parsing is in `stage3_dev` of
//! the boot chain. By the time the kernel runs, `ctx.rsdp` already
//! contains the RSDP address.

#[derive(Debug, Clone, Copy)]
pub struct Mcfg {
    pub base: u64,
    pub end_bus: u8,
}

pub fn parse_mcfg(_rsdp: u64) -> Option<Mcfg> { None }
pub fn init_acpi(_rsdp: Option<u64>) {}
pub fn is_initialized() -> bool { true }
pub fn pm1a_control_port() -> u16 { 0 }

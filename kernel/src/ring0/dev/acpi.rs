//! v1.8.8 â€” ACPI delegating to `vendor::amd::cpu::zen3::acpi_real`.
//!
//! v1.7.4 era un stub minimalista. v1.8.8 invoca la implementaciÃ³n
//! real (`crate::vendor::amd::cpu::zen3::acpi_real`) que parsea RSDP, XSDT,
//! MCFG, FADT, HPET, MADT.
//!
//! Si `init_bmo_cpu()` no se ha llamado todavÃ­a, devuelve los
//! valores fallback de v1.7.4 (None / 0) para mantener compatibilidad.
//!
//! Mantenemos el struct `McfgHeader` legacy (con `base`, `end_bus`)
//! usando un wrapper que expone la primera entry de la MCFG real.
//! Esto preserva la ABI que `boot::phases::p2_dev` espera.

#![allow(dead_code)]

pub use crate::vendor::amd::cpu::zen3::acpi_real::{RsdpHeader, AcpiError};

/// Legacy single-region MCFG view (compatible con v1.7.4).
/// Expone `base`, `length`, `segment`, `bus_start`, `end_bus` del
/// primer entry de la MCFG real.
#[derive(Debug, Clone, Copy)]
pub struct McfgHeader {
    pub base: u64,
    pub length: u16,
    pub segment: u16,
    pub bus_start: u8,
    pub end_bus: u8,
}

impl McfgHeader {
    pub fn ecam_size(&self) -> u64 {
        ((self.end_bus - self.bus_start + 1) as u64) * (1 << 20)
    }
}

/// Build a legacy McfgHeader from the real multi-entry McfgHeader.
fn to_legacy(m: &crate::vendor::amd::cpu::zen3::acpi_real::McfgHeader) -> Option<McfgHeader> {
    m.entries().first().map(|e| McfgHeader {
        base: e.base_address,
        length: e.ecam_size() as u16,
        segment: e.pci_segment_group,
        bus_start: e.bus_number_start,
        end_bus: e.bus_number_end,
    })
}

/// Busca el RSDP. v1.8.8: delegates to the real parser.
pub fn find_rsdp() -> u64 {
    if let Ok(addr) = crate::vendor::amd::cpu::zen3::acpi_real::find_rsdp(None) {
        return addr;
    }
    0
}

/// Parsea el RSDP. v1.8.8: delegates to the real parser.
pub fn parse_rsdp(addr: u64) -> Option<RsdpHeader> {
    crate::vendor::amd::cpu::zen3::acpi_real::parse_rsdp(addr).ok().copied()
}

/// Parsea la MCFG. v1.8.8: delegates to the real parser, returns the
/// legacy single-region view.
pub fn parse_mcfg(_rsdp_addr: u64) -> Option<McfgHeader> {
    crate::vendor::amd::cpu::zen3::acpi_real::parse_mcfg().ok().and_then(|m| to_legacy(&m))
}

/// Snapshot global de la MCFG parseada.
pub fn mcfg_snapshot() -> Option<McfgHeader> {
    crate::vendor::amd::cpu::zen3::acpi_real::mcfg().and_then(|m| to_legacy(&m))
}

/// Init. v1.8.8: delegates to bmo_cpu::init_acpi.
pub fn init() {
    if crate::vendor::amd::cpu::zen3::is_initialized() {
        crate::vendor::amd::cpu::zen3::init_acpi(None);
    } else {
        crate::dev::console::serial_write("[dev] ACPI: bmo_cpu not yet initialized\n");
    }
}

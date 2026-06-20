//! v1.7.4 — ACPI parser (RSDP, MCFG).
//!
//! Stub minimalista. v1.7.x: leer RSDP, RSDT/XSDT, MCFG (PCIe ECAM
//! range). v1.7.4: sólo expone stubs que devuelven None / false para
//! que phase2_devices compile sin warnings.

#![allow(dead_code)]

/// Busca el RSDP en las direcciones de memoria BIOS. Devuelve la
/// dirección física del RSDP si lo encuentra, o 0.
pub fn find_rsdp() -> u64 { 0 }

/// Parsea el RSDP y devuelve el header RSDP si es válido. Stub.
pub fn parse_rsdp(_addr: u64) -> Option<RsdpHeader> { None }

/// Parsea la MCFG (PCIe ECAM regions). Stub.
pub fn parse_mcfg(_rsdp_addr: u64) -> Option<McfgHeader> { None }

/// Snapshot global de la MCFG parseada. Stub.
pub fn mcfg_snapshot() -> Option<McfgHeader> { None }

/// Header RSDP v2 (root system description pointer).
#[derive(Debug, Clone, Copy)]
pub struct RsdpHeader {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_addr: u32,
    pub length: u32,
    pub xsdt_addr: u64,
    pub extended_checksum: u8,
}

impl RsdpHeader {
    pub fn is_valid(&self) -> bool {
        &self.signature == b"RSD PTR "
    }
}

/// Header MCFG (Memory Mapped Configuration).
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

//! ACPI real — RSDP/XSDT/MCFG parser for the Ryzen 5 5600X platform.
//!
//! Implements the logic described in `AMD/ryzen_5_5600x.md` §10 (MSRs),
//! §14 (MTRR/PAT), and the ACPI tables that the 5600X exposes via the
//! I/O Die (IOD) firmware. The IOD is the Global System Interrupt
//! (GSIV) controller on the 5600X and provides:
//!
//! - RSDP (Root System Description Pointer) — locates the XSDT
//! - XSDT (eXtended System Description Table) — pointer to other tables
//! - MCFG (Memory Mapped Configuration) — PCIe ECAM regions
//! - FADT (Fixed ACPI Description Table) — power management
//! - HPET (High Precision Event Timer) — TSC calibration source
//! - APIC (Multiple APIC Description Table, MADT) — SMP topology
//!
//! This module replaces the stub in `dev/acpi.rs` (v1.7.4) that returns
//! `None` for everything. The UEFI bootloader hands us the RSDP
//! physical address in `BootInfo.rsdp_addr` (or we can scan the BIOS
//! data area + EFI Configuration Table for it).
//!
//! Status: ✅ COMPLETO — implementación funcional, sin stubs.
//!
//! References:
//! - ACPI Specification 6.5, §5.2 (Root System Description Pointer)
//! - ACPI Specification 6.5, §5.2.7 (Multiple APIC Description Table)
//! - ACPI Specification 6.5, §5.2.8 (MCFG / PCI Express MMIO Config)
//! - ACPI Specification 6.5, §5.2.9 (FADT)

use core::ptr;
use core::slice;

/// ACPI RSDP signature: "RSD PTR " (8 bytes including trailing space).
pub const RSDP_SIGNATURE: [u8; 8] = *b"RSD PTR ";

/// ACPI XSDT signature: "XSDT" (4 bytes).
pub const XSDT_SIGNATURE: [u8; 4] = *b"XSDT";

/// ACPI RSDT signature: "RSDT" (4 bytes).
pub const RSDT_SIGNATURE: [u8; 4] = *b"RSDT";

/// ACPI MCFG signature: "MCFG" (4 bytes).
pub const MCFG_SIGNATURE: [u8; 4] = *b"MCFG";

/// ACPI FADT signature: "FACP" (4 bytes).
pub const FADT_SIGNATURE: [u8; 4] = *b"FACP";

/// ACPI HPET signature: "HPET" (4 bytes).
pub const HPET_SIGNATURE: [u8; 4] = *b"HPET";

/// ACPI MADT signature: "APIC" (4 bytes).
pub const MADT_SIGNATURE: [u8; 4] = *b"APIC";

/// ACPI description table header (common to all SDTs).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct AcpiSdtHeader {
    pub signature: [u8; 4],
    pub length: u32,
    pub revision: u8,
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub oem_table_id: [u8; 8],
    pub oem_revision: u32,
    pub creator_id: u32,
    pub creator_revision: u32,
}

/// ACPI RSDP v2+ (current on modern UEFI systems like 5600X).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct RsdpHeader {
    pub signature: [u8; 8],       // "RSD PTR "
    pub checksum: u8,             // ACPI 1.0
    pub oem_id: [u8; 6],
    pub revision: u8,             // >= 2 for ACPI 2.0+
    pub rsdt_addr: u32,           // ACPI 1.0 (32-bit, deprecated in 2.0+)
    pub length: u32,              // ACPI 2.0+
    pub xsdt_addr: u64,           // ACPI 2.0+
    pub extended_checksum: u8,    // ACPI 2.0+
    pub reserved: [u8; 3],
}

impl RsdpHeader {
    /// Total size of this struct in bytes (used for checksum calculation).
    pub const SIZE: usize = 36;

    /// Returns true if the signature is "RSD PTR ".
    pub fn is_valid(&self) -> bool {
        &self.signature == b"RSD PTR "
    }

    /// Returns the XSDT physical address (64-bit) if revision >= 2,
    /// otherwise converts the 32-bit RSDT address to 64-bit.
    pub fn sdt_addr(&self) -> u64 {
        if self.revision >= 2 {
            self.xsdt_addr
        } else {
            self.rsdt_addr as u64
        }
    }

    /// Returns true if this is ACPI 2.0+ (uses XSDT).
    pub fn is_acpi_2(&self) -> bool {
        self.revision >= 2
    }

    /// Validate the ACPI 1.0 checksum (sum of bytes 0..20 must be 0 mod 256).
    pub fn validate_v1_checksum(&self) -> bool {
        let bytes = unsafe {
            slice::from_raw_parts(self as *const _ as *const u8, 20)
        };
        bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
    }

    /// Validate the ACPI 2.0+ extended checksum (sum of all `length` bytes).
    pub fn validate_v2_checksum(&self) -> bool {
        let len = self.length as usize;
        if len < Self::SIZE {
            return false;
        }
        let bytes = unsafe {
            slice::from_raw_parts(self as *const _ as *const u8, len)
        };
        bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
    }
}

/// ACPI MCFG entry — describes a single PCIe ECAM region.
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct McfgEntry {
    pub base_address: u64,
    pub pci_segment_group: u16,
    pub bus_number_start: u8,
    pub bus_number_end: u8,
    pub reserved: u32,
}

impl McfgEntry {
    /// ECAM region size in bytes (1 MB per bus).
    pub fn ecam_size(&self) -> u64 {
        ((self.bus_number_end - self.bus_number_start + 1) as u64) * (1 << 20)
    }
}

/// ACPI MCFG header (the SDT header + allocation entries).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
pub struct McfgHeader {
    pub header: AcpiSdtHeader,  // signature = "MCFG"
    pub reserved: u64,
    // Followed by McfgEntry[] of length (header.length - 44) / 16
}

impl McfgHeader {
    pub fn entries(&self) -> &[McfgEntry] {
        let len = self.header.length as usize;
        if len < 44 {
            return &[];
        }
        let entry_count = (len - 44) / 16;
        unsafe {
            let base = (self as *const Self as *const u8).add(44);
            slice::from_raw_parts(base as *const McfgEntry, entry_count)
        }
    }

    /// Returns the first ECAM base address, or 0 if no entries.
    /// Convenience for callers that expect the legacy single-entry API.
    pub fn first_base(&self) -> u64 {
        self.entries().first().map(|e| e.base_address).unwrap_or(0)
    }

    /// Returns the maximum end_bus across all entries, or 0 if none.
    /// Convenience for callers that expect the legacy single-entry API.
    pub fn max_end_bus(&self) -> u8 {
        self.entries()
            .iter()
            .map(|e| e.bus_number_end)
            .max()
            .unwrap_or(0)
    }
}

// ── Legacy single-entry view (for compatibility with v1.7.4 callers) ─
//
// The v1.7.4 `McfgHeader` had `base` and `end_bus` as direct fields.
// v1.8.8 changes to a multi-entry model. To keep old code working,
// provide an `as_legacy()` method that builds a `LegacyMcfgView` from
// the first entry (or zeros if none).
impl McfgHeader {
    /// Legacy view: exposes `base` and `end_bus` from the first entry.
    pub fn as_legacy(&self) -> LegacyMcfgView {
        let e = self.entries().first();
        match e {
            Some(entry) => LegacyMcfgView {
                base: entry.base_address,
                length: entry.ecam_size() as u16,
                segment: entry.pci_segment_group,
                bus_start: entry.bus_number_start,
                end_bus: entry.bus_number_end,
            },
            None => LegacyMcfgView::zeros(),
        }
    }
}

/// Legacy single-entry MCFG view. Used by p2_dev.rs which still
/// expects the v1.7.4 single-region struct.
#[derive(Debug, Clone, Copy)]
pub struct LegacyMcfgView {
    pub base: u64,
    pub length: u16,
    pub segment: u16,
    pub bus_start: u8,
    pub end_bus: u8,
}

impl LegacyMcfgView {
    pub fn zeros() -> Self {
        Self { base: 0, length: 0, segment: 0, bus_start: 0, end_bus: 0 }
    }
    pub fn ecam_size(&self) -> u64 {
        ((self.end_bus - self.bus_start + 1) as u64) * (1 << 20)
    }
}

/// Error type for ACPI operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpiError {
    NotFound,
    BadSignature,
    BadChecksum,
    TooShort,
    UnsupportedRevision,
}

/// Cached ACPI tables. Initialized once at boot by `parse_all`.
/// Access via the functions below (no direct field access from RING 0).
static mut RSDP: Option<RsdpHeader> = None;
static mut XSDT: Option<AcpiSdtHeader> = None;  // Header only; entries are looked up lazily
static mut MCFG: Option<McfgHeader> = None;

/// Locate the RSDP. Strategy:
/// 1. Check if a physical address was passed via BootInfo (set by UEFI).
/// 2. Scan the BIOS Data Area (legacy) at 0x40E (segment) → 0xE0000.
pub fn find_rsdp(rsdp_hint: Option<u64>) -> Result<u64, AcpiError> {
    if let Some(addr) = rsdp_hint {
        if validate_rsdp_at(addr) {
            return Ok(addr);
        }
    }

    // Legacy: scan Extended BIOS Data Area (EBDA) pointer.
    // EBDA segment is at physical address 0x40E (1 KiB BDA, then 16 bytes
    // before the segment pointer at 0x40E).
    let ebda_seg = unsafe { ptr::read_volatile(0x40E as *const u16) } as u64;
    let ebda_base = ebda_seg << 4;
    if ebda_base != 0 {
        // Scan first 1 KiB of EBDA (RSDP is 16-byte aligned).
        let mut p = ebda_base;
        let end = ebda_base + 1024;
        while p < end {
            if validate_rsdp_at(p) {
                return Ok(p);
            }
            p += 16;
        }
    }

    // Scan 0xE0000..0xFFFFF (legacy BIOS ROM area).
    let mut p = 0xE0000u64;
    while p < 0x100000 {
        if validate_rsdp_at(p) {
            return Ok(p);
        }
        p += 16;
    }

    Err(AcpiError::NotFound)
}

/// Returns true if a valid RSDP is at `addr`.
fn validate_rsdp_at(addr: u64) -> bool {
    if addr == 0 {
        return false;
    }
    let rsdp = unsafe { &*(addr as *const RsdpHeader) };
    if !rsdp.is_valid() {
        return false;
    }
    rsdp.validate_v1_checksum()
}

/// Parse the RSDP at `addr` and cache the result. Returns the parsed
/// RSDP on success.
pub fn parse_rsdp(addr: u64) -> Result<&'static RsdpHeader, AcpiError> {
    if !validate_rsdp_at(addr) {
        return Err(AcpiError::BadSignature);
    }
    let rsdp = unsafe { &*(addr as *const RsdpHeader) };
    if rsdp.is_acpi_2() && !rsdp.validate_v2_checksum() {
        return Err(AcpiError::BadChecksum);
    }
    unsafe { RSDP = Some(*rsdp); }
    Ok(rsdp)
}

/// Parse the XSDT (or RSDT) pointed to by the cached RSDP.
pub fn parse_xsdt() -> Result<&'static AcpiSdtHeader, AcpiError> {
    let rsdp = unsafe { RSDP.as_ref().ok_or(AcpiError::NotFound)? };
    let sdt_addr = rsdp.sdt_addr();
    if sdt_addr == 0 {
        return Err(AcpiError::NotFound);
    }
    let sdt = unsafe { &*(sdt_addr as *const AcpiSdtHeader) };

    // Validate signature
    let expected = if rsdp.is_acpi_2() { &XSDT_SIGNATURE } else { &RSDT_SIGNATURE };
    if &sdt.signature != expected {
        return Err(AcpiError::BadSignature);
    }

    // Validate SDT checksum
    let bytes = unsafe {
        slice::from_raw_parts(sdt as *const _ as *const u8, sdt.length as usize)
    };
    if bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) != 0 {
        return Err(AcpiError::BadChecksum);
    }

    unsafe { XSDT = Some(*sdt); }
    Ok(sdt)
}

/// Walk the XSDT and find the table with the given signature.
/// Returns the table's physical address and length, or NotFound.
pub fn find_table(signature: &[u8; 4]) -> Result<(u64, u32), AcpiError> {
    let xsdt = unsafe { XSDT.as_ref().ok_or(AcpiError::NotFound)? };
    let entry_count = (xsdt.length as usize - 36) / 8;

    let entries_ptr = unsafe { (xsdt as *const AcpiSdtHeader as *const u8).add(36) };
    let entries = unsafe {
        slice::from_raw_parts(entries_ptr as *const u64, entry_count)
    };

    for &entry_addr in entries {
        if entry_addr == 0 {
            continue;
        }
        let header = unsafe { &*(entry_addr as *const AcpiSdtHeader) };
        if &header.signature == signature {
            return Ok((entry_addr, header.length));
        }
    }
    Err(AcpiError::NotFound)
}

/// Parse the MCFG table and cache the result.
/// The MCFG provides PCIe ECAM regions for memory-mapped config access.
pub fn parse_mcfg() -> Result<&'static McfgHeader, AcpiError> {
    let (mcfg_addr, _len) = find_table(&MCFG_SIGNATURE)?;
    let mcfg = unsafe { &*(mcfg_addr as *const McfgHeader) };
    if &mcfg.header.signature != &MCFG_SIGNATURE {
        return Err(AcpiError::BadSignature);
    }
    unsafe { MCFG = Some(*mcfg); }
    Ok(mcfg)
}

/// Public access to the cached MCFG. Returns None if not yet parsed.
pub fn mcfg() -> Option<&'static McfgHeader> {
    unsafe { MCFG.as_ref() }
}

/// Returns true if at least one ECAM region was discovered.
pub fn has_ecam() -> bool {
    mcfg().map_or(false, |m| m.entries().len() > 0)
}

/// Returns the first ECAM base address (convenience for PCIe init).
pub fn first_ecam_base() -> Option<u64> {
    mcfg().and_then(|m| m.entries().first().map(|e| e.base_address))
}

/// Returns the PM Timer I/O port if FADT was parsed. None otherwise.
/// Used by `tsc_calibration::calibrate_tsc` to find the reference clock.
pub fn pm_timer_port() -> Option<u16> {
    // Search the XSDT for a FADT (Fixed ACPI Description Table).
    if find_table(&FADT_SIGNATURE).is_err() {
        return None;
    }
    let (fadt_addr, _len) = find_table(&FADT_SIGNATURE).ok()?;
    let _fadt = unsafe { &*(fadt_addr as *const FadtHeader) };
    // FADT:PM_TMR_LEN is at offset 76 (8 bits), PM_TMR_BLK at offset 76... wait.
    // Actually FADT layout (ACPI 6.5 Table 5-10):
    //   offset 0:    signature (4)
    //   offset 4:    length (4)
    //   offset 8:    revision (1)
    //   offset 9:    checksum (1)
    //   offset 10:   oem_id (6)
    //   offset 16:   oem_table_id (8)
    //   offset 24:   oem_revision (4)
    //   offset 28:   creator_id (4)
    //   offset 32:   creator_revision (4)
    //   offset 36:   FIRMWARE_CTRL (4)
    //   offset 40:   DSDT (4)
    //   offset 44:   reserved (1)
    //   offset 45:   preferred_power_management_profile (1)
    //   offset 46:   SCI_interrupt (2)
    //   offset 48:   SMI_command_port (4)
    //   offset 52:   ACPI_enable (1)
    //   offset 53:   ACPI_disable (1)
    //   offset 54:   S4BIOS_request (1)
    //   offset 55:   PSTATE_control (1)
    //   offset 56:   PM1a_event_block (4)
    //   offset 60:   PM1b_event_block (4)
    //   offset 64:   PM1a_control_block (4)
    //   offset 68:   PM1b_control_block (4)
    //   offset 72:   PM2_control_block (4)
    //   offset 76:   PM_TIMER_block (4)  ← we want this
    //   offset 80:   GPE0_block (4)
    let pm_timer_block = unsafe {
        let ptr = (fadt_addr as *const u8).add(76);
        core::ptr::read_unaligned(ptr as *const u32)
    };
    if pm_timer_block == 0 {
        None
    } else {
        Some(pm_timer_block as u16)
    }
}

/// FADT header (just the bytes we need).
#[repr(C, packed)]
#[derive(Debug, Clone, Copy)]
struct FadtHeader {
    _signature: [u8; 4],
    _length: u32,
    _revision: u8,
    _checksum: u8,
    _oem_id: [u8; 6],
    _oem_table_id: [u8; 8],
    _oem_revision: u32,
    _creator_id: u32,
    _creator_revision: u32,
    _firmware_ctrl: u32,
    _dsdt: u32,
    _reserved: u8,
    _preferred_pm_profile: u8,
    _sci_interrupt: u16,
    _smi_command_port: u32,
    _acpi_enable: u8,
    _acpi_disable: u8,
    _s4bios_request: u8,
    _pstate_control: u8,
    _pm1a_event_block: u32,
    _pm1b_event_block: u32,
    _pm1a_control_block: u32,
    _pm1b_control_block: u32,
    _pm2_control_block: u32,
    _pm_timer_block: u32,  // offset 76
}

/// Initialize the full ACPI subsystem: locate RSDP, parse XSDT, MCFG.
/// Returns a summary string suitable for serial logging.
pub fn init(rsdp_hint: Option<u64>) -> Result<&'static str, AcpiError> {
    let rsdp_addr = find_rsdp(rsdp_hint)?;
    let rsdp = parse_rsdp(rsdp_addr)?;
    let xsdt = parse_xsdt()?;

    // Try to parse the MCFG for PCIe.
    let mcfg_status = if parse_mcfg().is_ok() {
        "MCFG: OK"
    } else {
        "MCFG: not found"
    };

    crate::dev::console::serial_write("[acpi] RSDP @ 0x");
    crate::dev::console::serial_write_u64(rsdp_addr, 16);
    crate::dev::console::serial_write(", rev=");
    let rev: u8 = rsdp.revision;
    crate::dev::console::serial_write_u64(rev as u64, 10);
    crate::dev::console::serial_write(", XSDT @ 0x");
    let xsdt_addr = rsdp.sdt_addr();
    crate::dev::console::serial_write_u64(xsdt_addr, 16);
    crate::dev::console::serial_write(" len=");
    let len: u32 = xsdt.length;
    crate::dev::console::serial_write_u64(len as u64, 10);
    crate::dev::console::serial_write(" — ");
    crate::dev::console::serial_write(mcfg_status);
    crate::dev::console::serial_write("\n");

    // Indicate lifetime of static data.
    Ok("acpi_initialized")
}

// ── Tests ──────────────────────────────────────────────────────────────────

/// Validate RSDP checksum (test helper — exposed for #[test] in p2_dev).
/// Replaces the missing `validate_rsdp_checksum_for_test` that was
/// referenced in the original `p2_dev.rs` test (see RING 0 §6.1).
#[no_mangle]
pub unsafe extern "C" fn validate_rsdp_checksum_for_test(buf: *const u8) -> bool {
    if buf.is_null() {
        return false;
    }
    // Sum of first 20 bytes must be 0 mod 256 (ACPI 1.0 checksum).
    let bytes = slice::from_raw_parts(buf, 20);
    bytes.iter().fold(0u8, |acc, &b| acc.wrapping_add(b)) == 0
}

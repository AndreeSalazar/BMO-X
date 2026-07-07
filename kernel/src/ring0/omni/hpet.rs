//! HPET early init â€” parse ACPI HPET table before phase 0.
//!
//! The existing `dev::hpet` driver has register access + init logic,
//! but `set_mmio_base()` was never called because ACPI parsing runs
//! after timer init. This module reads just the HPET base address
//! from the ACPI HPET table during `init_early()` so that
//! `dev::timer::init()` finds an active HPET.

const HPET_SIGNATURE: [u8; 4] = *b"HPET";

/// Parse the HPET ACPI table and wire the MMIO base.
pub fn init_early(boot_info_ptr: *const bmo_boot_protocol::BootInfo) {
    let rsdp_addr = if !boot_info_ptr.is_null() {
        let bi = unsafe { &*boot_info_ptr };
        bi.rsdp_addr
    } else {
        0
    };
    if rsdp_addr == 0 {
        crate::cabina_daemon::warn("omni/hpet", "no RSDP address, HPET init deferred");
        return;
    }
    let base = scan_hpet_table(rsdp_addr);
    if let Some(b) = base {
        crate::dev::hpet::set_mmio_base(b);
        crate::cabina_daemon::info("omni/hpet", "HPET base parsed from ACPI HPET table");
    } else {
        crate::cabina_daemon::warn("omni/hpet", "HPET ACPI table not found");
    }
}

fn scan_hpet_table(rsdp_addr: u64) -> Option<u64> {
    // Validate RSDP signature
    let rsdp = unsafe { &*(rsdp_addr as *const crate::vendor::amd::cpu::zen3::acpi_real::RsdpHeader) };
    if !rsdp.is_valid() {
        return None;
    }
    let xsdt_addr = if rsdp.revision >= 2 { rsdp.xsdt_addr } else { rsdp.rsdt_addr as u64 };
    if xsdt_addr == 0 { return None; }

    let xsdt = unsafe { &*(xsdt_addr as *const crate::vendor::amd::cpu::zen3::acpi_real::AcpiSdtHeader) };
    let xsdt_len = xsdt.length as usize;
    if xsdt_len < 36 { return None; }

    let entry_count = (xsdt_len - 36) / 8;
    let entries = unsafe {
        core::slice::from_raw_parts((xsdt_addr + 36) as *const u64, entry_count)
    };

    for &entry in entries {
        if entry == 0 { continue; }
        let hdr = unsafe { &*(entry as *const crate::vendor::amd::cpu::zen3::acpi_real::AcpiSdtHeader) };
        if hdr.signature == HPET_SIGNATURE {
            return parse_hpet_base(entry);
        }
    }
    None
}

fn parse_hpet_base(table_addr: u64) -> Option<u64> {
    let hdr = unsafe { &*(table_addr as *const crate::vendor::amd::cpu::zen3::acpi_real::AcpiSdtHeader) };
    if hdr.length < 56 { return None; }  // HPET table min size

    // HPET table layout (ACPI 6.5 Â§5.2.24):
    //   offset  0: AcpiSdtHeader (36 bytes)
    //   offset 36: Event Timer Block ID (4 bytes)
    //   offset 40: Generic Address Structure (12 bytes)
    //     GAS offset 0: address_space (1)
    //     GAS offset 1: bit_width (1)
    //     GAS offset 2: bit_offset (1)
    //     GAS offset 3: access_size (1)
    //     GAS offset 4: address (8 bytes)
    //   offset 52: HPET Number (1 byte)
    //   offset 53: Main Counter Minimum (2 bytes)
    //   offset 55: Page Protection (1 byte)
    let gas_addr = table_addr + 40;
    let address_space = unsafe { core::ptr::read_volatile(gas_addr as *const u8) };
    if address_space != 0 { return None; }  // Must be system memory

    let base = unsafe {
        let ptr = (gas_addr + 4) as *const u64;
        core::ptr::read_volatile(ptr)
    };
    if base == 0 { None } else { Some(base) }
}

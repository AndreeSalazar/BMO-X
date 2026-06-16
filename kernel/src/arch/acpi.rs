#![allow(dead_code)]

//! ACPI table parsing for UEFI-booted systems.
//!
//! The RSDP address is provided by the bootloader (via UEFI system table),
//! so no legacy BIOS memory scanning is needed.

use core::mem;

#[repr(C, packed)]
pub struct Rsdp {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
}

#[repr(C, packed)]
pub struct Rsdp2 {
    pub base: Rsdp,
    pub length: u32,
    pub xsdt_address: u64,
    pub extended_checksum: u8,
    pub _reserved: [u8; 3],
}

#[repr(C, packed)]
pub struct SdtHeader {
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

#[repr(C, packed)]
pub struct McfgEntry {
    pub base_address: u64,
    pub segment_group: u16,
    pub start_bus: u8,
    pub end_bus: u8,
    pub _reserved: u32,
}

pub struct EcamInfo {
    pub base_addr: u64,
    pub start_bus: u8,
    pub end_bus: u8,
}

/// Write a u64 in decimal to serial.
fn ser_u64(mut val: u64) {
    let mut buf = [0u8; 20];
    let mut i = buf.len();
    if val == 0 {
        i -= 1;
        buf[i] = b'0';
    } else {
        while val > 0 {
            i -= 1;
            buf[i] = b'0' + (val % 10) as u8;
            val /= 10;
        }
    }
    crate::drivers::serial::serial_write(core::str::from_utf8(&buf[i..]).unwrap_or("0"));
}

/// Write a u64 in hex to serial.
fn ser_hex(val: u64) {
    let hex = b"0123456789ABCDEF";
    crate::drivers::serial::serial_write("0x");
    for i in (0..16).rev() {
        crate::drivers::serial::serial_write_byte(hex[((val >> (i * 4)) & 0xF) as usize]);
    }
}

/// Validate an ACPI table by summing all bytes over its `length`; must equal 0.
fn validate_checksum(header: *const SdtHeader) -> bool {
    let len = unsafe { (*header).length } as usize;
    let bytes = header as *const u8;
    let mut sum: u8 = 0;
    for i in 0..len {
        sum = sum.wrapping_add(unsafe { *bytes.add(i) });
    }
    sum == 0
}

/// Validate the RSDP v1 checksum (first 20 bytes).
fn validate_rsdp_checksum(rsdp: *const Rsdp) -> bool {
    let bytes = rsdp as *const u8;
    let mut sum: u8 = 0;
    for i in 0..mem::size_of::<Rsdp>() {
        sum = sum.wrapping_add(unsafe { *bytes.add(i) });
    }
    sum == 0
}

/// Parse the MCFG table reachable from the RSDP at `rsdp_addr`.
///
/// `rsdp_addr` is the physical address provided by the UEFI bootloader in
/// `BootInfo`. Memory is assumed to be identity-mapped.
pub fn parse_mcfg(rsdp_addr: u64) -> Option<EcamInfo> {
    let ser = crate::drivers::serial::serial_write;

    if rsdp_addr == 0 {
        ser("[ACPI] RSDP address is NULL\n");
        return None;
    }

    let rsdp = rsdp_addr as *const Rsdp;

    if !validate_rsdp_checksum(rsdp) {
        ser("[ACPI] RSDP checksum invalid\n");
        return None;
    }

    let revision = unsafe { (*rsdp).revision };
    ser("[ACPI] RSDP found, revision=");
    ser_u64(revision as u64);
    ser("\n");

    if revision >= 2 {
        let rsdp2 = rsdp_addr as *const Rsdp2;
        let xsdt_addr = unsafe { (*rsdp2).xsdt_address };
        ser("[ACPI] Using XSDT at ");
        ser_hex(xsdt_addr);
        ser("\n");
        find_mcfg_in_xsdt(xsdt_addr)
    } else {
        let rsdt_addr = unsafe { (*rsdp).rsdt_address } as u64;
        ser("[ACPI] Using RSDT\n");
        find_mcfg_in_rsdt(rsdt_addr)
    }
}

fn find_mcfg_in_xsdt(xsdt_addr: u64) -> Option<EcamInfo> {
    let ser = crate::drivers::serial::serial_write;
    let header = xsdt_addr as *const SdtHeader;
    if !validate_checksum(header) {
        ser("[ACPI] XSDT checksum invalid\n");
        return None;
    }

    let total_len = unsafe { (*header).length } as usize;
    let header_size = mem::size_of::<SdtHeader>();
    let entries_len = total_len.checked_sub(header_size)?;
    let entry_count = entries_len / mem::size_of::<u64>();

    ser("[ACPI] XSDT entries: ");
    ser_u64(entry_count as u64);
    ser("\n");

    let entries_base = (xsdt_addr as usize + header_size) as *const u64;

    for i in 0..entry_count {
        let entry_addr = unsafe { entries_base.add(i).read_unaligned() };
        if let Some(info) = try_parse_mcfg(entry_addr) {
            return Some(info);
        }
    }
    ser("[ACPI] MCFG not found in XSDT\n");
    None
}

fn find_mcfg_in_rsdt(rsdt_addr: u64) -> Option<EcamInfo> {
    let ser = crate::drivers::serial::serial_write;
    let header = rsdt_addr as *const SdtHeader;
    if !validate_checksum(header) {
        ser("[ACPI] RSDT checksum invalid\n");
        return None;
    }

    let total_len = unsafe { (*header).length } as usize;
    let header_size = mem::size_of::<SdtHeader>();
    let entries_len = total_len.checked_sub(header_size)?;
    let entry_count = entries_len / mem::size_of::<u32>();

    ser("[ACPI] RSDT entries: ");
    ser_u64(entry_count as u64);
    ser("\n");

    let entries_base = (rsdt_addr as usize + header_size) as *const u32;

    for i in 0..entry_count {
        let entry_addr = unsafe { entries_base.add(i).read_unaligned() } as u64;
        if let Some(info) = try_parse_mcfg(entry_addr) {
            return Some(info);
        }
    }
    ser("[ACPI] MCFG not found in RSDT\n");
    None
}

fn try_parse_mcfg(table_addr: u64) -> Option<EcamInfo> {
    let ser = crate::drivers::serial::serial_write;
    let header = table_addr as *const SdtHeader;
    let sig = unsafe { (*header).signature };

    // Log signature of each table we check
    ser("[ACPI] Table: ");
    ser(core::str::from_utf8(&sig).unwrap_or("????"));
    ser("\n");

    if &sig != b"MCFG" {
        return None;
    }
    if !validate_checksum(header) {
        ser("[ACPI] MCFG checksum invalid\n");
        return None;
    }

    let header_size = mem::size_of::<SdtHeader>();
    let mcfg_entries_offset = header_size + 8;
    let total_len = unsafe { (*header).length } as usize;
    if total_len < mcfg_entries_offset + mem::size_of::<McfgEntry>() {
        return None;
    }

    let entry = (table_addr as usize + mcfg_entries_offset) as *const McfgEntry;
    let info = EcamInfo {
        base_addr: unsafe { entry.read_unaligned().base_address },
        start_bus: unsafe { (*entry).start_bus },
        end_bus: unsafe { (*entry).end_bus },
    };

    ser("[ACPI] MCFG found! ECAM base=");
    ser_hex(info.base_addr);
    ser("\n");

    Some(info)
}

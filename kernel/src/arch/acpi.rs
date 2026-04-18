//! ACPI RSDP discovery (minimal).

const RSDP_SIG: &[u8; 8] = b"RSD PTR ";

#[repr(C, packed)]
pub struct Rsdp {
    pub signature: [u8; 8],
    pub checksum: u8,
    pub oem_id: [u8; 6],
    pub revision: u8,
    pub rsdt_address: u32,
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

pub fn find_rsdp() -> Option<*const Rsdp> {
    let mut addr = 0x000E_0000usize;
    while addr < 0x000F_FFFF {
        let sig = unsafe { &*(addr as *const [u8; 8]) };
        if sig == RSDP_SIG {
            let rsdp = addr as *const Rsdp;
            if validate_checksum(rsdp) {
                return Some(rsdp);
            }
        }
        addr += 16;
    }
    None
}

fn validate_checksum(rsdp: *const Rsdp) -> bool {
    let bytes = rsdp as *const u8;
    let mut sum: u8 = 0;
    for i in 0..20 {
        sum = sum.wrapping_add(unsafe { *bytes.add(i) });
    }
    sum == 0
}

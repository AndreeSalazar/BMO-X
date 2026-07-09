//! devour.rs — Windows PE/COFF → BEF converter.
//!
//! Parses PE32+ with goblin, extracts sections (.text/.rdata/.data),
//! wraps them as BEF sections, and builds a BEF binary.

use alloc::vec::Vec;
use goblin::pe::PE;

pub fn devour_pe_to_bef(pe_bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    let pe = PE::parse(pe_bytes).map_err(|_| "goblin: invalid PE")?;

    if !pe.is_64 {
        return Err("32-bit PE not supported (use 64-bit)");
    }

    let entry = pe.entry as u64;

    // Collect sections
    let mut code = Vec::new();
    let mut rodata = Vec::new();
    let mut data = Vec::new();

    for section in &pe.sections {
        let name = section.name().unwrap_or("???");
        let section_bytes = pe_bytes.get(
            section.pointer_to_raw_data as usize..
            (section.pointer_to_raw_data + section.size_of_raw_data) as usize,
        );

        if let Some(bytes) = section_bytes {
            let characteristics = section.characteristics;
            let is_code = characteristics & 0x20000000 != 0; // IMAGE_SCN_CNT_CODE
            let is_data = characteristics & 0x00000040 != 0; // IMAGE_SCN_CNT_INITIALIZED_DATA
            let is_writable = characteristics & 0x80000000 != 0;

            if is_code {
                code.extend_from_slice(bytes);
            } else if is_data && is_writable {
                data.extend_from_slice(bytes);
            } else if is_data {
                rodata.extend_from_slice(bytes);
            }
        }
    }

    // Build minimal BEF
    let mut bef = Vec::with_capacity(4096);
    let bef_magic: u32 = u32::from_le_bytes(*b"BEF1");

    let mut sections: Vec<(&[u8], u32)> = Vec::new();
    if !code.is_empty()   { sections.push((&code, 0x01)); }
    if !rodata.is_empty() { sections.push((&rodata, 0x02)); }
    if !data.is_empty()   { sections.push((&data, 0x03)); }

    let section_count = sections.len() as u32;

    // Header
    bef.extend_from_slice(&bef_magic.to_le_bytes());
    bef.extend_from_slice(&1u32.to_le_bytes());     // version
    bef.extend_from_slice(&0u32.to_le_bytes());     // flags
    bef.extend_from_slice(&entry.to_le_bytes());    // entry_offset (absolute)
    bef.extend_from_slice(&0u32.to_le_bytes());     // arch
    bef.extend_from_slice(&section_count.to_le_bytes());
    bef.extend_from_slice(&0u32.to_le_bytes());     // file_size placeholder
    bef.extend_from_slice(&[0u8; 8]);               // reserved
    bef.extend_from_slice(&0u32.to_le_bytes());     // manifest_offset
    bef.extend_from_slice(&0u32.to_le_bytes());     // manifest_size
    bef.extend_from_slice(&0u32.to_le_bytes());     // _pad

    // Section table + data
    let table_offset = bef.len() as u64;
    let mut data_offsets = Vec::new();
    let mut data_offset = 48 + (section_count as u64) * 48;
    for (idx, (s, kind)) in sections.iter().enumerate() {
        let aligned = (data_offset + 7) & !7;
        data_offset = aligned;
        data_offsets.push(data_offset);

        bef.extend_from_slice(&(*kind as u32).to_le_bytes());
        bef.extend_from_slice(&0u32.to_le_bytes()); // flags
        bef.extend_from_slice(&(s.len() as u32).to_le_bytes());
        bef.extend_from_slice(&(s.len() as u32).to_le_bytes());
        bef.extend_from_slice(&data_offset.to_le_bytes());
        bef.extend_from_slice(&0u64.to_le_bytes()); // virt_addr
        bef.extend_from_slice(&8u32.to_le_bytes()); // alignment
        bef.extend_from_slice(&0u32.to_le_bytes()); // name_offset
        data_offset += s.len() as u64;
    }

    for (idx, (s, _)) in sections.iter().enumerate() {
        while bef.len() < data_offsets[idx] as usize { bef.push(0); }
        bef.extend_from_slice(s);
    }

    let file_size = bef.len() as u32;
    bef[20..24].copy_from_slice(&file_size.to_le_bytes());

    Ok(bef)
}

use alloc::vec::Vec;
use goblin::pe::PE;

pub fn devour_pe_to_bef(pe_bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    let pe = PE::parse(pe_bytes).map_err(|_| "goblin: invalid PE")?;
    if !pe.is_64 { return Err("32-bit PE not supported"); }

    let mut code = Vec::new();
    let mut rodata = Vec::new();
    let mut data = Vec::new();

    for section in &pe.sections {
        let section_bytes = pe_bytes.get(
            section.pointer_to_raw_data as usize..
            (section.pointer_to_raw_data + section.size_of_raw_data) as usize,
        );

        if let Some(bytes) = section_bytes {
            let characteristics = section.characteristics;
            let is_code = characteristics & 0x20000000 != 0;
            let is_writable = characteristics & 0x80000000 != 0;

            if is_code {
                code.extend_from_slice(bytes);
            } else if is_writable {
                data.extend_from_slice(bytes);
            } else {
                rodata.extend_from_slice(bytes);
            }
        }
    }

    let mut sections: Vec<(&[u8], u8, u8)> = Vec::new();
    if !code.is_empty()   { sections.push((&code, 0x01, 0x05)); }
    if !rodata.is_empty() { sections.push((&rodata, 0x02, 0x01)); }
    if !data.is_empty()   { sections.push((&data, 0x03, 0x03)); }

    let section_count = sections.len() as u32;
    let entry_offset = pe.entry as u64;

    let mut bef = Vec::with_capacity(4096);
    let bef_magic: u32 = 0x31464542; // "BEF1" LE

    // ── Header: 48 bytes ──────────────────────────────────────
    // [0..4)  magic(4)
    bef.extend_from_slice(&bef_magic.to_le_bytes());
    // [4..6)  version_major(2) = 1
    // [6..8)  version_minor(2) = 0
    bef.extend_from_slice(&1u16.to_le_bytes());
    bef.extend_from_slice(&0u16.to_le_bytes());
    // [8..12) flags(4) = EXECUTABLE
    bef.extend_from_slice(&1u32.to_le_bytes());
    // [12..13) arch(1) = X86_64
    // [13..16) _pad0(3)
    bef.push(1u8);
    bef.extend_from_slice(&[0u8; 3]);
    // [16..17) abi_version_major(1) = 1
    // [17..18) abi_version_minor(1) = 0
    // [18..24) _pad1(6)
    bef.push(1u8);
    bef.push(0u8);
    bef.extend_from_slice(&[0u8; 6]);
    // [24..32) entry_offset(8)
    bef.extend_from_slice(&entry_offset.to_le_bytes());
    // [32..40) section_table_offset(8) — placeholder
    let table_offset_pos = bef.len() as u32;
    bef.extend_from_slice(&0u64.to_le_bytes());
    // [40..44) section_count(4)
    bef.extend_from_slice(&section_count.to_le_bytes());
    // [44..48) total_size(4) — placeholder
    let total_size_pos = bef.len() as u32;
    bef.extend_from_slice(&0u32.to_le_bytes());

    // ── Section table: 48 bytes per entry ─────────────────────
    let section_table_offset = bef.len() as u64;
    let mut data_offsets = Vec::new();
    let mut data_offset = section_table_offset + (section_count as u64) * 48;

    for (s, kind, flags) in &sections {
        let aligned = (data_offset + 7) & !7;
        data_offset = aligned;
        data_offsets.push(data_offset);

        let mem_size = s.len() as u64;

        // SectionEntry: 48 bytes
        // [0..1)  kind(1)
        bef.push(*kind);
        // [1..4)  _pad(3)
        bef.extend_from_slice(&[0u8; 3]);
        // [4..8)  flags(4)
        bef.extend_from_slice(&(*flags as u32).to_le_bytes());
        // [8..16) file_offset(8)
        bef.extend_from_slice(&data_offset.to_le_bytes());
        // [16..24) file_size(8)
        bef.extend_from_slice(&mem_size.to_le_bytes());
        // [24..32) mem_size(8)
        bef.extend_from_slice(&mem_size.to_le_bytes());
        // [32..40) virt_addr(8) = 0
        bef.extend_from_slice(&0u64.to_le_bytes());
        // [40..42) alignment(2) = 8
        // [42..44) hash_index(2) = 0xFFFF
        // [44..48) _reserved(4) = 0
        bef.extend_from_slice(&8u16.to_le_bytes());
        bef.extend_from_slice(&0xFFFFu16.to_le_bytes());
        bef.extend_from_slice(&0u32.to_le_bytes());

        data_offset += mem_size;
    }

    // Patch section_table_offset
    let pos = table_offset_pos as usize;
    let table_off_bytes = section_table_offset.to_le_bytes();
    for i in 0..8 { bef[pos + i] = table_off_bytes[i]; }

    // ── Section data ──────────────────────────────────────────
    for (idx, (s, _, _)) in sections.iter().enumerate() {
        while (bef.len() as u64) < data_offsets[idx] { bef.push(0); }
        bef.extend_from_slice(s);
    }

    // Patch total_size
    let file_size = bef.len() as u32;
    let pos = total_size_pos as usize;
    let size_bytes = file_size.to_le_bytes();
    for i in 0..4 { bef[pos + i] = size_bytes[i]; }

    Ok(bef)
}

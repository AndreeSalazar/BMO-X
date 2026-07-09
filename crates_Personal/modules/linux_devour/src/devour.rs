//! devour.rs — ELF → BEF converter.
//!
//! Parses ELF64 with goblin, extracts PT_LOAD segments,
//! wraps them as BEF sections, and builds a BEF binary.

use alloc::vec::Vec;
use goblin::elf::Elf;

/// Metadata about a devoured ELF binary.
pub struct DevouredInfo {
    pub entry_point: u64,
    pub original_size: usize,
    pub bef_size: usize,
}

/// Devour an ELF binary → produce BEF bytes.
/// Returns the BEF binary ready for execution.
pub fn devour_elf_to_bef(elf_bytes: &[u8]) -> Result<Vec<u8>, &'static str> {
    let elf = Elf::parse(elf_bytes).map_err(|_| "goblin: invalid ELF")?;
    if !elf.is_64 { return Err("not 64-bit"); }

    // Build a minimal BEF: just code + rodata + data sections
    // The actual syscall patching happens in the shim, not here.
    // For Nivel 1, we just repackage the ELF as BEF.

    let mut code = Vec::new();
    let mut rodata = Vec::new();
    let mut data = Vec::new();
    let entry = elf.entry;

    for phdr in &elf.program_headers {
        if phdr.p_type != goblin::elf::program_header::PT_LOAD { continue; }
        if phdr.p_memsz == 0 { continue; }

        let file_off = phdr.p_offset as usize;
        let file_sz = phdr.p_filesz as usize;
        let mem_sz = phdr.p_memsz as usize;

        if file_off + file_sz > elf_bytes.len() {
            return Err("segment past EOF");
        }

        let flags = phdr.p_flags;
        let segment_data = &elf_bytes[file_off..file_off + file_sz];

        // Classify by flags
        if flags & goblin::elf::program_header::PF_X != 0 {
            // Executable → .code
            code.extend_from_slice(segment_data);
            if mem_sz > file_sz {
                code.resize(code.len() + (mem_sz - file_sz), 0);
            }
        } else if flags & goblin::elf::program_header::PF_W != 0 {
            // Writable → .data
            data.extend_from_slice(segment_data);
            if mem_sz > file_sz {
                data.resize(data.len() + (mem_sz - file_sz), 0);
            }
        } else {
            // Read-only → .rodata
            rodata.extend_from_slice(segment_data);
            if mem_sz > file_sz {
                rodata.resize(rodata.len() + (mem_sz - file_sz), 0);
            }
        }
    }

    // Build BEF binary manually (minimalist — no BefBuilder needed)
    // BEF header: magic(4) + version(4) + flags(4) + entry_offset(4) +
    //   arch(4) + section_count(4) + file_size(4) + reserved(8) +
    //   manifest_offset(4) + manifest_size(4) + _pad(4) = 48 bytes
    // Then section table, then section data.
    let mut bef = Vec::with_capacity(4096);
    let bef_magic: u32 = u32::from_le_bytes(*b"BEF1");

    // Build sections
    let mut sections: Vec<(&[u8], u32)> = Vec::new(); // (data, kind)
    if !code.is_empty()   { sections.push((&code, 0x01)); }    // SectionKind::Code=1
    if !rodata.is_empty() { sections.push((&rodata, 0x02)); }  // SectionKind::Rodata=2
    if !data.is_empty()   { sections.push((&data, 0x03)); }    // SectionKind::Data=3

    let section_count = sections.len() as u32;
    let entry_offset = 0u64; // entry relative to code section

    // Header (48 bytes)
    bef.extend_from_slice(&bef_magic.to_le_bytes());
    bef.extend_from_slice(&1u32.to_le_bytes());    // version major=1
    bef.extend_from_slice(&0u32.to_le_bytes());    // flags
    bef.extend_from_slice(&entry_offset.to_le_bytes());
    bef.extend_from_slice(&0u32.to_le_bytes());    // arch = x86_64
    bef.extend_from_slice(&section_count.to_le_bytes());
    bef.extend_from_slice(&0u32.to_le_bytes());    // file_size (placeholder)
    bef.extend_from_slice(&[0u8; 8]);              // reserved
    bef.extend_from_slice(&0u32.to_le_bytes());    // manifest_offset
    bef.extend_from_slice(&0u32.to_le_bytes());    // manifest_size
    bef.extend_from_slice(&0u32.to_le_bytes());    // _pad

    // Section table (48 bytes per entry × count)
    let table_offset = bef.len() as u64;
    let mut data_offsets = Vec::new();
    let mut data_offset = 48 + (section_count as u64) * 48;
    for (idx, (s, kind)) in sections.iter().enumerate() {
        let aligned = (data_offset + 7) & !7;
        data_offset = aligned;
        data_offsets.push(data_offset);

        let entry_bytes = [
            &(kind & 0xFF).to_le_bytes()[..],  // kind(4)
            &0u32.to_le_bytes()[..],            // flags(4)
            &(s.len() as u32).to_le_bytes()[..], // file_size(4)
            &(s.len() as u32).to_le_bytes()[..], // mem_size(4)
            &data_offset.to_le_bytes()[..],      // file_offset(8)
            &0u64.to_le_bytes()[..],             // virt_addr(8)
            &8u32.to_le_bytes()[..],             // alignment(4)
            &[0u8; 4],                           // name_offset(4)
        ].concat();
        bef.extend_from_slice(&entry_bytes);
        data_offset += s.len() as u64;
    }

    // Section data
    for (idx, (s, _)) in sections.iter().enumerate() {
        while bef.len() < data_offsets[idx] as usize {
            bef.push(0);
        }
        bef.extend_from_slice(s);
    }

    // Write final size
    let file_size = bef.len() as u32;
    bef[20..24].copy_from_slice(&file_size.to_le_bytes());

    Ok(bef)
}

//! ELF Parser for GSP Firmware — ELF64 + Section Headers + nvfw_bin_hdr
//!
//! The GA10x firmware is an ELF64 RISC-V container with:
//! - phnum=0 (no program headers)  
//! - Section headers containing the booter and GSP-RM code
//! - nvfw_bin_hdr structures inside sections pointing to actual code
//!
//! Key structures from nouveau:
//! - nvfw_bin_hdr: magic, data_offset, data_size, header_offset
//! - nvfw_hs_header_v2: sig offsets, patch locations
//! - nvfw_hs_load_header_v2: app offsets (IMEM/DMEM sizes)

use crate::console::Console;

const EI_CLASS: usize = 4;
const ELFCLASS64: u8 = 2;

/// Parsed section info
#[derive(Copy, Clone)]
pub struct ElfSection {
    pub name_offset: u32,
    pub sh_type: u32,
    pub offset: u64,
    pub size: u64,
    pub flags: u64,
}

/// Firmware info from the ELF
pub struct FirmwareInfo {
    pub entry_point: u64,
    pub e_machine: u16,
    pub e_type: u16,
    pub elf_class: u8,
    pub sections: [ElfSection; 16],
    pub num_sections: usize,
    // nvfw_bin_hdr data extracted from first section
    pub booter_data_offset: u32,
    pub booter_data_size: u32,
    pub booter_header_offset: u32,
    // Segment info for DMA  
    pub segments: [LoadSegment; 8],
    pub num_segments: usize,
}

#[derive(Copy, Clone)]
pub struct LoadSegment {
    pub file_offset: u64,
    pub phys_addr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub is_code: bool,
}

fn r16(d: &[u8], o: usize) -> u16 {
    if o + 2 > d.len() { return 0; }
    u16::from_le_bytes([d[o], d[o+1]])
}
fn r32(d: &[u8], o: usize) -> u32 {
    if o + 4 > d.len() { return 0; }
    u32::from_le_bytes([d[o], d[o+1], d[o+2], d[o+3]])
}
fn r64(d: &[u8], o: usize) -> u64 {
    if o + 8 > d.len() { return 0; }
    u64::from_le_bytes([d[o], d[o+1], d[o+2], d[o+3], d[o+4], d[o+5], d[o+6], d[o+7]])
}

pub fn parse_firmware(fw: &[u8], con: &mut Console) -> Option<FirmwareInfo> {
    if fw.len() < 64 { return None; }
    if fw[0] != 0x7F || fw[1] != b'E' || fw[2] != b'L' || fw[3] != b'F' {
        con.println("  ELF: Bad magic!");
        return None;
    }

    let elf_class = fw[EI_CLASS];
    let mut info = FirmwareInfo {
        entry_point: 0, e_machine: 0, e_type: 0, elf_class,
        sections: [ElfSection { name_offset: 0, sh_type: 0, offset: 0, size: 0, flags: 0 }; 16],
        num_sections: 0,
        booter_data_offset: 0, booter_data_size: 0, booter_header_offset: 0,
        segments: [LoadSegment { file_offset: 0, phys_addr: 0, file_size: 0, mem_size: 0, is_code: false }; 8],
        num_segments: 0,
    };

    if elf_class != ELFCLASS64 {
        con.print("  ELF: Not ELF64, class=");
        con.print_hex32(elf_class as u32);
        con.newline();
        // Still continue, but note it
    }

    // ELF64 header
    info.e_type = r16(fw, 16);
    info.e_machine = r16(fw, 18);
    info.entry_point = r64(fw, 24);
    let phoff = r64(fw, 32) as usize;
    let shoff = r64(fw, 40) as usize;
    let phnum = r16(fw, 56) as usize;
    let shentsize = r16(fw, 58) as usize;
    let shnum = r16(fw, 60) as usize;
    let shstrndx = r16(fw, 62) as usize;

    con.print("  ELF: machine=0x");
    con.print_hex32(info.e_machine as u32);
    con.print(" entry=0x");
    con.print_hex32(info.entry_point as u32);
    con.print(" phnum=");
    con.print_hex32(phnum as u32);
    con.print(" shnum=");
    con.print_hex32(shnum as u32);
    con.newline();

    // ── Parse section headers ──
    // ELF64 Shdr: sh_name(4) sh_type(4) sh_flags(8) sh_addr(8) sh_offset(8) sh_size(8) ...
    let max_sh = shnum.min(16);
    if shoff > 0 && shentsize >= 64 && shnum > 0 {
        // Get string table section offset for section names
        let strtab_off = if shstrndx < shnum {
            let str_sh = shoff + shstrndx * shentsize;
            r64(fw, str_sh + 24) as usize // sh_offset
        } else { 0 };

        for i in 0..max_sh {
            let off = shoff + i * shentsize;
            if off + 64 > fw.len() { break; }

            let sh_name = r32(fw, off);
            let sh_type = r32(fw, off + 4);
            let sh_flags = r64(fw, off + 8);
            let sh_offset = r64(fw, off + 24);
            let sh_size = r64(fw, off + 32);

            info.sections[i] = ElfSection {
                name_offset: sh_name,
                sh_type,
                offset: sh_offset,
                size: sh_size,
                flags: sh_flags,
            };
            info.num_sections = i + 1;

            // Print section info
            if sh_type != 0 && sh_size > 0 {
                con.print("  ELF: Sec[");
                con.print_hex32(i as u32);
                con.print("] type=");
                con.print_hex32(sh_type);
                con.print(" off=0x");
                con.print_hex32(sh_offset as u32);
                con.print(" sz=0x");
                con.print_hex32(sh_size as u32);

                // Try to print section name from string table
                if strtab_off > 0 && sh_name > 0 {
                    let name_off = strtab_off + sh_name as usize;
                    if name_off < fw.len() {
                        con.print(" \"");
                        let mut j = name_off;
                        let mut printed = 0;
                        while j < fw.len() && fw[j] != 0 && printed < 24 {
                            con.put_char(fw[j]);
                            j += 1;
                            printed += 1;
                        }
                        con.print("\"");
                    }
                }
                con.newline();
            }
        }
    }

    // ── Look for nvfw_bin_hdr in the first PROGBITS section ──
    // nvfw_bin_hdr: bin_magic(4) bin_ver(4) bin_size(4) header_offset(4) data_offset(4) data_size(4)
    for i in 0..info.num_sections {
        let sec = &info.sections[i];
        // SHT_PROGBITS = 1, skip NULL (0) and STRTAB (3)
        if sec.sh_type == 1 && sec.size >= 24 {
            let sec_off = sec.offset as usize;
            if sec_off + 24 <= fw.len() {
                let bin_magic = r32(fw, sec_off);
                let bin_ver = r32(fw, sec_off + 4);
                let bin_size = r32(fw, sec_off + 8);
                let header_offset = r32(fw, sec_off + 12);
                let data_offset = r32(fw, sec_off + 16);
                let data_size = r32(fw, sec_off + 20);

                con.print("  FW: Sec[");
                con.print_hex32(i as u32);
                con.print("] bin_magic=0x");
                con.print_hex32(bin_magic);
                con.print(" data_off=0x");
                con.print_hex32(data_offset);
                con.print(" data_sz=0x");
                con.print_hex32(data_size);
                con.newline();

                // Check if this looks like a valid nvfw_bin_hdr
                if data_offset > 0 && data_size > 0 && data_size < 0x1000000 {
                    info.booter_data_offset = sec_off as u32 + data_offset;
                    info.booter_data_size = data_size;
                    info.booter_header_offset = sec_off as u32 + header_offset;

                    con.print("  FW: Booter code at absolute offset 0x");
                    con.print_hex32(info.booter_data_offset);
                    con.print(" (");
                    con.print_hex32(data_size);
                    con.println(" bytes)");

                    // Create a segment for the booter
                    if info.num_segments < 8 {
                        info.segments[info.num_segments] = LoadSegment {
                            file_offset: info.booter_data_offset as u64,
                            phys_addr: 0, // Load to IMEM offset 0
                            file_size: data_size as u64,
                            mem_size: data_size as u64,
                            is_code: true,
                        };
                        info.num_segments += 1;
                    }
                    break; // Use first valid nvfw_bin_hdr
                }
            }
        }
    }

    con.print("  ELF: ");
    con.print_hex32(info.num_sections as u32);
    con.print(" sections, ");
    con.print_hex32(info.num_segments as u32);
    con.println(" loadable found.");

    Some(info)
}

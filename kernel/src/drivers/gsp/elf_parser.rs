//! ELF Parser for GSP Firmware — Supports BOTH ELF32 and ELF64
//!
//! The GSP firmware for GA10x might be ELF32 (RISC-V 32-bit Falcon)
//! or ELF64. We detect the class from e_ident[4] and parse accordingly.

use crate::console::Console;

// ELF identification indices
const EI_CLASS: usize = 4;
const ELFCLASS32: u8 = 1;
const ELFCLASS64: u8 = 2;

// Segment types
pub const PT_LOAD: u32 = 1;
pub const PF_X: u32 = 1;

/// Parsed segment info
#[derive(Copy, Clone)]
pub struct LoadSegment {
    pub file_offset: u64,
    pub phys_addr: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub is_code: bool,
}

/// Firmware info extracted from ELF
pub struct FirmwareInfo {
    pub entry_point: u64,
    pub e_machine: u16,
    pub e_type: u16,
    pub elf_class: u8,
    pub segments: [LoadSegment; 8],
    pub num_segments: usize,
}

/// Helper: read u16 little-endian from slice
fn r16(data: &[u8], off: usize) -> u16 {
    if off + 2 > data.len() { return 0; }
    u16::from_le_bytes([data[off], data[off + 1]])
}

/// Helper: read u32 little-endian from slice
fn r32(data: &[u8], off: usize) -> u32 {
    if off + 4 > data.len() { return 0; }
    u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
}

/// Helper: read u64 little-endian from slice
fn r64(data: &[u8], off: usize) -> u64 {
    if off + 8 > data.len() { return 0; }
    u64::from_le_bytes([
        data[off], data[off+1], data[off+2], data[off+3],
        data[off+4], data[off+5], data[off+6], data[off+7],
    ])
}

/// Parse the firmware ELF and extract loadable segments
pub fn parse_firmware(fw: &[u8], con: &mut Console) -> Option<FirmwareInfo> {
    if fw.len() < 64 { return None; }
    // Verify ELF magic
    if fw[0] != 0x7F || fw[1] != b'E' || fw[2] != b'L' || fw[3] != b'F' {
        con.println("  ELF: Bad magic!");
        return None;
    }

    let elf_class = fw[EI_CLASS];
    let mut info = FirmwareInfo {
        entry_point: 0,
        e_machine: 0,
        e_type: 0,
        elf_class,
        segments: [LoadSegment {
            file_offset: 0, phys_addr: 0, file_size: 0, mem_size: 0, is_code: false,
        }; 8],
        num_segments: 0,
    };

    match elf_class {
        ELFCLASS32 => {
            con.println("  ELF: Class = ELF32 (RISC-V 32-bit)");
            // ELF32 header layout:
            //  16: e_type (2)
            //  18: e_machine (2)
            //  20: e_version (4)
            //  24: e_entry (4)     ← 32-bit!
            //  28: e_phoff (4)
            //  32: e_shoff (4)
            //  36: e_flags (4)
            //  40: e_ehsize (2)
            //  42: e_phentsize (2)
            //  44: e_phnum (2)
            info.e_type = r16(fw, 16);
            info.e_machine = r16(fw, 18);
            info.entry_point = r32(fw, 24) as u64;
            let phoff = r32(fw, 28) as usize;
            let phentsize = r16(fw, 42) as usize;
            let phnum = r16(fw, 44) as usize;

            con.print("  ELF: entry=0x");
            con.print_hex32(info.entry_point as u32);
            con.print(" machine=0x");
            con.print_hex32(info.e_machine as u32);
            con.print(" phoff=0x");
            con.print_hex32(phoff as u32);
            con.print(" phnum=");
            con.print_hex32(phnum as u32);
            con.print(" phentsz=");
            con.print_hex32(phentsize as u32);
            con.newline();

            // Parse ELF32 program headers
            // ELF32 Phdr: p_type(4) p_offset(4) p_vaddr(4) p_paddr(4) p_filesz(4) p_memsz(4) p_flags(4) p_align(4)
            let max_segs = phnum.min(8);
            for i in 0..max_segs {
                let off = phoff + i * phentsize;
                if off + 32 > fw.len() { break; }

                let p_type = r32(fw, off);
                let p_offset = r32(fw, off + 4);
                let p_vaddr = r32(fw, off + 8);
                let p_paddr = r32(fw, off + 12);
                let p_filesz = r32(fw, off + 16);
                let p_memsz = r32(fw, off + 20);
                let p_flags = r32(fw, off + 24);

                con.print("  ELF: Phdr[");
                con.print_hex32(i as u32);
                con.print("] type=");
                con.print_hex32(p_type);
                con.print(" off=0x");
                con.print_hex32(p_offset);
                con.print(" pa=0x");
                con.print_hex32(p_paddr);
                con.print(" fsz=0x");
                con.print_hex32(p_filesz);
                con.print(if p_flags & PF_X != 0 { " X" } else { " D" });
                con.newline();

                if p_type == PT_LOAD && p_filesz > 0 {
                    let idx = info.num_segments;
                    if idx < 8 {
                        info.segments[idx] = LoadSegment {
                            file_offset: p_offset as u64,
                            phys_addr: p_paddr as u64,
                            file_size: p_filesz as u64,
                            mem_size: p_memsz as u64,
                            is_code: (p_flags & PF_X) != 0,
                        };
                        info.num_segments += 1;
                    }
                }
            }
        }
        ELFCLASS64 => {
            con.println("  ELF: Class = ELF64 (RISC-V 64-bit)");
            info.e_type = r16(fw, 16);
            info.e_machine = r16(fw, 18);
            info.entry_point = r64(fw, 24);
            let phoff = r64(fw, 32) as usize;
            let phentsize = r16(fw, 54) as usize;
            let phnum = r16(fw, 56) as usize;

            con.print("  ELF: entry=0x");
            con.print_hex32((info.entry_point >> 32) as u32);
            con.print_hex32(info.entry_point as u32);
            con.print(" machine=0x");
            con.print_hex32(info.e_machine as u32);
            con.print(" phnum=");
            con.print_hex32(phnum as u32);
            con.newline();

            // ELF64 Phdr: p_type(4) p_flags(4) p_offset(8) p_vaddr(8) p_paddr(8) p_filesz(8) p_memsz(8) p_align(8)
            let max_segs = phnum.min(8);
            for i in 0..max_segs {
                let off = phoff + i * phentsize;
                if off + 56 > fw.len() { break; }

                let p_type = r32(fw, off);
                let p_flags = r32(fw, off + 4);
                let p_offset = r64(fw, off + 8);
                let p_paddr = r64(fw, off + 24);
                let p_filesz = r64(fw, off + 32);
                let p_memsz = r64(fw, off + 40);

                con.print("  ELF: Phdr[");
                con.print_hex32(i as u32);
                con.print("] type=");
                con.print_hex32(p_type);
                con.print(" pa=0x");
                con.print_hex32(p_paddr as u32);
                con.print(" fsz=0x");
                con.print_hex32(p_filesz as u32);
                con.newline();

                if p_type == PT_LOAD && p_filesz > 0 {
                    let idx = info.num_segments;
                    if idx < 8 {
                        info.segments[idx] = LoadSegment {
                            file_offset: p_offset,
                            phys_addr: p_paddr,
                            file_size: p_filesz,
                            mem_size: p_memsz,
                            is_code: (p_flags & PF_X) != 0,
                        };
                        info.num_segments += 1;
                    }
                }
            }
        }
        _ => {
            con.print("  ELF: Unknown class: ");
            con.print_hex32(elf_class as u32);
            con.newline();
            return None;
        }
    }

    con.print("  ELF: ");
    con.print_hex32(info.num_segments as u32);
    con.println(" loadable segments found.");

    Some(info)
}

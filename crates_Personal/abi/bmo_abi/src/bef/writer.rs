use alloc::vec::Vec;
use alloc::vec;
use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};
use crate::bmo_abi::bef::{
    header::*,
    sections::*,
    relocations::Relocation,
    imports::ImportEntry,
    exports::ExportEntry,
    symbols::Symbol,
    tls::TlsTemplate,
    signing::{SectionHash, SignatureHeader, blake3_256},
};

pub struct BefSection {
    pub kind: SectionKind,
    pub flags: SectionFlags,
    pub data: Vec<u8>,
    pub mem_size: bx_u64,
    pub alignment: bx_u16,
    pub hash_index: bx_u16,
}

impl BefSection {
    pub fn new(kind: SectionKind, data: Vec<u8>) -> Self {
        let mem_size = data.len() as bx_u64;
        Self { kind, flags: SectionFlags::READ, data, mem_size, alignment: 8, hash_index: 0xFFFF }
    }

    pub fn code(data: Vec<u8>) -> Self {
        let mut s = Self::new(SectionKind::Code, data);
        s.flags = SectionFlags::READ | SectionFlags::EXEC;
        s.alignment = 4096;
        s
    }

    pub fn rodata(data: Vec<u8>) -> Self {
        let mut s = Self::new(SectionKind::RoData, data);
        s.flags = SectionFlags::READ;
        s
    }

    pub fn data(data: Vec<u8>) -> Self {
        let mut s = Self::new(SectionKind::Data, data);
        s.flags = SectionFlags::READ | SectionFlags::WRITE;
        s
    }

    pub fn bss(size: bx_u64) -> Self {
        let mut s = Self::new(SectionKind::Bss, Vec::new());
        s.flags = SectionFlags::READ | SectionFlags::WRITE;
        s.mem_size = size;
        s
    }

    pub fn imports(entries: Vec<ImportEntry>, strings: Vec<u8>) -> Self {
        let mut data = Vec::from(bytes_from_slice(&entries));
        data.extend_from_slice(&strings);
        Self::new(SectionKind::Imports, data)
    }

    pub fn exports(entries: Vec<ExportEntry>, strings: Vec<u8>) -> Self {
        let mut data = Vec::from(bytes_from_slice(&entries));
        data.extend_from_slice(&strings);
        Self::new(SectionKind::Exports, data)
    }

    pub fn symbols(entries: Vec<Symbol>, strings: Vec<u8>) -> Self {
        let mut data = Vec::from(bytes_from_slice(&entries));
        data.extend_from_slice(&strings);
        Self::new(SectionKind::Symbols, data)
    }

    pub fn relocs(entries: Vec<Relocation>) -> Self {
        Self::new(SectionKind::Relocs, Vec::from(bytes_from_slice(&entries)))
    }

    pub fn manifest_toml(toml: Vec<u8>) -> Self {
        Self::new(SectionKind::Manifest, toml)
    }

    pub fn tls(template: &TlsTemplate, tls_data: &[u8]) -> Self {
        let mut buf = Vec::from(bytes_from_struct(template));
        buf.extend_from_slice(tls_data);
        Self::new(SectionKind::Tls, buf)
    }
}

pub struct BefBuilder {
    pub header: BefHeader,
    pub sections: Vec<BefSection>,
    pub entry_offset: bx_u64,
}

impl BefBuilder {
    pub fn new() -> Self {
        Self { header: BefHeader::new_executable(), sections: Vec::new(), entry_offset: 0 }
    }

    pub fn add_section(&mut self, section: BefSection) {
        self.sections.push(section);
    }

    pub fn build(&mut self) -> Result<Vec<u8>, &'static str> {
        let count = self.sections.len() as bx_u32;
        if count == 0 { return Err("no sections"); }
        if count > 255 { return Err("too many sections"); }

        self.header.section_count = count;
        self.header.entry_offset = self.entry_offset;
        self.header.section_table_offset = BefHeader::SIZE as u64;

        let header_size = BefHeader::SIZE as u64;
        let table_size = (count as u64) * (SectionEntry::SIZE as u64);
        let table_offset = header_size;
        let sig_idx = self.sections.iter().position(|s| s.kind == SectionKind::Signature);

        let mut entries = Vec::with_capacity(count as usize);
        let mut file_off = header_size + table_size;

        for (i, section) in self.sections.iter().enumerate() {
            let mut entry = SectionEntry::ZERO;
            entry.kind = section.kind as u8;
            entry.flags = section.flags.bits();
            entry.mem_size = section.mem_size;
            entry.alignment = section.alignment;
            entry.hash_index = section.hash_index;

            if section.kind != SectionKind::Bss && !section.data.is_empty() {
                if sig_idx != Some(i) {
                    let align = (section.alignment as u64).max(8);
                    file_off = (file_off + align - 1) & !(align - 1);
                    entry.file_offset = file_off;
                    entry.file_size = section.data.len() as u64;
                    file_off += entry.file_size;
                }
            }
            entries.push(entry);
        }

        let total_size = file_off as usize;
        let mut buf = vec![0u8; total_size];

        let hdr = bytes_from_struct(&self.header);
        buf[..header_size as usize].copy_from_slice(hdr);

        let tbl = bytes_from_slice(&entries);
        buf[table_offset as usize..table_offset as usize + tbl.len()].copy_from_slice(tbl);

        let mut write_off = header_size + table_size;
        for (i, section) in self.sections.iter().enumerate() {
            if section.kind == SectionKind::Bss || section.data.is_empty() { continue; }
            if sig_idx == Some(i) { continue; }
            let align = (section.alignment as u64).max(8);
            write_off = (write_off + align - 1) & !(align - 1);
            let end = write_off + section.data.len() as u64;
            if end as usize > buf.len() { buf.resize(end as usize, 0); }
            buf[write_off as usize..end as usize].copy_from_slice(&section.data);
            write_off = end;
        }

        let mut section_hashes = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            let start = entry.file_offset as usize;
            let end = start + entry.file_size as usize;
            let section_bytes = if entry.file_size > 0 && end <= buf.len() { &buf[start..end] } else { &[] };
            let digest = blake3_256(section_bytes);
            section_hashes.push(SectionHash {
                section_index: i as u16, _pad: [0; 6], digest,
            });
        }

        let sig_header = SignatureHeader { hash_count: section_hashes.len() as u32, sig_algo: 0 };
        let mut sig_data = Vec::from(bytes_from_struct(&sig_header));
        sig_data.extend_from_slice(bytes_from_slice(&section_hashes));

        match sig_idx {
            Some(_) => {}
            None => {
                let align = 8u64;
                write_off = (write_off + align - 1) & !(align - 1);
                let end = write_off + sig_data.len() as u64;
                buf.resize(end as usize, 0);
                buf[write_off as usize..end as usize].copy_from_slice(&sig_data);
            }
        }

        let file_len = buf.len() as u32;
        buf[44..48].copy_from_slice(&file_len.to_le_bytes());
        Ok(buf)
    }
}

fn bytes_from_struct<T: Sized>(val: &T) -> &[u8] {
    unsafe { core::slice::from_raw_parts(val as *const T as *const u8, core::mem::size_of::<T>()) }
}

fn bytes_from_slice<T: Sized>(slice: &[T]) -> &[u8] {
    unsafe { core::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * core::mem::size_of::<T>()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_minimal_bef() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xCC; 64]));
        b.add_section(BefSection::rodata(b"hello\0".to_vec()));
        b.entry_offset = 0;
        let bytes = b.build().unwrap();
        assert!(bytes.len() > 48);
        let magic = u32::from_le_bytes(bytes[..4].try_into().unwrap());
        assert_eq!(magic, BEF_MAGIC);
    }

    #[test]
    fn build_all_sections() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 16]));
        b.add_section(BefSection::rodata(b"data".to_vec()));
        b.add_section(BefSection::data(vec![0x00; 32]));
        b.add_section(BefSection::bss(256));
        b.add_section(BefSection::manifest_toml(b"[identity]\nname = \"test\"\n".to_vec()));
        let bytes = b.build().unwrap();
        assert!(bytes.len() > 48);
    }
}

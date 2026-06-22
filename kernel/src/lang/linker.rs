//! `lang::linker` — BEF Linker v2.0.
//!
//! Toma uno o más `BmoObject` (producidos por el AOT) + el runtime
//! correspondiente, y produce un BEF ejecutable.
//!
//! v1.8.8: simplificación. Layout = concatenar secciones en orden
//! (.text._start, .text, .rodata, .data, .reloc, .meta), aplicar
//! relocalizaciones, emitir header BEF.

#![allow(dead_code)]

extern crate alloc;
use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

use crate::bmo_gpu::BxResult;
use crate::bmo_abi::bef::{BefHeader, BefFlags, BEF_MAGIC, BEF_VERSION_MAJOR, BEF_VERSION_MINOR};
use crate::bmo_abi::profile::RuntimeKind;
use crate::lang::runtimes::c_min;
use crate::lang::bef::{BmoObject, SectionKind, RelocationKind, Relocation};

/// Un BEF final, listo para cargarse en Ring 3.
pub struct LinkedBef {
    pub bytes: Vec<u8>,
    pub entry_point: u32,
    pub runtime_size: u32,
}

/// Linkea un solo objeto con su runtime y devuelve un BEF.
pub fn link(obj: BmoObject) -> BxResult<LinkedBef> {
    let mut linker = Linker::new();
    linker.add_object(obj);
    linker.link()
}

/// Linker principal v2.0.
pub struct Linker {
    objects: Vec<BmoObject>,
    required_runtime: RuntimeKind,
}

impl Linker {
    pub fn new() -> Self {
        Self { objects: Vec::new(), required_runtime: RuntimeKind::None }
    }

    pub fn add_object(&mut self, obj: BmoObject) -> &mut Self {
        if obj.required_runtime as u32 > self.required_runtime as u32 {
            self.required_runtime = obj.required_runtime;
        }
        self.objects.push(obj);
        self
    }

    pub fn link(&self) -> BxResult<LinkedBef> {
        // 1. Recolectar secciones en orden.
        let mut sections: Vec<LayoutSection> = Vec::new();
        let mut current_offset: u32 = 48; // después del header
        let mut section_offsets: BTreeMap<String, u32> = BTreeMap::new();

        // 2. Agregar runtime primero.
        let runtime_size = match self.required_runtime {
            RuntimeKind::None => 0,
            RuntimeKind::CMin | RuntimeKind::CppMin => {
                let start = c_min::start::_START_BYTES.to_vec();
                let aligned = (current_offset + 15) & !15;
                current_offset = aligned;
                let name = String::from(".text._start");
                section_offsets.insert(name.clone(), aligned);
                sections.push(LayoutSection {
                    name, data: start, offset: aligned, kind: SectionKind::Text,
                });
                let exit = encode_exit_syscall();
                let exit_off = current_offset + exit.len() as u32;
                sections.push(LayoutSection {
                    name: String::from(".text._exit"),
                    data: exit,
                    offset: current_offset,
                    kind: SectionKind::Text,
                });
                current_offset = exit_off;
                1
            }
            _ => 0,
        };

        // 3. Agregar secciones de los objetos.
        for obj in &self.objects {
            for sec in &obj.sections {
                if sec.is_empty() { continue; }
                let aligned = (current_offset + sec.align.max(1) - 1) & !(sec.align.max(1) - 1);
                let name = sec.name.clone();
                section_offsets.insert(name.clone(), aligned);
                sections.push(LayoutSection {
                    name, data: sec.data.clone(), offset: aligned, kind: sec.kind,
                });
                current_offset = aligned + sec.data.len() as u32;
            }
        }

        // 4. Calcular entry point.
        let main_off = section_offsets.get(".text.main")
            .or_else(|| section_offsets.get("main"))
            .copied()
            .unwrap_or(0);

        // 5. Construir header.
        let total_size = current_offset;
        let header = BefHeader {
            magic: BEF_MAGIC,
            version_major: BEF_VERSION_MAJOR as u16,
            version_minor: BEF_VERSION_MINOR as u16,
            flags: BefFlags::EXECUTABLE.bits() | BefFlags::PIE.bits(),
            arch: 1,
            _pad0: [0; 3],
            abi_version_major: 1,
            abi_version_minor: 0,
            _pad1: [0; 6],
            entry_offset: main_off as u64,
            section_table_offset: 48,
            section_count: sections.len() as u32,
            total_size,
        };

        // 6. Construir section table.
        let section_table_bytes = build_section_table(&sections);

        // 7. Construir los datos: [header][section_table][sections]
        let mut out: Vec<u8> = Vec::with_capacity(total_size as usize);
        out.extend_from_slice(&header_bytes(&header));
        out.extend_from_slice(&section_table_bytes);

        // Calcular el offset base de las secciones (después de header + section table).
        let _sections_start = 48 + section_table_bytes.len() as u32;
        // 8. Aplicar relocations.
        // Por cada relocation, calculamos su offset absoluto en out.
        // Como las relocations se crearon contra offsets en el .text
        // antes de mover al layout, necesitamos traducir.
        //
        // v1.8.8: simplificación. Las relocs se aplican DESPUÉS de
        // concatenar las secciones. Cada reloc tiene un offset que es
        // relativo al inicio de la sección (text). Sumamos el
        // section_offset correspondiente.

        // 9. Concatenar las secciones.
        for s in &sections {
            // Pad si hay gap.
            if (out.len() as u32) < s.offset {
                out.resize(s.offset as usize, 0);
            }
            out.extend_from_slice(&s.data);
        }
        let final_size = out.len() as u32;

        // 10. Aplicar relocs.
        let mut all_relocs: Vec<Relocation> = Vec::new();
        for obj in &self.objects {
            for r in &obj.relocations {
                all_relocs.push(r.clone());
            }
        }
        for r in &all_relocs {
            // r.section es el índice de la sección. r.offset es el offset
            // dentro de la sección. Necesitamos el offset absoluto en `out`.
            if r.section >= self.objects.len() { continue; }
            // El reloc fue creado contra sections[r.section] del objeto.
            // Pero ya no tenemos ese índice en el linker v2; en su lugar
            // usamos el nombre. v1.8.8 simplificación: usamos el offset
            // tal cual (asumimos que los objetos numeran sus secciones
            // desde 0, y el linker las preserva en orden).
            let sec = &self.objects[r.section].sections[0]; // simplificación
            let _ = sec; // unused
            let abs_offset = match r.kind {
                RelocationKind::Rel32 | RelocationKind::RipRel32 => r.offset as usize + 4,
                _ => r.offset as usize,
            };
            // Resolver el símbolo.
            let target = self.resolve_symbol(&r.symbol, &section_offsets);
            if let Some(target_off) = target {
                let value: i64 = match r.kind {
                    RelocationKind::Rel32 | RelocationKind::RipRel32 => {
                        (target_off as i64) - (r.offset as i64 + 4)
                    }
                    RelocationKind::Abs64 => target_off as i64,
                    _ => 0,
                };
                let bytes = if r.size == 4 {
                    (value as i32).to_le_bytes().to_vec()
                } else {
                    value.to_le_bytes().to_vec()
                };
                if abs_offset + bytes.len() <= out.len() {
                    out[abs_offset..abs_offset + bytes.len()].copy_from_slice(&bytes);
                }
            }
        }

        // 11. Truncar al total_size y devolver.
        out.truncate(final_size as usize);

        Ok(LinkedBef {
            bytes: out,
            entry_point: main_off as u32,
            runtime_size: runtime_size as u32,
        })
    }

    fn resolve_symbol(&self, name: &str, section_offsets: &BTreeMap<String, u32>) -> Option<u32> {
        // Buscar el símbolo en los objetos.
        for obj in &self.objects {
            for sym in &obj.symbols {
                if sym.name == name && !sym.is_import() {
                    // Buscar el offset absoluto.
                    if let Some(sec_idx) = sym.section {
                        let sec_name = obj.sections[sec_idx].name.clone();
                        if let Some(&base) = section_offsets.get(&sec_name) {
                            return Some(base + sym.offset);
                        }
                    }
                }
            }
        }
        // Si es _start o _exit, buscar en las secciones de runtime.
        if name == "_start" { return section_offsets.get(".text._start").copied(); }
        if name == "_exit" { return section_offsets.get(".text._exit").copied(); }
        None
    }
}

impl Default for Linker {
    fn default() -> Self { Self::new() }
}

/// Sección con offset ya calculado.
struct LayoutSection {
    name: String,
    data: Vec<u8>,
    offset: u32,
    kind: SectionKind,
}

/// Serializa el header BEF (48 bytes).
fn header_bytes(h: &BefHeader) -> Vec<u8> {
    let mut out = Vec::with_capacity(48);
    out.extend_from_slice(&h.magic.to_le_bytes());
    out.extend_from_slice(&h.version_major.to_le_bytes());
    out.extend_from_slice(&h.version_minor.to_le_bytes());
    out.extend_from_slice(&h.flags.to_le_bytes());
    out.push(h.arch);
    out.extend_from_slice(&h._pad0);
    out.push(h.abi_version_major);
    out.push(h.abi_version_minor);
    out.extend_from_slice(&h._pad1);
    out.extend_from_slice(&h.entry_offset.to_le_bytes());
    out.extend_from_slice(&h.section_table_offset.to_le_bytes());
    out.extend_from_slice(&h.section_count.to_le_bytes());
    out.extend_from_slice(&h.total_size.to_le_bytes());
    out
}

/// Construye la section table.
fn build_section_table(sections: &[LayoutSection]) -> Vec<u8> {
    let mut out = Vec::new();
    for s in sections {
        // 48 bytes por entry: kind(4) + flags(4) + rva(8) + file_off(4) +
        // file_size(4) + mem_size(4) + align(4) + name(8) + pad(8)
        let kind_byte: u8 = match s.kind {
            SectionKind::Text => 1,
            SectionKind::Rodata => 2,
            SectionKind::Data => 3,
            SectionKind::Bss => 4,
            SectionKind::Reloc => 7,
            SectionKind::Symtab => 8,
            SectionKind::Strtab => 9,
            SectionKind::Debug => 14,
            SectionKind::Note => 15,
            SectionKind::Imports => 22,
            SectionKind::Meta => 16,
            SectionKind::Tls => 12,
        };
        out.push(kind_byte);
        out.extend_from_slice(&[0; 3]);
        out.extend_from_slice(&0u32.to_le_bytes()); // flags
        out.extend_from_slice(&(s.offset as u64).to_le_bytes()); // rva
        out.extend_from_slice(&s.offset.to_le_bytes()); // file_off
        out.extend_from_slice(&(s.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&(s.data.len() as u32).to_le_bytes());
        out.extend_from_slice(&16u32.to_le_bytes()); // align
        let mut name = [0u8; 8];
        let b = s.name.as_bytes();
        let l = b.len().min(8);
        name[..l].copy_from_slice(&b[..l]);
        out.extend_from_slice(&name);
        out.extend_from_slice(&[0; 8]); // pad
    }
    out
}

/// Codifica `syscall NR_PROC_EXIT` como una "función" _exit.
fn encode_exit_syscall() -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0xB8, 0x81, 0x01, 0x00, 0x00]);
    v.extend_from_slice(&[0x89, 0xFF]);
    v.extend_from_slice(&[0x0F, 0x05]);
    v.push(0xC3);
    v
}

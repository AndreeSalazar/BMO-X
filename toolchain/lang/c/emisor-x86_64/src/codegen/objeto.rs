//! **BMO C writes an OBJECT (`.bo`)** -- E2 of `docs/plan/PLAN_EL_ENLAZADOR.md`.
//!
//! [fase]     IMAGEN
//! [aparece]  BANCO
//! [carril]   VERDE -- lo caza `cargo test`: el banco lee el objeto con el contrato
//!
//! Es la fase IMAGEN --colocar secciones y cerrar referencias-- y su fallo
//! aparece en el BANCO: `src/tests/objeto.rs` lee el objeto con el mismo
//! contrato que usara el enlazador, asi que una reloc mal puesta se ve en
//! `cargo test` y no tres pasos mas alla, al enlazar.
//!
//! An image (`.bex`) and an object share every byte the codegen emits. What
//! changes is WHO closes the references:
//!
//! ```text
//!                           image (.bex)                object (.bo)
//!    call f  (f here)       codegen writes rel32        codegen writes rel32
//!    call f  (f elsewhere)  ERROR "no hay enlazado"     Rel32 -> undefined f
//!    lea [rip+string]       codegen, loader's pages     Rel32 -> .rodata + off
//!    lea [rip+global]       codegen, loader's pages     Rel32 -> .data/.bss + off
//!    lea [rip+extern g]     (it had its own copy)       Rel32 -> undefined g
//!    pointer in data        SeccionAbs64                the same SeccionAbs64
//!    pointer to extern      ERROR                       Abs64 -> undefined
//! ```
//!
//! ** The rip-relative ones are the reason this file exists. The codegen
//! computes them assuming the loader's layout of ONE unit -- code, then rodata
//! on the next page, then data. Linked with other units, other code sits in
//! between and every one of those distances would read the wrong bytes without
//! failing. In an object they are left at zero and handed to the linker.
//!
//! The contract these bytes follow is `bmo_abi::bef::objeto`, and
//! `compile_to_object` refuses to return anything `objeto::read` would reject.

use super::*;
use bmo_abi::bef::header::BefFlags;
use bmo_abi::bef::objeto::{REL_CODE, REL_DATA};
use bmo_abi::bef::relocations::{Relocation, RelocationKind};
use bmo_abi::bef::symbols::{name_hash, Symbol, SymbolBinding, SymbolKind, SymbolVisibility, SECTION_UNDEFINED};

/// What a reference left for the linker points at.
#[derive(Clone, Debug)]
pub(super) enum Destino {
    /// A place inside one of this unit's own sections.
    Seccion(SectionKind, u64),
    /// A name: defined in this unit or, if not, in another one.
    Simbolo(String),
}

/// Compiles ONE unit to an object instead of a program.
pub fn compile_to_object(program: &Program) -> Result<Vec<u8>> {
    let mut cg = Codegen::new(TargetProfile::Ring3App);
    cg.objeto = true;
    cg.enlace = program.enlace.clone();
    cg.emit_program(program)?;
    let bytes = cg.build_object();
    // The codegen must not hand out what the contract refuses: it is checked
    // here, once, with the reason -- not discovered by the linker later.
    if let Err(falta) = bmo_abi::bef::objeto::read(&bytes) {
        return Err(CError::new(0, format!(
            "el objeto emitido no cumple el contrato (bef::objeto): {falta:?} -- esto es un bug del compilador"
        )));
    }
    Ok(bytes)
}

impl Codegen {
    /// Is `name` a function this unit only knows by its prototype?
    pub(super) fn solo_prototipo(&self, name: &str) -> bool {
        !self.known_functions.contains(name) && self.enlace.prototipos.iter().any(|(n, ..)| n == name)
    }

    pub(super) fn build_object(&mut self) -> Vec<u8> {
        let all = core::mem::take(&mut self.code);
        let code = all[..self.instruction_end].to_vec();
        let rodata = all[self.instruction_end..self.string_data_end].to_vec();
        let data = all[self.string_data_end..].to_vec();
        let data_len = data.len() as u64;

        let mut b = BefBuilder::new();
        let mut flags = BefFlags::OBJECT.bits();
        if self.quiere_pantalla && !self.sabe_componerse {
            flags |= BefFlags::WANTS_SCREEN.bits();
        }
        b.header.flags = flags;
        b.entry_offset = 0;

        // -- Sections, and the table index each one gets. --
        let mut indice: Vec<(SectionKind, u8, u64)> = Vec::new();
        b.add_section(BefSection::code(code.clone()));
        indice.push((SectionKind::Code, 0, code.len() as u64));
        if !rodata.is_empty() {
            indice.push((SectionKind::RoData, indice.len() as u8, rodata.len() as u64));
            b.add_section(BefSection::rodata(rodata));
        }
        if !data.is_empty() {
            indice.push((SectionKind::Data, indice.len() as u8, data_len));
            b.add_section(BefSection::data(data));
        }
        if self.bss_len > 0 {
            indice.push((SectionKind::Bss, indice.len() as u8, self.bss_len as u64));
            b.add_section(BefSection::bss(self.bss_len as u64));
        }

        let mut entradas: Vec<Symbol> = Vec::new();
        let mut cadenas: Vec<u8> = Vec::new();
        let mut por_nombre: HashMap<String, u32> = HashMap::new();
        let mut por_seccion: HashMap<u8, u32> = HashMap::new();
        let mut push = |entradas: &mut Vec<Symbol>, nombre: &str, kind: SymbolKind, local: bool, sec: u8, off: u64, size: u64| -> u32 {
            let name_off = cadenas.len() as u32;
            cadenas.extend_from_slice(nombre.as_bytes());
            cadenas.push(0);
            entradas.push(Symbol {
                name_off,
                name_hash: name_hash(nombre),
                virt_addr: off,
                size,
                kind: kind as u8,
                binding: if local { SymbolBinding::Local } else { SymbolBinding::Global } as u8,
                visibility: SymbolVisibility::Default as u8,
                section_idx: sec,
                _reserved: 0,
            });
            (entradas.len() - 1) as u32
        };

        // One Local symbol per section: what a reference "inside this unit"
        // is relative to.
        for (kind, idx, len) in &indice {
            let n = match kind {
                SectionKind::Code => ".code",
                SectionKind::RoData => ".rodata",
                SectionKind::Data => ".data",
                _ => ".bss",
            };
            let i = push(&mut entradas, n, SymbolKind::Section, true, *idx, 0, *len);
            por_seccion.insert(*kind as u8, i);
        }
        let idx_de = |kind: SectionKind| indice.iter().find(|(k, ..)| *k == kind).map(|e| e.1);

        // Functions, in code order. A function the program did not write --a
        // synthesized one, like `__bmo_syscall_stub`-- is this unit's private
        // copy, and so is a `static` one.
        let escritas = self.known_functions.clone();
        let mut funcs: Vec<(usize, String)> = self.function_offsets.iter().map(|(n, o)| (*o, n.clone())).collect();
        funcs.sort();
        for (i, (off, n)) in funcs.iter().enumerate() {
            let fin = funcs.get(i + 1).map(|e| e.0).unwrap_or(self.instruction_end);
            let local = self.enlace.estaticos.contains(n) || !escritas.contains(n);
            let s = push(&mut entradas, n, SymbolKind::Function, local, 0, *off as u64, (fin - off) as u64);
            por_nombre.insert(n.clone(), s);
        }

        // Globals, in memory order. `func.name` statics and the compiler's own
        // `__bmo_*` buffers are private; an extern-only one is not defined here
        // (its bytes are dead padding: every emission path needs the name to
        // exist, and 8 bytes is cheaper than teaching all of them).
        let mut globs: Vec<(u32, String)> = self
            .global_offsets
            .iter()
            .filter(|(n, _)| !self.enlace.solo_externos.contains(*n))
            .map(|(n, (o, _))| (*o, n.clone()))
            .collect();
        globs.sort();
        let fin_total = data_len + self.bss_len as u64;
        for (i, (off, n)) in globs.iter().enumerate() {
            let off = *off as u64;
            let fin = globs.get(i + 1).map(|e| e.0 as u64).unwrap_or(fin_total);
            let (sec, rel, lim) = if off < data_len {
                (idx_de(SectionKind::Data), off, data_len)
            } else {
                (idx_de(SectionKind::Bss), off - data_len, fin_total)
            };
            let Some(sec) = sec else { continue };
            let local = self.enlace.estaticos.contains(n) || n.contains('.') || n.starts_with("__bmo");
            let s = push(&mut entradas, n, SymbolKind::Object, local, sec, rel, fin.min(lim) - off);
            por_nombre.insert(n.clone(), s);
        }

        // Undefined names, sorted so the object is the same bytes every time.
        let mut faltan: Vec<String> = self
            .obj_rel32
            .iter()
            .map(|(_, d)| d)
            .chain(self.obj_abs64.iter().map(|(_, d, _)| d))
            .filter_map(|d| match d {
                Destino::Simbolo(n) if !por_nombre.contains_key(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        faltan.sort();
        faltan.dedup();
        for n in faltan {
            let kind = if self.enlace.solo_externos.contains(&n) { SymbolKind::Object } else { SymbolKind::Function };
            let s = push(&mut entradas, &n, kind, false, SECTION_UNDEFINED, 0, 0);
            por_nombre.insert(n, s);
        }

        // -- Relocations. --
        let mut relocs = core::mem::take(&mut self.relocs);
        let resolver = |d: &Destino| -> (u32, i64) {
            match d {
                Destino::Seccion(k, off) => (por_seccion[&(*k as u8)], *off as i64),
                Destino::Simbolo(n) => (por_nombre[n], 0),
            }
        };
        for (at, d) in &self.obj_rel32 {
            let (sym, base) = resolver(d);
            relocs.push(Relocation {
                offset: *at as u64,
                symbol_idx: sym,
                kind: RelocationKind::Rel32 as u8,
                target_section: REL_CODE,
                _pad: [0; 2],
                addend: base - 4,
            });
        }
        for (at, d, suma) in &self.obj_abs64 {
            let (sym, base) = resolver(d);
            relocs.push(Relocation {
                offset: *at as u64,
                symbol_idx: sym,
                kind: RelocationKind::Abs64 as u8,
                target_section: REL_DATA,
                _pad: [0; 2],
                addend: base + suma,
            });
        }

        b.add_section(BefSection::symbols(entradas, cadenas));
        if !relocs.is_empty() {
            b.add_section(BefSection::relocs(relocs));
        }
        b.build().unwrap_or_default()
    }
}

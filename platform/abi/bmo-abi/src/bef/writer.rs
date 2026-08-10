use crate::bmo_abi::bef::{
    exports::ExportEntry,
    header::*,
    imports::ImportEntry,
    relocations::Relocation,
    sections::*,
    signing::{blake3_256, SectionHash, SignatureHeader},
    symbols::Symbol,
    tls::TlsTemplate,
};
use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u64};
use alloc::vec;
use alloc::vec::Vec;

/// * A cuanto se alinea el OFFSET EN FICHERO de una seccion. **Ocho bytes.**
///
/// # Por que no es el `alignment` de la seccion
///
/// Antes se usaba `section.alignment`, que vale **4096** en los tres frontends
/// porque es lo que la seccion necesita EN MEMORIA: el cargador coloca cada una
/// en su propia pagina. Usar el mismo numero para el fichero metia un agujero de
/// hasta 4095 bytes antes de cada seccion, y en `holac.bex` eso era
///
/// ```text
/// 3 952 bytes de hueco antes de `code` + 2 642 antes de `rodata`
///   = 6 594 de 8 432 bytes de fichero, o sea el 78% aire
/// ```
///
/// **Son dos requisitos distintos que compartian un campo.** La alineacion en
/// memoria la sigue declarando `entry.alignment` y la sigue honrando el
/// cargador; esta solo tiene que dejar los datos donde se puedan leer.
///
/// # Por que ocho basta
///
/// El cargador **COPIA**: `ring0/task/proc.rs` hace un `copy_nonoverlapping`
/// desde `file_offset`, al que le da igual donde empiece. Y `bex::inspect` solo
/// exige que `alignment` sea potencia de dos y que `file_offset + file_size`
/// quepa en el archivo -- comprobado, no supuesto. Los ocho bytes son para que
/// las secciones que se leen como structs (`Symbols`, `Relocations`, `Imports`,
/// de 24 bytes cada entrada) queden alineadas a `u64`.
///
/// # [!] Cuando dejaria de bastar
///
/// Si algun dia BMO quisiera **mapear el fichero directamente** a las paginas
/// del proceso en vez de copiarlo --demand paging--, entonces `file_offset`
/// tendria que ser **congruente** con la direccion virtual modulo el tamano de
/// pagina, que es la regla `p_offset == p_vaddr (mod pagesize)` de ELF. No es
/// "alineado a pagina": es congruente, y son cosas distintas. Mientras el
/// cargador copie, esto no hace falta.
const ALINEACION_EN_FICHERO: u64 = 8;

/// Redondea `n` hacia arriba al multiplo de `a`, que ha de ser potencia de dos.
fn alinea(n: u64, a: u64) -> u64 {
    (n + a - 1) & !(a - 1)
}

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
        Self {
            kind,
            flags: SectionFlags::READ,
            data,
            mem_size,
            alignment: 8,
            hash_index: 0xFFFF,
        }
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
        Self {
            header: BefHeader::new_executable(),
            sections: Vec::new(),
            entry_offset: 0,
        }
    }

    pub fn add_section(&mut self, section: BefSection) {
        self.sections.push(section);
    }

    /// Escribe el `.bex` entero, **con su seccion `Signature`**.
    ///
    /// == ** LOS HASHES YA SE CALCULABAN. NADIE PODIA ENCONTRARLOS ==
    ///
    /// Esta funcion lleva desde siempre computando el BLAKE3 de cada seccion y
    /// escribiendo el bloque al final del fichero... **sin declararlo en la
    /// tabla de secciones**. O sea: cada `.bex` del sistema viajaba con la
    /// prueba de su propia integridad pegada detras, y como no habia entrada que
    /// la nombrara, para cualquier lector eran bytes de relleno. Escritos, y
    /// perfectamente invisibles.
    ///
    /// Ahora la seccion se declara. El cargador la encuentra por su tipo
    /// (`SectionKind::Signature`) igual que encuentra el codigo.
    ///
    /// == Lo que esto compra, y es lo que se buscaba ==
    ///
    /// Un sector que se lee "bien" y trae datos equivocados **no lo caza ningun
    /// contador de bytes**: el fichero mide lo que debe y esta corrupto por
    /// dentro. Solo lo ve el contenido. Con la seccion declarada, el cargador
    /// compara y rechaza con un motivo en vez de admitir una imagen con un
    /// agujero y morir doscientas instrucciones despues en otro sitio.
    ///
    /// ** Y de regalo, el que no era obvio: **un binario en FAT32 puede traer
    /// firma**. Hasta hoy la firma vivia como atributo `:firma` de ESTRATOS, asi
    /// que un `.bex` en FAT32 no PODIA traerla -- la asimetria era del formato,
    /// no del gate. Con el hash dentro del propio fichero, la prueba viaja con
    /// el a donde vaya.
    ///
    /// == La seccion NO se hashea a si misma ==
    ///
    /// Obvio dicho asi y facil de olvidar escribiendo: su contenido son los
    /// hashes de las demas, y no puede contener el suyo propio. Se excluye, y el
    /// lector tiene que saberlo -- por eso esta escrito aqui y en `bex.rs`.
    ///
    /// == Y va la ULTIMA, siempre ==
    ///
    /// `SectionHash` guarda el **indice** de la seccion que describe. Insertarla
    /// en medio correria los indices de todo lo que venga detras y cada hash
    /// pasaria a describir a su vecina. Anadirla al final no mueve a nadie.
    pub fn build(&mut self) -> Result<Vec<u8>, &'static str> {
        // La seccion de firma se anade sola si el llamante no puso una. Es la
        // unica que este escritor fabrica por su cuenta, y lo hace porque el
        // dato --el hash de lo que acaba de escribir-- solo lo tiene el.
        if !self
            .sections
            .iter()
            .any(|s| s.kind == SectionKind::Signature)
        {
            let cuantas = self.sections.len();
            // Tamano exacto y conocido de antemano: cabecera + una entrada por
            // cada seccion QUE NO ES ESTA. Saberlo antes de calcular nada es lo
            // que rompe la pescadilla -- su offset se puede reservar en la misma
            // pasada que los demas.
            let bytes = core::mem::size_of::<SignatureHeader>() + cuantas * SectionHash::SIZE;
            let mut sec = BefSection::new(SectionKind::Signature, vec![0u8; bytes]);
            sec.alignment = 8;
            self.sections.push(sec);
        }

        let count = self.sections.len() as bx_u32;
        if count == 0 {
            return Err("no sections");
        }
        if count > 255 {
            return Err("too many sections");
        }

        self.header.section_count = count;
        self.header.entry_offset = self.entry_offset;
        self.header.section_table_offset = BefHeader::SIZE as u64;

        let header_size = BefHeader::SIZE as u64;
        let table_size = (count as u64) * (SectionEntry::SIZE as u64);
        let table_offset = header_size;
        let sig_idx = self
            .sections
            .iter()
            .position(|s| s.kind == SectionKind::Signature);

        let mut entries = Vec::with_capacity(count as usize);
        let mut file_off = header_size + table_size;

        for (i, section) in self.sections.iter().enumerate() {
            let mut entry = SectionEntry::ZERO;
            entry.kind = section.kind as u8;
            entry.flags = section.flags.bits();
            entry.mem_size = section.mem_size;
            entry.alignment = section.alignment;
            entry.hash_index = section.hash_index;

            // * LA FIRMA TAMBIEN RESERVA SU SITIO. Antes se la saltaba aqui y
            // sus bytes se pegaban al final del fichero sin entrada que los
            // nombrara -- escritos e invisibles. Reservar su hueco en la misma
            // pasada se puede porque su tamano es conocido de antemano.
            if section.kind != SectionKind::Bss && !section.data.is_empty() {
                file_off = alinea(file_off, ALINEACION_EN_FICHERO);
                entry.file_offset = file_off;
                entry.file_size = section.data.len() as u64;
                file_off += entry.file_size;
            }
            entries.push(entry);
        }

        let total_size = file_off as usize;
        let mut buf = vec![0u8; total_size];

        let hdr = bytes_from_struct(&self.header);
        buf[..header_size as usize].copy_from_slice(hdr);

        let tbl = bytes_from_slice(&entries);
        buf[table_offset as usize..table_offset as usize + tbl.len()].copy_from_slice(tbl);

        // * SE ESCRIBE DONDE LA TABLA DICE, y no se recalcula.
        //
        // Aqui habia una segunda copia de la formula de alineacion, identica a
        // la de arriba. Dos cuentas separadas que TIENEN que dar lo mismo son un
        // bug esperando a que alguien toque una sola: la tabla declararia un
        // offset y los bytes estarian en otro, y el fallo apareceria al CARGAR
        // --no al compilar--, con el kernel leyendo relleno como si fuera codigo.
        //
        // Ahora el destino sale de `entries[i].file_offset`, que es exactamente
        // lo que el fichero declara. Imposible que divirjan.
        for (i, section) in self.sections.iter().enumerate() {
            if section.kind == SectionKind::Bss || section.data.is_empty() {
                continue;
            }
            if sig_idx == Some(i) {
                continue;
            }
            let write_off = entries[i].file_offset;
            let end = write_off + section.data.len() as u64;
            if end as usize > buf.len() {
                buf.resize(end as usize, 0);
            }
            buf[write_off as usize..end as usize].copy_from_slice(&section.data);
        }

        // ** EL HASH DE CADA SECCION, MENOS EL DE LA PROPIA FIRMA.
        //
        // No puede contener el suyo: su contenido son los hashes de las demas.
        // Es obvio dicho asi y facil de olvidar escribiendo -- y el sintoma
        // seria un fichero que nunca verifica, con el hash de un bloque de
        // ceros guardado dentro del bloque que deja de ser ceros al guardarlo.
        let mut section_hashes = Vec::new();
        for (i, entry) in entries.iter().enumerate() {
            if sig_idx == Some(i) {
                continue;
            }
            let start = entry.file_offset as usize;
            let end = start + entry.file_size as usize;
            let section_bytes = if entry.file_size > 0 && end <= buf.len() {
                &buf[start..end]
            } else {
                // Una `Bss` no tiene bytes, y su hash es el del vacio. Se apunta
                // igual en vez de saltarla: asi el indice de cada entrada es el
                // indice REAL de su seccion y el lector no tiene que reconstruir
                // ninguna correspondencia.
                &[]
            };
            let digest = blake3_256(section_bytes);
            section_hashes.push(SectionHash {
                section_index: i as u16,
                _pad: [0; 6],
                digest,
            });
        }

        let sig_header = SignatureHeader {
            hash_count: section_hashes.len() as u32,
            // `0` = solo hashes, sin firma Ed25519 encima. Eso comprueba
            // INTEGRIDAD --que llego lo que se escribio-- y no AUTORIA. Son dos
            // preguntas distintas y esta contesta la primera; decir cual con un
            // numero evita que alguien lea la segunda donde no esta.
            sig_algo: 0,
        };
        let mut sig_data = Vec::from(bytes_from_struct(&sig_header));
        sig_data.extend_from_slice(bytes_from_slice(&section_hashes));

        // Se escribe DONDE LA TABLA DICE, igual que todo lo demas. El hueco se
        // reservo arriba con este tamano exacto.
        if let Some(i) = sig_idx {
            let off = entries[i].file_offset as usize;
            let fin = off + entries[i].file_size as usize;
            if sig_data.len() != entries[i].file_size as usize {
                return Err("el hueco de la firma no mide lo que la firma");
            }
            if fin > buf.len() {
                buf.resize(fin, 0);
            }
            buf[off..fin].copy_from_slice(&sig_data);
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
    unsafe {
        core::slice::from_raw_parts(
            slice.as_ptr() as *const u8,
            slice.len() * core::mem::size_of::<T>(),
        )
    }
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
        b.add_section(BefSection::manifest_toml(
            b"[identity]\nname = \"test\"\n".to_vec(),
        ));
        let bytes = b.build().unwrap();
        assert!(bytes.len() > 48);
    }
}

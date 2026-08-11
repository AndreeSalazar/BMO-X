use crate::bmo_abi::bef::{
    exports::ExportEntry,
    header::*,
    imports::ImportEntry,
    relocations::Relocation,
    requisitos,
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

/// * A cuanto se alinea el offset de una seccion **QUE SE CARGA**. Un SECTOR.
///
/// # El caso que esto hace posible (2026-08-10)
///
/// La nota de arriba decia que ocho bytes bastan *"mientras el cargador copie"*,
/// y avisaba de que mapear el fichero directo pediria congruencia. La pieza B de
/// `docs/EL_CONTRATO_DE_CARGA.md` no es ninguna de las dos cosas y cae justo en
/// medio: el cargador **sigue copiando**, pero la copia la hace **el disco**,
/// escribiendo cada seccion en los marcos del proceso sin pasar por un bufer.
///
/// El marco `p` de una seccion recibe los bytes `[file_offset + p*4096, +4096)`.
/// Para que eso sea una lectura de disco --sectores enteros, sin cabeza ni cola
/// sueltas-- basta con que `file_offset` sea multiplo de 512: como 4096 lo es,
/// todos los marcos que vengan detras caen alineados solos.
///
/// **No hace falta congruencia modulo pagina.** Alinear a 4096 costaria hasta
/// 4095 bytes por seccion --el 78% de aire que la nota de arriba cuenta que se
/// quito-- y no compra nada mas que esto.
///
/// # Lo que cuesta
///
/// Hasta 511 bytes por seccion cargable. En `gui.bex`, con tres, son 1,5 KB de
/// 308 KB: **el 0,5%**. A cambio, el camino rapido del disco existe siempre en
/// vez de cuando el fichero salga alineado por suerte.
const ALINEACION_DE_LO_CARGABLE: u64 = 512;

/// Se carga esta seccion en la memoria del proceso? Es la misma regla que
/// `bex::is_loadable` en el kernel, y por eso son cuatro tipos y no cinco.
fn se_carga(k: SectionKind) -> bool {
    matches!(
        k,
        SectionKind::Code | SectionKind::RoData | SectionKind::Data | SectionKind::Bss
    )
}

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

/// Un requisito que declara el PROGRAMA, no el escritor.
///
/// Es [`requisitos::Declaracion`] con el motivo en propiedad: el builder se
/// construye por partes y el motivo tiene que sobrevivir hasta `build()`.
pub struct RequisitoDeclarado {
    pub clase: bx_u16,
    pub unidad: bx_u16,
    pub obligatorio: bool,
    pub cantidad: bx_u64,
    pub motivo: alloc::string::String,
}

pub struct BefBuilder {
    pub header: BefHeader,
    pub sections: Vec<BefSection>,
    pub entry_offset: bx_u64,
    /// Lo que el programa pide y el escritor no puede deducir. Ver
    /// [`BefBuilder::requerir`].
    pub requisitos: Vec<RequisitoDeclarado>,
}

impl BefBuilder {
    pub fn new() -> Self {
        Self {
            header: BefHeader::new_executable(),
            sections: Vec::new(),
            entry_offset: 0,
            requisitos: Vec::new(),
        }
    }

    pub fn add_section(&mut self, section: BefSection) {
        self.sections.push(section);
    }

    /// **Declara un requisito que el escritor NO puede deducir.**
    ///
    /// La memoria de la imagen se calcula sola en [`build`](Self::build) -- el
    /// escritor acaba de escribir las secciones y sabe lo que ocupan. Lo que no
    /// puede saber es que el programa quiera la pantalla en exclusiva, o que
    /// necesite tener seis megas de recursos residentes mientras corre. Eso solo
    /// lo sabe quien lo escribio, y esto es por donde lo dice.
    ///
    /// El `motivo` no es documentacion: es **lo que sale por el "no"** cuando el
    /// sistema no puede conceder. Por eso [`requisitos::construir`] rechaza un
    /// requisito obligatorio sin motivo -- seria volver al rechazo que no se
    /// puede contestar.
    pub fn requerir(&mut self, r: RequisitoDeclarado) {
        self.requisitos.push(r);
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
        // ** LOS REQUISITOS, POR EL MISMO MOTIVO QUE LA FIRMA (2026-08-10).
        //
        // La firma se fabrica aqui porque *"el dato solo lo tiene el"*. La
        // memoria que va a ocupar la imagen es exactamente igual de deducible:
        // el escritor acaba de colocar las secciones y sabe lo que suman.
        //
        // Hasta hoy quien lo deducia era **el kernel** --`bex::necesita`
        // recorriendo la tabla en Ring 0-- y esa deduccion se equivoco dos
        // veces, con el mismo sintoma las dos: `MAX_BEX` subido de 1 a 4 MiB
        // porque un programa media mas de lo que alguien habia supuesto de
        // antemano. Un supuesto que hay que subir es un supuesto que no deberia
        // existir: el fichero lo sabe y ahora lo dice.
        //
        // Lo que el escritor NO puede deducir --pantalla, audio, recursos que
        // el programa quiera residentes-- entra por `requerir()`. Ver
        // `docs/EL_CONTRATO_DE_CARGA.md`, pieza C.
        if !self
            .sections
            .iter()
            .any(|s| s.kind == SectionKind::Requisitos)
        {
            // Lo que se mapea EN EL PROCESO: codigo, datos, y los ceros
            // declarados. No es el tamano del fichero y no se parece: un paquete
            // con un WAD dentro mide seis megas y ocupa ochocientos kilos.
            let memoria: u64 = self
                .sections
                .iter()
                .filter(|s| {
                    matches!(
                        s.kind,
                        SectionKind::Code
                            | SectionKind::RoData
                            | SectionKind::Data
                            | SectionKind::Bss
                    )
                })
                .fold(0u64, |a, s| a.saturating_add(s.mem_size));

            let mut decls = vec![requisitos::Declaracion {
                clase: requisitos::CLASE_MEMORIA,
                unidad: requisitos::UNIDAD_BYTES,
                obligatorio: true,
                cantidad: memoria,
                motivo: "codigo, datos y ceros declarados de la imagen",
            }];
            for r in self.requisitos.iter() {
                decls.push(requisitos::Declaracion {
                    clase: r.clase,
                    unidad: r.unidad,
                    obligatorio: r.obligatorio,
                    cantidad: r.cantidad,
                    motivo: &r.motivo,
                });
            }
            let datos = requisitos::construir(&decls)?;
            let mut sec = BefSection::new(SectionKind::Requisitos, datos);
            sec.alignment = 8;
            self.sections.push(sec);
        }

        // La seccion de firma se anade sola si el llamante no puso una. Es la
        // unica que este escritor fabrica por su cuenta, y lo hace porque el
        // dato --el hash de lo que acaba de escribir-- solo lo tiene el.
        //
        // [!] Y va DESPUES de los requisitos a proposito: reserva una entrada
        // por seccion existente, asi que si los requisitos entraran detras se
        // quedarian sin hash -- la unica seccion del fichero que dice lo que hay
        // que concederle a un programa, y sin nada con que comprobar que llego
        // entera.
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

        // Primero la tabla con lo que NO depende de la disposicion; los offsets
        // se reparten despues, y en otro orden. Ver el bloque de abajo.
        for section in self.sections.iter() {
            let mut entry = SectionEntry::ZERO;
            entry.kind = section.kind as u8;
            entry.flags = section.flags.bits();
            entry.mem_size = section.mem_size;
            entry.alignment = section.alignment;
            entry.hash_index = section.hash_index;
            entries.push(entry);
        }

        // ** LO QUE EL CARGADOR USA VA DELANTE. TODO. (2026-08-10)
        //
        // === El numero que lo obliga ===
        //
        // El cargador del kernel dejo de leer el fichero entero: pregunta a la
        // tabla que necesita --codigo, datos, relocations y hashes-- y trae solo
        // hasta donde acaba lo ultimo de eso. Con la disposicion de antes, esa
        // cuenta daba **el fichero entero y el ahorro era CERO**, porque la
        // firma se colocaba al final, DETRAS de los recursos:
        //
        // ```text
        //   [cab][tabla][Code][RoData][Data][Relocs][Resources][Signature]
        //                                              ^ el WAD    ^ y esto detras
        //   necesita -> hasta aqui ------------------------------------->
        // ```
        //
        // Basta con que UNA seccion que el cargador mira quede detras del bulto
        // para que el bulto haya que traerlo igual. Ordenando:
        //
        // ```text
        //   [cab][tabla][Code][RoData][Data][Relocs][Signature][Resources]
        //   necesita -> hasta aqui ------------------------->
        // ```
        //
        // === Por que se puede, y por que no rompe nada ===
        //
        // El ORDEN DE LA TABLA no cambia: los indices siguen siendo los mismos,
        // y son ellos los que usan las relocations (`SeccionAbs64` guarda el
        // indice de la seccion destino) y los hashes (`section_index`). Lo unico
        // que se reordena es **donde caen los bytes dentro del fichero**, y eso
        // ya nadie lo supone: el cargador lee `file_offset` de la tabla, nunca
        // una constante. Es la misma propiedad que hace posible empaquetar.
        //
        // La firma sigue calculandose la ULTIMA --su contenido son los hashes de
        // las demas-- pero su HUECO se reserva aqui, que es lo que permitio
        // declararla en la tabla el 2026-08-09. Reservar y rellenar son dos
        // momentos distintos, y esa separacion es justo la que se cobra ahora.
        fn el_cargador_la_usa(k: SectionKind) -> bool {
            matches!(
                k,
                SectionKind::Code
                    | SectionKind::RoData
                    | SectionKind::Data
                    | SectionKind::Bss
                    | SectionKind::Relocs
                    | SectionKind::Signature
                    // Los requisitos los lee el cargador ANTES que nada: son lo
                    // que decide si hay que leer el resto. Detras del bulto
                    // obligarian a traerse el bulto para poder preguntar.
                    | SectionKind::Requisitos
            )
        }
        for delante in [true, false] {
            for (i, section) in self.sections.iter().enumerate() {
                if el_cargador_la_usa(section.kind) != delante {
                    continue;
                }
                // La `Bss` no ocupa fichero: los ceros se declaran y no viajan.
                if section.kind == SectionKind::Bss || section.data.is_empty() {
                    continue;
                }
                // Lo que se carga, a sector; lo demas, a ocho. Ver
                // `ALINEACION_DE_LO_CARGABLE`: es lo que permite que el disco
                // escriba una seccion directamente en los marcos del proceso.
                file_off = alinea(
                    file_off,
                    if se_carga(section.kind) {
                        ALINEACION_DE_LO_CARGABLE
                    } else {
                        ALINEACION_EN_FICHERO
                    },
                );
                entries[i].file_offset = file_off;
                entries[i].file_size = section.data.len() as u64;
                file_off += entries[i].file_size;
            }
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

    /// Saca los bytes de una seccion por su tipo, leyendo la tabla como la lee
    /// el cargador. Sin esto las pruebas de abajo mirarian offsets a mano.
    fn seccion_de(bytes: &[u8], kind: SectionKind) -> Option<&[u8]> {
        let count = u32::from_le_bytes(bytes[40..44].try_into().ok()?) as usize;
        let tabla = u64::from_le_bytes(bytes[32..40].try_into().ok()?) as usize;
        for i in 0..count {
            let e = tabla + i * SectionEntry::SIZE;
            if bytes[e] != kind as u8 {
                continue;
            }
            let off = u64::from_le_bytes(bytes[e + 8..e + 16].try_into().ok()?) as usize;
            let len = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().ok()?) as usize;
            return bytes.get(off..off + len);
        }
        None
    }

    /// ** LO QUE SE CARGA EMPIEZA EN FRONTERA DE SECTOR.
    ///
    /// Es la condicion que hace que el disco pueda escribir una seccion
    /// **directamente en los marcos del proceso**: el marco `p` recibe
    /// `[file_offset + p*4096, +4096)`, y eso solo son sectores enteros si
    /// `file_offset` es multiplo de 512.
    ///
    /// Se prueba con secciones de tamano DELIBERADAMENTE feo --17, 4095-- para
    /// que la de detras solo pueda quedar alineada si alguien la alinea. Con
    /// tamanos redondos la prueba pasaria sin que el codigo hiciera nada.
    #[test]
    fn lo_cargable_empieza_en_frontera_de_sector() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 17]));
        b.add_section(BefSection::rodata(vec![0x11; 4095]));
        b.add_section(BefSection::data(vec![0x22; 3]));
        let bytes = b.build().unwrap();

        let count = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        let tabla = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
        let mut vistas = 0;
        for i in 0..count {
            let e = tabla + i * SectionEntry::SIZE;
            let kind = SectionKind::from_u8(bytes[e]).unwrap();
            let off = u64::from_le_bytes(bytes[e + 8..e + 16].try_into().unwrap());
            let len = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().unwrap());
            if len == 0 || !se_carga(kind) {
                continue;
            }
            assert_eq!(off % 512, 0, "la seccion {kind:?} empieza a mitad de sector");
            vistas += 1;
        }
        assert_eq!(vistas, 3, "la prueba no llego a mirar las tres");
    }

    /// Y lo que NO se carga sigue a ocho bytes: alinear a sector una tabla de
    /// simbolos o unos recursos seria pagar hasta 511 bytes por seccion a cambio
    /// de nada -- el cargador no los pone en marcos.
    #[test]
    fn lo_que_no_se_carga_no_paga_la_alineacion() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 17]));
        let mut rec = BefSection::new(SectionKind::Resources, vec![0x22; 10]);
        rec.alignment = 8;
        b.add_section(rec);
        let bytes = b.build().unwrap();

        let sec = seccion_de(&bytes, SectionKind::Resources).unwrap();
        assert_eq!(sec.len(), 10);
        // Si los recursos se alinearan a sector, el fichero creceria hasta el
        // siguiente multiplo de 512 sin que nadie lo use.
        assert!(bytes.len() < 2048, "el fichero engordo por alinear de mas: {}", bytes.len());
    }

    /// ** TODO `.bex` SALE DECLARADO, sin que nadie lo pida.
    ///
    /// Es la pieza C entera en una prueba: el escritor sabe lo que ocupan las
    /// secciones que acaba de colocar, asi que lo dice. Si esto se rompe, el
    /// kernel vuelve a tener que deducirlo -- y deducirlo es lo que costo dos
    /// subidas de `MAX_BEX`.
    #[test]
    fn todo_bex_declara_su_memoria_sin_pedirlo() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 4096]));
        b.add_section(BefSection::rodata(vec![0x11; 1024]));
        b.add_section(BefSection::bss(8192));
        let bytes = b.build().unwrap();

        let sec = seccion_de(&bytes, SectionKind::Requisitos).expect("tiene que estar");
        let t = requisitos::Tabla::abrir(sec).expect("y tiene que abrirse");
        assert_eq!(
            t.total_de(requisitos::CLASE_MEMORIA),
            4096 + 1024 + 8192,
            "la memoria declarada no es la que se escribio"
        );
        let r = t.requisito(0).unwrap();
        assert!(r.es_obligatorio(), "sin memoria no arranca nadie");
        assert!(!t.motivo(&r).is_empty(), "un obligatorio sin motivo no se puede contestar");
    }

    /// La `Bss` cuenta y NO viaja. Es el escalon 0 de `LA_RAM.md` visto desde el
    /// otro lado: los ceros no ocupan fichero **pero si ocupan RAM**, asi que un
    /// requisito que los ignorara pediria de menos y el programa se quedaria sin
    /// sitio justo donde el fichero no daba ninguna pista.
    #[test]
    fn los_ceros_declarados_cuentan_como_memoria() {
        let mut con = BefBuilder::new();
        con.add_section(BefSection::code(vec![0xC3; 64]));
        con.add_section(BefSection::bss(1_000_000));
        let a = con.build().unwrap();

        let mut sin = BefBuilder::new();
        sin.add_section(BefSection::code(vec![0xC3; 64]));
        let b2 = sin.build().unwrap();

        let ta = requisitos::Tabla::abrir(seccion_de(&a, SectionKind::Requisitos).unwrap()).unwrap();
        let tb = requisitos::Tabla::abrir(seccion_de(&b2, SectionKind::Requisitos).unwrap()).unwrap();
        assert_eq!(
            ta.total_de(requisitos::CLASE_MEMORIA) - tb.total_de(requisitos::CLASE_MEMORIA),
            1_000_000
        );
        assert!(a.len() < 1_000_000, "y el millon de ceros NO viajo en el fichero");
    }

    /// Lo que el escritor no puede deducir entra por `requerir`, y sale con su
    /// motivo dentro del fichero.
    #[test]
    fn lo_que_pide_el_programa_viaja_con_su_motivo() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 64]));
        b.requerir(RequisitoDeclarado {
            clase: requisitos::CLASE_PANTALLA,
            unidad: requisitos::UNIDAD_UNIDADES,
            obligatorio: true,
            cantidad: 1,
            motivo: "dibuja el marco entero, no una ventana".into(),
        });
        let bytes = b.build().unwrap();

        let sec = seccion_de(&bytes, SectionKind::Requisitos).unwrap();
        let t = requisitos::Tabla::abrir(sec).unwrap();
        let pantalla = t
            .iter()
            .find(|r| r.clase == requisitos::CLASE_PANTALLA)
            .expect("lo que se pidio tiene que estar");
        assert_eq!(t.motivo(&pantalla), "dibuja el marco entero, no una ventana");
    }

    /// ** Y LA SECCION DE REQUISITOS TIENE SU HASH.
    ///
    /// Es la que dice lo que hay que concederle a un programa. Si viajara sin
    /// hash, seria el unico renglon del fichero que se puede cambiar por el
    /// camino sin que nada lo note -- y cambiarlo es pedirle al sistema mas
    /// memoria, o la pantalla, en nombre de un binario que no lo pidio.
    #[test]
    fn los_requisitos_tambien_se_firman() {
        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 64]));
        let bytes = b.build().unwrap();

        let count = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        let tabla = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
        let idx_req = (0..count)
            .find(|i| bytes[tabla + i * SectionEntry::SIZE] == SectionKind::Requisitos as u8)
            .expect("esta en la tabla");

        let firma = seccion_de(&bytes, SectionKind::Signature).unwrap();
        let cuantos = u32::from_le_bytes(firma[0..4].try_into().unwrap()) as usize;
        let cubierta = (0..cuantos).any(|k| {
            let e = 8 + k * SectionHash::SIZE;
            u16::from_le_bytes(firma[e..e + 2].try_into().unwrap()) as usize == idx_req
        });
        assert!(cubierta, "los requisitos viajan sin hash");
    }

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

    /// ** LO QUE EL CARGADOR USA VA DELANTE DEL BULTO.
    ///
    /// Es la propiedad de la que depende el escalon 2 de `LA_RAM.md`: el kernel
    /// suma hasta donde acaba lo ultimo que mira --codigo, datos, relocations,
    /// hashes-- y trae solo eso. Si una sola de esas secciones quedara detras de
    /// los recursos, habria que traerse los recursos igual y el ahorro seria
    /// **cero**, que es exactamente lo que pasaba antes de ordenarlas.
    ///
    /// Se comprueba con un recurso GRANDE a proposito: con uno pequeno, la
    /// prueba pasaria por casualidad si los offsets se solaparan mal.
    #[test]
    fn lo_del_cargador_va_antes_que_los_recursos() {
        use super::super::sections::SectionKind;

        let mut b = BefBuilder::new();
        b.add_section(BefSection::code(vec![0xC3; 4096]));
        b.add_section(BefSection::data(vec![0x11; 1024]));
        let mut recursos = BefSection::new(SectionKind::Resources, vec![0x22; 1024 * 1024]);
        recursos.alignment = 8;
        b.add_section(recursos);
        let bytes = b.build().unwrap();

        // Se leen los offsets DE LA TABLA, que es lo que lee el cargador.
        let count = u32::from_le_bytes(bytes[40..44].try_into().unwrap()) as usize;
        let tabla = u64::from_le_bytes(bytes[32..40].try_into().unwrap()) as usize;
        let mut fin_del_cargador = 0u64;
        let mut inicio_recursos = u64::MAX;
        for i in 0..count {
            let e = tabla + i * SectionEntry::SIZE;
            let kind = bytes[e];
            let off = u64::from_le_bytes(bytes[e + 8..e + 16].try_into().unwrap());
            let len = u64::from_le_bytes(bytes[e + 16..e + 24].try_into().unwrap());
            if len == 0 {
                continue;
            }
            if kind == SectionKind::Resources as u8 {
                inicio_recursos = off;
            } else {
                fin_del_cargador = fin_del_cargador.max(off + len);
            }
        }
        assert!(inicio_recursos != u64::MAX, "no se escribio la seccion de recursos");
        assert!(
            fin_del_cargador <= inicio_recursos,
            "algo que el cargador usa cae DETRAS de los recursos: fin={fin_del_cargador} recursos={inicio_recursos}"
        );
        // Y el ahorro es real: lo que hay que traer es una fraccion del fichero.
        assert!(
            fin_del_cargador * 4 < bytes.len() as u64,
            "el prefijo cargable no es mucho menor que el fichero: {fin_del_cargador} de {}",
            bytes.len()
        );
    }
}

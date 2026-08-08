//! Ring 0 BEX admission gate.
//!
//! This module performs the allocation-free part of loading a BEX image:
//! validate the x86-64/ABI contract and create a fixed-size mapping plan.
//! It deliberately does not execute code.  The process subsystem will later
//! consume this plan to allocate user pages, copy sections and enter Ring 3.

// Keep this parser `no_std` and allocation-free.  `bmo-abi` is the canonical
// producer/validator, but its complete API intentionally uses `alloc`, which
// is not available until the kernel has initialized a process allocator.
// These offsets are the stable BEX v1 (= BEF1) wire contract.
const BEX_MAGIC: u32 = u32::from_le_bytes(*b"BEF1");
const BEX_HEADER_SIZE: usize = 48;
const BEX_SECTION_SIZE: usize = 48;
const BEX_VERSION_MAJOR: u16 = 1;
const BEX_ARCH_X86_64: u8 = 0x01;
const BEX_ENDIAN_LITTLE: u8 = 0x00;
const BEX_FLAG_EXECUTABLE: u32 = 1 << 0;
pub const SECTION_CODE: u8 = 0x01;
pub const SECTION_RODATA: u8 = 0x02;
pub const SECTION_DATA: u8 = 0x03;
pub const SECTION_BSS: u8 = 0x04;
/// Tabla de relocations. **NO se mapea** --no es memoria del programa-- pero si
/// se LEE: dice que punteros de `.data` hay que rellenar con direcciones que
/// solo se conocen aqui. Ver `BexLoadPlan::relocs_*`.
pub const SECTION_RELOCS: u8 = 0x07;

/// Esta seccion se MAPEA en el espacio del programa?
///
/// * LA REGLA: solo cuatro tipos son memoria del programa. Todo lo demas
/// --imports, exports, manifiesto, firma, simbolos, depuracion, recursos, y
/// **cualquier tipo que este kernel no conozca**-- es data para OTRO: para el
/// enlazador, para el verificador, para el runtime de un lenguaje que Ring 0
/// no tiene por que saber que existe.
///
/// Un tipo desconocido se SALTA, no se rechaza. Es lo que ha mantenido vivo a
/// ELF treinta anos: la seccion que no te incumbe no es un error, es data que
/// no vas a abrir. Asi un lenguaje nuevo puede meter sus metadatos en el
/// contenedor sin pedirle permiso al kernel ni anadirle un campo que entender.
///
/// (Antes se mapeaban TODAS: un manifiesto o una tabla de depuracion acababan
/// en el espacio de usuario como memoria escribible. Gasto y superficie de
/// ataque a cambio de nada.)
pub fn is_loadable(kind: u8) -> bool {
    matches!(kind, SECTION_CODE | SECTION_RODATA | SECTION_DATA | SECTION_BSS)
}
pub const SECTION_FLAG_EXEC: u32 = 1 << 2;
pub const SECTION_FLAG_WRITE: u32 = 1 << 1;

/// The first BEX process supports a compact, auditable section table.
pub const MAX_BEX_SECTIONS: usize = 16;

#[derive(Clone, Copy)]
pub struct BexMapping {
    pub kind: u8,
    pub flags: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub alignment: u16,
}

const EMPTY_MAPPING: BexMapping = BexMapping {
    kind: 0,
    flags: 0,
    file_offset: 0,
    file_size: 0,
    mem_size: 0,
    alignment: 0,
};

/// Validated inputs required by the future Ring 3 mapper.
pub struct BexLoadPlan {
    /// Offset within the executable Code section; it is not a Ring 0 address.
    pub entry_offset: u64,
    /// SOLO las secciones que se mapean (ver `is_loadable`).
    pub sections: [BexMapping; MAX_BEX_SECTIONS],
    pub section_count: usize,
    /// Cuantas secciones se saltaron por no ser memoria del programa
    /// (manifiesto, firma, depuracion... o un tipo que este kernel no conoce).
    /// Se cuenta para poder DECIRLO, no para decidir nada con ello.
    pub skipped_sections: usize,
    /// * Donde esta la tabla de relocations DENTRO DEL FICHERO, si la hay.
    ///
    /// No es un `BexMapping` porque no se mapea: el cargador la lee, aplica lo
    /// que dice sobre las secciones ya copiadas, y la olvida. Cero paginas en el
    /// proceso.
    ///
    /// `relocs_file_size == 0` significa "este programa no tiene punteros que
    /// rellenar", que es el caso de todos los `.bex` escritos hasta hoy.
    pub relocs_file_offset: u64,
    pub relocs_file_size: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BexError {
    TooSmall,
    InvalidHeader,
    UnsupportedArchitecture,
    /// La imagen viene en un orden de bytes que este kernel no lee.
    UnsupportedEndianness,
    /// La imagen declara usar una extension de CPU cuyo estado este kernel
    /// todavia no sabe preservar en un cambio de contexto.
    UnsupportedCpuFeature,
    AbiMismatch,
    NotExecutable,
    TooManySections,
    SectionTableOutOfBounds,
    InvalidSection,
    MissingCode,
    EntryOutsideCode,
}

/// Validate an untrusted BEX image and produce a fixed-size mapping plan.
///
/// No `alloc`, file access, relocation, page-table mutation or control transfer
/// happens here.  This boundary is therefore safe to call before a process is
/// admitted to the kernel.
pub fn inspect(bytes: &[u8]) -> Result<BexLoadPlan, BexError> {
    if bytes.len() < BEX_HEADER_SIZE {
        return Err(BexError::TooSmall);
    }
    let magic = read_u32(bytes, 0).ok_or(BexError::TooSmall)?;
    let version_major = read_u16(bytes, 4).ok_or(BexError::TooSmall)?;
    let flags = read_u32(bytes, 8).ok_or(BexError::TooSmall)?;
    let arch = *bytes.get(12).ok_or(BexError::TooSmall)?;
    let endianness = *bytes.get(13).ok_or(BexError::TooSmall)?;
    let cpu_features = read_u16(bytes, 14).ok_or(BexError::TooSmall)?;
    let abi_major = *bytes.get(16).ok_or(BexError::TooSmall)?;
    let abi_minor = *bytes.get(17).ok_or(BexError::TooSmall)?;
    let entry_offset = read_u64(bytes, 24).ok_or(BexError::TooSmall)?;
    let section_table_offset = read_u64(bytes, 32).ok_or(BexError::TooSmall)?;
    let section_count = read_u32(bytes, 40).ok_or(BexError::TooSmall)? as usize;
    if magic != BEX_MAGIC || version_major != BEX_VERSION_MAJOR || section_count == 0 {
        return Err(BexError::InvalidHeader);
    }
    if arch != BEX_ARCH_X86_64 {
        return Err(BexError::UnsupportedArchitecture);
    }
    // Orden de bytes: hoy este kernel solo lee little-endian. Comprobarlo
    // cuesta una comparacion y evita que el dia del PowerPC una imagen se
    // cargue del reves y falle de mil formas raras en vez de una clara.
    if endianness != BEX_ENDIAN_LITTLE {
        return Err(BexError::UnsupportedEndianness);
    }
    // * Extensiones de CPU DECLARADAS por la imagen.
    //
    // Un bit que no conozco = una parte del estado del procesador que no se
    // que existe y que por tanto NO voy a preservar en el cambio de contexto.
    // Y hoy `trap.rs` usa FXSAVE, que guarda x87 y SSE pero NO la mitad alta
    // de los YMM: un programa con AVX se corromperia en silencio a la primera
    // interrupcion del temporizador.
    //
    // Asi que se RECHAZA, y ese rechazo es la mejora de verdad: convierte una
    // corrupcion silenciosa en un "no" con nombre, HOY, antes de que exista
    // el XSAVE. Cuando el kernel sepa guardar el estado ancho, esta linea se
    // relaja -- no antes.
    if cpu_features != 0 {
        return Err(BexError::UnsupportedCpuFeature);
    }
    let supported_abi = (abi_major == 1 && abi_minor == 0)
        || (abi_major == 2 && abi_minor == 0);
    if !supported_abi {
        return Err(BexError::AbiMismatch);
    }
    if flags & BEX_FLAG_EXECUTABLE == 0 {
        return Err(BexError::NotExecutable);
    }

    let count = section_count;
    if count > MAX_BEX_SECTIONS {
        return Err(BexError::TooManySections);
    }
    let table_size = count.checked_mul(BEX_SECTION_SIZE).ok_or(BexError::SectionTableOutOfBounds)?;
    let table_start = section_table_offset as usize;
    let table_end = table_start.checked_add(table_size).ok_or(BexError::SectionTableOutOfBounds)?;
    if table_end > bytes.len() {
        return Err(BexError::SectionTableOutOfBounds);
    }

    let mut plan = BexLoadPlan {
        entry_offset,
        sections: [EMPTY_MAPPING; MAX_BEX_SECTIONS],
        section_count: 0,
        skipped_sections: 0,
        relocs_file_offset: 0,
        relocs_file_size: 0,
    };
    let mut code_size = None;
    let mut loadable = 0usize;
    let mut skipped = 0usize;

    for index in 0..count {
        let offset = table_start + index * BEX_SECTION_SIZE;
        let kind = *bytes.get(offset).ok_or(BexError::InvalidSection)?;
        let section_flags = read_u32(bytes, offset + 4).ok_or(BexError::InvalidSection)?;
        let file_offset = read_u64(bytes, offset + 8).ok_or(BexError::InvalidSection)?;
        let file_size = read_u64(bytes, offset + 16).ok_or(BexError::InvalidSection)?;
        let mem_size = read_u64(bytes, offset + 24).ok_or(BexError::InvalidSection)?;
        let alignment_raw = read_u16(bytes, offset + 40).ok_or(BexError::InvalidSection)?;
        if kind == 0 || file_size > mem_size || (kind != SECTION_BSS && file_size == 0) {
            return Err(BexError::InvalidSection);
        }
        if kind != SECTION_BSS {
            let end = file_offset.checked_add(file_size).ok_or(BexError::InvalidSection)?;
            if end as usize > bytes.len() {
                return Err(BexError::InvalidSection);
            }
        }
        let alignment = if alignment_raw == 0 { 8 } else { alignment_raw };
        if !alignment.is_power_of_two() {
            return Err(BexError::InvalidSection);
        }
        if kind == SECTION_CODE {
            if section_flags & SECTION_FLAG_EXEC == 0 {
                return Err(BexError::InvalidSection);
            }
            code_size = Some(mem_size);
        }
        // * LAS RELOCATIONS se apuntan pero NO se mapean.
        //
        // No son memoria del programa --el proceso nunca las ve-- pero el
        // cargador las necesita para rellenar los punteros de `.data` con
        // direcciones que solo se conocen al colocar las secciones. Sus limites
        // ya se validaron arriba contra el tamano del archivo, igual que las
        // demas.
        //
        // Va antes del filtro de `is_loadable` para que NO cuente como
        // "saltada": saltada significa "no la usa nadie", y esta si.
        if kind == SECTION_RELOCS {
            plan.relocs_file_offset = file_offset;
            plan.relocs_file_size = file_size;
            continue;
        }
        // * Solo lo CARGABLE entra al plan. Lo demas se valido (sus limites
        // tienen que caber en el archivo: una seccion mal formada sigue siendo
        // un rechazo) pero no se mapea. Ver `is_loadable`.
        if !is_loadable(kind) {
            skipped += 1;
            continue;
        }
        plan.sections[loadable] = BexMapping {
            kind,
            flags: section_flags,
            file_offset,
            file_size,
            mem_size,
            alignment,
        };
        loadable += 1;
    }
    // El plan solo describe lo que se mapea.
    plan.section_count = loadable;
    plan.skipped_sections = skipped;

    let code_size = code_size.ok_or(BexError::MissingCode)?;
    if entry_offset >= code_size {
        return Err(BexError::EntryOutsideCode);
    }
    Ok(plan)
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let end = offset.checked_add(2)?;
    Some(u16::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    Some(u32::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

fn read_u64(bytes: &[u8], offset: usize) -> Option<u64> {
    let end = offset.checked_add(8)?;
    Some(u64::from_le_bytes(bytes.get(offset..end)?.try_into().ok()?))
}

/// * UNA RELOCATION, leida del fichero. Ver `SECTION_RELOCS`.
///
/// Tamano y disposicion fijados por `bmo_abi::bef::relocations::Relocation`, que
/// este kernel **no importa a proposito**: `bmo-abi` es el CONTRATO y aqui se
/// implementa contra el leyendo bytes, igual que con la tabla de secciones. Si
/// el struct cambiara de forma, `RELOC_SIZE` y estos offsets son el unico sitio
/// a tocar.
pub const RELOC_SIZE: usize = 24;
/// `SeccionAbs64`: escribe la direccion de `(seccion, offset)`. El unico tipo
/// que este cargador aplica; cualquier otro se rechaza diciendolo, porque
/// aplicar una reloc que no se entiende es escribir un numero inventado en la
/// memoria de un proceso.
pub const RELOC_SECCION_ABS64: u8 = 0x04;

/// Lo que hace falta de una reloc, ya descodificado.
#[derive(Clone, Copy)]
pub struct BexReloc {
    /// En que seccion se escribe (`0` = code, `1` = data, `2` = rodata).
    ///
    /// [!] **Esta numeracion NO es la de `SECTION_*`** (donde code=1, rodata=2,
    /// data=3): es la del propio struct de relocations, donde data y rodata
    /// estan cambiados. Ver la nota en `bmo_abi::bef::relocations`.
    pub donde_sec: u8,
    /// Offset dentro de esa seccion.
    pub donde_off: u64,
    /// En que seccion vive el destino, misma numeracion que `donde_sec`.
    pub destino_sec: u8,
    /// Offset del destino dentro de su seccion.
    pub destino_off: i64,
    pub kind: u8,
}

/// Descodifica la reloc numero `n` de la tabla, o `None` si no cabe.
pub fn leer_reloc(bytes: &[u8], tabla_off: u64, tabla_size: u64, n: usize) -> Option<BexReloc> {
    let dentro = n.checked_mul(RELOC_SIZE)?;
    if (dentro + RELOC_SIZE) as u64 > tabla_size {
        return None;
    }
    let base = (tabla_off as usize).checked_add(dentro)?;
    Some(BexReloc {
        donde_off: read_u64(bytes, base)?,
        destino_sec: read_u32(bytes, base + 8)? as u8,
        kind: *bytes.get(base + 12)?,
        donde_sec: *bytes.get(base + 13)?,
        destino_off: read_u64(bytes, base + 16)? as i64,
    })
}

/// Cuantas relocations hay en la tabla.
pub fn cuantas_relocs(tabla_size: u64) -> usize {
    (tabla_size as usize) / RELOC_SIZE
}

/// Report the currently available BEX admission capability over serial.
pub fn announce() {
    crate::ring0::dev::console::serial_write(
        "[bex] x86-64 admission gate ready; Ring 3 mapping pending storage/process phase\n",
    );
}

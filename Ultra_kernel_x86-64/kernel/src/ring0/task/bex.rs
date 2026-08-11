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
/// La seccion de HASHES: un BLAKE3 por cada una de las demas. **NO se mapea**
/// --no es memoria del programa-- pero si se LEE: es con lo que se cierra cada
/// seccion cuando termina de aterrizar. Ver `task/aterrizaje.rs` y
/// `BexLoadPlan::firma_*`.
pub const SECTION_SIGNATURE: u8 = 0x0F;
/// **LO QUE EL PROGRAMA REQUIERE, Y EL PORQUE.** Tampoco se mapea, y tambien se
/// LEE: es lo que sustituye a que el kernel deduzca. Ver
/// `bmo_abi::bef::requisitos` y `docs/EL_CONTRATO_DE_CARGA.md`.
///
/// Se declara aqui --y no solo en `bmo-abi`-- por la misma regla que el resto de
/// este fichero: `bmo-abi` es el CONTRATO y esto lo implementa contra el leyendo
/// bytes, sin importar su API con `alloc`.
pub const SECTION_REQUISITOS: u8 = 0x15;

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
    /// ** SU INDICE EN LA TABLA DEL FICHERO, no en este plan.
    ///
    /// El plan solo lleva lo cargable, asi que sus posiciones **no** son las del
    /// fichero: una imagen con `Code, Manifest, Data` deja `Data` en el hueco 1
    /// del plan y en el 2 del fichero. Y la tabla de hashes indexa por el del
    /// FICHERO. Confundirlos comprueba el `Code` contra el digest del `Data` --
    /// que no cuadra, y manda a buscar una corrupcion que no existe.
    pub indice: usize,
}

const EMPTY_MAPPING: BexMapping = BexMapping {
    kind: 0,
    flags: 0,
    file_offset: 0,
    file_size: 0,
    mem_size: 0,
    alignment: 0,
    indice: usize::MAX,
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
    /// Indice de la tabla de relocations en el FICHERO, para buscar su hash.
    /// `usize::MAX` si no hay.
    pub relocs_indice: usize,

    /// ** DONDE ESTA LA TABLA DE HASHES, para que la comprobacion la haga QUIEN
    /// COPIA y no este modulo.
    ///
    /// Antes esto se resolvia aqui dentro y se comprobaba todo de una pasada
    /// sobre el bufer de la imagen. El sitio era el equivocado: entre ese bufer
    /// y la memoria del proceso hay una COPIA, asi que se estaba certificando el
    /// origen y no el destino. Ahora `inspect` dice donde estan los digests y
    /// `proc::admit_payload` cierra cada seccion con el suyo **al aterrizar**.
    /// Ver `task/aterrizaje.rs`.
    ///
    /// `firma_file_size == 0` = la imagen no trae firma. Es lo normal en las que
    /// el kernel embebe, que no pasan por el escritor.
    pub firma_file_offset: u64,
    pub firma_file_size: u64,
    /// Indice de la propia seccion de firma. No puede contener su hash, y hace
    /// falta saber cual es para excluirla.
    pub firma_indice: usize,
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
    /// ** LLEGARON MENOS BYTES DE LOS QUE LA IMAGEN DICE MEDIR.
    ///
    /// La cabecera lleva `total_size`, asi que **el fichero declara su propio
    /// tamano** y el cargador puede comprobarlo sin preguntarle al sistema de
    /// ficheros. Sin esta comprobacion, una lectura corta se manifiesta como
    /// `InvalidSection` --la tabla apunta mas alla de lo leido-- y eso manda a
    /// buscar un fallo de FORMATO donde lo que hay es un fallo de TRANSPORTE.
    ///
    /// Son dos sitios distintos donde mirar, y confundirlos cuesta una tarde.
    ImagenIncompleta,
    /// ** UNA SECCION NO CUADRA CON SU HASH.
    ///
    /// La imagen llego ENTERA --el tamano cuadra-- y **por dentro no es la que
    /// se escribio**. Es el fallo que ningun contador de bytes puede ver: un
    /// sector que se lee sin error y trae datos de otro sitio da un fichero del
    /// tamano correcto y corrupto.
    ///
    /// Antes de existir esto, ese caso se manifestaba mucho mas tarde y en otro
    /// sitio: un `.bex` con un agujero pasa la admision, arranca, y muere
    /// doscientas instrucciones despues con un `#PF` que no se parece en nada a
    /// su causa.
    HashNoCuadra,
    /// ** EL PROLOGO NO TRAJO LA TABLA DE SECCIONES ENTERA.
    ///
    /// Distinto de `SectionTableOutOfBounds`, que dice "la tabla cae fuera del
    /// FICHERO" -- eso es una imagen mal formada. Esto dice "la tabla cae fuera
    /// de lo que se leyo por delante", que es un problema del cargador y se
    /// arregla leyendo mas, no rechazando el programa.
    PrologoCorto,
    /// ** UNA SECCION QUE HAY QUE CARGAR SE QUEDO SIN LEER.
    ///
    /// El cargador pregunta al formato cuanto necesita y lee eso. Si despues
    /// resulta que algo cargable cae mas alla, la cuenta y la lectura no
    /// cuadran: es un fallo del cargador, no del fichero, y se dice como tal en
    /// vez de disfrazarse de `InvalidSection`.
    SeccionNoLeida,
}

impl BexError {
    /// El nombre del fallo, para que CABINA lo diga en vez de callarlo.
    ///
    /// ** `admit_payload` hacia `Err(_) => log("payload failed BEX admission")`:
    /// once motivos distintos entrando por la misma puerta y saliendo con la
    /// misma frase. Un cargador que sabe POR QUE rechaza y no lo dice obliga a
    /// adivinar entre "el fichero llego a medias", "la arquitectura no es esta"
    /// y "el entry cae fuera del codigo" -- que se arreglan en tres sitios que
    /// no se parecen en nada.
    pub fn name(&self) -> &'static str {
        match self {
            BexError::TooSmall => "la imagen no llega ni a la cabecera",
            BexError::InvalidHeader => "cabecera invalida (magic, version o 0 secciones)",
            BexError::UnsupportedArchitecture => "otra arquitectura",
            BexError::UnsupportedEndianness => "otro orden de bytes",
            BexError::UnsupportedCpuFeature => "pide una extension de CPU que no se preserva",
            BexError::AbiMismatch => "otra version del ABI",
            BexError::NotExecutable => "la seccion de codigo no es ejecutable",
            BexError::TooManySections => "demasiadas secciones",
            BexError::SectionTableOutOfBounds => "la tabla de secciones cae fuera",
            BexError::InvalidSection => "una seccion es invalida o cae fuera",
            BexError::MissingCode => "no hay seccion de codigo",
            BexError::EntryOutsideCode => "el entry cae fuera del codigo",
            BexError::ImagenIncompleta => "LLEGARON MENOS BYTES DE LOS QUE LA IMAGEN DICE",
            BexError::HashNoCuadra => "una seccion NO CUADRA con su hash: la imagen esta corrupta",
            BexError::PrologoCorto => "la tabla de secciones no cabe en el prologo leido",
            BexError::SeccionNoLeida => "una seccion cargable se quedo sin leer",
        }
    }
}

/// **QUE NECESITA DE VERDAD ESTE FICHERO.** Devuelve cuantos bytes hay que leer.
///
/// === El escalon 2 de `LA_RAM.md`, en una funcion ===
///
/// El cargador leia el fichero ENTERO a un estatico de 4 MiB, y despues se
/// quedaba con el codigo y los datos. Con un paquete que lleva un WAD dentro,
/// eso es traerse cinco megabytes de bodega para ejecutar ochocientos kilos.
///
/// ** La pregunta correcta no es *"cuanto mide"* sino **"que necesita"**, y el
/// fichero sabe contestarla: la tabla de secciones esta en el byte 48 --el
/// escritor la pone siempre ahi-- y dice donde acaba cada cosa. De todas ellas,
/// el cargador solo toca cuatro:
///
/// | | Para que |
/// |---|---|
/// | `Code`, `RoData`, `Data` | se copian al espacio del proceso |
/// | `Bss` | no ocupa fichero: son ceros que se declaran |
/// | `Relocs` | se leen, se aplican y se olvidan |
/// | `Signature` | los hashes con los que se comprueba lo anterior |
///
/// Todo lo demas --recursos, simbolos, depuracion, manifiesto, y **cualquier
/// tipo que este kernel no conozca**-- es data para otro. Los recursos se leen
/// en EJECUCION, por `TASK_OP_MI_PAQUETE`, y por su propia puerta: el cargador
/// no tiene por que adelantarlos a RAM para que el programa los pida despues.
///
/// > **La RAM es la zona donde se ejecuta, no la bodega. La bodega es el disco.**
///
/// `Err(PrologoCorto)` si la tabla no cabe en lo que se le paso: quien llama
/// puede leer mas y volver a preguntar, que es distinto de rechazar la imagen.
pub fn necesita(prologo: &[u8]) -> Result<usize, BexError> {
    if prologo.len() < BEX_HEADER_SIZE {
        return Err(BexError::TooSmall);
    }
    let magic = read_u32(prologo, 0).ok_or(BexError::TooSmall)?;
    let count = read_u32(prologo, 40).ok_or(BexError::TooSmall)? as usize;
    if magic != BEX_MAGIC || count == 0 {
        return Err(BexError::InvalidHeader);
    }
    if count > MAX_BEX_SECTIONS {
        return Err(BexError::TooManySections);
    }
    let tabla = read_u64(prologo, 32).ok_or(BexError::TooSmall)? as usize;
    let fin_tabla = tabla
        .checked_add(count * BEX_SECTION_SIZE)
        .ok_or(BexError::SectionTableOutOfBounds)?;
    if fin_tabla > prologo.len() {
        return Err(BexError::PrologoCorto);
    }

    // Empieza en el fin de la tabla: la cabecera y la tabla siempre hacen falta,
    // aunque un fichero no tuviera ni una seccion que cargar.
    let mut hasta = fin_tabla;
    for i in 0..count {
        let e = tabla + i * BEX_SECTION_SIZE;
        let kind = *prologo.get(e).ok_or(BexError::InvalidSection)?;
        // La `Bss` no ocupa fichero: es el escalon 0, los ceros se declaran y no
        // viajan. Pedir sus bytes seria deshacerlo desde el otro lado.
        if kind == SECTION_BSS {
            continue;
        }
        // Los REQUISITOS entran en la cuenta aunque hoy nadie los lea todavia,
        // y es a proposito: la seccion la coloca el escritor en el grupo de
        // delante, asi que **hoy cae dentro de lo que se trae por casualidad**.
        // Una correccion que depende de la disposicion es una correccion que se
        // rompe el dia que alguien reordene el fichero, y el sintoma seria que
        // el cargador no encuentra lo que un programa pide.
        if !is_loadable(kind)
            && kind != SECTION_RELOCS
            && kind != SECTION_SIGNATURE
            && kind != SECTION_REQUISITOS
        {
            continue;
        }
        let off = read_u64(prologo, e + 8).ok_or(BexError::InvalidSection)? as usize;
        let len = read_u64(prologo, e + 16).ok_or(BexError::InvalidSection)? as usize;
        let fin = off.checked_add(len).ok_or(BexError::InvalidSection)?;
        if fin > hasta {
            hasta = fin;
        }
    }
    Ok(hasta)
}


/// Validate an untrusted BEX image and produce a fixed-size mapping plan.
///
/// No `alloc`, file access, relocation, page-table mutation or control transfer
/// happens here.  This boundary is therefore safe to call before a process is
/// admitted to the kernel.
///
/// ## Los DOS limites, que no son el mismo (2026-08-10)
///
/// - `bytes` es **lo que se leyo**: el prologo mas las secciones que el cargador
///   va a usar. Lo que se toque tiene que caber aqui.
/// - `tam_fichero` es **lo que mide el fichero en el disco**. Es contra este
///   contra el que se comprueba el `total_size` de la cabecera y contra el que
///   se validan los limites declarados de TODAS las secciones, incluidas las que
///   no se leyeron.
///
/// ** Confundirlos convierte una imagen cortada en una imagen valida, que es el
/// fallo que `ImagenIncompleta` existe para cazar. Por eso son dos parametros y
/// no se deduce uno del otro: antes coincidian porque se leia el fichero entero,
/// y una igualdad que se cumple por casualidad es una igualdad que un dia no.
pub fn inspect(bytes: &[u8], tam_fichero: usize) -> Result<BexLoadPlan, BexError> {
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
    // ** LA IMAGEN DECLARA SU PROPIO TAMANO: se comprueba antes que nada mas.
    //
    // `total_size` esta en la cabecera desde que existe el formato --lo pone
    // `BefBuilder::build`-- y hasta hoy no lo miraba nadie. Con el, el cargador
    // sabe si le llego el fichero ENTERO sin tener que preguntarle al sistema de
    // ficheros cuanto media: el dato viaja dentro.
    //
    // Va delante de la tabla de secciones a proposito. Si faltan bytes, la tabla
    // apunta mas alla de lo leido y la primera seccion que se salga contesta
    // `InvalidSection` -- que es cierto y es la pista equivocada: manda a mirar
    // el FORMATO cuando lo que fallo es el TRANSPORTE.
    //
    // `0` se acepta porque las imagenes que el kernel EMBEBE no pasan por
    // `BefBuilder::build` y lo dejan sin poner. Comprobar solo cuando el dato
    // existe es mejor que rechazar a quien nunca prometio nada.
    // Contra `tam_fichero` y NO contra `bytes.len()`: desde el escalon 2 lo
    // segundo es "lo que hizo falta leer", que puede ser una fraccion. Ver la
    // nota de los dos limites en la cabecera.
    let total_size = read_u32(bytes, 44).ok_or(BexError::TooSmall)? as usize;
    if total_size != 0 && tam_fichero < total_size {
        return Err(BexError::ImagenIncompleta);
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

    // ** LA INTEGRIDAD YA NO SE COMPRUEBA AQUI, Y ES A PROPOSITO (2026-08-10).
    //
    // Aqui vivia `verificar_hashes`: una pasada que comprobaba los BLAKE3 de
    // todas las secciones sobre `bytes`. Funcionaba y estaba en el sitio
    // equivocado, porque **entre `bytes` y la memoria del proceso hay una
    // copia**. Lo que certificaba era el origen; lo que hace falta certificar es
    // lo que el proceso va a EJECUTAR.
    //
    // Ahora `inspect` hace lo suyo --decir DONDE estan los digests, ver
    // `firma_file_offset`-- y quien copia cierra cada seccion con el suyo en
    // cuanto termina de aterrizar. El fallo pasa de `cabecera invalida` (que
    // manda a mirar el formato) a `la seccion Code no cuadra con su hash` (que
    // dice que el formato estaba bien y fallo el transporte).
    //
    // Ver `task/aterrizaje.rs` y `docs/EL_CONTRATO_DE_CARGA.md`, pieza A.

    let mut plan = BexLoadPlan {
        entry_offset,
        sections: [EMPTY_MAPPING; MAX_BEX_SECTIONS],
        section_count: 0,
        skipped_sections: 0,
        relocs_file_offset: 0,
        relocs_file_size: 0,
        relocs_indice: usize::MAX,
        firma_file_offset: 0,
        firma_file_size: 0,
        firma_indice: usize::MAX,
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
            // Contra el FICHERO: una seccion que se sale del archivo es una
            // imagen mal formada, se vaya a leer o no.
            if end as usize > tam_fichero {
                return Err(BexError::InvalidSection);
            }
            // Y contra lo LEIDO, solo para lo que se va a tocar. Que los
            // recursos caigan mas alla del prologo es lo normal desde el
            // escalon 2 -- que caiga el codigo es que la cuenta de `necesita`
            // y la lectura no cuadran, y eso es un fallo del cargador con
            // nombre propio en vez de un `InvalidSection` que manda a mirar el
            // formato.
            let lo_usa = is_loadable(kind) || kind == SECTION_RELOCS || kind == SECTION_SIGNATURE;
            if lo_usa && end as usize > bytes.len() {
                return Err(BexError::SeccionNoLeida);
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
            plan.relocs_indice = index;
            continue;
        }
        // * LA FIRMA se apunta y tampoco se mapea: no es memoria del programa,
        // es la prueba de que lo demas llego entero. Quien la usa es el bucle
        // que copia, seccion por seccion. Ver `task/aterrizaje.rs`.
        if kind == SECTION_SIGNATURE {
            plan.firma_file_offset = file_offset;
            plan.firma_file_size = file_size;
            plan.firma_indice = index;
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
            indice: index,
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

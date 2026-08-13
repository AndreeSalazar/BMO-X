//! Ring 0 BEX admission gate.
//!
//! This module performs the allocation-free part of loading a BEX image:
//! validate the x86-64/ABI contract and create a fixed-size mapping plan.
//! It deliberately does not execute code.  The process subsystem will later
//! consume this plan to allocate user pages, copy sections and enter Ring 3.

use bmo_bex_gate as gate;

// ** EL CONTRATO YA NO SE ESCRIBE AQUI. Se re-exporta de la puerta.
//
// Aqui vivian los mismos numeros que en `bmo-abi` y en `bmo-bex-gate`: magic,
// tamano de cabecera, tipos de seccion, banderas. Tres copias del mismo contrato,
// y la unica forma de que no se separaran era que nadie las tocara nunca.
//
// Se re-exportan en vez de borrarse porque medio kernel las nombra por su nombre
// viejo (`bex::SECTION_CODE`), y renombrar cincuenta sitios para no ganar nada
// seria ruido. Lo que importa es que **ya no hay tres definiciones, hay una**.
pub use gate::{
    CODE as SECTION_CODE, DATA as SECTION_DATA, RELOCS as SECTION_RELOCS,
    REQUISITOS as SECTION_REQUISITOS, RODATA as SECTION_RODATA, BSS as SECTION_BSS,
    SIGNATURE as SECTION_SIGNATURE, SECCION_FLAG_EXEC as SECTION_FLAG_EXEC,
    se_carga as is_loadable,
};

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

/// **Por que no se admitio.**
///
/// == Dos clases de motivo, y por eso son dos variantes ==
///
/// - `Formato` es lo que dice **la puerta** (`bmo-bex-gate`): la imagen esta mal
///   formada. Ese veredicto es el mismo que da el toolchain al compilar, porque
///   es literalmente el mismo codigo.
/// - Lo demas es lo que **solo el que carga puede saber**: que una seccion no
///   cuadro con su hash al aterrizar. Eso no se puede saber mirando el fichero;
///   se sabe habiendolo traido.
///
/// Antes esto tenia veinte variantes que repetian una a una las del validador de
/// `bmo-abi`. Dos listas de motivos que **tienen** que decir lo mismo son dos
/// listas que un dia dejan de decirlo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BexError {
    /// La imagen esta mal formada. Ver [`bmo_bex_gate::Falta`].
    Formato(gate::Falta),
    /// ** UNA SECCION NO CUADRA CON SU HASH.
    ///
    /// La imagen llego ENTERA --el tamano cuadra-- y **por dentro no es la que
    /// se escribio**. Es el fallo que ningun contador de bytes puede ver: un
    /// sector que se lee sin error y trae datos de otro sitio da un fichero del
    /// tamano correcto y corrupto.
    ///
    /// No lo puede decir la puerta, y por eso vive aqui: la puerta mira **el
    /// fichero**, y esto solo se sabe mirando **lo que aterrizo**. Ver
    /// `task/aterrizaje.rs`.
    HashNoCuadra,
    /// ** EL PROLOGO NO TRAJO LA TABLA DE SECCIONES ENTERA.
    ///
    /// Quien llama puede leer mas bytes y volver a preguntar, que es distinto de
    /// rechazar la imagen. Es la traduccion de `Falta::TablaFueraDeLoLeido` a la
    /// unica accion que tiene sentido para un cargador: **volver a intentarlo con
    /// mas**.
    PrologoCorto,
}

impl BexError {
    /// Una linea corta, en el idioma del sistema. La usan CABINA y el shell.
    ///
    /// ** EL MOTIVO, CON SU NOMBRE. Aqui hubo un `Err(_)` durante meses: trece
    /// motivos distintos entraban por la misma puerta y salian con la frase
    /// *"payload failed BEX admission"*. Un cargador que sabe por que rechaza y
    /// no lo dice obliga a adivinar entre "el fichero llego a medias", "otra
    /// arquitectura" y "el entry cae fuera" -- tres cosas que se arreglan en tres
    /// sitios que no se parecen en nada. Costo una tanda de fotos el 2026-08-09.
    pub fn name(&self) -> &'static str {
        match self {
            BexError::Formato(f) => f.nombre(),
            BexError::HashNoCuadra => "una seccion NO CUADRA con su hash: la imagen esta corrupta",
            BexError::PrologoCorto => "la tabla de secciones no cabe en el prologo leido",
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
    // Se le pregunta a la puerta, no se recorre la tabla otra vez. `usize::MAX`
    // como tamano de fichero porque aqui **no se esta validando la imagen**: se
    // esta preguntando cuanto hay que traer, y los limites de verdad se
    // comprueban en `inspect` con el tamano real. Meter aqui un tamano inventado
    // rechazaria imagenes buenas por una cuenta que ni siquiera es esta.
    match gate::revisar(prologo, usize::MAX) {
        Ok(rev) => Ok(rev.hasta_donde_hace_falta() as usize),
        // La tabla no cabio en lo leido: quien llama puede traer mas y volver a
        // preguntar. Es la unica falta que se traduce a una ACCION en vez de a
        // un rechazo, y por eso es la unica que se distingue aqui.
        Err(gate::Falta::TablaFueraDeLoLeido) => Err(BexError::PrologoCorto),
        Err(f) => Err(BexError::Formato(f)),
    }
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
    // ** LA DECISION NO SE TOMA AQUI (2026-08-10).
    //
    // Aqui vivian doscientas lineas de comprobaciones --magic, version, ABI,
    // limites, solapamientos, banderas-- que eran **las mismas** que las de
    // `bmo-abi::bef::validator`, escritas otra vez porque aquella usa `alloc` y
    // en Ring 0 no hay a quien pedirle memoria.
    //
    // Dos copias de una decision son dos decisiones esperando a separarse. Y no
    // se podian compartir mientras la decision viviera **incrustada en dos
    // trabajos distintos**: alli construyendo mensajes, aqui construyendo el
    // plan de mapeo.
    //
    // Ahora la decision es una cosa por su cuenta (`bmo-bex-gate`: sin `alloc`,
    // sin dependencias) y este modulo hace lo unico que solo el puede hacer:
    // **el plan**. Ninguno de los dos consumidores es dueno del veredicto, asi
    // que ninguno puede desviarse de el.
    let rev = gate::revisar(bytes, tam_fichero).map_err(BexError::Formato)?;

    let mut plan = BexLoadPlan {
        entry_offset: rev.entry_offset(),
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
    let mut loadable = 0usize;
    let mut skipped = 0usize;

    for s in rev.secciones() {
        // * LAS RELOCATIONS se apuntan y NO se mapean: no son memoria del
        // programa --el proceso nunca las ve-- pero el cargador las necesita
        // para rellenar punteros con direcciones que solo se conocen al colocar
        // las secciones.
        if s.kind == gate::RELOCS {
            plan.relocs_file_offset = s.file_offset;
            plan.relocs_file_size = s.file_size;
            plan.relocs_indice = s.indice;
            continue;
        }
        // * LA FIRMA tampoco se mapea, y tambien se lee: es con lo que se cierra
        // cada seccion al aterrizar. Ver `task/aterrizaje.rs`.
        if s.kind == gate::SIGNATURE {
            plan.firma_file_offset = s.file_offset;
            plan.firma_file_size = s.file_size;
            plan.firma_indice = s.indice;
            continue;
        }
        // * Y lo demas --manifiesto, requisitos, recursos, simbolos, o un tipo
        // que este kernel no conoce-- se valido y no se mapea. Ver
        // `gate::se_carga`: un tipo desconocido se SALTA, no se rechaza.
        if !gate::se_carga(s.kind) {
            skipped += 1;
            continue;
        }
        plan.sections[loadable] = BexMapping {
            kind: s.kind,
            flags: s.flags,
            file_offset: s.file_offset,
            file_size: s.file_size,
            mem_size: s.mem_size,
            alignment: s.alignment,
            indice: s.indice,
        };
        loadable += 1;
    }

    // El plan solo describe lo que se mapea.
    plan.section_count = loadable;
    plan.skipped_sections = skipped;
    Ok(plan)
}

// `read_u16` se fue con `inspect`: los campos de 16 bits los lee la puerta. Los
// dos que quedan leen las RELOCATIONS, que es trabajo del que CARGA y no del que
// decide -- por eso se quedan aqui y no bajan al crate de la puerta.

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
/// Tamano de una relocation. **Ya no se escribe aqui**: viene de
/// `bmo-bex-gate`, que es el crate que los dos lados comparten.
///
/// === Por que se movio (2026-08-12) ===
///
/// Aqui decia: *"si el struct cambiara de forma, `RELOC_SIZE` y estos offsets
/// son el unico sitio a tocar"*. Y **"el unico sitio a tocar" es la definicion
/// de una duplicacion que se olvida**: mover un campo del struct de `bmo-abi`
/// compila igual, pasa todos los tests del toolchain, y este cargador escribe
/// una direccion equivocada dentro de un proceso. Corrupcion silenciosa, no un
/// fallo.
///
/// El kernel sigue sin importar `bmo-abi` --trae `alloc`-- pero SI importa
/// `bmo-bex-gate`, que no tiene dependencias. Asi que los offsets viven ahi, y
/// `bmo-abi` los clava a su struct con `offset_of!` en
/// `tests/bef_dos_lectores.rs`. De dos verdades a una verdad y una prueba.
pub use gate::RELOC_SIZE;
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
    // Los offsets vienen del crate compartido, no de aqui. Ver `RELOC_SIZE`.
    Some(BexReloc {
        donde_off: read_u64(bytes, base + gate::reloc::OFFSET)?,
        destino_sec: read_u32(bytes, base + gate::reloc::SYMBOL_IDX)? as u8,
        kind: *bytes.get(base + gate::reloc::KIND)?,
        donde_sec: *bytes.get(base + gate::reloc::TARGET_SECTION)?,
        destino_off: read_u64(bytes, base + gate::reloc::ADDEND)? as i64,
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

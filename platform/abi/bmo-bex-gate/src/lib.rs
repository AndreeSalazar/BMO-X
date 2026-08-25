//! **LA DECISION: es admisible este BEX?** Y nada mas.
//!
//! ## Por que existe este crate
//!
//! La respuesta a *"se puede ejecutar esto?"* estaba escrita **dos veces**:
//!
//! ```text
//!   bmo-abi/bef/validator.rs   1.281 lineas   con alloc   decide Y explica
//!   kernel/task/bex.rs           ~200 lineas   sin alloc   decide Y planifica
//! ```
//!
//! Y no estaban duplicadas por descuido. Estaban duplicadas porque **la decision
//! vivia incrustada en dos trabajos distintos**: una construye mensajes de error
//! con `String` para el que compila, la otra construye el plan de mapeo para el
//! que ejecuta. Compartirlas era imposible mientras la decision no fuera una cosa
//! por su cuenta.
//!
//! Aqui es una cosa por su cuenta. Los dos siguen haciendo lo suyo encima:
//!
//! ```text
//!                     bmo-bex-gate        <- la DECISION
//!                      /          \
//!       validator (alloc)          bex.rs (Ring 0)
//!       anade MENSAJES             anade el PLAN
//! ```
//!
//! **Ninguno de los dos es dueno de la decision, asi que ninguno puede desviarse
//! de ella.** Que es distinto de tener una prueba que compare los dos: eso caza
//! la divergencia despues de escribirla; esto la hace imposible.
//!
//! ## Y lo que esto NO hace
//!
//! No mapea, no reserva, no lee disco, no explica en prosa, y **no opina**. Una
//! imagen que pasa por aqui es una imagen **bien formada**, y eso no quiere decir
//! que sea buena, ni segura, ni tuya. Quien decide si se ejecuta es el sistema,
//! con esto y con lo demas -- la firma, los requisitos, quien la lanza.
//!
//! > Bien formado no es lo mismo que de fiar. Confundirlo es como creerse un
//! > documento porque la letra es bonita.

#![no_std]
#![forbid(unsafe_code)]

// -- El contrato en el cable ------------------------------------------------
//
// Estos numeros son **BEX v1 (= BEF1)** y no se deducen de ningun struct de Rust
// a proposito: el fichero viene del disco, no de un `#[repr(C)]` que casualmente
// coincida. Si el formato cambiara, este es el unico sitio a tocar.

/// `"BEF1"` en little-endian.
pub const MAGIC: u32 = u32::from_le_bytes(*b"BEF1");
/// Bytes de la cabecera.
pub const CABECERA: usize = 48;
/// Bytes de cada entrada de la tabla de secciones.
pub const ENTRADA: usize = 48;
/// La unica version mayor que este sistema lee.
pub const VERSION_MAYOR: u16 = 1;
/// x86-64.
pub const ARCH_X86_64: u8 = 0x01;
/// Little-endian.
pub const ENDIAN_LE: u8 = 0x00;

/// Cuantas secciones admite una tabla. **Auditable a ojo**, que es el motivo:
/// una tabla que no se puede leer entera en una pantalla es una tabla en la que
/// se puede esconder algo.
pub const MAX_SECCIONES: usize = 16;

// -- Tipos de seccion --------------------------------------------------------

pub const CODE: u8 = 0x01;
pub const RODATA: u8 = 0x02;
pub const DATA: u8 = 0x03;
pub const BSS: u8 = 0x04;
pub const RELOCS: u8 = 0x07;
pub const SIGNATURE: u8 = 0x0F;
pub const REQUISITOS: u8 = 0x15;

/// Se mapea en el espacio del programa?
///
/// **LA REGLA**: solo cuatro tipos son memoria del programa. Todo lo demas
/// --manifiesto, firma, simbolos, recursos, y **cualquier tipo desconocido**--
/// es data para otro, y se salta. Un tipo que no me incumbe no es un error: es
/// data que no voy a abrir. Es lo que ha mantenido vivo a ELF treinta anos.
pub fn se_carga(kind: u8) -> bool {
    matches!(kind, CODE | RODATA | DATA | BSS)
}

/// El cargador la LEE aunque no la mapee.
pub fn se_lee(kind: u8) -> bool {
    se_carga(kind) || matches!(kind, RELOCS | SIGNATURE | REQUISITOS)
}

// -- Banderas de la cabecera -------------------------------------------------

pub const FLAG_EJECUTABLE: u32 = 1 << 0;
pub const FLAG_COMPRIMIDO: u32 = 1 << 4;
pub const FLAG_FIRMADO: u32 = 1 << 5;
pub const FLAG_RECARGABLE: u32 = 1 << 7;

/// Banderas que **cambian lo que significan las secciones** y que no implementa
/// nadie en este sistema.
///
/// [!] Y esto NO contradice la regla de que un tipo de seccion desconocido se
/// salta. Una SECCION que no me incumbe es data para otro y no afecta a lo que
/// yo hago con las mias. Una BANDERA que no entiendo **cambia el significado de
/// las secciones que si me incumben**: `COMPRIMIDO` dice que los bytes del
/// fichero no son los bytes que van a memoria. Ignorarla es cargar un bloque
/// comprimido en crudo y saltar a el.
///
/// Saltarse una seccion es tolerancia. Saltarse una bandera es leer mal a
/// proposito.
pub const FLAGS_NO_IMPLEMENTADAS: u32 = FLAG_COMPRIMIDO | FLAG_RECARGABLE;

pub const SECCION_FLAG_EXEC: u32 = 1 << 2;

/// **Por que no se admite.** Cada una manda a mirar un sitio distinto, que es la
/// razon de que sean variantes y no un booleano.
///
/// Es `Copy` y sin datos prestados a proposito: cruza la frontera de Ring 0 sin
/// reservar nada, y quien quiera contarlo con numeros los saca del fichero, que
/// sigue teniendo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Falta {
    NoLlegaNiALaCabecera,
    /// Magic, version mayor, o cero secciones.
    CabeceraInvalida,
    OtraArquitectura,
    OtroOrdenDeBytes,
    /// Declara una extension de CPU cuyo estado el sistema no sabe preservar.
    ExtensionDeCpuQueNoSePreserva,
    OtraVersionDelAbi,
    NoEsEjecutable,
    /// Ver [`FLAGS_NO_IMPLEMENTADAS`].
    PideAlgoQueNadieImplementa,
    /// Dice `FIRMADO` y no trae seccion de firma.
    CabeceraQueSeDesmiente,
    DemasiadasSecciones,
    /// La tabla no cabe en lo que se paso. Quien llama puede leer mas y volver.
    TablaFueraDeLoLeido,
    TablaFueraDelFichero,
    SeccionInvalida,
    /// Dos secciones se pelean por los mismos bytes del fichero.
    SeccionesSeSolapan,
    /// Una seccion declara bytes que caen fuera del fichero.
    SeccionFueraDelFichero,
    SinCodigo,
    LaCodigoNoEsEjecutable,
    EntryFueraDelCodigo,
    /// La cabecera dice medir mas de lo que el fichero mide.
    ImagenIncompleta,
}

impl Falta {
    /// Una linea corta, en el idioma del sistema.
    ///
    /// `&'static str` y no `String`: esto lo imprime CABINA en Ring 0, donde no
    /// hay a quien pedirle memoria. Quien quiera una frase larga con numeros
    /// dentro --el toolchain-- la construye encima, que para eso tiene `alloc`.
    pub fn nombre(self) -> &'static str {
        match self {
            Falta::NoLlegaNiALaCabecera => "la imagen no llega ni a la cabecera",
            Falta::CabeceraInvalida => "cabecera invalida (magic, version o 0 secciones)",
            Falta::OtraArquitectura => "otra arquitectura",
            Falta::OtroOrdenDeBytes => "otro orden de bytes",
            Falta::ExtensionDeCpuQueNoSePreserva => "pide una extension de CPU que no se preserva",
            Falta::OtraVersionDelAbi => "otra version del ABI",
            Falta::NoEsEjecutable => "no esta marcado como ejecutable",
            Falta::PideAlgoQueNadieImplementa => "la cabecera pide algo que este sistema no hace",
            Falta::CabeceraQueSeDesmiente => "dice venir firmado y no trae firma",
            Falta::DemasiadasSecciones => "demasiadas secciones",
            Falta::TablaFueraDeLoLeido => "la tabla de secciones no cabe en lo leido",
            Falta::TablaFueraDelFichero => "la tabla de secciones cae fuera del fichero",
            Falta::SeccionInvalida => "una seccion esta mal formada",
            Falta::SeccionesSeSolapan => "dos secciones se pelean por los mismos bytes",
            Falta::SeccionFueraDelFichero => "una seccion cae fuera del fichero",
            Falta::SinCodigo => "no hay seccion de codigo",
            Falta::LaCodigoNoEsEjecutable => "la seccion de codigo no es ejecutable",
            Falta::EntryFueraDelCodigo => "el punto de entrada cae fuera del codigo",
            Falta::ImagenIncompleta => "llegaron menos bytes de los que la imagen dice medir",
        }
    }
}

/// Una seccion, ya comprobada. Los numeros son los del fichero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Seccion {
    /// Su indice **en la tabla del fichero**. Es con lo que la firma la nombra.
    pub indice: usize,
    pub kind: u8,
    pub flags: u32,
    pub file_offset: u64,
    pub file_size: u64,
    pub mem_size: u64,
    pub alignment: u16,
}

/// **Una imagen que ya paso la puerta.** Solo se puede construir con
/// [`revisar`], asi que tener una es la prueba de que las comprobaciones
/// corrieron.
///
/// Es el patron de "estado ilegal irrepresentable": quien recibe esto no tiene
/// que acordarse de validar, porque no existe una forma de llegar aqui sin haber
/// validado.
#[derive(Clone, Copy)]
pub struct Revisada<'a> {
    prologo: &'a [u8],
    tabla: usize,
    cuantas: usize,
    entry_offset: u64,
}

impl<'a> Revisada<'a> {
    pub fn entry_offset(&self) -> u64 {
        self.entry_offset
    }
    pub fn cuantas(&self) -> usize {
        self.cuantas
    }
    /// La seccion `i`. El indice es el **de la tabla del fichero**.
    pub fn seccion(&self, i: usize) -> Option<Seccion> {
        if i >= self.cuantas {
            return None;
        }
        let e = self.tabla + i * ENTRADA;
        Some(Seccion {
            indice: i,
            kind: *self.prologo.get(e)?,
            flags: u32_en(self.prologo, e + 4)?,
            file_offset: u64_en(self.prologo, e + 8)?,
            file_size: u64_en(self.prologo, e + 16)?,
            mem_size: u64_en(self.prologo, e + 24)?,
            alignment: match u16_en(self.prologo, e + 40)? {
                0 => 8,
                a => a,
            },
        })
    }
    /// Recorre las secciones en el orden del fichero.
    pub fn secciones(&self) -> impl Iterator<Item = Seccion> + '_ {
        (0..self.cuantas).filter_map(move |i| self.seccion(i))
    }
    /// La primera seccion de un tipo, si la hay.
    pub fn buscar(&self, kind: u8) -> Option<Seccion> {
        self.secciones().find(|s| s.kind == kind)
    }
    /// Hasta que byte del fichero hace falta leer para tener **todo lo que el
    /// cargador toca**: codigo, datos, relocations, hashes y requisitos.
    ///
    /// Los recursos van detras y no entran: se leen en ejecucion, por su puerta.
    pub fn hasta_donde_hace_falta(&self) -> u64 {
        let mut hasta = (self.tabla + self.cuantas * ENTRADA) as u64;
        for s in self.secciones() {
            if s.kind == BSS || !se_lee(s.kind) {
                continue;
            }
            let fin = s.file_offset.saturating_add(s.file_size);
            if fin > hasta {
                hasta = fin;
            }
        }
        hasta
    }
}

/// **LA PUERTA.** Comprueba una imagen BEX y no hace nada mas.
///
/// - `prologo`: los primeros bytes del fichero. Tiene que llegar al menos a la
///   cabecera y a la tabla de secciones entera; con **2 KiB sobra para cualquier
///   `.bex` que pueda existir** (48 + 16*48 = 816 bytes).
/// - `tam_fichero`: lo que mide el archivo ENTERO en el disco.
///
/// == Los dos numeros no son el mismo, y confundirlos es el bug ==
///
/// Los limites de las secciones se comprueban contra `tam_fichero` --el fichero
/// completo-- y **no** contra lo que quepa en `prologo`. Desde que el cargador
/// trae las secciones una a una, "no esta en el prologo" es la situacion normal
/// de todas ellas. Medirlas contra el prologo rechazaria toda imagen que no
/// cupiera en dos kilos, o sea todas.
pub fn revisar(prologo: &[u8], tam_fichero: usize) -> Result<Revisada<'_>, Falta> {
    if prologo.len() < CABECERA {
        return Err(Falta::NoLlegaNiALaCabecera);
    }

    let magic = u32_en(prologo, 0).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let version_mayor = u16_en(prologo, 4).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let flags = u32_en(prologo, 8).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let arch = *prologo.get(12).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let endian = *prologo.get(13).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let cpu = u16_en(prologo, 14).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let abi_mayor = *prologo.get(16).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let abi_menor = *prologo.get(17).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let entry_offset = u64_en(prologo, 24).ok_or(Falta::NoLlegaNiALaCabecera)?;
    let tabla = u64_en(prologo, 32).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let cuantas = u32_en(prologo, 40).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;
    let total_size = u32_en(prologo, 44).ok_or(Falta::NoLlegaNiALaCabecera)? as usize;

    if magic != MAGIC || version_mayor != VERSION_MAYOR || cuantas == 0 {
        return Err(Falta::CabeceraInvalida);
    }

    // ** LA IMAGEN DECLARA SU PROPIO TAMANO, y se comprueba antes que nada mas.
    //
    // Va delante de la tabla a proposito: si faltan bytes, las secciones apuntan
    // mas alla y la primera que se salga contesta `SeccionFueraDelFichero` -- que
    // es cierto y es la pista equivocada, porque manda a mirar el FORMATO cuando
    // lo que fallo es el TRANSPORTE.
    //
    // `0` se acepta: las imagenes que un kernel EMBEBE no pasan por el escritor y
    // lo dejan sin poner. Comprobar solo cuando el dato existe es mejor que
    // rechazar a quien nunca prometio nada.
    if total_size != 0 && tam_fichero < total_size {
        return Err(Falta::ImagenIncompleta);
    }

    if arch != ARCH_X86_64 {
        return Err(Falta::OtraArquitectura);
    }
    if endian != ENDIAN_LE {
        return Err(Falta::OtroOrdenDeBytes);
    }
    // Un bit que no conozco = una parte del estado del procesador que no se que
    // existe y que por tanto NO voy a preservar en un cambio de contexto. Se
    // rechaza, y ese rechazo es la mejora: convierte una corrupcion silenciosa en
    // un "no" con nombre.
    if cpu != 0 {
        return Err(Falta::ExtensionDeCpuQueNoSePreserva);
    }
    if !((abi_mayor == 1 || abi_mayor == 2) && abi_menor == 0) {
        return Err(Falta::OtraVersionDelAbi);
    }
    if flags & FLAG_EJECUTABLE == 0 {
        return Err(Falta::NoEsEjecutable);
    }
    if flags & FLAGS_NO_IMPLEMENTADAS != 0 {
        return Err(Falta::PideAlgoQueNadieImplementa);
    }
    if cuantas > MAX_SECCIONES {
        return Err(Falta::DemasiadasSecciones);
    }

    let bytes_tabla = cuantas.checked_mul(ENTRADA).ok_or(Falta::TablaFueraDelFichero)?;
    let fin_tabla = tabla.checked_add(bytes_tabla).ok_or(Falta::TablaFueraDelFichero)?;
    if fin_tabla > tam_fichero {
        return Err(Falta::TablaFueraDelFichero);
    }
    // Distinto de lo anterior: la tabla SI cabe en el fichero, pero no en lo que
    // se leyo. Quien llama puede traer mas y volver a preguntar, que no es lo
    // mismo que rechazar la imagen.
    if fin_tabla > prologo.len() {
        return Err(Falta::TablaFueraDeLoLeido);
    }

    let rev = Revisada { prologo, tabla, cuantas, entry_offset };

    // -- Cada seccion por su cuenta --
    let mut hay_codigo = false;
    let mut tam_codigo = 0u64;
    let mut hay_firma = false;
    for s in rev.secciones() {
        if s.kind == 0 || s.file_size > s.mem_size {
            return Err(Falta::SeccionInvalida);
        }
        // Solo la Bss puede no ocupar fichero: sus ceros se declaran y no viajan.
        if s.kind != BSS && s.file_size == 0 {
            return Err(Falta::SeccionInvalida);
        }
        if !s.alignment.is_power_of_two() {
            return Err(Falta::SeccionInvalida);
        }
        if s.kind != BSS {
            let fin = s
                .file_offset
                .checked_add(s.file_size)
                .ok_or(Falta::SeccionInvalida)?;
            if fin > tam_fichero as u64 {
                return Err(Falta::SeccionFueraDelFichero);
            }
        }
        if s.kind == CODE {
            if s.flags & SECCION_FLAG_EXEC == 0 {
                return Err(Falta::LaCodigoNoEsEjecutable);
            }
            hay_codigo = true;
            tam_codigo = s.mem_size;
        }
        if s.kind == SIGNATURE {
            hay_firma = true;
        }
    }

    // ** QUE NO HAYA DOS PELEANDOSE POR LOS MISMOS BYTES.
    //
    // Cada una ya paso sus limites: ninguna se sale del fichero. Y aun asi la
    // tabla puede decir que `.code` vive en `[100, 200)` y `.data` en `[150, 250)`
    // -- dos afirmaciones que **no pueden ser ciertas a la vez**. Un cargador que
    // se lo cree monta un proceso donde cincuenta bytes son a la vez codigo
    // ejecutable y datos escribibles, que es la forma clasica de meter codigo en
    // una pagina que deberia ser de solo lectura.
    //
    // Todas contra todas y no solo las contiguas: la tabla no tiene por que venir
    // ordenada por offset, y ordenarla aqui seria reordenar algo que viene del
    // disco. Con dieciseis de tope son 120 comparaciones de dos enteros.
    for a in rev.secciones() {
        if a.kind == BSS || a.file_size == 0 {
            continue;
        }
        let fa = a.file_offset.saturating_add(a.file_size);
        for b in rev.secciones().skip(a.indice + 1) {
            if b.kind == BSS || b.file_size == 0 {
                continue;
            }
            let fb = b.file_offset.saturating_add(b.file_size);
            if a.file_offset < fb && b.file_offset < fa {
                return Err(Falta::SeccionesSeSolapan);
            }
        }
    }

    if !hay_codigo {
        return Err(Falta::SinCodigo);
    }
    if entry_offset >= tam_codigo {
        return Err(Falta::EntryFueraDelCodigo);
    }
    // ** LA MENTIRA MAS BARATA DE CONTAR: un bit puesto a mano en un binario
    // cualquiera lo hace parecer avalado por alguien. Se rechaza el fichero
    // entero en vez de bajarle el nivel -- uno que miente sobre su propia
    // identidad no es un extranjero, es uno que se hace pasar por otra cosa.
    if flags & FLAG_FIRMADO != 0 && !hay_firma {
        return Err(Falta::CabeceraQueSeDesmiente);
    }

    Ok(rev)
}

// -- Lectores acotados -------------------------------------------------------
//
// Los bytes vienen del disco, asi que **nada se indexa sin comprobar**. Un
// `bytes[o+3]` en un lector de formato es un panic en Ring 0 esperando a un
// fichero mal escrito -- y este crate lo compila `#![forbid(unsafe_code)]` para
// que eso no sea una promesa sino una imposibilidad.

fn u16_en(b: &[u8], o: usize) -> Option<u16> {
    Some(u16::from_le_bytes(b.get(o..o.checked_add(2)?)?.try_into().ok()?))
}
fn u32_en(b: &[u8], o: usize) -> Option<u32> {
    Some(u32::from_le_bytes(b.get(o..o.checked_add(4)?)?.try_into().ok()?))
}
fn u64_en(b: &[u8], o: usize) -> Option<u64> {
    Some(u64::from_le_bytes(b.get(o..o.checked_add(8)?)?.try_into().ok()?))
}

#[cfg(test)]
mod tests;

// == ** LA DISPOSICION DEL FORMATO, EN UN SOLO SITIO =========================
//
// # El agujero que esto cierra
//
// El BEF tiene **DOS lectores y ningun compilador entre ellos**:
//
// ```text
//    bmo-abi/bef/*.rs        structs con `repr(C)`, para el toolchain
//    kernel/task/bex.rs      bytes a mano, porque el kernel NO importa bmo-abi
// ```
//
// Y no importarlo es una decision correcta --`bmo-abi` trae `alloc` y el kernel
// no puede-- pero tiene un precio que hasta hoy nadie pagaba: **los offsets
// estaban escritos dos veces**, una como campos de un struct y otra como
// literales dentro de `leer_reloc`. El propio comentario del kernel lo decia:
//
// > *"Tamano y disposicion fijados por `bmo_abi::bef::relocations::Relocation`,
// >  que este kernel no importa a proposito... si el struct cambiara de forma,
// >  estos offsets son el unico sitio a tocar."*
//
// "El unico sitio a tocar" **es la definicion de una duplicacion que se olvida**.
// Mover un campo del struct compila igual, pasa todos los tests del toolchain, y
// el cargador escribe la direccion equivocada dentro de un proceso.
//
// # La salida: no vigilar la copia, QUITARLA
//
// Este crate ya lo comparten los dos --el kernel lo importa para la puerta, y
// `bmo-verify` para no separarse de el-- y no tiene dependencias. Asi que los
// offsets viven aqui, los usa el kernel, y `bmo-abi` los CLAVA a su struct con
// `offset_of!` en una prueba.
//
// De dos verdades que hay que mantener a mano se pasa a una verdad y una prueba
// que la ata. Es el mismo movimiento que el guardian de `bmo.h`, salvo que alli
// los nombres no se podian unificar y aqui si.

/// Bytes que ocupa una relocation. Espejo de `size_of::<Relocation>()`.
pub const RELOC_SIZE: usize = 24;

/// Offsets dentro de una relocation, en el orden en que estan.
///
/// [!] `SYMBOL_IDX` es de 32 bits en el formato y el cargador solo usa su byte
/// bajo: el indice de seccion cabe de sobra. Leer los cuatro y truncar es lo
/// correcto -- leer solo el byte funcionaria hoy y se rompeia el dia que el
/// formato use el resto del campo.
pub mod reloc {
    /// `offset`: donde se escribe, dentro de su seccion. `u64`.
    pub const OFFSET: usize = 0;
    /// `symbol_idx`: en el `SeccionAbs64` es la SECCION del destino. `u32`.
    pub const SYMBOL_IDX: usize = 8;
    /// `kind`: que clase de relocation es. `u8`.
    pub const KIND: usize = 12;
    /// `target_section`: en que seccion se escribe. `u8`.
    ///
    /// [!] Su numeracion **NO es la de `SectionKind`**: aqui code/data/rodata
    /// son 0/1/2 y alli 1/3/2. Cruzar las dos tablas acierta en rodata y falla
    /// en las otras dos, o sea que parece funcionar a medias.
    pub const TARGET_SECTION: usize = 13;
    /// `addend`: offset del destino dentro de su seccion. `i64` con signo.
    pub const ADDEND: usize = 16;
}

/// **CABE ESTA RELOCATION DENTRO DE LA SECCION QUE DICE PARCHEAR?**
///
/// # Por que esta regla vive AQUI y no en el cargador (2026-08-25)
///
/// El toolchain juzga un `.bex` con DOS capas --`revisar()` y
/// `bmo_abi::bef::validator`-- y el cargador del kernel solo con la primera.
/// Esta comprobacion vivia unicamente en la segunda, o sea que **un `.bex`
/// copiado a mano al FAT32 entraba sin que nadie mirara sus relocations**.
///
/// La respuesta obvia era anadirle la comprobacion al kernel. Es la
/// equivocada: serian **dos copias de la misma decision**, que es exactamente
/// el problema que `bmo-bex-gate` se creo el 2026-08-10 para terminar.
///
/// ```text
///    la REGLA        vive aqui, una vez, sin alloc y sin dependencias
///    los DATOS       los pone cada llamante, porque cada uno tiene otros
/// ```
///
/// [!] Y hacen falta los dos, porque `revisar()` **no puede** hacerlo: en el
/// kernel recibe solo el PROLOGO del fichero, y la tabla de relocations vive
/// mucho mas alla --la de DOOM son 30.840 bytes al final--. No es que no se
/// quisiera: es que ahi todavia no estan esos bytes. Por eso la regla es una
/// funcion suelta y no una linea mas dentro de `revisar`.
///
/// # Que pasa si no se comprueba
///
/// Las secciones se colocan **seguidas** desde `USER_IMAGE_BASE`, asi que un
/// `offset` mas grande que su seccion no se sale de la imagen: **cae en la
/// SIGUIENTE**. El cargador comprueba que el destino este dentro de la pagina
/// que esta parcheando --lo esta-- y escribe.
///
/// > Una reloc que dice `.data + 0x9000` en una `.data` de 0x400 no falla:
/// > **acierta en otra seccion.** Y como el hash de cada seccion se cierra
/// > ANTES de parchear, tampoco lo caza el hash.
///
/// No es una fuga fuera del proceso --el marco es suyo-- pero si es un
/// programa que se corrompe a si mismo en silencio, que es la clase de fallo
/// que tarda semanas en atribuirse.
///
/// # Los parametros, y por que `mem` y `fichero` son dos
///
/// Una `.bss` ocupa en memoria y no en el fichero, y una `.data` con relleno
/// tiene `mem_size > file_size`. Se parchea sobre lo que hay **en memoria**,
/// asi que manda `mem`; se pasa `fichero` porque una seccion cuyo `mem` fuera
/// menor ya seria invalida y aqui se ve gratis.
///
/// `parche` son los bytes que la relocation escribe: 8 para `SeccionAbs64`.
pub fn reloc_cabe(offset: u64, parche: u64, fichero: u64, mem: u64) -> bool {
    // El tope es el mayor de los dos: `validator` lo hace asi desde el
    // principio y aqui se conserva el mismo criterio A PROPOSITO -- dos jueces
    // que dan veredictos distintos sobre el mismo fichero son peor que uno.
    let tope = if mem > fichero { mem } else { fichero };
    match offset.checked_add(parche) {
        // ** El desbordamiento es un NO, no un panico. `offset` viene del
        // fichero, o sea de fuera: `u64::MAX` es un valor que alguien puede
        // escribir, y en `release` un `+` normal daria la vuelta y diria que si.
        None => false,
        Some(fin) => fin <= tope,
    }
}

/// Bytes que ocupa una entrada de la tabla de secciones.
pub const SECTION_ENTRY_SIZE: usize = 48;

/// Offsets dentro de una entrada de la tabla de secciones.
pub mod seccion {
    /// `kind`: `SectionKind as u8`.
    pub const KIND: usize = 0;
    /// `flags`. `u32`.
    pub const FLAGS: usize = 4;
    /// `file_offset`: donde estan sus bytes en el fichero. `u64`.
    pub const FILE_OFFSET: usize = 8;
    /// `file_size`: cuantos hay. **`0` es legal si la seccion es `Bss`.**
    pub const FILE_SIZE: usize = 16;
    /// `mem_size`: cuanto ocupa ya cargada. Nunca menor que `file_size`.
    pub const MEM_SIZE: usize = 24;
    /// `virt_addr`: donde la quiere el fichero. `0` = decide el cargador.
    pub const VIRT_ADDR: usize = 32;
    /// `alignment`: potencia de dos. `u16`.
    pub const ALIGNMENT: usize = 40;
    /// `hash_index`: que seccion `Signature` lleva su digest, o `0xFFFF`.
    pub const HASH_INDEX: usize = 42;
}

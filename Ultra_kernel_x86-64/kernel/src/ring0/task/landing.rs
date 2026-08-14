//! **EL CIERRE DE UNA SECCION AL ATERRIZAR.**
//!
//! Cada `.bex` trae un BLAKE3 por seccion. Hasta hoy se comprobaban todos de una
//! pasada, dentro de `bex::inspect`, sobre el bufer entero de la imagen. Esto lo
//! parte en el momento correcto: **cada seccion se cierra con su hash cuando
//! termina de aterrizar, y antes de mapearse.**
//!
//! ## Por que el momento importa tanto
//!
//! Comprobar el bufer contesta *"lo que lei cuadra con lo que se escribio"*.
//! Comprobar al aterrizar contesta *"lo que este proceso va a EJECUTAR cuadra
//! con lo que se escribio"*, que es una pregunta estrictamente mas fuerte: entre
//! las dos hay una copia, y una copia es un sitio donde las cosas se rompen.
//!
//! Y cambia lo que se puede decir cuando falla:
//!
//! ```text
//!   antes    FAULT proc: cabecera invalida (magic, version o 0 secciones)
//!   ahora    FAULT proc: la seccion Code no cuadra con su hash
//! ```
//!
//! Lo primero manda a mirar el FORMATO. Lo segundo dice que el formato estaba
//! bien y que lo que fallo fue el TRANSPORTE, en que seccion, y cuantos bytes
//! llegaron. Son dos sitios que no se parecen en nada, y el 2026-08-10 se
//! perdio una tanda de fotos justo en esa distincion.
//!
//! ## Y por eso esto no es una linea de depuracion
//!
//! > **Una medida que hay que anadir para diagnosticar es una medida que no
//! > existe el dia que hace falta.**
//!
//! El plan del 08-10 era anadir un volcado de los primeros bytes, mirarlo en una
//! foto y quitarlo. Esto lo sustituye: el sistema lo dice **siempre**, en el
//! sitio exacto, sin que nadie lo pida y sin que haya que reconstruir el kernel
//! para preguntarlo.
//!
//! ## Lo que esto NO garantiza, dicho
//!
//! Un `.bex` **sin** seccion de firma pasa entero. Las imagenes que el kernel
//! embebe no van por el escritor y nunca prometieron un hash; exigirle una
//! prueba a quien no la prometio seria dejar de arrancar. Lo que no se puede es
//! fingir que se comprobo algo que no se ha llegado a mirar -- por eso
//! [`Aterrizaje::cerrar`] distingue *"cuadra"* de *"no habia con que
//! comparar"*, y quien lo llama puede contarlas por separado.
//!
//! ## Y por que vive en su propio modulo
//!
//! Porque el que viene detras es el escalon en el que el disco escribe **en las
//! paginas del proceso** y desaparece el bufer de 4 MiB. Cuando eso pase, lo
//! unico que cambia es de donde salen los trozos: el codigo que los cuenta y los
//! cierra es este, sin tocar. Ver `docs/EL_CONTRATO_DE_CARGA.md`, pieza B.

use super::bex::{self, BexError};

/// Bytes de un digest BLAKE3.
pub const DIGEST: usize = 32;

/// El nombre de una seccion, para decirlo en voz alta.
///
/// Un indice no sirve: *"la seccion 3 no cuadra"* obliga a abrir el fichero con
/// otra herramienta para saber cual era, y eso es justo lo que no se puede hacer
/// con una foto de una pantalla.
pub fn nombre(kind: u8) -> &'static str {
    match kind {
        bex::SECTION_CODE => "Code",
        bex::SECTION_RODATA => "RoData",
        bex::SECTION_DATA => "Data",
        bex::SECTION_BSS => "Bss",
        bex::SECTION_RELOCS => "Relocs",
        bex::SECTION_SIGNATURE => "Signature",
        _ => "(desconocida)",
    }
}

/// Como termino el cierre de una seccion.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Cierre {
    /// Los bytes que aterrizaron son los que se escribieron.
    Cuadra,
    /// La imagen no declara hash para esta seccion. **No es un fallo y no es un
    /// exito**: es que no habia con que comparar, y se cuenta aparte para poder
    /// decir cuanta de la imagen quedo de verdad cubierta.
    SinFirma,
}

/// **Una seccion aterrizando.** Se alimenta con los trozos segun llegan y se
/// cierra cuando termina.
///
/// El orden importa: BLAKE3 es un hash de una tira de bytes, asi que los trozos
/// tienen que entrar **en orden de fichero**. Es la misma condicion que ya
/// cumple el bucle que copia pagina a pagina, y se dice aqui para que nadie
/// reordene ese bucle sin enterarse.
pub struct Aterrizaje {
    kind: u8,
    hasher: bmo_hash::Hasher,
    esperado: Option<[u8; DIGEST]>,
    vistos: u64,
}

impl Aterrizaje {
    /// Abre el cierre de una seccion. `esperado` es su digest declarado, o
    /// `None` si la imagen no trae firma para ella.
    pub fn abrir(kind: u8, esperado: Option<[u8; DIGEST]>) -> Self {
        Self { kind, hasher: bmo_hash::Hasher::new(), esperado, vistos: 0 }
    }

    /// Un trozo que acaba de aterrizar. Se le pasa **lo que se copio**, no la
    /// pagina entera: el relleno de ceros del final de una seccion no es parte
    /// de ella y meterlo cambiaria el hash de toda imagen que no acabe en
    /// frontera de pagina -- o sea de casi todas.
    pub fn trozo(&mut self, d: &[u8]) {
        if d.is_empty() {
            return;
        }
        self.hasher.update(d);
        self.vistos += d.len() as u64;
    }

    /// Cuantos bytes han pasado por aqui. Es el numero que sale en el fallo:
    /// dice si el problema fue que llego OTRA COSA o que llego DE MENOS.
    pub fn vistos(&self) -> u64 {
        self.vistos
    }

    /// **Cierra y compara.** Lo dice en CABINA cuando no cuadra, con el nombre
    /// de la seccion y los bytes que se vieron.
    ///
    /// Se dice aqui y no en quien llama a proposito: este es el unico punto del
    /// sistema que tiene a la vez el nombre de la seccion, el digest esperado y
    /// la cuenta. Subirlo a quien llama seria pasarle tres cosas para que
    /// escriba la misma linea, y la linea acabaria escribiendose de dos formas.
    pub fn cerrar(self) -> Result<Cierre, BexError> {
        let esperado = match self.esperado {
            Some(e) => e,
            None => return Ok(Cierre::SinFirma),
        };
        let calculado = self.hasher.finalize();
        if calculado != esperado {
            crate::ring0::cabina::fault(
                "proc",
                match self.kind {
                    bex::SECTION_CODE => "la seccion Code no cuadra con su hash",
                    bex::SECTION_RODATA => "la seccion RoData no cuadra con su hash",
                    bex::SECTION_DATA => "la seccion Data no cuadra con su hash",
                    bex::SECTION_BSS => "la seccion Bss no cuadra con su hash",
                    bex::SECTION_RELOCS => "la seccion Relocs no cuadra con su hash",
                    _ => "una seccion no cuadra con su hash",
                },
                self.vistos,
            );
            return Err(BexError::HashNoCuadra);
        }
        Ok(Cierre::Cuadra)
    }
}

/// **Los digests declarados por la imagen**, ya localizados.
///
/// Se saca UNA vez por imagen y despues se le pregunta por indice de seccion.
/// Guarda offsets, no copias: los 32 bytes de cada digest se sacan cuando hacen
/// falta, que es una vez por seccion.
///
/// [!] Toma prestados los bytes donde vive la seccion `Signature`. Hoy es el
/// bufer de la imagen; cuando llegue la pieza B sera un trozo pequeno leido
/// aparte, y **esta estructura no cambia** -- por eso recibe el rango de la
/// firma y no la imagen entera.
pub struct Firmas<'a> {
    /// Los bytes de la seccion `Signature`, desde su primer byte.
    firma: &'a [u8],
    /// Cuantas entries declara su cabecera.
    cuantos: usize,
    /// El indice de la propia seccion de firma: no puede contener su hash, y el
    /// escritor la excluye. Se guarda para excluirla tambien al leer.
    propio: usize,
}

impl<'a> Firmas<'a> {
    /// Cabecera de la seccion de firma: `hash_count` (u32) + `sig_algo` (u32).
    const CAB: usize = 8;
    /// Una entrada: `section_index` (u16) + relleno (6) + digest (32).
    const ENTRADA: usize = 40;

    /// Abre la tabla de digests sobre los bytes de la seccion `Signature`.
    ///
    /// `None` cuando no hay firma o cuando su cabecera no cuadra con lo que
    /// mide -- y las dos cosas se tratan igual a proposito: una tabla que
    /// promete mas entries de las que caben no es una imagen sin firma, pero
    /// **tampoco es una con la que se pueda comprobar nada**, y arrancar la
    /// comprobacion sobre ella seria leer bytes de la seccion siguiente.
    pub fn abrir(firma: &'a [u8], indice_propio: usize) -> Option<Self> {
        if firma.len() < Self::CAB {
            return None;
        }
        let cuantos = u32::from_le_bytes(firma.get(0..4)?.try_into().ok()?) as usize;
        let necesita = Self::CAB.checked_add(cuantos.checked_mul(Self::ENTRADA)?)?;
        if necesita > firma.len() {
            return None;
        }
        Some(Self { firma, cuantos, propio: indice_propio })
    }

    /// El digest declarado para la seccion `idx`, si lo hay.
    pub fn digest_de(&self, idx: usize) -> Option<[u8; DIGEST]> {
        if idx == self.propio {
            return None;
        }
        for k in 0..self.cuantos {
            let e = Self::CAB + k * Self::ENTRADA;
            let quien = u16::from_le_bytes(self.firma.get(e..e + 2)?.try_into().ok()?) as usize;
            if quien != idx {
                continue;
            }
            let mut d = [0u8; DIGEST];
            d.copy_from_slice(self.firma.get(e + 8..e + 8 + DIGEST)?);
            return Some(d);
        }
        None
    }

    /// Cuantos digests declara. Se usa para poder decir cuanta de la imagen
    /// quedo cubierta, que es distinto de decir que "verifico".
    pub fn cuantos(&self) -> usize {
        self.cuantos
    }
}

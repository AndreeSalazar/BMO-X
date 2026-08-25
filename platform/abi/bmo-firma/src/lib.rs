//! **DE QUIEN es este `.bex`.** La otra mitad del gate.
//!
//! generacion: abuelo -- solo depende de la aritmetica
//!
//! # *** LA PREGUNTA QUE ESTE CRATE EXISTE PARA HACER
//!
//! El arbol ya sabia distinguir las dos preguntas. Lo tiene escrito el
//! validador, palabra por palabra:
//!
//! > *"lo que lleva por defecto son **hashes**, que contestan `llego lo que se
//! > escribio`. Eso es INTEGRIDAD. `SIGNED` promete otra cosa: **AUTORIA**."*
//!
//! Y hasta el 2026-08-25 el cargador solo sabia contestar la primera:
//!
//! ```text
//!    BLAKE3 por seccion    llego lo que se escribio        [X] desde siempre
//!    :firma de ESTRATOS    el nodo trae su hash            [X] desde el 18-08
//!    Ed25519               **lo firmo QUIEN YO DIGO**      <- esto
//! ```
//!
//! # ** Y AQUI ESTA LA TRAMPA QUE HACE FALTA VER ANTES DE ESCRIBIR NADA
//!
//! El formato guarda la firma asi:
//!
//! ```text
//!    Ed25519Signature = sig[64] || pubkey[32]
//! ```
//!
//! **La clave publica viaja DENTRO de la firma.** O sea que comprobar la firma
//! contra esa clave siempre da que si -- porque quien firmo eligio las dos
//! cosas. Cualquiera genera un par de claves, firma el binario, y mete su clave
//! al lado.
//!
//! > Una firma que trae su propia clave demuestra que **nadie la ha tocado
//! > desde que se firmo**. No demuestra **quien firmo**. Y confundir las dos es
//! > tener un control que se pasa solo.
//!
//! *** Es la MISMA forma de C1, tres veces ya: una comprobacion que corre, que
//! da verde, y que no protege de nada.
//!
//! ```text
//!    C1 (24-08)   `verify_ed25519` decia SI a una firma de ceros
//!    C3 (25-08)   la firma de ceros PASABA otra vez, por matematicas
//!    aqui         la firma cuadra... con la clave que trajo el firmante
//! ```
//!
//! ## Por eso este crate pide un ANCLA, y sin ella no da nada por bueno
//!
//! `examinar` recibe **las claves en las que el sistema confia** y comprueba que
//! la del fichero sea una de ellas. Con un ancla vacia, **toda firma es de un
//! desconocido** -- que es la respuesta correcta cuando no se ha decidido en
//! quien confiar, y no "adelante".
//!
//! * Y el ancla NO vive aqui, que es lo que C1 dejo escrito y vale igual:
//!
//! > *"quien quiera permitir binarios sin firmar lo decide **arriba, en la
//! > politica, donde se ve** -- no dentro del verificador."*
//!
//! # Lo que este crate NO hace
//!
//! - **No hashea.** La cadena de digests la calcula quien llama, porque cada
//!   consumidor la tiene de otra forma: el cargador la va sacando de la seccion
//!   mientras aterriza, y el toolchain la tiene entera en memoria. Es la misma
//!   division que `bmo-bex-gate::reloc_cabe`: **la REGLA aqui, los DATOS alli.**
//! - **No firma.** `bmo-cripto` se coge sin la bandera `firmar`.

#![no_std]
#![forbid(unsafe_code)]

use bmo_cripto::ed25519;

/// `hash_count` (u32) + `sig_algo` (u32).
pub const CABECERA: usize = 8;
/// `section_index` (u16) + relleno (6) + digest (32).
pub const ENTRADA: usize = 40;
/// `sig` (64) + `pubkey` (32).
pub const BLOQUE_FIRMA: usize = 96;

/// Solo hashes: integridad, no autoria.
pub const ALGO_NINGUNO: u32 = 0;
/// Ed25519 sobre la cadena de digests.
pub const ALGO_ED25519: u32 = 1;

/// **Que se sabe de la autoria de este `.bex`.**
///
/// [!] No hay variante que diga *"bueno"* a secas, y es a proposito -- la misma
/// forma que `Firma` en `bmo-abi` estreno el 24-08. La unica que permite
/// ejecutar es [`Veredicto::Firmado`], y **lleva dentro cual de las claves del
/// ancla lo firmo**: quien la reciba puede decirlo por su nombre en vez de
/// contestar un `true` que no distingue a nadie.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Veredicto {
    /// `sig_algo = 0`. La seccion solo trae hashes.
    ///
    /// **No es un fallo**: es lo que trae hoy todo `.bex` del arbol. Dice
    /// *"llego lo que se escribio"* y no dice quien lo escribio.
    SoloIntegridad,
    /// La firma cuadra **y** la clave esta en el ancla. `clave` es su indice.
    Firmado { clave: usize },
    /// La firma no corresponde a estos bytes. O los tocaron, o la clave no es
    /// la que firmo.
    NoCuadra,
    /// *** La firma cuadra **y la clave no la conozco**.
    ///
    /// Es el veredicto que este crate existe para poder dar. Sin el, esto seria
    /// indistinguible de `Firmado` -- y ese es exactamente el agujero.
    AutorDesconocido,
    /// `sig_algo` dice un algoritmo que no se implementa aqui.
    ///
    /// ** Se rechaza en vez de ignorarse. Un `.bex` que declara un algoritmo que
    /// no entiendo puede estar firmado perfectamente **por otro**, y tratarlo
    /// como "sin firma" seria degradarlo en silencio a un control mas flojo.
    AlgoritmoDesconocido(u32),
    /// La seccion no mide lo que su cabecera promete, o le falta el bloque de
    /// firma. No se puede comprobar nada sobre ella.
    SeccionRota,
}

impl Veredicto {
    /// **Puede ejecutarse esto?** Y la respuesta depende de la POLITICA, no de
    /// aqui: `exige_firma` la pone quien llama.
    ///
    /// ```text
    ///    exige_firma = false   SoloIntegridad pasa (es lo de hoy)
    ///    exige_firma = true    solo pasa Firmado
    /// ```
    ///
    /// * Que sea un parametro y no una constante es lo que impide que este crate
    /// tenga una opinion sobre un sistema que no conoce. El kernel puede exigir
    /// firma en ESTRATOS y no en FAT32 --que no PUEDE traerla-- sin que aqui
    /// haya un `if` sobre sistemas de ficheros.
    pub fn permite_ejecutar(self, exige_firma: bool) -> bool {
        match self {
            Veredicto::Firmado { .. } => true,
            Veredicto::SoloIntegridad => !exige_firma,
            _ => false,
        }
    }

    /// Una linea corta para CABINA y para el shell. **Cada una manda a un sitio
    /// distinto**, que es lo que un `false` no puede hacer.
    pub fn motivo(self) -> &'static str {
        match self {
            Veredicto::SoloIntegridad => "solo trae hashes: integridad, no autoria",
            Veredicto::Firmado { .. } => "firmado por una clave conocida",
            Veredicto::NoCuadra => "la firma NO corresponde a estos bytes",
            Veredicto::AutorDesconocido => "firma buena, AUTOR DESCONOCIDO: esa clave no esta en el ancla",
            Veredicto::AlgoritmoDesconocido(_) => "firmado con un algoritmo que este sistema no implementa",
            Veredicto::SeccionRota => "la seccion de firma no mide lo que promete",
        }
    }
}

fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

/// **Donde empieza el bloque de firma dentro de la seccion**, o `None` si la
/// seccion no da para el.
///
/// Va aparte porque lo necesitan los dos lados --el que escribe y el que lee--
/// y porque la cuenta se hace **en `u64`**: `hash_count` viene del fichero, o
/// sea de fuera, y `8 + n*40` con `n` hostil da la vuelta en 32 bits y contesta
/// un desplazamiento pequeno que cae DENTRO de la seccion. Es el mismo fallo
/// que `LasCuentasNoCaben` en la cara que viaja.
pub fn donde_esta_la_firma(seccion: &[u8]) -> Option<usize> {
    let cuantos = u32_en(seccion, 0)? as u64;
    let inicio = CABECERA as u64 + cuantos * ENTRADA as u64;
    let fin = inicio + BLOQUE_FIRMA as u64;
    if fin > seccion.len() as u64 {
        return None;
    }
    Some(inicio as usize)
}

/// **EL GATE.** Que se sabe de la autoria de este `.bex`.
///
/// - `seccion`: los bytes de la seccion `Signature`, desde su primer byte.
/// - `cadena`: el hash de la cadena de digests -- **lo que se firma**. Lo
///   calcula quien llama; ver la cabecera.
/// - `ancla`: las claves publicas en las que el sistema confia. **Vacio
///   significa que no se confia en nadie**, no que valga cualquiera.
///
/// # El orden, y por que es este
///
/// ```text
///    1. la seccion mide lo que promete    o no hay nada que mirar
///    2. sig_algo                          0 -> integridad y se acabo
///    3. la firma cuadra con la cadena     la aritmetica
///    4. *** la clave esta en el ANCLA     la pregunta de verdad
/// ```
///
/// ** El 4 va DESPUES del 3 a proposito, aunque comparar 32 bytes sea mas barato
/// que verificar una firma. Si fuera antes, una clave desconocida saldria como
/// desconocida **aunque su firma tampoco cuadrara** -- y entonces el motivo
/// mandaria a revisar en quien se confia cuando el fichero esta roto.
///
/// > **Primero si los bytes son los que se firmaron. Despues, de quien.**
pub fn examinar(seccion: &[u8], cadena: &[u8; 32], ancla: &[[u8; 32]]) -> Veredicto {
    if seccion.len() < CABECERA {
        return Veredicto::SeccionRota;
    }
    let algo = match u32_en(seccion, 4) {
        Some(a) => a,
        None => return Veredicto::SeccionRota,
    };
    if algo == ALGO_NINGUNO {
        return Veredicto::SoloIntegridad;
    }
    if algo != ALGO_ED25519 {
        return Veredicto::AlgoritmoDesconocido(algo);
    }

    let Some(off) = donde_esta_la_firma(seccion) else {
        return Veredicto::SeccionRota;
    };
    let mut sig = [0u8; ed25519::FIRMA];
    let mut pk = [0u8; ed25519::CLAVE];
    sig.copy_from_slice(&seccion[off..off + 64]);
    pk.copy_from_slice(&seccion[off + 64..off + 96]);

    if !ed25519::verificar(&pk, &cadena[..], &sig) {
        return Veredicto::NoCuadra;
    }

    // *** Y AQUI ES DONDE ESTO DEJA DE SER UNA COMPROBACION QUE SE PASA SOLA.
    //
    // La firma cuadra con la clave que traia el fichero, que es lo unico que la
    // aritmetica puede decir. Lo que decide si eso vale algo es si esa clave la
    // conoce el sistema.
    for (i, k) in ancla.iter().enumerate() {
        if *k == pk {
            return Veredicto::Firmado { clave: i };
        }
    }
    Veredicto::AutorDesconocido
}

#[cfg(test)]
mod pruebas;

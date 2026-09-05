//! Firma + integridad de binarios BEF.
//!
//! Esquema BEF:
//!   - Hash por seccion: BLAKE3 256-bit.
//!   - Firma del archivo entero: Ed25519 sobre el conjunto de hashes.
//!   - Claves publicas confiables en /system/trust/*.pub.
//!
//! ## Cadena de confianza
//!   - Cada BEF cargado -> BLAKE3 -> apuntado en la bitacora de CABINA
//!   - Arranque: kernel -> compositor -> app, cada uno con su suma
//!
//! *(La version anterior de esta nota citaba `timeback` para el journal y el
//! rollback. Ese crate se borro el 2026-08-02 por llevar seis meses sin un solo
//! dependiente -- ESTRATOS hace ese trabajo, y mejor: copy-on-write donde nada
//! se sobreescribe, asi que el "snapshot valido anterior" es el superbloque de
//! la generacion anterior y no hay que journalizar nada aparte.)*
//!
//! =======================================================================
//! * DISENO PENDIENTE: firma del vendedor + licencia por dueno
//! =======================================================================
//!
//! > **Estado: IDEA, no implementada.** Escrita aqui el 2026-08-02 porque es
//! > una decision de producto tanto como tecnica, y porque el esqueleto de
//! > abajo es exactamente donde va.
//!
//! ### Lo que hay hoy, dicho sin adornos
//!
//! Lo que BMO-X comprueba antes de ejecutar es **un BLAKE3 del contenido**.
//! Eso prueba **integridad** --que el fichero no se corrompio ni se toco-- y
//! **no prueba autoria**: quien pueda escribir en el volumen recalcula la suma
//! y ya. Es un checksum, no una firma.
//!
//! ### La idea
//!
//! Dos firmas, cada una contestando una pregunta distinta:
//!
//! ```text
//!   1. FIRMA DEL VENDEDOR   Ed25519 sobre los hashes de seccion
//!      -> contesta: "esto salio de MI, y nadie lo ha tocado"
//!
//!   2. LICENCIA POR DUENO   objeto firmado por el vendedor que nombra
//!                           la clave publica del comprador
//!      -> contesta: "esta copia se emitio para ESTE dueno"
//! ```
//!
//! Cada licencia es unica y no se repite --es un par de claves, como en
//! Bitcoin-- y **el dueno se queda con la suya**. Nadie se la puede revocar,
//! porque la clave la tiene el.
//!
//! ### * Lo que esto SI hace, y es mas de lo que parece
//!
//! - **Procedencia**: este binario salio de ese vendedor, sin tocar. Cubre la
//!   cadena de suministro entera, que es hoy una preocupacion real y creciente.
//! - **Atribucion**: esta copia se emitio a este dueno. Es un recibo firmado,
//!   no repudiable por ninguna de las dos partes.
//! - **Imposibilidad de SUPLANTAR**: un binario modificado no puede seguir
//!   diciendo que es del vendedor. Puede correr --si el dueno quiere-- pero
//!   corre como lo que es: un programa sin firma.
//! - **Coste cero en ejecucion**: se verifica **una vez al cargar**. No hay
//!   nada corriendo durante la partida.
//!
//! ### * Lo que esto NO hace, y es estructural
//!
//! **No impide copiar, y no puede.** Impedirlo exigiria que la maquina
//! guardase un secreto **de su propio dueno**, y BMO-X es incapaz de eso por
//! construccion: el dueno lee el log de Ring 0 con F11 y CABINA lo confiesa
//! todo. No es una carencia que tapar -- es la tesis del sistema.
//!
//! La analogia con Bitcoin aguanta en las claves y **se rompe aqui**: la
//! seguridad de Bitcoin es que una RED se pone de acuerdo en que una moneda no
//! se gasta dos veces. Aqui no hay red. Una maquina local no puede impedir una
//! copia local.
//!
//! Asi que esto es un **recibo notarizado, no un candado**. Y hay que venderlo
//! como lo que es: si se promete lo otro, se esta prometiendo Denuvo, que es
//! justo lo que este sistema no puede ni quiere hacer.
//!
//! ### Por que encaja con la licencia
//!
//! [!] Este parrafo ha cambiado de premisa DOS VECES en tres dias -- Techne
//! v2.0 (royalty del 7%), Simbiosis v1.0 (trueque), y desde el 2026-09-06
//! **Apache-2.0**. Que haya sobrevivido a las tres dice algo util: lo que
//! este modulo necesitaba de la licencia no era ninguna de ellas.
//!
//! Lo que hace falta es que el modelo sea de **buena fe con auditoria** y no
//! de prevencion. Nunca dependio de impedir copias --Apache-2.0 las permite
//! todas explicitamente-- sino de que se pueda **demostrar** que corre y de
//! quien es. Bajo Apache eso no se debilita: se vuelve lo unico que queda.
//! Cualquiera puede bifurcar este arbol y firmar sus binarios con SU clave;
//! lo que no puede es firmarlos con la del dueno. La firma dejo de ser un
//! instrumento de cobro y se quedo con el oficio que siempre tuvo -- decir la
//! verdad sobre un binario. Ver `LICENSE` y `NOTICE`.
//!
//! Esto hace real esa parte. Y el comprador que mas lo paga no es el que teme
//! la pirateria: es el banco o el organismo publico que necesita decir *"puedo
//! demostrar exactamente que se esta ejecutando en esta maquina"*. Que es el
//! objetivo declarado del proyecto.
//!
//! ### La regla que lo mantiene coherente
//!
//! > **Quien decide en que claves se confia es el DUENO.**
//!
//! Puede anadir la del vendedor, la suya, o ninguna. Un sistema donde la lista
//! de confianza la fija el fabricante es exactamente el modelo del firmware
//! firmado de una GPU: el fabricante manteniendo el control **frente al**
//! dueno. La direccion contraria a esta.
//!
//! ### Lo que falta para implementarlo
//!
//! 1. **Ed25519 de verdad** -- hoy [`SigAlgorithm`] lo nombra y nada lo hace.
//! 2. **Donde vive el llavero**: `/system/trust/*.pub` esta escrito arriba y no
//!    existe. Con ESTRATOS montado, un objeto del volumen es mejor sitio que un
//!    fichero suelto -- y su historia queda en el grafo.
//! 3. **Revocacion**: poder decir "esta clave ya no vale", y que eso tambien
//!    sea una decision del dueno.
//! 4. **El objeto licencia**: su formato, y si es transferible. Que lo sea o no
//!    es politica del vendedor, no del sistema -- y el sistema debe poder
//!    expresar las dos.
//!
//! ### * El factor fisico (USB), y la trampa que tiene
//!
//! La idea de un aparato USB como segundo factor es correcta, pero **hay dos
//! versiones y solo una vale**:
//!
//! ```text
//!   HASH GUARDADO EN EL USB   el sistema LEE un secreto del aparato
//!                             -> quien lea el USB lo copia. Es una
//!                                contrasena en un palito.
//!
//!   EL USB FIRMA UN RETO      el sistema manda un numero aleatorio y el
//!                             aparato devuelve una FIRMA. La clave privada
//!                             NUNCA sale.
//!                             -> copiar el trafico no sirve de nada.
//! ```
//!
//! La segunda es la que usan los bancos, y en BMO-X necesita un driver de la
//! clase **CCID** de USB -- otra clase distinta de HID, que es la que hay hoy.
//! Mientras eso no exista, un USB de almacenamiento con un fichero de clave es
//! el intermedio honesto: **es "algo que tienes", y es copiable**. Sirve, y hay
//! que llamarlo por su nombre en vez de venderlo como lo otro.
//!
//! ### * Y lo que de verdad vende esto no es la firma
//!
//! Es la **jerarquia**, y en BMO-X no es una funcionalidad: es la forma del
//! sistema. Un cajero no es que no tenga *permiso* para autorizar una
//! transferencia grande -- **no tiene el handle**, asi que la operacion no le
//! existe. No hay comprobacion que saltarse porque no hay comprobacion.
//!
//! Sobre eso, las practicas que un banco ya tiene se escriben solas: cuatro
//! ojos son **dos handles en dos procesos**, la segregacion de funciones es
//! que **ninguno tenga los dos**, y la pista de auditoria inmutable **ya
//! existe** -- ESTRATOS no sobreescribe nada, asi que el auditor no tiene que
//! fiarse de que nadie borro: desciende por los estratos y lo ve.
//!
//! Ver el README, seccion *"Why a capability system is what a bank actually
//! wants"*, donde esta el argumento entero.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u8};

/// Hash BLAKE3 256-bit de una seccion.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SectionHash {
    pub section_index: bx_u16,
    pub _pad: [bx_u8; 6],
    pub digest: [bx_u8; 32],
}
const _: () = assert!(core::mem::size_of::<SectionHash>() == 40);

impl SectionHash {
    pub const SIZE: usize = 40;
    pub const ZERO: Self = Self {
        section_index: 0xFFFF,
        _pad: [0; 6],
        digest: [0; 32],
    };
}

/// Algoritmo de firma soportado.
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigAlgorithm {
    None = 0,
    Ed25519 = 1,
}

/// Cabecera de la seccion Signature.
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct SignatureHeader {
    pub hash_count: bx_u32,
    pub sig_algo: bx_u32,
}
const _: () = assert!(core::mem::size_of::<SignatureHeader>() == 8);

/// Firma Ed25519 completa (64 bytes signature + 32 bytes public key).
#[repr(C, align(8))]
#[derive(Debug, Clone, Copy)]
pub struct Ed25519Signature {
    /// Ed25519 signature (R || S), 64 bytes.
    pub sig: [bx_u8; 64],
    /// Ed25519 public key, 32 bytes.
    pub pubkey: [bx_u8; 32],
}
const _: () = assert!(core::mem::size_of::<Ed25519Signature>() == 96);

impl SignatureHeader {
    pub const SIGNATURE_SIZE: u32 = 96; // Ed25519 sig(64) + pubkey(32)
}

/// Hash BLAKE3 256-bit del buffer indicado.
pub fn blake3_256(bytes: &[u8]) -> [u8; 32] {
    crate::bef::blake3::hash(bytes)
}

/// Verifica que un hash precomputado coincida con los bytes provistos.
pub fn verify(expected: &SectionHash, bytes: &[u8]) -> bool {
    let computed = blake3_256(bytes);
    &computed[..] == &expected.digest[..]
}

/// Compute a chain-of-trust hash for the entire BEF (all section digests combined).
/// Used by TimeBack for boot-time integrity verification.
pub fn chain_hash(hashes: &[SectionHash]) -> [u8; 32] {
    let mut combined = alloc::vec::Vec::with_capacity(hashes.len() * 32);
    for h in hashes {
        combined.extend_from_slice(&h.digest);
    }
    blake3_256(&combined)
}

/// Lo que se sabe de una firma. **No hay variante que diga "verificada"**, y esa
/// ausencia es el punto entero de este tipo.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Firma {
    /// Los 96 bytes son ceros: la imagen viene SIN FIRMAR. No es un fallo --asi
    /// se construye hoy-- pero tampoco es una garantia de nada.
    SinFirmar,
    /// Trae una firma de verdad, y **aqui no se puede comprobar**: no hay
    /// Ed25519 en el arbol todavia. Quien reciba esto tiene que RECHAZAR.
    NoSePuedeComprobar,
}

/// **Que trae esta firma?** Y ojo al nombre: no dice `verify`.
///
/// # *** POR QUE ESTO YA NO DEVUELVE `bool` (2026-08-24)
///
/// Se llamaba `verify_ed25519` y devolvia `true` cuando los 96 bytes eran
/// ceros. O sea que una funcion llamada **verificar** contestaba **si** a la
/// ausencia total de firma:
///
/// ```text
///    if verify_ed25519(&sig, msg) { ejecutar(); }   // <- parece correcto
/// ```
///
/// ** Ese `if` es el ataque de *signature stripping* servido en bandeja: el
/// atacante no rompe la criptografia, **borra la firma**, y el guardia dice que
/// pase. Y lo peor es que la linea se lee bien; nadie la miraria dos veces.
///
/// Hoy no lo llamaba nadie, asi que no habia agujero -- habia una **trampa
/// cargada esperando al primer cliente**. La auditoria del 24-08 la encontro.
///
/// [!] La correccion no es cambiar el `true` por un `false`: es QUITAR EL BOOL.
/// Un booleano no puede distinguir "sin firmar" de "firmada y no comprobada", y
/// esas dos cosas piden decisiones distintas. Con este `enum`, escribir el
/// codigo peligroso obliga a nombrar la variante -- y nadie escribe
/// `if matches!(x, Firma::NoSePuedeComprobar) { ejecutar() }` por descuido.
///
/// > Un guardia que puede decir que si por omision no es un guardia.
pub fn examinar_ed25519(sig: &Ed25519Signature, _message: &[u8]) -> Firma {
    let sin_firmar = sig.sig.iter().all(|&b| b == 0) && sig.pubkey.iter().all(|&b| b == 0);
    if sin_firmar {
        Firma::SinFirmar
    } else {
        // ** Hay Ed25519 que comprobar y no hay con que. SHA-256, HMAC, X25519
        // y AES-GCM ya estan; Ed25519 pide SHA-512 y aritmetica de Edwards, que
        // es el siguiente escalon y todavia no esta escrito.
        Firma::NoSePuedeComprobar
    }
}

//! Firma + integridad de binarios BEF.
//!
//! Esquema BEF:
//!   - Hash por sección: BLAKE3 256-bit.
//!   - Firma del archivo entero: Ed25519 sobre el conjunto de hashes.
//!   - Claves públicas confiables en /system/trust/*.pub.
//!
//! ## Cadena de confianza
//!   - Cada BEF cargado → BLAKE3 → apuntado en la bitácora de CABINA
//!   - Arranque: kernel → compositor → app, cada uno con su suma
//!
//! *(La versión anterior de esta nota citaba `timeback` para el journal y el
//! rollback. Ese crate se borró el 2026-08-02 por llevar seis meses sin un solo
//! dependiente — ESTRATOS hace ese trabajo, y mejor: copy-on-write donde nada
//! se sobreescribe, así que el "snapshot válido anterior" es el superbloque de
//! la generación anterior y no hay que journalizar nada aparte.)*
//!
//! ═══════════════════════════════════════════════════════════════════════
//! ★ DISEÑO PENDIENTE: firma del vendedor + licencia por dueño
//! ═══════════════════════════════════════════════════════════════════════
//!
//! > **Estado: IDEA, no implementada.** Escrita aquí el 2026-08-02 porque es
//! > una decisión de producto tanto como técnica, y porque el esqueleto de
//! > abajo es exactamente donde va.
//!
//! ### Lo que hay hoy, dicho sin adornos
//!
//! Lo que BMO-X comprueba antes de ejecutar es **un BLAKE3 del contenido**.
//! Eso prueba **integridad** —que el fichero no se corrompió ni se tocó— y
//! **no prueba autoría**: quien pueda escribir en el volumen recalcula la suma
//! y ya. Es un checksum, no una firma.
//!
//! ### La idea
//!
//! Dos firmas, cada una contestando una pregunta distinta:
//!
//! ```text
//!   1. FIRMA DEL VENDEDOR   Ed25519 sobre los hashes de sección
//!      -> contesta: "esto salio de MI, y nadie lo ha tocado"
//!
//!   2. LICENCIA POR DUENO   objeto firmado por el vendedor que nombra
//!                           la clave publica del comprador
//!      -> contesta: "esta copia se emitio para ESTE dueno"
//! ```
//!
//! Cada licencia es única y no se repite —es un par de claves, como en
//! Bitcoin— y **el dueño se queda con la suya**. Nadie se la puede revocar,
//! porque la clave la tiene él.
//!
//! ### ★ Lo que esto SÍ hace, y es más de lo que parece
//!
//! - **Procedencia**: este binario salió de ese vendedor, sin tocar. Cubre la
//!   cadena de suministro entera, que es hoy una preocupación real y creciente.
//! - **Atribución**: esta copia se emitió a este dueño. Es un recibo firmado,
//!   no repudiable por ninguna de las dos partes.
//! - **Imposibilidad de SUPLANTAR**: un binario modificado no puede seguir
//!   diciendo que es del vendedor. Puede correr —si el dueño quiere— pero
//!   corre como lo que es: un programa sin firma.
//! - **Coste cero en ejecución**: se verifica **una vez al cargar**. No hay
//!   nada corriendo durante la partida.
//!
//! ### ★ Lo que esto NO hace, y es estructural
//!
//! **No impide copiar, y no puede.** Impedirlo exigiría que la máquina
//! guardase un secreto **de su propio dueño**, y BMO-X es incapaz de eso por
//! construcción: el dueño lee el log de Ring 0 con F11 y CABINA lo confiesa
//! todo. No es una carencia que tapar — es la tesis del sistema.
//!
//! La analogía con Bitcoin aguanta en las claves y **se rompe aquí**: la
//! seguridad de Bitcoin es que una RED se pone de acuerdo en que una moneda no
//! se gasta dos veces. Aquí no hay red. Una máquina local no puede impedir una
//! copia local.
//!
//! Así que esto es un **recibo notarizado, no un candado**. Y hay que venderlo
//! como lo que es: si se promete lo otro, se está prometiendo Denuvo, que es
//! justo lo que este sistema no puede ni quiere hacer.
//!
//! ### Por qué encaja con la licencia Techne
//!
//! Techne v2.0 ya es un modelo de **buena fe con auditoría** —libre para
//! individuos y por debajo de USD 1M/año, comercial por encima—, no de
//! prevención. Nunca dependió de impedir copias: depende de que se pueda
//! **demostrar** qué corre y de quién es.
//!
//! Esto hace real esa parte. Y el comprador que más lo paga no es el que teme
//! la piratería: es el banco o el organismo público que necesita decir *"puedo
//! demostrar exactamente qué se está ejecutando en esta máquina"*. Que es el
//! objetivo declarado del proyecto.
//!
//! ### La regla que lo mantiene coherente
//!
//! > **Quien decide en qué claves se confía es el DUEÑO.**
//!
//! Puede añadir la del vendedor, la suya, o ninguna. Un sistema donde la lista
//! de confianza la fija el fabricante es exactamente el modelo del firmware
//! firmado de una GPU: el fabricante manteniendo el control **frente al**
//! dueño. La dirección contraria a ésta.
//!
//! ### Lo que falta para implementarlo
//!
//! 1. **Ed25519 de verdad** — hoy [`SigAlgorithm`] lo nombra y nada lo hace.
//! 2. **Dónde vive el llavero**: `/system/trust/*.pub` está escrito arriba y no
//!    existe. Con ESTRATOS montado, un objeto del volumen es mejor sitio que un
//!    fichero suelto — y su historia queda en el grafo.
//! 3. **Revocación**: poder decir "esta clave ya no vale", y que eso también
//!    sea una decisión del dueño.
//! 4. **El objeto licencia**: su formato, y si es transferible. Que lo sea o no
//!    es política del vendedor, no del sistema — y el sistema debe poder
//!    expresar las dos.

#![allow(dead_code)]

use crate::bmo_abi::primitives::{bx_u16, bx_u32, bx_u8};

/// Hash BLAKE3 256-bit de una sección.
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

/// Cabecera de la sección Signature.
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

/// Verify an Ed25519 signature. Currently a stub — relies on external
/// ed25519-dalek or similar crate for actual verification.
/// Returns true if sig_algo is None (unsigned binaries are allowed in dev).
pub fn verify_ed25519(_sig: &Ed25519Signature, _message: &[u8]) -> bool {
    // TODO: integrate ed25519-dalek when available
    // In dev, allow unsigned binaries (all-zeros signature).
    // Check if the signature is all zeros (unsigned).
    let is_unsigned = _sig.sig.iter().all(|&b| b == 0) && _sig.pubkey.iter().all(|&b| b == 0);
    if is_unsigned {
        return true;
    }
    // Signed binaries: cannot verify yet, reject.
    false
}

//! **EL FICHERO: donde esta la firma y que es lo que se firma.**
//!
//! [carril]  VERDE     una herramienta del anfitrion: si se equivoca, lo dice
//!                     en la consola antes de tocar nada
//! [cuesta]  NADA      no viaja a la maquina: corre en el anfitrion
//! [riesgo]  ESPEJO    hay DOS lectores del formato de la seccion `Signature`
//!                     --este y `ring0/task/landing.rs`-- y el compilador no
//!                     puede comprobar que coincidan
//!
//! # Que es exactamente lo que se firma
//!
//! No los bytes del `.bex`. **La CADENA**: el BLAKE3 de los digests declarados,
//! concatenados en el orden en que estan guardados.
//!
//! ```text
//!    seccion Signature = [hash_count][sig_algo] [idx pad digest] x n [sig pubkey]
//!                         <--- 8 --->            <---- 40 ---->      <-- 96 -->
//!
//!    cadena = BLAKE3( digest_0 || digest_1 || ... || digest_n-1 )
//! ```
//!
//! ** Y ESO ATA EL CONJUNTO, no cada pieza por separado. Cambiar un byte del
//! codigo rompe su digest, que rompe la cadena, que rompe la firma. **Quitar
//! una seccion entera** tambien: sobra o falta un digest y la cadena es otra.
//!
//! # [!] POR QUE LOS DIGESTS SE LEEN DEL FICHERO Y NO SE RECALCULAN
//!
//! La tentacion era calcular los digests aqui y hashearlos. Habria sido una
//! cadena **parecida** a la del kernel, y parecida no sirve: el kernel hashea
//! los bytes que el fichero DECLARA, en el orden en que estan guardados.
//!
//! Asi que esto hace las dos cosas por separado, y en este orden:
//!
//! ```text
//!    1. COMPROBAR  cada digest declarado contra los bytes reales  -> o NO se firma
//!    2. FIRMAR     la cadena, sobre los digests TAL COMO ESTAN
//! ```
//!
//! *** El 1 es lo que impide firmar un fichero roto. **Una firma sobre un
//! binario corrupto no lo arregla: lo acredita.** Y el 2 hashea exactamente los
//! mismos bytes en el mismo orden que `landing.rs::cadena()`, porque son los
//! mismos bytes: los del fichero.

/// Cabecera de la seccion: `hash_count` (u32) + `sig_algo` (u32).
pub const CAB: usize = 8;
/// Una entrada: `section_index` (u16) + relleno (6) + digest (32).
pub const ENTRADA: usize = 40;
/// `SectionKind::Signature`.
const SECCION_FIRMA: u8 = 0x0F;
/// Una fila de la tabla de secciones.
const FILA: usize = 48;
/// `BefHeader.flags`, y `BefFlags::SIGNED` dentro.
const FLAGS: usize = 8;
const BIT_SIGNED: u32 = 1 << 5;

/// Lo que hay que saber de un `.bex` para firmarlo.
pub struct Bex {
    /// Donde empieza la seccion `Signature` en el fichero.
    pub sec_off: usize,
    /// Cuanto mide.
    pub sec_len: usize,
    /// Su indice en la tabla, que es el que NO lleva digest.
    pub indice: usize,
    /// Cuantos digests declara.
    pub cuantos: usize,
}

fn u32_en(b: &[u8], i: usize) -> Option<u32> {
    Some(u32::from_le_bytes([
        *b.get(i)?,
        *b.get(i + 1)?,
        *b.get(i + 2)?,
        *b.get(i + 3)?,
    ]))
}

fn u64_en(b: &[u8], i: usize) -> Option<u64> {
    let mut v = [0u8; 8];
    v.copy_from_slice(b.get(i..i + 8)?);
    Some(u64::from_le_bytes(v))
}

/// La tabla de secciones: `(offset, cuantas)`.
fn tabla(b: &[u8]) -> Option<(usize, usize)> {
    Some((u64_en(b, 32)? as usize, u32_en(b, 40)? as usize))
}

impl Bex {
    /// Localiza la seccion de firma leyendo la tabla, **como la lee el kernel**.
    ///
    /// * Se lee de la tabla y no se calcula. Una herramienta que se saca por su
    /// cuenta un offset que el fichero ya declara es una herramienta que un dia
    /// mira a otro sitio -- y `abi_layout.rs` tiene una prueba que existe
    /// justamente porque eso ya paso una vez.
    pub fn abrir(b: &[u8]) -> Result<Bex, String> {
        let (t, count) = tabla(b).ok_or("el fichero no llega ni a la tabla de secciones")?;
        for i in 0..count {
            let e = t + i * FILA;
            if *b.get(e).ok_or("la tabla se sale del fichero")? != SECCION_FIRMA {
                continue;
            }
            let sec_off = u64_en(b, e + 8).ok_or("la fila no trae offset")? as usize;
            let sec_len = u64_en(b, e + 16).ok_or("la fila no trae tamano")? as usize;
            if sec_off + sec_len > b.len() {
                return Err("la seccion Signature se sale del fichero".into());
            }
            if sec_len < CAB {
                return Err("la seccion Signature no llega ni a su cabecera".into());
            }
            let cuantos = u32_en(b, sec_off).unwrap() as usize;
            if CAB + cuantos * ENTRADA > sec_len {
                return Err("la seccion promete mas digests de los que caben".into());
            }
            return Ok(Bex { sec_off, sec_len, indice: i, cuantos });
        }
        Err("este .bex no trae seccion Signature".into())
    }

    /// Los bytes de la seccion.
    pub fn seccion<'a>(&self, b: &'a [u8]) -> &'a [u8] {
        &b[self.sec_off..self.sec_off + self.sec_len]
    }

    /// **PASO 1: cada digest declarado contra los bytes reales.**
    ///
    /// Ninguna herramienta de este arbol firma sin pasar por aqui.
    pub fn comprobar(&self, b: &[u8]) -> Result<(), String> {
        let (t, count) = tabla(b).ok_or("sin tabla")?;
        for k in 0..self.cuantos {
            let h = self.sec_off + CAB + k * ENTRADA;
            let idx = u16::from_le_bytes([b[h], b[h + 1]]) as usize;
            if idx == self.indice {
                // La seccion de firma no lleva su propio hash: su contenido son
                // los hashes de las demas.
                continue;
            }
            if idx >= count {
                return Err(format!("un digest apunta a la seccion {idx}, que no existe"));
            }
            let e = t + idx * FILA;
            let off = u64_en(b, e + 8).ok_or("fila sin offset")? as usize;
            let len = u64_en(b, e + 16).ok_or("fila sin tamano")? as usize;
            // Una `Bss` no tiene bytes y su hash es el del vacio. Se comprueba
            // igual: saltarla seria dejar de mirar una fila que si dice algo.
            let datos = if len == 0 { &b[0..0] } else { &b[off..off + len] };
            if bmo_hash::hash(datos)[..] != b[h + 8..h + 40] {
                return Err(format!(
                    "la seccion {idx} NO cuadra con su digest -- este .bex esta \
                     roto o lo tocaron, y firmarlo seria acreditarlo"
                ));
            }
        }
        Ok(())
    }

    /// **PASO 2: la cadena**, sobre los digests tal como estan guardados.
    pub fn cadena(&self, b: &[u8]) -> [u8; 32] {
        let mut h = bmo_hash::Hasher::new();
        for k in 0..self.cuantos {
            let e = self.sec_off + CAB + k * ENTRADA;
            h.update(&b[e + 8..e + 8 + 32]);
        }
        h.finalize()
    }

    /// Donde caen los 96 bytes de la firma, dentro del fichero.
    pub fn hueco(&self, b: &[u8]) -> Result<usize, String> {
        match bmo_firma::donde_esta_la_firma(self.seccion(b)) {
            Some(o) => Ok(self.sec_off + o),
            None => Err("este .bex no reserva sitio para la firma: lo escribio un \
                 toolchain anterior al 2026-09-10. Reconstruyelo."
                .into()),
        }
    }

    /// El `sig_algo` que trae hoy.
    pub fn algo(&self, b: &[u8]) -> u32 {
        u32_en(b, self.sec_off + 4).unwrap_or(0)
    }

    /// **Estampa la firma.** Sin mover un solo byte de lo demas.
    pub fn estampar(&self, b: &mut [u8], sig: &[u8; 64], pk: &[u8; 32]) -> Result<(), String> {
        let o = self.hueco(b)?;
        b[o..o + 64].copy_from_slice(sig);
        b[o + 64..o + 96].copy_from_slice(pk);
        b[self.sec_off + 4..self.sec_off + 8]
            .copy_from_slice(&bmo_firma::ALGO_ED25519.to_le_bytes());
        // ** Y EL FICHERO LO DICE DE SI MISMO. `BefFlags::SIGNED` sin
        // `sig_algo != 0` es un binario mintiendo --lo caza `validator.rs`-- y
        // al reves es un binario que no cuenta lo que trae. Los dos se ponen
        // aqui, en la misma funcion, que es la unica forma de que no se separen.
        let f = u32_en(b, FLAGS).unwrap_or(0) | BIT_SIGNED;
        b[FLAGS..FLAGS + 4].copy_from_slice(&f.to_le_bytes());
        Ok(())
    }
}

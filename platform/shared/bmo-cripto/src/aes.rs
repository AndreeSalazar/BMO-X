//! **AES-128 y AES-256.** Solo cifrar bloques, y eso no es una carencia.
//!
//! # *** LO QUE ESTE FICHERO NO TIENE: DESCIFRAR
//!
//! No hay `descifrar_bloque`, y no falta. **GCM nunca usa la operacion inversa
//! de AES**: cifra un contador y hace `xor` con el texto, asi que descifrar es
//! exactamente el mismo camino con la misma funcion.
//!
//! ** Eso borra la mitad del codigo --InvSubBytes, InvMixColumns, la expansion
//! al reves-- y con ella una familia entera de fallos. Escribir lo que no se usa
//! es escribir lo que nadie prueba.
//!
//! # [!] Y LA DECISION INCOMODA, DICHA EN VOZ ALTA: LA TABLA
//!
//! `SBOX` es una tabla de 256 bytes que se indexa **con datos que dependen de la
//! clave**. Y una tabla indexada por un secreto puede filtrarlo por la CACHE:
//! quien comparta el CPU puede medir que lineas se tocaron y deducir la clave.
//! Es el ataque clasico contra AES por software, y **hay que decir donde estamos
//! respecto a el**:
//!
//! ```text
//!    T-TABLES (4 KiB)   lo que ataca la literatura clasica. AQUI NO SE USAN
//!    S-BOX (256 B)      cabe en cuatro lineas de cache, y con las 16 consultas
//!                       de una ronda se tocan casi todas -> mucho mas dificil
//!    BITSLICED          inmune, y es otro fichero: la logica de AES escrita
//!                       como operaciones de bits sobre 8 registros
//!    AES-NI             el silicio TIENE la instruccion. Inmune y mas rapido
//! ```
//!
//! *** **Esto NO es inmune, y por eso se escribe aqui y no en un `TODO`.** Es
//! aceptable para lo primero que va a cifrar --un canal donde no hay otro
//! proceso midiendo la cache de este-- y **no lo es para una maquina que corra
//! codigo de terceros a la vez**.
//!
//! ** Y la salida ya esta identificada y encaja con la casa: **`aesenc` es una
//! instruccion de esta maquina**, o sea una fila de `arch/x86_64/intrinsics.toml`
//! -- exactamente como entro AVX2. Es la ley 24: el hardware se perfila, y una
//! maquina que tiene la instruccion no deberia estar imitandola.
//!
//! [!] Lo que NO se puede hacer es fingir que el problema no existe. Una
//! criptografia mal escrita funciona y no protege; una escrita con una debilidad
//! CONOCIDA Y ANOTADA se puede decidir, que es distinto.

/// Bytes de un bloque de AES. Son 16 en todas las variantes.
pub const BLOQUE: usize = 16;

/// La caja de sustitucion de AES (FIPS 197).
///
/// ** No es magia: cada byte es el inverso multiplicativo en GF(2^8) seguido de
/// una transformacion afin. Se guarda como tabla porque calcularlo cada vez son
/// unas cuarenta operaciones por byte.
const SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

/// **Multiplicar por 2 en GF(2^8)**, que es la operacion de MixColumns.
///
/// ** Sin `if`: el `(b >> 7)` saca el bit que se va a salir y lo convierte en
/// una mascara. Un `if b & 0x80 != 0` daria lo mismo y **dependeria del dato**,
/// que es justo lo que no se puede hacer aqui.
fn por2(b: u8) -> u8 {
    let alto = 0u8.wrapping_sub(b >> 7);
    (b << 1) ^ (alto & 0x1B)
}

/// Las claves de ronda, ya expandidas.
///
/// ** El array es de 15 rondas --lo que pide AES-256-- y `rondas` dice cuantas
/// valen de verdad. Un tipo por tamano de clave habria duplicado el cifrado
/// entero para cambiar un numero.
pub struct Aes {
    rk: [[u8; BLOQUE]; 15],
    rondas: usize,
}

impl Aes {
    /// **Expande una clave de 16 o 32 bytes.** `None` si mide otra cosa.
    ///
    /// [!] AES-192 no esta, y es deliberado: no lo usa TLS 1.3, y un tamano de
    /// clave que nadie ejercita es un camino que nadie prueba.
    pub fn nueva(clave: &[u8]) -> Option<Aes> {
        let nk = match clave.len() {
            16 => 4usize,
            32 => 8,
            _ => return None,
        };
        let rondas = nk + 6;
        let palabras = 4 * (rondas + 1);
        let mut w = [[0u8; 4]; 60];
        for i in 0..nk {
            w[i].copy_from_slice(&clave[i * 4..i * 4 + 4]);
        }
        let mut rcon = 1u8;
        for i in nk..palabras {
            let mut t = w[i - 1];
            if i % nk == 0 {
                // Gira, sustituye, y suma la constante de ronda.
                t = [SBOX[t[1] as usize] ^ rcon, SBOX[t[2] as usize], SBOX[t[3] as usize], SBOX[t[0] as usize]];
                rcon = por2(rcon);
            } else if nk > 6 && i % nk == 4 {
                // ** SOLO EN AES-256, y es el paso que casi siempre se olvida:
                // con claves de 8 palabras hay una sustitucion EXTRA a mitad.
                // Sin ella, AES-256 cifra perfectamente y da otra cosa.
                t = [SBOX[t[0] as usize], SBOX[t[1] as usize], SBOX[t[2] as usize], SBOX[t[3] as usize]];
            }
            for j in 0..4 {
                w[i][j] = w[i - nk][j] ^ t[j];
            }
        }
        let mut rk = [[0u8; BLOQUE]; 15];
        for r in 0..=rondas {
            for c in 0..4 {
                rk[r][c * 4..c * 4 + 4].copy_from_slice(&w[r * 4 + c]);
            }
        }
        Some(Aes { rk, rondas })
    }

    /// **Cifra un bloque de 16 bytes, en su sitio.**
    pub fn cifrar_bloque(&self, b: &mut [u8; BLOQUE]) {
        for i in 0..BLOQUE {
            b[i] ^= self.rk[0][i];
        }
        for r in 1..self.rondas {
            self.ronda(b, r, true);
        }
        // ** La ultima ronda NO lleva MixColumns. No es una optimizacion: sin
        // esa asimetria, cifrar y descifrar no serian inversos, y quitarla es
        // el fallo mas comun al escribir AES a mano.
        self.ronda(b, self.rondas, false);
    }

    fn ronda(&self, b: &mut [u8; BLOQUE], r: usize, mezclar: bool) {
        // SubBytes
        for x in b.iter_mut() {
            *x = SBOX[*x as usize];
        }
        // ShiftRows. El estado va por COLUMNAS: el byte `i` es fila `i%4`,
        // columna `i/4`. La fila `n` se gira `n` sitios a la izquierda.
        let t = *b;
        for c in 0..4 {
            for f in 0..4 {
                b[c * 4 + f] = t[((c + f) % 4) * 4 + f];
            }
        }
        if mezclar {
            // MixColumns: cada columna por una matriz fija en GF(2^8).
            for c in 0..4 {
                let s = [b[c * 4], b[c * 4 + 1], b[c * 4 + 2], b[c * 4 + 3]];
                let todo = s[0] ^ s[1] ^ s[2] ^ s[3];
                for f in 0..4 {
                    b[c * 4 + f] = s[f] ^ todo ^ por2(s[f] ^ s[(f + 1) % 4]);
                }
            }
        }
        for i in 0..BLOQUE {
            b[i] ^= self.rk[r][i];
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn de_hex(s: &str) -> alloc::vec::Vec<u8> {
        let b = s.as_bytes();
        (0..b.len() / 2)
            .map(|i| {
                let hi = (b[i * 2] as char).to_digit(16).unwrap() as u8;
                let lo = (b[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
                (hi << 4) | lo
            })
            .collect()
    }
    fn hex(b: &[u8]) -> alloc::string::String {
        use core::fmt::Write;
        let mut s = alloc::string::String::new();
        for x in b {
            let _ = write!(s, "{:02x}", x);
        }
        s
    }

    /// *** LOS VECTORES DEL FIPS 197, apendice C. La regla 1 del crate.
    #[test]
    fn los_vectores_del_fips_197() {
        let clave = de_hex("000102030405060708090a0b0c0d0e0f");
        let a = Aes::nueva(&clave).unwrap();
        let mut b = [0u8; 16];
        b.copy_from_slice(&de_hex("00112233445566778899aabbccddeeff"));
        a.cifrar_bloque(&mut b);
        assert_eq!(hex(&b), "69c4e0d86a7b0430d8cdb78070b4c55a", "AES-128");
    }

    /// *** Y EL DE 256, que es el que caza la sustitucion EXTRA.
    ///
    /// ** Con claves de 8 palabras hay una `SubWord` a mitad de la expansion que
    /// no existe en AES-128. Sin ella, AES-256 **cifra perfectamente** --16
    /// bytes que cambian con la entrada-- y da otra cosa. Los vectores de 128
    /// pasarian igual.
    #[test]
    fn el_de_256_caza_la_sustitucion_extra() {
        let clave = de_hex("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
        let a = Aes::nueva(&clave).unwrap();
        let mut b = [0u8; 16];
        b.copy_from_slice(&de_hex("00112233445566778899aabbccddeeff"));
        a.cifrar_bloque(&mut b);
        assert_eq!(hex(&b), "8ea2b7ca516745bfeafc49904b496089", "AES-256");
    }

    /// Un tamano de clave que no se soporta se dice, no se recorta.
    #[test]
    fn una_clave_de_otro_tamano_se_contesta_que_no() {
        assert!(Aes::nueva(&[0u8; 24]).is_none(), "AES-192 no esta, y se dice");
        assert!(Aes::nueva(&[0u8; 15]).is_none());
        assert!(Aes::nueva(&[]).is_none());
    }
}

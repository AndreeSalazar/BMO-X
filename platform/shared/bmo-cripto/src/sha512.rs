//! **SHA-512.** El ladrillo que le faltaba a la FIRMA.
//!
//! # Por que esta pieza y por que ahora
//!
//! `PLAN_SEGURIDAD.md` C3 pide Ed25519, y Ed25519 no es solo curva eliptica:
//! **RFC 8032 lo define con SHA-512 dentro**, en tres sitios distintos --al
//! derivar la clave, al calcular el `nonce` y al calcular el reto `k`--. Sin
//! esto no hay firma, y sin firma el `.bex` sigue sin poder decir quien lo hizo.
//!
//! ```text
//!    SHA-256   [X] 24-08   debajo de HMAC, HKDF y del transcript de TLS
//!    SHA-512   [X] 25-08   debajo de Ed25519, y de NADA MAS por ahora
//! ```
//!
//! ** Y eso ultimo hay que decirlo: **esto no se anade porque sea "el hermano
//! grande" ni porque sea mas seguro.** SHA-256 sigue siendo el que usa todo lo
//! demas. SHA-512 entra porque una firma concreta lo exige por especificacion, y
//! si Ed25519 no hiciera falta, esta pagina no existiria.
//!
//! # Que cambia respecto a SHA-256, y son cuatro cosas
//!
//! No es "lo mismo con numeros mas grandes". Es la misma FORMA con **cuatro**
//! parametros distintos, y confundir uno da un hash que parece bueno y no
//! coincide con el de nadie:
//!
//! ```text
//!    palabra    32 bits  ->  64 bits
//!    bloque     64 B     ->  128 B
//!    rondas     64       ->  80
//!    el largo   u64      ->  **u128** al rellenar (16 bytes, no 8)
//! ```
//!
//! *** Y LAS ROTACIONES SON OTRAS. `ror(e,6)^ror(e,11)^ror(e,25)` es de
//! SHA-256; aqui son `14, 18, 41`. Copiar el fichero de al lado y ensanchar los
//! tipos compila, corre, y da un hash equivocado -- que es exactamente la clase
//! de fallo que los vectores existen para cazar.
//!
//! # Y por que se ESCRIBE, igual que su hermano
//!
//! Por lo mismo que dice `sha256.rs`, y no se repite entero: son unas decenas de
//! lineas de cuenta, y **el dia que esto firme un `.bex`, quien lo audite tiene
//! que poder leerlas**. Una cadena de confianza que empieza en un `Cargo.toml`
//! no es una cadena de confianza.
//!
//! [!] Y lo mismo que promete el otro: **no es resistente a ataques de tiempo, y
//! no tiene por que serlo.** Un hash no lleva secreto dentro. Lo que si lo lleva
//! es la firma que vendra encima, y esa sera su pagina.

/// Bytes que mide un hash de SHA-512.
pub const LARGO: usize = 64;

/// Bytes de un bloque. **128, no 64**: es el doble que SHA-256 y es el primer
/// sitio donde se equivoca quien copia el fichero de al lado.
pub const BLOQUE: usize = 128;

/// **Las ocho raices**: los 64 bits de la parte fraccionaria de la raiz cuadrada
/// de los ocho primeros primos. Son las mismas raices que SHA-256 **con mas
/// cifras**, no otras -- y por eso las primeras se parecen tanto.
const RAICES: [u64; 8] = [
    0x6a09e667f3bcc908,
    0xbb67ae8584caa73b,
    0x3c6ef372fe94f82b,
    0xa54ff53a5f1d36f1,
    0x510e527fade682d1,
    0x9b05688c2b3e6c1f,
    0x1f83d9abfb41bd6b,
    0x5be0cd19137e2179,
];

/// Las **80** constantes de ronda: la parte fraccionaria de la raiz CUBICA de
/// los ochenta primeros primos.
const K: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc,
    0x3956c25bf348b538, 0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118,
    0xd807aa98a3030242, 0x12835b0145706fbe, 0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2,
    0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235, 0xc19bf174cf692694,
    0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
    0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5,
    0x983e5152ee66dfab, 0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4,
    0xc6e00bf33da88fc2, 0xd5a79147930aa725, 0x06ca6351e003826f, 0x142929670a0e6e70,
    0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed, 0x53380d139d95b3df,
    0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30,
    0xd192e819d6ef5218, 0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8,
    0x19a4c116b8d2d0c8, 0x1e376c085141ab53, 0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8,
    0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373, 0x682e6ff3d6b2b8a3,
    0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
    0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b,
    0xca273eceea26619c, 0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178,
    0x06f067aa72176fba, 0x0a637dc5a2c898a6, 0x113f9804bef90dae, 0x1b710b35131c471b,
    0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc, 0x431d67c49c100d4c,
    0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

fn ror(x: u64, n: u32) -> u64 {
    x.rotate_right(n)
}

/// **El estado de un hash a medias.** Se alimenta por trozos, igual que
/// [`super::sha256::Sha256`] y por el mismo motivo: lo que hace falta de verdad
/// es hashear un fichero que no cabe en un buffer.
#[derive(Clone)]
pub struct Sha512 {
    h: [u64; 8],
    resto: [u8; BLOQUE],
    pendientes: usize,
    /// **Bytes totales.** Se escribe en BITS al cerrar, y en `u128`.
    ///
    /// [!] En SHA-256 el largo cabe en `u64`; aqui el campo son **128 bits**. Un
    /// `u64` funcionaria para cualquier fichero real y **daria un relleno
    /// distinto**, porque son ocho bytes menos en el ultimo bloque. Compila,
    /// corre y no coincide con nadie.
    total: u128,
}

impl Default for Sha512 {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Sha512 {
    pub fn nuevo() -> Self {
        Sha512 { h: RAICES, resto: [0; BLOQUE], pendientes: 0, total: 0 }
    }

    /// **Mezcla UN bloque de 128 bytes.** El corazon, y la unica parte que no se
    /// parece a nada de al lado.
    fn bloque(&mut self, b: &[u8]) {
        // 1. Los 128 bytes, como 16 palabras de 64 bits BIG-ENDIAN.
        //
        // [!] Big-endian, y esta maquina es little-endian. Leerlo del modo
        // nativo da un hash que tiene 64 bytes y cambia con la entrada, o sea
        // que parece bueno -- y no coincide con el del resto del mundo.
        let mut w = [0u64; 80];
        for i in 0..16 {
            let mut v = [0u8; 8];
            v.copy_from_slice(&b[i * 8..i * 8 + 8]);
            w[i] = u64::from_be_bytes(v);
        }
        // 2. Las otras 64. **Los desplazamientos son 1, 8, 7 y 19, 61, 6** --
        // en SHA-256 son 7, 18, 3 y 17, 19, 10.
        for i in 16..80 {
            let s0 = ror(w[i - 15], 1) ^ ror(w[i - 15], 8) ^ (w[i - 15] >> 7);
            let s1 = ror(w[i - 2], 19) ^ ror(w[i - 2], 61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        // 3. Las 80 rondas. `ch` y `maj` son IGUALES que en SHA-256; lo que
        // cambia son las rotaciones de `s0` y `s1`.
        let [mut a, mut bb, mut c, mut d, mut e, mut f, mut g, mut hh] = self.h;
        for i in 0..80 {
            let s1 = ror(e, 14) ^ ror(e, 18) ^ ror(e, 41);
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = ror(a, 28) ^ ror(a, 34) ^ ror(a, 39);
            let maj = (a & bb) ^ (a & c) ^ (bb & c);
            let t2 = s0.wrapping_add(maj);

            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = bb;
            bb = a;
            a = t1.wrapping_add(t2);
        }

        // 4. Y se SUMAN al estado. Sin esta suma el hash de un fichero seria el
        // de sus ultimos 128 bytes -- y pasaria todas las pruebas cortas.
        let nuevos = [a, bb, c, d, e, f, g, hh];
        for i in 0..8 {
            self.h[i] = self.h[i].wrapping_add(nuevos[i]);
        }
    }

    /// Mete mas bytes. Se puede llamar tantas veces como haga falta.
    pub fn mete(&mut self, datos: &[u8]) {
        self.total = self.total.wrapping_add(datos.len() as u128);
        let mut i = 0;

        if self.pendientes > 0 {
            let hueco = BLOQUE - self.pendientes;
            let cuantos = hueco.min(datos.len());
            self.resto[self.pendientes..self.pendientes + cuantos]
                .copy_from_slice(&datos[..cuantos]);
            self.pendientes += cuantos;
            i = cuantos;
            if self.pendientes == BLOQUE {
                let b = self.resto;
                self.bloque(&b);
                self.pendientes = 0;
            }
        }

        while i + BLOQUE <= datos.len() {
            let b: [u8; BLOQUE] = datos[i..i + BLOQUE].try_into().unwrap_or([0; BLOQUE]);
            self.bloque(&b);
            i += BLOQUE;
        }

        let sobra = datos.len() - i;
        if sobra > 0 {
            self.resto[..sobra].copy_from_slice(&datos[i..]);
            self.pendientes = sobra;
        }
    }

    /// **Cierra y devuelve los 64 bytes.**
    ///
    /// El relleno es el de la especificacion: un `0x80`, tantos ceros como haga
    /// falta, y el largo **en BITS** en los ultimos **16** bytes, big-endian.
    ///
    /// [!] El `* 8` va en una sola linea y con su nombre, por el mismo motivo
    /// que en SHA-256: es el sitio clasico donde se pierde un factor de ocho.
    pub fn cierra(mut self) -> [u8; LARGO] {
        let bits: u128 = self.total.wrapping_mul(8);

        // El `0x80` y los ceros. Si lo que queda no da para los 16 bytes del
        // largo, se cierra este bloque y el largo va en el siguiente.
        self.mete(&[0x80]);
        while self.pendientes != BLOQUE - 16 {
            self.mete(&[0x00]);
        }
        // ** `mete` acaba de sumar los bytes del relleno a `total`, asi que el
        // largo que se escribe es el de ANTES, guardado arriba. Escribir
        // `self.total` aqui contaria el relleno como mensaje.
        let b = bits.to_be_bytes();
        self.mete(&b);
        debug_assert_eq!(self.pendientes, 0, "el relleno tiene que cerrar el bloque");

        let mut out = [0u8; LARGO];
        for i in 0..8 {
            out[i * 8..i * 8 + 8].copy_from_slice(&self.h[i].to_be_bytes());
        }
        out
    }
}

/// SHA-512 de un mensaje que ya esta entero en memoria.
pub fn hash(datos: &[u8]) -> [u8; LARGO] {
    let mut s = Sha512::nuevo();
    s.mete(datos);
    s.cierra()
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn hex(b: &[u8]) -> alloc::string::String {
        use core::fmt::Write;
        let mut s = alloc::string::String::new();
        for x in b {
            let _ = write!(s, "{x:02x}");
        }
        s
    }

    /// **Los vectores de FIPS 180-4**, que es la unica autoridad que vale.
    ///
    /// *** Y el de la cadena VACIA no es un capricho: es el unico que ejercita
    /// el relleno cuando NO hay mensaje, o sea un bloque entero de relleno. Un
    /// error ahi no lo caza ningun mensaje corto.
    #[test]
    fn los_vectores_de_nist() {
        assert_eq!(
            hex(&hash(b"abc")),
            "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a\
             2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f"
        );
        assert_eq!(
            hex(&hash(b"")),
            "cf83e1357eefb8bdf1542850d66d8007d620e4050b5715dc83f4a921d36ce9ce\
             47d0d13c5d85f2b0ff8318d2877eec2f63b931bd47417a81a538327af927da3e"
        );
        // 112 bytes: cruza el bloque de 128 y obliga a que el largo caiga en un
        // SEGUNDO bloque. Es el caso que un `u64` de largo pasaria y un `u128`
        // no -- o al reves, que es como se descubre el fallo.
        assert_eq!(
            hex(&hash(
                b"abcdefghbcdefghicdefghijdefghijkefghijklfghijklmghijklmn\
                  hijklmnoijklmnopjklmnopqklmnopqrlmnopqrsmnopqrstnopqrstu"
            )),
            "8e959b75dae313da8cf4f72814fc143f8f7779c6eb9f7fa17299aeadb6889018\
             501d289e4900f7e4331b99dec4b5433ac7d329eeb6dd26545e96e55b874be909"
        );
    }

    /// **Alimentarlo a trozos da lo mismo que de una vez.**
    ///
    /// Es lo que hace util el tipo con estado, y es donde vive el fallo de
    /// contabilidad: si `mete` perdiera o duplicara un byte en la frontera de un
    /// bloque, un mensaje de una sola pieza seguiria dando bien.
    #[test]
    fn por_trozos_da_lo_mismo_que_de_golpe() {
        let msg: alloc::vec::Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
        let de_golpe = hash(&msg);
        for corte in [1usize, 63, 64, 127, 128, 129, 255, 256, 999] {
            let mut s = Sha512::nuevo();
            for trozo in msg.chunks(corte) {
                s.mete(trozo);
            }
            assert_eq!(s.cierra(), de_golpe, "cortando de {corte} en {corte}");
        }
    }

    /// **El borde del relleno**, que es donde SHA-512 se separa de SHA-256.
    ///
    /// Con 111 bytes el largo cabe justo; con 112 ya no y hace falta otro
    /// bloque. Si el campo del largo fueran 8 bytes en vez de 16, el borde
    /// caeria en 119/120 y **estos dos largos darian un hash equivocado los
    /// dos** -- sin que ningun mensaje corto lo notara.
    #[test]
    fn el_relleno_cambia_de_bloque_donde_debe() {
        for n in [110usize, 111, 112, 113, 127, 128, 129] {
            let msg = alloc::vec![0x61u8; n];
            let h = hash(&msg);
            // No se comprueba el valor --no hay vector oficial para estos-- sino
            // que la funcion CONTESTA y que dos largos distintos no coinciden,
            // que es lo que pasaria si el relleno se comiera el ultimo byte.
            let otro = hash(&alloc::vec![0x61u8; n + 1]);
            assert_ne!(h, otro, "con {n} y {} bytes no puede dar lo mismo", n + 1);
        }
    }
}

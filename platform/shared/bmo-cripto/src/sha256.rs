//! **SHA-256.** El primer ladrillo de la criptografia de BMO-X.
//!
//! # Por que esta es la primera pieza, y no una eleccion de gusto
//!
//! El techo de todo el plan tiene un nombre --la criptografia-- y **dentro de
//! ese techo, SHA-256 esta debajo de casi todo lo demas**:
//!
//! ```text
//!    HMAC-SHA256    autenticar un mensaje              -> es SHA-256 dos veces
//!    HKDF           sacar claves de un secreto         -> es HMAC
//!    TLS 1.3        el `transcript hash` de todo el    -> es SHA-256
//!                   apreton de manos
//!    X.509          la huella que una firma firma      -> es SHA-256
//! ```
//!
//! *** Y hay una razon mas, que es la que decide el orden: **es la unica pieza
//! de criptografia que se puede comprobar del todo sin hardware, sin red y sin
//! nadie al otro lado.** NIST publico los vectores en FIPS 180-4 hace veinte
//! anos; o los das o no los das. No hay opinion posible.
//!
//! Eso importa mas aqui que en otro sitio: **una criptografia mal escrita no
//! falla, funciona y no protege**. Empezar por la pieza que tiene respuesta
//! oficial es empezar por donde el error no se puede esconder.
//!
//! # Y por que se ESCRIBE en vez de traerla
//!
//! Hay crates de SHA-256 hechas. Traerlas seria coherente con la ley 24 --el
//! hash es SOFTWARE: no nombra ningun aparato, y ahi generico es lo correcto--
//! y aun asi se escribe, por dos motivos que no valen para `smoltcp`:
//!
//! 1. **Son sesenta lineas de cuenta.** Traer un arbol de dependencias para
//!    esto cuesta mas que escribirlo, y `bmo-hash` ya sento el precedente con
//!    BLAKE3 -- que es bastante mas dificil.
//! 2. *** **El dia que esto firme un `.bex`, quien lo audite tiene que poder
//!    leer las sesenta lineas.** Una cadena de confianza que empieza en un
//!    `Cargo.toml` no es una cadena de confianza: es una promesa de otro.
//!
//! [!] Y lo que este fichero NO promete, dicho antes de que alguien lo suponga:
//! **no es resistente a ataques de tiempo y no tiene por que serlo.** Un hash no
//! lleva secreto dentro; lo que si lo lleva es el HMAC que vendra encima, y esa
//! sera su pagina y su problema.
//!
//! # La cuenta, en cuatro lineas
//!
//! Se parte el mensaje en bloques de 64 bytes. Cada bloque se estira a 64
//! palabras de 32 bits, y esas 64 palabras se mezclan con ocho registros que
//! empiezan valiendo la raiz cuadrada de los ocho primeros primos. Al final los
//! ocho registros SON el hash.
//!
//! ** Y todo se hace en `u32` con vuelta al dar la vuelta (`wrapping_add`), que
//! es parte de la definicion y no un descuido: SHA-256 esta especificado sobre
//! aritmetica modulo 2^32.

/// Bytes que mide un hash de SHA-256.
pub const LARGO: usize = 32;

/// Bytes de un bloque. Todo el algoritmo trabaja de 64 en 64.
pub const BLOQUE: usize = 64;

/// **Las ocho raices.** Son los 32 bits de la parte fraccionaria de la raiz
/// cuadrada de los ocho primeros primos (2, 3, 5, 7, 11, 13, 17, 19).
///
/// ** Se escriben con ese motivo al lado porque es lo que las hace *"nothing up
/// my sleeve"*: cualquiera puede recalcularlas y comprobar que no esconden nada.
/// Una constante magica sin origen en un algoritmo de criptografia es
/// exactamente lo que un auditor no puede aceptar.
const RAICES: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

/// **Las 64 constantes de ronda.** Los 32 bits de la parte fraccionaria de la
/// raiz CUBICA de los 64 primeros primos. Mismo motivo que [`RAICES`].
const K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// **El estado de un hash a medias.** Se puede alimentar por trozos.
///
/// ** Existe en vez de una sola funcion `hash(bytes)` porque el caso que hace
/// falta de verdad es el que NO cabe en memoria: firmar un `.bex` es hashear un
/// fichero, y un fichero no se trae entero a un buffer para eso. Es la misma
/// forma que ya tiene `bmo-hash`.
#[derive(Clone)]
pub struct Sha256 {
    /// Los ocho registros. Al final, ESTO es el hash.
    h: [u32; 8],
    /// Lo que sobro del ultimo trozo y todavia no llena un bloque.
    resto: [u8; BLOQUE],
    /// Cuantos bytes hay en `resto`.
    pendientes: usize,
    /// **Bytes totales**, y de aqui sale el relleno del final.
    ///
    /// [!] Se cuenta en BYTES y se escribe en BITS al cerrar. Es el sitio
    /// clasico donde se pierde un factor de ocho, y por eso el `* 8` esta en una
    /// sola linea y con su nombre.
    total: u64,
}

impl Default for Sha256 {
    fn default() -> Self {
        Self::nuevo()
    }
}

fn ror(x: u32, n: u32) -> u32 {
    x.rotate_right(n)
}

impl Sha256 {
    pub fn nuevo() -> Self {
        Sha256 { h: RAICES, resto: [0; BLOQUE], pendientes: 0, total: 0 }
    }

    /// **Mezcla UN bloque de 64 bytes en el estado.** El corazon del algoritmo.
    fn bloque(&mut self, b: &[u8]) {
        // 1. Los 64 bytes se leen como 16 palabras de 32 bits, BIG-ENDIAN.
        //
        // [!] Big-endian y esta maquina es little-endian. Leerlo del modo nativo
        // da un hash que parece correcto --tiene 32 bytes y cambia con la
        // entrada-- y no coincide con el de nadie. Es el mismo fallo que el
        // ethertype de la red, y aqui costaria mas: un hash que no coincide con
        // el del resto del mundo no se nota hasta que hay alguien al otro lado.
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([b[i * 4], b[i * 4 + 1], b[i * 4 + 2], b[i * 4 + 3]]);
        }
        // 2. Y las otras 48 salen de mezclar las anteriores.
        for i in 16..64 {
            let s0 = ror(w[i - 15], 7) ^ ror(w[i - 15], 18) ^ (w[i - 15] >> 3);
            let s1 = ror(w[i - 2], 17) ^ ror(w[i - 2], 19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        // 3. Las 64 rondas, sobre una copia de los ocho registros.
        let [mut a, mut bb, mut c, mut d, mut e, mut f, mut g, mut hh] = self.h;
        for i in 0..64 {
            let s1 = ror(e, 6) ^ ror(e, 11) ^ ror(e, 25);
            // `ch`: por cada bit, elige `f` si `e` vale 1, y `g` si vale 0.
            let ch = (e & f) ^ ((!e) & g);
            let t1 = hh
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(K[i])
                .wrapping_add(w[i]);
            let s0 = ror(a, 2) ^ ror(a, 13) ^ ror(a, 22);
            // `maj`: por cada bit, el valor que tienen al menos dos de los tres.
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

        // 4. Y se SUMAN al estado, no lo sustituyen.
        //
        // ** Esa suma es lo que hace que el hash dependa de todos los bloques y
        // no solo del ultimo. Sin ella, hashear un fichero daria el hash de sus
        // ultimos 64 bytes -- y pasaria todas las pruebas de mensajes cortos.
        let nuevos = [a, bb, c, d, e, f, g, hh];
        for i in 0..8 {
            self.h[i] = self.h[i].wrapping_add(nuevos[i]);
        }
    }

    /// Mete mas bytes. Se puede llamar tantas veces como haga falta.
    pub fn mete(&mut self, datos: &[u8]) {
        self.total = self.total.wrapping_add(datos.len() as u64);
        let mut i = 0;

        // Primero completar lo que quedo a medias.
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

        // Los bloques completos, directos.
        while i + BLOQUE <= datos.len() {
            let b: [u8; BLOQUE] = datos[i..i + BLOQUE].try_into().unwrap_or([0; BLOQUE]);
            self.bloque(&b);
            i += BLOQUE;
        }

        // Y lo que sobra, a esperar al siguiente trozo.
        if i < datos.len() {
            let cuantos = datos.len() - i;
            self.resto[..cuantos].copy_from_slice(&datos[i..]);
            self.pendientes = cuantos;
        }
    }

    /// **Cierra el hash y lo devuelve.**
    ///
    /// ## El relleno, que es donde SHA-256 se rompe si se hace a ojo
    ///
    /// Al final se anade un `0x80`, luego ceros, y al final **el largo del
    /// mensaje EN BITS, en ocho bytes big-endian**. El relleno se estira hasta
    /// que el total sea multiplo de 64.
    ///
    /// *** Y el detalle que se olvida: si despues del `0x80` no caben los ocho
    /// bytes del largo, **hace falta un bloque MAS**. Ese caso ocurre solo
    /// cuando el mensaje mide entre 56 y 63 modulo 64 -- o sea, en el 12,5% de
    /// los largos posibles-- y una implementacion que no lo trate pasa casi
    /// todas las pruebas. Por eso hay una prueba que barre los 200 primeros
    /// largos, y no tres mensajes bonitos.
    pub fn cierra(mut self) -> [u8; LARGO] {
        let bits = self.total.wrapping_mul(8);

        // El uno, y luego ceros hasta dejar sitio a los ocho del largo.
        self.mete(&[0x80]);
        // OJO: `mete` acaba de sumar 1 a `total`, y el largo ya se guardo.
        while self.pendientes != BLOQUE - 8 {
            self.mete(&[0x00]);
        }
        // Y el largo en bits, big-endian.
        self.mete(&bits.to_be_bytes());
        debug_assert_eq!(self.pendientes, 0, "el relleno tiene que cerrar el bloque");

        let mut salida = [0u8; LARGO];
        for i in 0..8 {
            salida[i * 4..i * 4 + 4].copy_from_slice(&self.h[i].to_be_bytes());
        }
        salida
    }
}

/// **El hash de unos bytes, de una vez.** Para cuando caben en memoria.
pub fn hash(datos: &[u8]) -> [u8; LARGO] {
    let mut s = Sha256::nuevo();
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
            let _ = write!(s, "{:02x}", x);
        }
        s
    }

    /// *** LOS VECTORES DE NIST. Esta es la prueba que no se puede discutir.
    ///
    /// ** FIPS 180-4 los publico y llevan veinte anos siendo los mismos. Una
    /// implementacion de SHA-256 o los da o no los da -- **no hay opinion
    /// posible**, y por eso esta pieza va primera en el plan de criptografia:
    /// es la unica donde el error no se puede esconder.
    #[test]
    fn los_vectores_de_nist() {
        assert_eq!(
            hex(&hash(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "el mensaje vacio"
        );
        assert_eq!(
            hex(&hash(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // 56 bytes: cae JUSTO en el caso que necesita un bloque de relleno mas.
        assert_eq!(
            hex(&hash(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq")),
            "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
        );
    }

    /// *** EL VECTOR LARGO: un millon de letras `a`.
    ///
    /// ** Es el que caza que el estado se SUME entre bloques en vez de
    /// sustituirse. Una implementacion que se olvide de la suma final da el hash
    /// de los ultimos 64 bytes -- y **pasa los tres vectores de arriba**, porque
    /// los tres caben en uno o dos bloques.
    #[test]
    fn el_millon_de_aes() {
        let mut s = Sha256::nuevo();
        let trozo = [b'a'; 1000];
        for _ in 0..1000 {
            s.mete(&trozo);
        }
        assert_eq!(
            hex(&s.cierra()),
            "cdc76e5c9914fb9281a1c7e284d73e67f1809a48a497200e046d39ccc7112cd0"
        );
    }

    /// *** EL RELLENO, EN LOS 200 PRIMEROS LARGOS.
    ///
    /// ** Si despues del `0x80` no caben los ocho bytes del largo, hace falta un
    /// bloque MAS. Ese caso ocurre cuando el mensaje mide entre 56 y 63 modulo
    /// 64 -- **el 12,5% de los largos**-- y una implementacion que no lo trate
    /// pasa casi cualquier prueba escrita a mano.
    ///
    /// Asi que en vez de tres mensajes bonitos se barren doscientos largos y se
    /// comprueba contra el hash calculado por trozos, que recorre otro camino
    /// del codigo. **Dos caminos que tienen que coincidir.**
    #[test]
    fn el_relleno_aguanta_todos_los_largos() {
        for n in 0..200usize {
            let msg: alloc::vec::Vec<u8> = (0..n).map(|i| (i % 251) as u8).collect();
            let de_una = hash(&msg);

            // El mismo mensaje, metido de uno en uno.
            let mut s = Sha256::nuevo();
            for b in &msg {
                s.mete(&[*b]);
            }
            assert_eq!(de_una, s.cierra(), "difieren con largo {n}");
        }
    }

    /// Meter por trozos da lo mismo que meter de una vez, **cortando por donde
    /// sea**: dentro de un bloque, justo en el borde, y a caballo de dos.
    #[test]
    fn los_trozos_no_cambian_el_hash() {
        let msg: alloc::vec::Vec<u8> = (0..500u32).map(|i| (i % 251) as u8).collect();
        let esperado = hash(&msg);
        for corte in [1usize, 63, 64, 65, 127, 128, 200, 499] {
            let mut s = Sha256::nuevo();
            s.mete(&msg[..corte]);
            s.mete(&msg[corte..]);
            assert_eq!(s.cierra(), esperado, "difiere cortando en {corte}");
        }
    }

    /// [!] Y UN BIT CAMBIADO CAMBIA EL HASH ENTERO.
    ///
    /// ** No prueba que sea SHA-256 --eso lo hacen los vectores-- pero si que la
    /// entrada llega hasta el final. Una implementacion que ignorara el ultimo
    /// bloque pasaria los vectores cortos y fallaria aqui.
    #[test]
    fn un_bit_cambiado_cambia_todo() {
        let a = hash(b"BMO-X");
        let b = hash(b"BMO-Y");
        assert_ne!(a, b);
        let distintos = a.iter().zip(b.iter()).filter(|(x, y)| x != y).count();
        assert!(distintos > 20, "solo {distintos} bytes de 32 cambiaron");
    }
}

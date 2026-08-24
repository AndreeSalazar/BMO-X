//! **AES-GCM.** Cifrar y autenticar a la vez, que son dos cosas y una sola
//! llamada.
//!
//! # Que hace, y por que cifrar sin autenticar esta roto
//!
//! Cifrar esconde el contenido. **No impide cambiarlo.** Un modo de flujo --y
//! GCM lo es-- hace `xor` entre el texto y una secuencia; quien intercepte el
//! mensaje puede voltear un bit del cifrado y **voltea exactamente ese bit del
//! claro**, sin conocer la clave ni el contenido.
//!
//! ```text
//!    solo cifrado   el atacante no LEE, pero ESCRIBE
//!    GCM            si toca un bit, la etiqueta no cuadra y se rechaza
//! ```
//!
//! *** Por eso las dos cosas van juntas y en una sola operacion: **un diseno que
//! deje autenticar como un paso aparte es un diseno donde alguien se lo va a
//! saltar.**
//!
//! # [!!] LA REGLA QUE PUEDE DESTRUIRLO TODO: EL NONCE NO SE REPITE
//!
//! Con la misma clave, **usar dos veces el mismo nonce no debilita GCM: lo
//! rompe entero.**
//!
//! ```text
//!    dos mensajes con el mismo nonce
//!       -> el `xor` de los dos cifrados ES el `xor` de los dos claros
//!       -> y peor: se puede despejar la clave de AUTENTICACION `H`
//!       -> con `H`, el atacante FIRMA lo que quiera. Para siempre
//! ```
//!
//! ** No es una recomendacion: es la condicion bajo la que este fichero
//! funciona. Un nonce repetido no da un mensaje mas debil, da **un canal en el
//! que el atacante escribe con tu firma**.
//!
//! *** Y por eso el nonce entra por parametro y este fichero NO lo genera: quien
//! llama es el unico que sabe si su contador ya se uso. En TLS 1.3 sale de un
//! contador de mensajes, no de un aleatorio -- justamente para que no se pueda
//! repetir por mala suerte.
//!
//! # Y el nonce es de 96 bits, y solo de 96
//!
//! GCM admite otros tamanos con un paso extra. **Aqui no**, y es a proposito:
//! TLS 1.3 exige 96 bits, el camino de 96 es el unico que se va a ejercitar, y
//! un camino que nadie prueba es un camino que nadie sabe si funciona.

use crate::aes::{Aes, BLOQUE};

/// Bytes de la etiqueta de autenticacion.
pub const ETIQUETA: usize = 16;
/// Bytes del nonce. **Solo 96 bits**: ver la cabecera.
pub const NONCE: usize = 12;

/// Un valor de 128 bits, como dos palabras. La primera lleva los bytes 0..8.
type B128 = [u64; 2];

fn de_bytes(b: &[u8; BLOQUE]) -> B128 {
    let mut hi = [0u8; 8];
    let mut lo = [0u8; 8];
    hi.copy_from_slice(&b[..8]);
    lo.copy_from_slice(&b[8..]);
    [u64::from_be_bytes(hi), u64::from_be_bytes(lo)]
}

fn a_bytes(v: &B128) -> [u8; BLOQUE] {
    let mut b = [0u8; BLOQUE];
    b[..8].copy_from_slice(&v[0].to_be_bytes());
    b[8..].copy_from_slice(&v[1].to_be_bytes());
    b
}

/// **El producto de GHASH: multiplicar en GF(2^128).**
///
/// ## Por que bit a bit y no con una tabla
///
/// La version rapida precalcula una tabla de 4 KiB y la indexa **con el dato**.
/// Es exactamente el problema de las T-tables de AES, en otro sitio: quien
/// comparta el CPU mide que lineas de cache se tocaron.
///
/// *** Aqui se hace bit a bit: 128 vueltas que hacen **siempre lo mismo**. Lo
/// que el bit decide es una mascara --todo unos o todo ceros-- y un `xor`, no
/// una rama ni un indice.
///
/// ** El campo es `GF(2)[x] / (x^128 + x^7 + x^2 + x + 1)`, y esa `R` de
/// `0xE1...` es ese polinomio escrito al reves: GCM numera los bits con el MAS
/// significativo primero, al contrario que casi todo lo demas. Es el detalle que
/// hace que una implementacion escrita "de memoria" de un resultado que parece
/// correcto y no lo es.
fn producto(x: &mut B128, h: &B128) {
    let mut z: B128 = [0, 0];
    let mut v = *h;
    for i in 0..128 {
        // El bit `i` de `x`, contando desde el MAS significativo.
        let bit = if i < 64 {
            (x[0] >> (63 - i)) & 1
        } else {
            (x[1] >> (127 - i)) & 1
        };
        let m = 0u64.wrapping_sub(bit);
        z[0] ^= v[0] & m;
        z[1] ^= v[1] & m;

        // `v` se desplaza uno hacia el bit menos significativo, y si se salia
        // un uno, se reduce con el polinomio.
        let salio = v[1] & 1;
        v[1] = (v[1] >> 1) | (v[0] << 63);
        v[0] >>= 1;
        let rm = 0u64.wrapping_sub(salio);
        v[0] ^= 0xE100_0000_0000_0000u64 & rm;
    }
    *x = z;
}

/// Mete un bloque en el acumulador de GHASH: `y = (y ^ bloque) * h`.
fn absorber(y: &mut B128, bloque: &[u8], h: &B128) {
    let mut b = [0u8; BLOQUE];
    let n = bloque.len().min(BLOQUE);
    b[..n].copy_from_slice(&bloque[..n]);
    let t = de_bytes(&b);
    y[0] ^= t[0];
    y[1] ^= t[1];
    producto(y, h);
}

/// GHASH sobre los datos adicionales y el texto cifrado, con sus largos.
fn ghash(h: &B128, adicional: &[u8], cifrado: &[u8]) -> B128 {
    let mut y: B128 = [0, 0];
    for t in adicional.chunks(BLOQUE) {
        absorber(&mut y, t, h);
    }
    for t in cifrado.chunks(BLOQUE) {
        absorber(&mut y, t, h);
    }
    // ** EL BLOQUE DE LARGOS, EN BITS, y es lo que impide mover bytes de un
    // lado al otro. Sin el, un mensaje con 3 bytes de adicional y 5 de texto
    // daria la misma etiqueta que uno con 5 y 3 -- y entonces un atacante puede
    // mover la frontera entre lo que va firmado y lo que va cifrado.
    let mut largos = [0u8; BLOQUE];
    largos[..8].copy_from_slice(&((adicional.len() as u64) * 8).to_be_bytes());
    largos[8..].copy_from_slice(&((cifrado.len() as u64) * 8).to_be_bytes());
    absorber(&mut y, &largos, h);
    y
}

/// El contador de CTR: los 32 bits bajos suben, el resto no se toca.
fn siguiente(bloque: &mut [u8; BLOQUE]) {
    for i in (12..16).rev() {
        let (v, dio_vuelta) = bloque[i].overflowing_add(1);
        bloque[i] = v;
        if !dio_vuelta {
            break;
        }
    }
}

/// Cifra o descifra en su sitio con CTR. **Es la misma funcion para las dos
/// cosas**: `xor` con la secuencia que sale de cifrar el contador.
fn ctr(a: &Aes, j0: &[u8; BLOQUE], datos: &mut [u8]) {
    let mut cnt = *j0;
    for trozo in datos.chunks_mut(BLOQUE) {
        // ** El contador empieza en `J0 + 1`: el `J0` pelado se reserva para la
        // etiqueta. Si se reutilizara, la secuencia del primer bloque y la
        // mascara de la etiqueta serian la misma -- y de ahi se despeja una con
        // la otra.
        siguiente(&mut cnt);
        let mut s = cnt;
        a.cifrar_bloque(&mut s);
        for (i, b) in trozo.iter_mut().enumerate() {
            *b ^= s[i];
        }
    }
}

/// Prepara `H` y `J0` a partir de la clave y el nonce.
fn preparar(a: &Aes, nonce: &[u8; NONCE]) -> (B128, [u8; BLOQUE]) {
    // `H` es AES(0), y es la clave de AUTENTICACION. No se elige: sale de la
    // de cifrado, y por eso repetir un nonce la deja al descubierto.
    let mut h = [0u8; BLOQUE];
    a.cifrar_bloque(&mut h);

    let mut j0 = [0u8; BLOQUE];
    j0[..NONCE].copy_from_slice(nonce);
    j0[BLOQUE - 1] = 1;
    (de_bytes(&h), j0)
}

/// **Compara dos etiquetas en tiempo constante.**
///
/// Mismo motivo que en `hmac::iguales`, y aqui es todavia mas directo: quien
/// pueda medir cuanto tarda un rechazo puede **construir una etiqueta valida
/// byte a byte** sin conocer la clave.
fn etiquetas_iguales(a: &[u8; ETIQUETA], b: &[u8; ETIQUETA]) -> bool {
    let mut sobra = 0u8;
    for i in 0..ETIQUETA {
        sobra |= a[i] ^ b[i];
    }
    sobra == 0
}

/// **SELLAR**: cifra `texto` en su sitio y devuelve su etiqueta.
///
/// `adicional` viaja **en claro** y va autenticado: es para cabeceras que el
/// otro lado necesita leer antes de descifrar.
///
/// [!!] **El `nonce` no se puede repetir con la misma clave.** Ver la cabecera
/// del fichero: repetirlo no debilita esto, lo rompe entero.
pub fn sellar(
    clave: &[u8],
    nonce: &[u8; NONCE],
    adicional: &[u8],
    texto: &mut [u8],
) -> Option<[u8; ETIQUETA]> {
    let a = Aes::nueva(clave)?;
    let (h, j0) = preparar(&a, nonce);
    ctr(&a, &j0, texto);

    let s = ghash(&h, adicional, texto);
    let mut mascara = j0;
    a.cifrar_bloque(&mut mascara);
    let sb = a_bytes(&s);
    let mut t = [0u8; ETIQUETA];
    for i in 0..ETIQUETA {
        t[i] = sb[i] ^ mascara[i];
    }
    Some(t)
}

/// **ABRIR**: comprueba la etiqueta y, **solo si cuadra**, descifra en su sitio.
///
/// ## *** EL ORDEN NO SE PUEDE INVERTIR, Y ES EL FALLO CLASICO
///
/// Lo comodo es descifrar y luego comprobar. **Y esta roto**: durante ese rato
/// el llamante tiene en la mano un texto que nadie ha autenticado, y basta que
/// lo mire --o que lo pase a un parser-- para que un atacante que manda basura
/// consiga que su basura se procese. Es la familia entera de los ataques de
/// *padding oracle*, y de ahi salio la regla:
///
/// > **No se toca un byte que no este autenticado.**
///
/// ** Aqui se puede hacer bien porque GHASH va sobre el CIFRADO, no sobre el
/// claro: la etiqueta se puede calcular sin descifrar nada. Se comprueba, y si
/// no cuadra **el buffer del llamante sale intacto** y esto devuelve `false`.
///
/// [!] Y devuelve `bool` y no `Result`: un `Result` invita a un `?` que propaga
/// y sigue. Aqui no hay nada que propagar -- o la etiqueta cuadra o no hay
/// mensaje.
pub fn abrir(
    clave: &[u8],
    nonce: &[u8; NONCE],
    adicional: &[u8],
    texto: &mut [u8],
    etiqueta: &[u8; ETIQUETA],
) -> bool {
    let Some(a) = Aes::nueva(clave) else {
        return false;
    };
    let (h, j0) = preparar(&a, nonce);

    // 1. La etiqueta, sobre el CIFRADO que todavia no se ha tocado.
    let s = ghash(&h, adicional, texto);
    let mut mascara = j0;
    a.cifrar_bloque(&mut mascara);
    let sb = a_bytes(&s);
    let mut esperada = [0u8; ETIQUETA];
    for i in 0..ETIQUETA {
        esperada[i] = sb[i] ^ mascara[i];
    }

    // 2. Y solo entonces.
    if !etiquetas_iguales(&esperada, etiqueta) {
        return false;
    }
    ctr(&a, &j0, texto);
    true
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
    fn nonce(s: &str) -> [u8; NONCE] {
        let v = de_hex(s);
        let mut n = [0u8; NONCE];
        n.copy_from_slice(&v);
        n
    }

    /// *** LOS VECTORES DE GCM. La regla 1 del crate.
    #[test]
    fn los_vectores_de_gcm() {
        // Caso 1: clave de ceros, sin texto y sin adicional. Solo etiqueta.
        let k = [0u8; 16];
        let mut vacio: [u8; 0] = [];
        let t = sellar(&k, &[0u8; 12], &[], &mut vacio).unwrap();
        assert_eq!(hex(&t), "58e2fccefa7e3061367f1d57a4e7455a", "caso 1");

        // Caso 2: un bloque de ceros.
        let mut p = [0u8; 16];
        let t = sellar(&k, &[0u8; 12], &[], &mut p).unwrap();
        assert_eq!(hex(&p), "0388dace60b6a392f328c2b971b2fe78", "caso 2, cifrado");
        assert_eq!(hex(&t), "ab6e47d42cec13bdf53a67b21257bddf", "caso 2, etiqueta");
    }

    /// *** EL CASO 3: clave de verdad, texto largo, nonce de verdad.
    ///
    /// ** Es el que ejercita el camino entero -- cuatro bloques de CTR con el
    /// contador subiendo, y GHASH sobre mas de un bloque.
    ///
    /// [!] Este vector caza una cosa que ningun test propio caza: **el bloque
    /// de largos**. Escribi 60 bytes donde el vector lleva 64 --confundi este
    /// caso con el 4-- y el CIFRADO salio correcto igual, porque CTR no sabe
    /// cuanto mide el mensaje. Lo unico que se rompio fue la etiqueta. Sin el
    /// vector oficial, eso pasa.
    #[test]
    fn el_caso_tres_con_texto_largo() {
        let k = de_hex("feffe9928665731c6d6a8f9467308308");
        let mut p = de_hex(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255",
        );
        assert_eq!(p.len(), 64);
        let n = nonce("cafebabefacedbaddecaf888");
        let t = sellar(&k, &n, &[], &mut p).unwrap();
        assert_eq!(
            hex(&p),
            "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e\
             21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091473f5985"
        );
        assert_eq!(hex(&t), "4d5c2af327cd64a62cf35abd2ba6fab4");
    }

    /// *** EL CASO 4: el mismo texto CORTADO, y con datos adicionales.
    ///
    /// ** Y es la pareja del anterior a proposito: mismo clave y mismo nonce,
    /// 60 bytes en vez de 64, y 20 bytes de adicional. El cifrado es **el mismo
    /// prefijo**, byte por byte -- y aun asi la etiqueta no se parece en nada.
    /// Eso es exactamente lo que tiene que pasar.
    #[test]
    fn el_caso_cuatro_con_adicional() {
        let k = de_hex("feffe9928665731c6d6a8f9467308308");
        let mut p = de_hex(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b39",
        );
        assert_eq!(p.len(), 60);
        let a = de_hex("feedfacedeadbeeffeedfacedeadbeefabaddad2");
        let n = nonce("cafebabefacedbaddecaf888");
        let t = sellar(&k, &n, &a, &mut p).unwrap();
        assert_eq!(
            hex(&p),
            "42831ec2217774244b7221b784d0d49ce3aa212f2c02a4e035c17e2329aca12e\
             21d514b25466931c7d8f6a5aac84aa051ba30b396a0aac973d58e091"
        );
        assert_eq!(hex(&t), "5bc94fbc3221a5db94fae95ae7121a47");
        assert!(abrir(&k, &n, &a, &mut p, &t));
    }

    /// *** Y AES-256, que no es solo "la misma con mas rondas" hasta que se
    /// comprueba.
    #[test]
    fn el_caso_de_doscientos_cincuenta_y_seis() {
        let k = de_hex(
            "feffe9928665731c6d6a8f9467308308feffe9928665731c6d6a8f9467308308",
        );
        let mut p = de_hex(
            "d9313225f88406e5a55909c5aff5269a86a7a9531534f7da2e4c303d8a318a72\
             1c3c0c95956809532fcf0e2449a6b525b16aedf5aa0de657ba637b391aafd255",
        );
        let n = nonce("cafebabefacedbaddecaf888");
        let t = sellar(&k, &n, &[], &mut p).unwrap();
        assert_eq!(
            hex(&p),
            "522dc1f099567d07f47f37a32a84427d643a8cdcbfe5c0c97598a2bd2555d1aa\
             8cb08e48590dbb3da7b08b1056828838c5f61e6393ba7a0abcc9f662898015ad"
        );
        assert_eq!(hex(&t), "b094dac5d93471bdec1a502270e3cc6c");
    }

    /// Lo que se sella se abre, y sale igual.
    #[test]
    fn lo_sellado_se_abre() {
        let k = [7u8; 32];
        let n = [3u8; NONCE];
        let original = b"BMO-X no es una distro".to_vec();
        let mut buf = original.clone();
        let t = sellar(&k, &n, b"cabecera", &mut buf).unwrap();
        assert_ne!(buf, original, "no cifro nada");
        assert!(abrir(&k, &n, b"cabecera", &mut buf, &t));
        assert_eq!(buf, original);
    }

    /// *** UN BIT CAMBIADO SE RECHAZA, Y EL BUFER SALE INTACTO.
    ///
    /// ** Las dos mitades importan. Que se rechace es lo que hace que GCM sea
    /// mas que cifrado; que **el buffer no se toque** es lo que impide que el
    /// llamante mire un texto que nadie autentico -- la regla que evita la
    /// familia entera de los *padding oracle*.
    #[test]
    fn tocar_un_bit_se_rechaza_sin_descifrar() {
        let k = [7u8; 16];
        let n = [3u8; NONCE];
        let mut buf = b"un mensaje cualquiera".to_vec();
        let t = sellar(&k, &n, &[], &mut buf).unwrap();
        let cifrado = buf.clone();

        // El texto, tocado.
        buf[5] ^= 1;
        let tocado = buf.clone();
        assert!(!abrir(&k, &n, &[], &mut buf, &t), "acepto un texto tocado");
        assert_eq!(buf, tocado, "*** LO DESCIFRO IGUAL: el orden esta invertido");

        // La etiqueta, tocada.
        let mut buf2 = cifrado.clone();
        let mut t2 = t;
        t2[0] ^= 1;
        assert!(!abrir(&k, &n, &[], &mut buf2, &t2));
        assert_eq!(buf2, cifrado, "el bufer tiene que salir intacto");
    }

    /// Y el ADICIONAL tambien va firmado, aunque viaje en claro.
    #[test]
    fn cambiar_lo_adicional_tambien_se_caza() {
        let k = [7u8; 16];
        let n = [3u8; NONCE];
        let mut buf = b"cuerpo".to_vec();
        let t = sellar(&k, &n, b"de: alicia", &mut buf).unwrap();
        assert!(!abrir(&k, &n, b"de: benito", &mut buf, &t), "el remite no iba firmado");
        assert!(abrir(&k, &n, b"de: alicia", &mut buf, &t));
    }

    /// [!] El bloque de largos impide **mover la frontera** entre lo adicional
    /// y el texto.
    ///
    /// ** Sin el, tres bytes de cabecera con cinco de cuerpo darian la misma
    /// etiqueta que cinco con tres -- y un atacante podria correr esa linea.
    #[test]
    fn no_se_puede_mover_la_frontera() {
        let k = [7u8; 16];
        let n = [3u8; NONCE];
        let mut a = b"12345".to_vec();
        let ta = sellar(&k, &n, b"abc", &mut a).unwrap();
        let mut b = b"345".to_vec();
        let tb = sellar(&k, &n, b"abc12", &mut b).unwrap();
        assert_ne!(ta, tb);
    }
}

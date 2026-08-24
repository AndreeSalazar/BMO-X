//! **HMAC-SHA256.** El segundo ladrillo, y el primero que LLEVA UN SECRETO.
//!
//! # Que hace, y por que no vale con hashear la clave y el mensaje juntos
//!
//! Un hash contesta *"estos bytes son los mismos?"*. Un HMAC contesta algo
//! distinto y mas fuerte: ***"estos bytes los escribio alguien que sabe la
//! clave?"***.
//!
//! Y la construccion parece rebuscada --dos pasadas de hash con dos rellenos--
//! hasta que se ve que la version ingenua esta rota:
//!
//! ```text
//!    INGENUO   sha256(clave || mensaje)
//!    ROTO      porque SHA-256 se puede CONTINUAR: quien vea esa etiqueta
//!              puede calcular la de `mensaje || relleno || lo_que_quiera`
//!              **sin saber la clave**. Es el "length extension attack"
//! ```
//!
//! *** El HMAC lo cierra metiendo el resultado dentro de un SEGUNDO hash: lo que
//! sale ya no es un estado que se pueda continuar, es la salida de una funcion
//! que empieza de cero. Por eso son dos pasadas y no una capricho.
//!
//! ```text
//!    HMAC(k, m) = H( (k ^ opad) || H( (k ^ ipad) || m ) )
//! ```
//!
//! # [!] Y ESTA PAGINA SI LLEVA SECRETO DENTRO
//!
//! Es la regla 2 del crate, y aqui es donde empieza a aplicarse:
//!
//! ** **Comparar dos etiquetas con `==` es un fallo de seguridad**, aunque el
//! resultado sea correcto. Un `==` de arrays sale en cuanto encuentra el primer
//! byte distinto, asi que **tarda mas cuanto mas acierta** -- y quien pueda
//! medir ese tiempo puede adivinar la etiqueta byte a byte, sin saber la clave.
//!
//! Por eso existe [`iguales`] y por eso **no** hay un `PartialEq` aqui. Es la
//! primera vez en todo este arbol que el TIEMPO que tarda algo es parte de si
//! es correcto.
//!
//! [!] Lo que esta pieza NO promete: que la clave no quede en memoria despues.
//! Borrarla pide que el compilador no se salte el borrado, y eso es `volatil` --
//! que INTI todavia no tiene y Rust resuelve con `write_volatile`. Se dice aqui
//! en vez de fingir que la clave se evapora.

use crate::sha256::{self, Sha256, BLOQUE, LARGO};

/// **HMAC-SHA256.** La etiqueta de `mensaje` bajo `clave`.
pub fn hmac(clave: &[u8], mensaje: &[u8]) -> [u8; LARGO] {
    // ** UNA CLAVE MAS LARGA QUE UN BLOQUE SE HASHEA PRIMERO.
    //
    // Lo dice el RFC 2104 y no es un detalle: sin esto, dos claves distintas de
    // mas de 64 bytes podrian dar la misma etiqueta, porque solo se usarian sus
    // primeros 64. Es un caso que casi nunca se prueba y que rompe la promesa
    // entera de la funcion.
    let mut k = [0u8; BLOQUE];
    if clave.len() > BLOQUE {
        let h = sha256::hash(clave);
        k[..LARGO].copy_from_slice(&h);
    } else {
        k[..clave.len()].copy_from_slice(clave);
    }

    // Los dos rellenos del RFC. Son constantes y son distintos entre si: si
    // fueran iguales las dos pasadas usarian la misma clave derivada y la
    // segunda no anadiria nada.
    let mut ipad = [0x36u8; BLOQUE];
    let mut opad = [0x5Cu8; BLOQUE];
    for i in 0..BLOQUE {
        ipad[i] ^= k[i];
        opad[i] ^= k[i];
    }

    let mut dentro = Sha256::nuevo();
    dentro.mete(&ipad);
    dentro.mete(mensaje);
    let interior = dentro.cierra();

    let mut fuera = Sha256::nuevo();
    fuera.mete(&opad);
    fuera.mete(&interior);
    fuera.cierra()
}

/// **Son iguales estas dos etiquetas? EN TIEMPO CONSTANTE.**
///
/// *** POR QUE ESTA FUNCION EXISTE, Y POR QUE NO SE USA `==`
///
/// Un `==` de arrays sale en cuanto encuentra el primer byte distinto. Eso lo
/// hace **mas lento cuanto mas acierta**, y quien pueda medir ese tiempo puede
/// adivinar la etiqueta **byte a byte**: prueba los 256 valores del primero, se
/// queda con el que tardo mas, y repite. Treinta y dos rondas de 256 intentos
/// en vez de 2^256.
///
/// ** Esto recorre SIEMPRE los 32 bytes y acumula las diferencias con un `or`.
/// El resultado no depende de donde estaba el fallo, y el tiempo tampoco.
///
/// [!] Y devuelve `bool` a proposito, no `Ordering`: saber CUAL es mayor no
/// hace falta para nada aqui, y una funcion que lo dijera invitaria a un
/// `match` con salidas tempranas.
pub fn iguales(a: &[u8; LARGO], b: &[u8; LARGO]) -> bool {
    let mut sobra = 0u8;
    for i in 0..LARGO {
        sobra |= a[i] ^ b[i];
    }
    sobra == 0
}

// ===================================================================
//  HKDF -- de un secreto salen las claves
// ===================================================================
//
//  ## Por que un secreto compartido NO se usa como clave
//
//  Lo que sale de un intercambio de claves --de X25519, cuando exista-- es un
//  numero, y un numero no es una clave: **sus bits no estan repartidos por
//  igual**, y hace falta UNA por cada cosa (cifrar del cliente al servidor, del
//  servidor al cliente, autenticar cada sentido...).
//
//  *** HKDF hace las dos cosas en dos pasos con nombres exactos:
//
//  ```text
//     EXTRAER   del secreto desigual sale una llave uniforme
//     EXPANDIR  de esa llave salen TODAS las que hagan falta, y cada una
//               lleva su ETIQUETA -- que es lo que impide que la de cifrar
//               y la de autenticar acaben siendo la misma
//  ```
//
//  ** Y las dos son HMAC. Por eso van en este fichero: no son un algoritmo
//  nuevo, son SHA-256 usado con cuidado.

/// **EXTRAER**: del secreto desigual, una llave uniforme de 32 bytes.
///
/// ** La `sal` puede ser vacia y sigue siendo correcto --el RFC 5869 lo dice--
/// pero cuando la hay, mejora: dos apretones con el mismo secreto y sal
/// distinta dan llaves distintas.
///
/// [!] Fijate en el orden de los argumentos: **la SAL es la clave del HMAC, y
/// el secreto es el mensaje.** Es al reves de lo que la intuicion dice, y
/// cambiarlo da una funcion que parece funcionar y no es HKDF.
pub fn extraer(sal: &[u8], secreto: &[u8]) -> [u8; LARGO] {
    hmac(sal, secreto)
}

/// **EXPANDIR**: de la llave salen `salida.len()` bytes, atados a `etiqueta`.
///
/// *** LA ETIQUETA ES LO QUE SEPARA UNA CLAVE DE OTRA. Dos llamadas con la
/// misma llave y etiquetas distintas dan bytes que no tienen relacion; con la
/// misma etiqueta dan los mismos. Por eso en TLS cada clave tiene su nombre
/// escrito --"c hs traffic", "s hs traffic"...-- y no un numero de orden.
///
/// ** El contador de un byte al final de cada bloque es lo que hace que la
/// segunda vuelta no repita la primera, y es tambien lo que pone el techo:
/// **255 bloques de 32 bytes**. Mas que eso no se puede pedir, y esta funcion
/// contesta `false` en vez de dar bytes repetidos.
pub fn expandir(llave: &[u8; LARGO], etiqueta: &[u8], salida: &mut [u8]) -> bool {
    let bloques = salida.len().div_ceil(LARGO);
    if bloques > 255 {
        return false;
    }
    let mut previo = [0u8; LARGO];
    let mut hay_previo = false;
    let mut puesto = 0usize;

    for n in 1..=bloques {
        // T(n) = HMAC(llave, T(n-1) || etiqueta || n)
        //
        // [!] Y EL BUFER TIENE UN TECHO, que se dice en vez de esconderlo: la
        // etiqueta cabe en 256 bytes. En TLS 1.3 las etiquetas son cortas --"c
        // hs traffic" son once-- asi que sobra de largo; y una etiqueta que no
        // quepa recibe un `false`, no un recorte silencioso.
        //
        // ** Sin monton no hay otra forma, y meterle un `Vec` a un crate que
        // corre en Ring 0 seria cambiar donde vive esto por comodidad.
        let mut entrada: [u8; LARGO + 256 + 1] = [0; LARGO + 256 + 1];
        let mut k = 0usize;
        if hay_previo {
            entrada[..LARGO].copy_from_slice(&previo);
            k = LARGO;
        }
        if etiqueta.len() > 256 {
            return false;
        }
        entrada[k..k + etiqueta.len()].copy_from_slice(etiqueta);
        k += etiqueta.len();
        entrada[k] = n as u8;
        k += 1;

        previo = hmac(llave, &entrada[..k]);
        hay_previo = true;

        let cuantos = core::cmp::min(LARGO, salida.len() - puesto);
        salida[puesto..puesto + cuantos].copy_from_slice(&previo[..cuantos]);
        puesto += cuantos;
    }
    true
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

    /// *** LOS VECTORES DEL RFC 4231. La regla 1 del crate.
    ///
    /// ** Nada entra aqui sin su respuesta publicada, y HMAC-SHA256 la tiene:
    /// el RFC 4231 trae siete casos. Estos son los tres que cubren lo que se
    /// puede escribir mal.
    #[test]
    fn los_vectores_del_rfc_4231() {
        // Caso 1: clave corta.
        let k = [0x0bu8; 20];
        assert_eq!(
            hex(&hmac(&k, b"Hi There")),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
        // Caso 2: la clave ES el mensaje de otro. Cambiar clave por mensaje da
        // otra cosa, y este caso lo caza.
        assert_eq!(
            hex(&hmac(b"Jefe", b"what do ya want for nothing?")),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
        // Caso 3: clave y mensaje largos, todo 0xaa / 0xdd.
        let k3 = [0xaau8; 20];
        let m3 = [0xddu8; 50];
        assert_eq!(
            hex(&hmac(&k3, &m3)),
            "773ea91e36800e46854db8ebd09181a72959098b3ef8c122d9635514ced565fe"
        );
    }

    /// *** UNA CLAVE MAS LARGA QUE UN BLOQUE SE HASHEA PRIMERO (RFC 4231, caso 6).
    ///
    /// ** Sin esta rama, dos claves distintas de mas de 64 bytes que compartan
    /// sus primeros 64 darian **la misma etiqueta** -- y eso rompe la promesa
    /// entera de la funcion. Es un caso que casi nunca se prueba a mano.
    #[test]
    fn una_clave_larguisima_se_hashea_antes() {
        let k = [0xaau8; 131];
        assert_eq!(
            hex(&hmac(&k, b"Test Using Larger Than Block-Size Key - Hash Key First")),
            "60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54"
        );

        // Y la prueba de que la rama HACE FALTA: dos claves de 131 bytes que
        // comparten los primeros 64 tienen que dar etiquetas DISTINTAS.
        let mut a = [0x11u8; 131];
        let mut b = [0x11u8; 131];
        a[100] = 1;
        b[100] = 2;
        assert_ne!(hmac(&a, b"m"), hmac(&b, b"m"), "los ultimos bytes cuentan");
    }

    /// [!] Y HMAC NO ES `sha256(clave || mensaje)`.
    ///
    /// ** La version ingenua esta rota por extension de longitud: quien vea la
    /// etiqueta puede calcular la de un mensaje mas largo sin saber la clave.
    /// Esta prueba no demuestra que HMAC lo arregle --eso lo dice el RFC-- pero
    /// si que **no se colo la version ingenua por descuido**, que es el fallo
    /// que un vector de prueba solo caza si se mira.
    #[test]
    fn no_es_el_hash_ingenuo() {
        let clave = b"clave";
        let msg = b"mensaje";
        let mut junto = alloc::vec::Vec::new();
        junto.extend_from_slice(clave);
        junto.extend_from_slice(msg);
        assert_ne!(hmac(clave, msg), crate::sha256::hash(&junto));
    }

    /// La comparacion en tiempo constante contesta lo mismo que `==`.
    ///
    /// ** No se puede medir aqui que TARDE lo mismo --eso pide un banco de
    /// tiempos-- asi que lo que se fija es lo otro: que no se cuele un bug al
    /// escribirla sin salidas tempranas.
    #[test]
    fn iguales_contesta_lo_mismo_que_comparar() {
        let a = hmac(b"k", b"m");
        let b = hmac(b"k", b"m");
        let c = hmac(b"k", b"n");
        assert!(iguales(&a, &b));
        assert!(!iguales(&a, &c));
        // Y con UNA sola diferencia, en el ultimo byte: el caso que una
        // comparacion con salida temprana tardaria mas en resolver.
        let mut d = a;
        d[LARGO - 1] ^= 1;
        assert!(!iguales(&a, &d));
    }

    /// *** LOS VECTORES DEL RFC 5869 para HKDF.
    #[test]
    fn los_vectores_de_hkdf() {
        // Caso 1 del RFC 5869.
        let ikm = [0x0bu8; 22];
        let sal: [u8; 13] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
        let info: [u8; 10] = [0xf0, 0xf1, 0xf2, 0xf3, 0xf4, 0xf5, 0xf6, 0xf7, 0xf8, 0xf9];

        let prk = extraer(&sal, &ikm);
        assert_eq!(
            hex(&prk),
            "077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5"
        );

        let mut okm = [0u8; 42];
        assert!(expandir(&prk, &info, &mut okm));
        assert_eq!(
            hex(&okm),
            "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
        );
    }

    /// *** LA ETIQUETA ES LO QUE SEPARA UNA CLAVE DE OTRA.
    ///
    /// ** Si esto fallara, en TLS la clave de cifrar y la de autenticar
    /// acabarian siendo la misma -- y entonces todo "funciona" y no protege,
    /// que es la frase que gobierna este crate entero.
    #[test]
    fn dos_etiquetas_dan_claves_sin_relacion() {
        let prk = extraer(b"sal", b"secreto");
        let mut a = [0u8; 32];
        let mut b = [0u8; 32];
        assert!(expandir(&prk, b"cifrar", &mut a));
        assert!(expandir(&prk, b"autenticar", &mut b));
        assert_ne!(a, b);

        // Y la misma etiqueta da lo mismo: es una funcion, no un generador.
        let mut c = [0u8; 32];
        assert!(expandir(&prk, b"cifrar", &mut c));
        assert_eq!(a, c);
    }

    /// El techo son 255 bloques, y se dice en vez de dar bytes repetidos.
    #[test]
    fn pedir_mas_de_la_cuenta_se_contesta_que_no() {
        let prk = extraer(b"", b"s");
        let mut poco = [0u8; 64];
        assert!(expandir(&prk, b"x", &mut poco));
        // 255 bloques de 32 son 8.160 bytes: uno mas y ya no se puede.
        let mut demasiado = alloc::vec![0u8; 255 * 32 + 1];
        assert!(!expandir(&prk, b"x", &mut demasiado));
    }
}

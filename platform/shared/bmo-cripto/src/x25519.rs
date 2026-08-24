//! **X25519.** La pieza dificil, y la que hace posible todo lo demas.
//!
//! # Que resuelve, y por que parece magia
//!
//! Dos maquinas que **nunca se han visto** y que hablan por un cable **que
//! cualquiera puede leer** acaban las dos con el mismo secreto de 32 bytes, y
//! quien escuchaba no lo tiene.
//!
//! ```text
//!    Alicia          el cable             Benito
//!    ------          --------             ------
//!    a (secreto)                          b (secreto)
//!    A = a*G     -->    A, B      <--     B = b*G
//!    a*B = a*b*G                          b*A = a*b*G      <- EL MISMO
//! ```
//!
//! ** Funciona porque `a*(b*G)` y `b*(a*G)` son el mismo punto, y porque **ir de
//! `a*G` a `a` no se sabe hacer**. Eso ultimo es el problema del logaritmo
//! discreto, y es toda la seguridad que hay aqui.
//!
//! # Por que Curve25519 y no otra
//!
//! Porque **no tiene casos especiales**. Otras curvas piden comprobar si el
//! punto que te mandaron esta de verdad en la curva, si es el infinito, si es de
//! orden pequeno -- y cada comprobacion que falta es un agujero. En X25519:
//!
//! ```text
//!    cualquier cadena de 32 bytes es una `u` valida
//!    no hay punto en el infinito que codificar
//!    no hay que comprobar que el punto este en la curva
//! ```
//!
//! *** Eso no es comodidad: es que **los fallos de los que la gente se olvida
//! aqui no existen**. Y esa es exactamente la clase de decision que este
//! proyecto persigue -- la que no depende de que alguien se acuerde.
//!
//! # [!] Y LO QUE ESTA PIEZA NO TRAE, dicho antes de que se suponga
//!
//! **No genera claves.** `secreto_a_publico` convierte un secreto en su publica,
//! y de donde salgan esos 32 bytes es otro problema: hace falta un generador de
//! aleatorios que no exista en este crate todavia.
//!
//! *** Y esa ausencia es la mas peligrosa de todo el techo, porque **una clave
//! predecible es PEOR que no cifrar**: no cifrar se nota, y cifrar con una clave
//! que otro puede adivinar parece que protege. Ese sera su propio fichero y su
//! propia pagina.

use crate::campo25519 as f;
use crate::campo25519::Fe;

/// Bytes de una clave, de una publica y de un secreto compartido. Los tres 32.
pub const LARGO: usize = 32;

/// **El punto base**, y es un `9`. Nada mas.
///
/// ** Que la constante mas importante de la curva sea el numero nueve es parte
/// de su diseno: no hay nada escondido en un punto base que se puede escribir
/// entero en una linea. Es el mismo criterio que las raices de SHA-256.
pub const BASE: [u8; LARGO] = {
    let mut b = [0u8; LARGO];
    b[0] = 9;
    b
};

/// `(486662 - 2) / 4`. La constante de la curva que entra en cada vuelta.
const A24: Fe = [121665, 0, 0, 0, 0];

/// **AJUSTA EL ESCALAR.** Tres lineas del RFC 7748, y cada una tapa un agujero.
///
/// ```text
///    k[0]  &= 248     borra los tres bits BAJOS
///    k[31] &= 127     borra el bit ALTO
///    k[31] |= 64      pone el bit 254
/// ```
///
/// *** Y las tres tienen motivo, que es lo que las hace no parecer supersticion:
///
/// 1. **Los tres bits bajos a cero** hacen el escalar multiplo de 8, que es el
///    cofactor de la curva. Con eso, un punto de orden pequeno --de los que un
///    atacante puede mandar a proposito-- se convierte en el neutro y **no filtra
///    nada del secreto**. Sin esto, quien te mande un punto malicioso aprende
///    tres bits de tu clave.
///
/// 2. **El bit alto a cero** porque el escalar es de 255 bits, no de 256.
///
/// 3. **El bit 254 a uno** fija el tamano: la escalera hace SIEMPRE las mismas
///    255 vueltas pase lo que pase. Sin el, una clave que empezara por ceros
///    daria menos vueltas -- **y el tiempo contaria cuantos ceros tiene la
///    clave.**
///
/// ** Se aplica sobre una COPIA y no sobre lo que dio el llamante: cambiarle el
/// buffer a quien te llama es la clase de sorpresa que este proyecto no tiene.
fn ajustar(k: &[u8; LARGO]) -> [u8; LARGO] {
    let mut e = *k;
    e[0] &= 248;
    e[31] &= 127;
    e[31] |= 64;
    e
}

/// **La escalera de Montgomery.** El corazon de X25519.
///
/// ## Que hace y por que esta escrita asi
///
/// Multiplica el punto `u` por el escalar `k`, mirando **un bit por vuelta, de
/// arriba a abajo**. En cada vuelta mantiene dos puntos que se diferencian
/// siempre en `u`, y segun el bit los actualiza en un orden o en el otro.
///
/// *** Y la propiedad que la hace valer para criptografia: **hace exactamente el
/// mismo trabajo mire lo que mire el bit.** No hay `if`, no hay bucle que dure
/// mas o menos, no hay tabla que se consulte en un sitio distinto. Lo unico que
/// el bit decide es un INTERCAMBIO, y ese intercambio se hace siempre --
/// `campo25519::intercambio` cambia una mascara, no un camino.
///
/// ** Sin eso, el escalar --que ES la clave privada-- se filtraria por el
/// tiempo: 255 bits, y cada uno decidiendo una rama.
///
/// [!] Y solo se lleva la coordenada `u`. La `v` no hace falta para multiplicar
/// en una curva de Montgomery, y no llevarla es lo que hace que **cualquier
/// cadena de 32 bytes sea una entrada valida**: no hay nada que comprobar
/// porque no hay nada que pueda estar mal.
fn escalera(k: &[u8; LARGO], u: &Fe) -> Fe {
    let x1 = *u;
    let mut x2 = f::UNO;
    let mut z2 = f::CERO;
    let mut x3 = *u;
    let mut z3 = f::UNO;
    let mut cambiado: u64 = 0;

    // De 254 a 0: son los 255 bits que `ajustar` dejo, y **siempre los mismos**.
    for t in (0..255).rev() {
        let bit = ((k[t >> 3] >> (t & 7)) & 1) as u64;
        cambiado ^= bit;
        f::intercambio(cambiado, &mut x2, &mut x3);
        f::intercambio(cambiado, &mut z2, &mut z3);
        cambiado = bit;

        // Las diez operaciones de una vuelta. Es la formula del RFC 7748 tal
        // cual: cambiarle el orden a esto no la mejora, la rompe.
        let a = f::suma(&x2, &z2);
        let aa = f::cuadrado(&a);
        let b = f::resta(&x2, &z2);
        let bb = f::cuadrado(&b);
        let e = f::resta(&aa, &bb);
        let c = f::suma(&x3, &z3);
        let d = f::resta(&x3, &z3);
        let da = f::mul(&d, &a);
        let cb = f::mul(&c, &b);
        x3 = f::cuadrado(&f::suma(&da, &cb));
        z3 = f::mul(&x1, &f::cuadrado(&f::resta(&da, &cb)));
        x2 = f::mul(&aa, &bb);
        // *** `AA` Y NO `BB`, Y AQUI SE ESCRIBE POR QUE.
        //
        // ** Hay DOS formulas publicadas para esta linea y **parecen
        // intercambiables**:
        //
        //    RFC 7748    z_2 = E * (AA + a24*E)   con a24 = 121665
        //    la clasica  z_2 = E * (BB + a24*E)   con a24 = 121666
        //
        // Son la misma: `AA = BB + E`, asi que `AA + 121665*E` es
        // `BB + 121666*E`. **Pero cruzarlas --usar `BB` con 121665-- da un
        // resultado que se sale por E**, y eso es lo que se escribio primero.
        //
        // *** Y el fallo NO SE VE: el campo sigue cuadrando, `a * a^-1` sigue
        // dando 1, el punto sale con sus 32 bytes y con pinta de bueno. Lo unico
        // que lo caza son los vectores del RFC -- que es exactamente para lo que
        // la regla 1 de este crate dice que nada entra sin ellos.
        z2 = f::mul(&e, &f::suma(&aa, &f::mul(&A24, &e)));
    }
    // El ultimo intercambio, fuera del bucle: el bit 0 lo dejo pendiente.
    f::intercambio(cambiado, &mut x2, &mut x3);
    f::intercambio(cambiado, &mut z2, &mut z3);

    // La escalera trabaja en coordenadas proyectivas --un punto es `x/z`-- y la
    // division se hace UNA vez, al final. Es lo que ahorra 254 inversiones.
    f::mul(&x2, &f::invertir(&z2))
}

/// **La operacion de X25519**: multiplica el punto `u` por el escalar `k`.
///
/// Es la unica funcion que hace falta: con el punto base da una clave publica,
/// y con la publica del otro da el secreto compartido.
pub fn x25519(k: &[u8; LARGO], u: &[u8; LARGO]) -> [u8; LARGO] {
    let e = ajustar(k);
    let punto = f::desde_bytes(u);
    f::a_bytes(&escalera(&e, &punto))
}

/// **De un secreto de 32 bytes, su clave publica.** Es `k` por el punto base.
///
/// [!] De donde salgan esos 32 bytes es otro problema, y es el mas peligroso
/// que le queda a este crate: **sin un generador de aleatorios de verdad, una
/// clave predecible es peor que no cifrar** -- porque parece que protege.
pub fn secreto_a_publico(secreto: &[u8; LARGO]) -> [u8; LARGO] {
    x25519(secreto, &BASE)
}

/// **El secreto compartido**: mi secreto por la publica del otro.
///
/// ## [!] Y LA COMPROBACION QUE HAY QUE HACER ENCIMA, dicha aqui
///
/// Si el resultado sale **todo ceros**, la publica que te mandaron era de orden
/// pequeno y el secreto no vale nada. El RFC 7748 dice que quien lo use en un
/// protocolo **tiene que mirarlo**, y esta funcion no puede hacerlo por ti
/// porque no sabe si tu protocolo lo permite.
///
/// *** Devolver `Option` seria mentir en la direccion contraria: haria pensar
/// que la comprobacion ya esta hecha en todos los casos. Asi que se devuelve el
/// resultado, se dice aqui, y existe [`es_cero`] para preguntarlo.
pub fn secreto_compartido(mi_secreto: &[u8; LARGO], su_publica: &[u8; LARGO]) -> [u8; LARGO] {
    x25519(mi_secreto, su_publica)
}

/// **Salio todo ceros?** En tiempo constante.
///
/// Un secreto compartido de ceros significa que la publica del otro era de
/// orden pequeno: o se equivoco, o lo hizo a proposito. En los dos casos, seguir
/// es hablar con una clave que el atacante conoce.
pub fn es_cero(s: &[u8; LARGO]) -> bool {
    let mut sobra = 0u8;
    for b in s.iter() {
        sobra |= *b;
    }
    sobra == 0
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn de_hex(s: &str) -> [u8; 32] {
        let b = s.as_bytes();
        let mut o = [0u8; 32];
        for i in 0..32 {
            let hi = (b[i * 2] as char).to_digit(16).unwrap() as u8;
            let lo = (b[i * 2 + 1] as char).to_digit(16).unwrap() as u8;
            o[i] = (hi << 4) | lo;
        }
        o
    }
    fn hex(b: &[u8]) -> alloc::string::String {
        use core::fmt::Write;
        let mut s = alloc::string::String::new();
        for x in b {
            let _ = write!(s, "{:02x}", x);
        }
        s
    }

    /// *** LOS VECTORES DEL RFC 7748, seccion 5.2. La regla 1 del crate.
    #[test]
    fn los_vectores_del_rfc_7748() {
        let k = de_hex("a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4");
        let u = de_hex("e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c");
        assert_eq!(
            hex(&x25519(&k, &u)),
            "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552"
        );

        let k = de_hex("4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d");
        let u = de_hex("e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493");
        assert_eq!(
            hex(&x25519(&k, &u)),
            "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957"
        );
    }

    /// *** EL APRETON ENTERO (RFC 7748, seccion 6.1), que es para lo que existe.
    ///
    /// ** Alicia y Benito no se han visto, sus publicas viajan por un cable que
    /// cualquiera puede leer, y los dos acaban con **los mismos 32 bytes**. Esta
    /// es la prueba que dice que la pieza sirve, y no solo que calcula.
    #[test]
    fn alicia_y_benito_llegan_al_mismo_secreto() {
        let a_priv = de_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let b_priv = de_hex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");

        let a_pub = secreto_a_publico(&a_priv);
        let b_pub = secreto_a_publico(&b_priv);
        assert_eq!(
            hex(&a_pub),
            "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a"
        );
        assert_eq!(
            hex(&b_pub),
            "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f"
        );

        let s1 = secreto_compartido(&a_priv, &b_pub);
        let s2 = secreto_compartido(&b_priv, &a_pub);
        assert_eq!(s1, s2, "el apreton no llego al mismo sitio");
        assert_eq!(
            hex(&s1),
            "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742"
        );
        assert!(!es_cero(&s1));
    }

    /// *** UN PUNTO DE ORDEN PEQUENO DA CERO, y por eso hay que mirarlo.
    ///
    /// ** El `0` como clave publica es el caso mas simple: quien lo mande hace
    /// que el secreto compartido sea cero **sea cual sea tu clave privada**. Si
    /// nadie lo comprueba, las dos partes siguen hablando con una clave que el
    /// atacante conoce -- y todo *"funciona"*.
    ///
    /// Es exactamente la frase que gobierna este crate, en su forma mas literal.
    #[test]
    fn una_publica_de_orden_pequeno_da_cero_y_se_puede_ver() {
        let mi = de_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let malo = [0u8; 32];
        let s = secreto_compartido(&mi, &malo);
        assert!(es_cero(&s), "tiene que salir cero para que se pueda cazar");

        // Y el `1`, que es el otro de la lista corta.
        let mut uno = [0u8; 32];
        uno[0] = 1;
        assert!(es_cero(&secreto_compartido(&mi, &uno)));
    }

    /// [!] EL BIT 255 DE LA `u` SE IGNORA, y eso lo exige el RFC.
    ///
    /// ** Si no se ignorara, el mismo punto tendria DOS codificaciones -- y dos
    /// claves publicas "distintas" que dan el mismo secreto es justo la clase de
    /// ambiguedad por la que se cuelan los ataques de repeticion.
    #[test]
    fn el_bit_alto_de_la_u_no_cuenta() {
        let k = de_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let u = de_hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let mut con_bit = u;
        con_bit[31] |= 0x80;
        assert_eq!(x25519(&k, &u), x25519(&k, &con_bit));
    }

    /// El ajuste del escalar se hace sobre una COPIA: al llamante no se le toca
    /// el buffer. Y dos secretos que solo se diferencien en los bits que el
    /// ajuste borra dan **la misma publica**, que es la prueba de que el ajuste
    /// se aplica de verdad.
    #[test]
    fn el_ajuste_no_toca_lo_que_le_dieron_y_si_se_aplica() {
        let base = de_hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let mut copia = base;
        let _ = secreto_a_publico(&copia);
        assert_eq!(copia, base, "le cambio el buffer a quien llamo");

        // Los tres bits bajos y el alto los borra el ajuste: cambiarlos no puede
        // cambiar el resultado.
        copia[0] |= 0b111;
        copia[31] |= 0x80;
        copia[31] |= 0x40;
        assert_eq!(secreto_a_publico(&copia), secreto_a_publico(&base));
    }

    /// La aritmetica del campo se sostiene sola: `a * a^-1 = 1`.
    ///
    /// ** No prueba que sea Curve25519 --eso lo dicen los vectores-- pero si que
    /// la inversion y el producto se entienden entre ellos. Un fallo aqui daria
    /// puntos que parecen de la curva y no lo son.
    #[test]
    fn el_inverso_deshace_el_producto() {
        use crate::campo25519 as f;
        for semilla in [2u64, 3, 12345, 0x7FFF_FFFF_FFFF] {
            let a: f::Fe = [semilla, semilla + 1, semilla + 2, semilla + 3, semilla + 4];
            let r = f::mul(&a, &f::invertir(&a));
            assert_eq!(f::a_bytes(&r), f::a_bytes(&f::UNO), "fallo con {semilla}");
        }
    }
}

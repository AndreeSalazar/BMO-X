//! **Los vectores del RFC 8032, y seis formas de decir que no.**
//!
//! La regla del crate: *"nada entra sin sus vectores oficiales"*. Y aqui hace
//! mas falta que en ningun otro sitio, porque **una firma mal comprobada no
//! falla: dice que si.**
//!
//! ## Por que hay tantas pruebas NEGATIVAS
//!
//! Un verificador que devolviera `true` siempre pasaria los cuatro vectores del
//! RFC de un tiron. **Los positivos no prueban un verificador: prueban un
//! calculador.** Lo que separa a los dos es lo que rechaza.
//!
//! * Y la ultima de la lista tiene nombre y fecha: el 2026-08-24 se quito del
//! arbol un `verify_ed25519` que contestaba `true` a una firma de **ceros**. Esa
//! firma esta aqui abajo, y ahora tiene quien le diga que no.

use super::*;

fn hex(s: &str) -> alloc::vec::Vec<u8> {
    let b = s.as_bytes();
    let mut out = alloc::vec::Vec::with_capacity(b.len() / 2);
    let val = |c: u8| -> u8 {
        match c {
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            _ => panic!("no es hex"),
        }
    };
    let mut i = 0;
    while i + 1 < b.len() {
        out.push((val(b[i]) << 4) | val(b[i + 1]));
        i += 2;
    }
    out
}

fn k32(s: &str) -> [u8; 32] {
    let v = hex(s);
    let mut a = [0u8; 32];
    a.copy_from_slice(&v);
    a
}
fn k64(s: &str) -> [u8; 64] {
    let v = hex(s);
    let mut a = [0u8; 64];
    a.copy_from_slice(&v);
    a
}

/// Los cuatro casos de RFC 8032, seccion 7.1: `(publica, mensaje, firma)`.
const VECTORES: &[(&str, &str, &str)] = &[
    // TEST 1 -- el MENSAJE VACIO. No es un caso raro: es el que comprueba que
    // el hash del reto se alimenta en el orden bueno aunque no haya mensaje.
    (
        "d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a",
        "",
        "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
    ),
    // TEST 2 -- un solo byte.
    (
        "3d4017c3e843895a92b70aa74d1b7ebc9c982ccf2ec4968cc0cd55f12af4660c",
        "72",
        "92a009a9f0d4cab8720e820b5f642540a2b27b5416503f8fb3762223ebdb69da085ac1e43e15996e458f3613d0f11d8c387b2eaeb4302aeeb00d291612bb0c00",
    ),
    // TEST 3 -- dos bytes.
    (
        "fc51cd8e6218a1a38da47ed00230f0580816ed13ba3303ac5deb911548908025",
        "af82",
        "6291d657deec24024827e69c3abe01a30ce548a284743a445e3680d7db5ac3ac18ff9b538d16f290ae67f760984dc6594a7c15e9716ed28dc027beceea1ec40a",
    ),
    // TEST SHA(abc) -- 64 bytes de mensaje, o sea el bloque entero de SHA-512.
    (
        "ec172b93ad5e563bf4932c70e1245034c35467ef2efd4d64ebf819683467e2bf",
        "ddaf35a193617abacc417349ae20413112e6fa4e89a97ea20a9eeee64b55d39a2192992a274fc1a836ba3c23a3feebbd454d4423643ce80e2a9ac94fa54ca49f",
        "dc2a4459e7369633a52b1bf277839a00201009a3efbf3ecb69bea2186c26b58909351fc9ac90b3ecfdfbc7c66431e0303dca179c138ac17ad9bef1177331a704",
    ),
];

/// **Los cuatro vectores del RFC verifican.** Si esto falla, no hace falta mirar
/// nada mas de este fichero.
#[test]
fn los_vectores_del_rfc_8032() {
    for (i, (pk, msg, sig)) in VECTORES.iter().enumerate() {
        assert!(
            verificar(&k32(pk), &hex(msg), &k64(sig)),
            "el vector {i} del RFC tiene que verificar"
        );
    }
}

/// **Un bit del MENSAJE y la firma deja de valer.** Es lo que una firma existe
/// para dar, y el unico de los negativos que un usuario nota.
#[test]
fn un_bit_del_mensaje_tumba_la_firma() {
    let (pk, msg, sig) = VECTORES[3];
    let mut m = hex(msg);
    for i in 0..m.len() {
        m[i] ^= 1;
        assert!(
            !verificar(&k32(pk), &m, &k64(sig)),
            "cambiando el byte {i} del mensaje NO puede seguir valiendo"
        );
        m[i] ^= 1;
    }
}

/// **Un bit de la FIRMA, en las dos mitades.**
///
/// Se prueban las dos --`R` y `S`-- porque **son dos caminos distintos dentro de
/// `verificar`**: `R` pasa por la descompresion y por la comparacion final, y
/// `S` por el rango y por la multiplicacion. Romper una no comprueba la otra.
#[test]
fn un_bit_de_la_firma_la_tumba() {
    let (pk, msg, sig) = VECTORES[1];
    let m = hex(msg);
    let base = k64(sig);
    for i in 0..64 {
        let mut s = base;
        s[i] ^= 0x40;
        assert!(
            !verificar(&k32(pk), &m, &s),
            "cambiando el byte {i} de la firma NO puede seguir valiendo"
        );
    }
}

/// **La firma de otro mensaje, con la clave buena.** El ataque mas obvio y el
/// que un `true` constante deja pasar.
#[test]
fn la_firma_de_otro_mensaje_no_vale_aqui() {
    let (pk, _, _) = VECTORES[1];
    let (_, _, sig_de_otro) = VECTORES[2];
    assert!(!verificar(&k32(pk), &hex("72"), &k64(sig_de_otro)));
}

/// **La clave publica de otro.** Cambia quien firma, no que se firmo.
#[test]
fn la_clave_de_otro_no_vale() {
    let (_, msg, sig) = VECTORES[2];
    let (otra, _, _) = VECTORES[1];
    assert!(!verificar(&k32(otra), &hex(msg), &k64(sig)));
}

/// *** **MALEABILIDAD: `S + L` tiene que ser RECHAZADA.**
///
/// Sin la comprobacion de rango, a una firma valida se le suma `L` a la `S` y
/// sale **otra firma que tambien verifica**: mismo mensaje, misma clave, dos
/// firmas distintas.
///
/// Y eso rompe cualquier cosa que use la firma como identidad -- dos `.bex` con
/// bytes distintos y la misma autoria, o un registro que cree haber visto dos
/// entregas donde hubo una. Es la unica prueba de este fichero que no habla de
/// criptografia sino de **contabilidad**.
#[test]
fn sumarle_el_orden_del_grupo_a_s_no_cuela() {
    let (pk, msg, sig) = VECTORES[1];
    let base = k64(sig);
    assert!(verificar(&k32(pk), &hex(msg), &base), "la de partida vale");

    let mut malea = base;
    // S + L, en little-endian y con acarreo.
    let mut acarreo = 0u16;
    for i in 0..32 {
        let v = malea[32 + i] as u16 + L[i] as u16 + acarreo;
        malea[32 + i] = (v & 0xFF) as u8;
        acarreo = v >> 8;
    }
    assert_ne!(malea, base, "la S tiene que haber cambiado");
    assert!(
        !verificar(&k32(pk), &hex(msg), &malea),
        "S + L es la MISMA firma con otros bytes: hay que rechazarla"
    );
}

/// *** **LA FIRMA DE CEROS, que es la que empezo todo esto.**
///
/// El 2026-08-24 se quito de `bmo-abi` un `verify_ed25519` que hacia esto:
///
/// ```text
///    let is_unsigned = sig todo ceros && pubkey todo ceros;
///    if is_unsigned { return true; }
/// ```
///
/// O sea que **para pasar el control no habia que falsificar una firma: habia
/// que borrarla**. No lo llamaba nadie, y esa era la unica razon de que no fuera
/// un agujero.
///
/// Aqui esta el mismo caso, con las cuatro combinaciones, y **las cuatro tienen
/// que dar `false`**.
#[test]
fn la_firma_de_ceros_ya_tiene_quien_le_diga_que_no() {
    let (pk, msg, sig) = VECTORES[0];
    let ceros32 = [0u8; 32];
    let ceros64 = [0u8; 64];
    let m = hex(msg);

    assert!(!verificar(&ceros32, &m, &ceros64), "todo a cero");
    assert!(!verificar(&k32(pk), &m, &ceros64), "clave buena, firma de ceros");
    assert!(!verificar(&ceros32, &m, &k64(sig)), "firma buena, clave de ceros");
    assert!(!verificar(&ceros32, &m, &ceros64), "y otra vez, por si acaso");
}

/// **Una clave publica que no es ningun punto de la curva.**
///
/// La mayoria de los 32 bytes posibles no lo son. Lo que se comprueba es que
/// `verificar` conteste `false` y **no se lleve el programa por delante** -- que
/// en Ring 0 no seria un test rojo, seria la maquina parada.
#[test]
fn una_clave_que_no_esta_en_la_curva_se_rechaza_sin_estallar() {
    let (_, msg, sig) = VECTORES[1];
    let m = hex(msg);
    let s = k64(sig);
    let mut rechazadas = 0;
    for semilla in 0u8..64 {
        let mut pk = [semilla; 32];
        pk[31] = semilla;
        if !verificar(&pk, &m, &s) {
            rechazadas += 1;
        }
    }
    assert_eq!(rechazadas, 64, "ninguna de esas puede valer");
}

/// **El generador se descomprime.** Va aparte porque si esto fallara, TODO lo
/// demas fallaria y el motivo no se veria: `verificar` contestaria `false` a los
/// cuatro vectores y pareceria un fallo de la curva.
#[test]
fn el_generador_se_descomprime() {
    let b = descomprimir(&BASE).expect("el generador tiene que descomprimir");
    assert_eq!(comprimir(&b), BASE, "y volver a comprimir da lo mismo");
}

/// **`[L]B` es el neutro.** Es la definicion de que `L` sea el orden del grupo,
/// y de paso comprueba la multiplicacion con un escalar grande.
///
/// * Si la suma de puntos estuviera mal en el caso de doblar, esto no daria el
/// neutro -- y es un fallo que los vectores del RFC podrian no destapar, porque
/// alli el escalar es el que es.
#[test]
fn el_orden_del_grupo_lleva_el_generador_al_neutro() {
    let b = descomprimir(&BASE).unwrap();
    let r = por_escalar(&L, &b);
    assert_eq!(comprimir(&r), comprimir(&NEUTRO), "[L]B tiene que ser el neutro");
}

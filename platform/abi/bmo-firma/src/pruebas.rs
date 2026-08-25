//! **El gate, contra firmas de verdad.**
//!
//! Se firman secciones con `bmo-cripto` --la bandera `firmar`, que aqui SI se
//! enciende porque un gate no se puede probar contra firmas que no existen-- y
//! se comprueba que cada veredicto salga por su motivo.
//!
//! ## Lo que estas pruebas tienen que separar
//!
//! Un gate que devolviera `Firmado` siempre pasaria la mitad de esto. Lo que lo
//! distingue de un sello de goma son los **cuatro noes distintos**, y sobre todo
//! el que da nombre al crate:
//!
//! > **`AutorDesconocido`: la firma cuadra, y la clave no la conozco.**
//!
//! Ese es el unico caso en que la aritmetica dice que si y la respuesta es que
//! no. Si el gate lo confundiera con `Firmado`, todo lo demas seguiria en verde.

use super::*;

extern crate std;
use std::vec::Vec;

use bmo_cripto::ed25519;

const SEMILLA_A: [u8; 32] = [0x11; 32];
const SEMILLA_B: [u8; 32] = [0x22; 32];

/// Una cadena de digests cualquiera. Lo que se firma.
const CADENA: [u8; 32] = [0xAB; 32];

/// Arma una seccion `Signature` como la escribe el formato.
///
/// * Se construye A MANO y no con el escritor de `bmo-abi`, por lo mismo que en
/// `bmo-maqueta-cara`: si la armara su pareja, un fallo del escritor daria bytes
/// que el gate rechazaria... y el banco saldria verde porque rechazar es lo que
/// hace.
fn seccion(hashes: usize, algo: u32, firma: Option<([u8; 64], [u8; 32])>) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&(hashes as u32).to_le_bytes());
    v.extend_from_slice(&algo.to_le_bytes());
    for k in 0..hashes {
        v.extend_from_slice(&(k as u16).to_le_bytes());
        v.extend_from_slice(&[0u8; 6]);
        v.extend_from_slice(&[k as u8; 32]);
    }
    if let Some((sig, pk)) = firma {
        v.extend_from_slice(&sig);
        v.extend_from_slice(&pk);
    }
    v
}

fn firmada_por(semilla: &[u8; 32], cadena: &[u8; 32], hashes: usize) -> (Vec<u8>, [u8; 32]) {
    let pk = ed25519::publica_de(semilla);
    let sig = ed25519::firmar(semilla, &cadena[..]);
    (seccion(hashes, ALGO_ED25519, Some((sig, pk))), pk)
}

/// **Lo que trae hoy todo `.bex` del arbol**: hashes y nada mas.
///
/// [!] Y NO es un fallo. Es `SoloIntegridad`, que dice la verdad -- y lo que
/// decide si con eso se puede ejecutar es la POLITICA, no este crate.
#[test]
fn una_seccion_de_hoy_dice_solo_integridad() {
    let s = seccion(4, ALGO_NINGUNO, None);
    let v = examinar(&s, &CADENA, &[]);
    assert_eq!(v, Veredicto::SoloIntegridad);
    assert!(v.permite_ejecutar(false), "sin exigir firma, pasa");
    assert!(!v.permite_ejecutar(true), "exigiendo firma, no");
}

/// El caso bueno: firmada, y la clave esta en el ancla.
#[test]
fn firmada_por_una_clave_del_ancla() {
    let (s, pk) = firmada_por(&SEMILLA_A, &CADENA, 3);
    let ancla = [pk];
    let v = examinar(&s, &CADENA, &ancla);
    assert_eq!(v, Veredicto::Firmado { clave: 0 });
    assert!(v.permite_ejecutar(true), "esto SI puede ejecutarse");
}

/// Y con varias claves en el ancla, dice CUAL. Un `true` no podria.
#[test]
fn el_ancla_dice_cual_de_sus_claves_firmo() {
    let (s, pk_b) = firmada_por(&SEMILLA_B, &CADENA, 1);
    let ancla = [ed25519::publica_de(&SEMILLA_A), pk_b, [0u8; 32]];
    assert_eq!(examinar(&s, &CADENA, &ancla), Veredicto::Firmado { clave: 1 });
}

/// *** **LA PRUEBA QUE DA NOMBRE AL CRATE.**
///
/// La firma es perfecta: la hizo una clave de verdad sobre esta cadena, y la
/// aritmetica lo confirma. **Y la respuesta es que no**, porque esa clave no
/// esta en el ancla.
///
/// Sin esto, el gate diria `Firmado` a cualquiera que se genere un par de claves
/// -- que es lo que pasaria si se cableara Ed25519 sin ancla, porque **la clave
/// publica viaja DENTRO de la firma**.
#[test]
fn una_firma_impecable_de_un_desconocido_se_rechaza() {
    let (s, pk) = firmada_por(&SEMILLA_B, &CADENA, 2);

    // Con su clave en el ancla: pasa.
    assert_eq!(examinar(&s, &CADENA, &[pk]), Veredicto::Firmado { clave: 0 });

    // Con el ancla de OTRO: la misma firma, el mismo fichero, y no pasa.
    let otro = ed25519::publica_de(&SEMILLA_A);
    let v = examinar(&s, &CADENA, &[otro]);
    assert_eq!(v, Veredicto::AutorDesconocido);
    assert!(!v.permite_ejecutar(true));
    assert!(!v.permite_ejecutar(false), "ni siquiera sin exigir firma");
}

/// **Y con el ancla VACIA no vale nadie.** Es la respuesta correcta cuando no se
/// ha decidido en quien confiar -- no "adelante".
#[test]
fn con_el_ancla_vacia_toda_firma_es_de_un_desconocido() {
    let (s, _) = firmada_por(&SEMILLA_A, &CADENA, 1);
    assert_eq!(examinar(&s, &CADENA, &[]), Veredicto::AutorDesconocido);
}

/// **La cadena cambio**: la firma es de OTROS bytes. Ese es el caso que la firma
/// existe para cazar.
#[test]
fn si_la_cadena_cambia_la_firma_no_cuadra() {
    let (s, pk) = firmada_por(&SEMILLA_A, &CADENA, 3);
    let mut otra = CADENA;
    otra[7] ^= 1;
    assert_eq!(examinar(&s, &otra, &[pk]), Veredicto::NoCuadra);
}

/// Tocar la firma, byte a byte. Los 64 de `sig` tienen que dar `NoCuadra`, y los
/// 32 de la clave tienen que dar **otra cosa** -- porque cambiar la clave no
/// rompe la aritmetica de la misma forma.
#[test]
fn tocar_la_firma_o_la_clave_no_dan_el_mismo_no() {
    let (base, pk) = firmada_por(&SEMILLA_A, &CADENA, 2);
    let off = donde_esta_la_firma(&base).unwrap();
    for i in 0..96 {
        let mut s = base.clone();
        s[off + i] ^= 0x40;
        let v = examinar(&s, &CADENA, &[pk]);
        assert_ne!(v, Veredicto::Firmado { clave: 0 }, "el byte {i} no puede seguir valiendo");
        assert!(
            matches!(v, Veredicto::NoCuadra | Veredicto::AutorDesconocido),
            "el byte {i} dio {v:?}"
        );
    }
}

/// **Un algoritmo desconocido se RECHAZA, no se ignora.**
///
/// Un `.bex` que declara un algoritmo que este sistema no implementa puede estar
/// firmado perfectamente por otro. Tratarlo como "sin firma" seria degradarlo en
/// silencio a un control mas flojo.
#[test]
fn un_algoritmo_que_no_conozco_no_se_degrada_a_sin_firma() {
    let s = seccion(2, 7, Some(([0u8; 64], [0u8; 32])));
    let v = examinar(&s, &CADENA, &[]);
    assert_eq!(v, Veredicto::AlgoritmoDesconocido(7));
    assert!(!v.permite_ejecutar(false), "ni aun sin exigir firma");
}

/// **Una cabecera que promete mas entradas de las que hay.**
///
/// Es el mismo ataque que `LasCuentasNoCaben` en la cara que viaja: `8 + n*40`
/// con `n` hostil da la vuelta en 32 bits y apunta DENTRO de la seccion. Por eso
/// la cuenta se hace en `u64`.
#[test]
fn una_cabecera_que_miente_sobre_su_tamano_es_seccion_rota() {
    let mut s = seccion(2, ALGO_ED25519, Some(([0u8; 64], [0u8; 32])));
    s[0..4].copy_from_slice(&9999u32.to_le_bytes());
    assert_eq!(examinar(&s, &CADENA, &[]), Veredicto::SeccionRota);

    // Y el desbordamiento de verdad.
    let mut s = seccion(2, ALGO_ED25519, Some(([0u8; 64], [0u8; 32])));
    s[0..4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(examinar(&s, &CADENA, &[]), Veredicto::SeccionRota);
}

/// Una seccion mas corta que su cabecera, y una firmada a la que le falta el
/// bloque de firma. Dos formas de estar rota y las dos se dicen igual.
#[test]
fn una_seccion_truncada_no_estalla() {
    for n in 0..CABECERA {
        let s = seccion(1, ALGO_ED25519, None);
        assert_eq!(examinar(&s[..n], &CADENA, &[]), Veredicto::SeccionRota);
    }
    // Firmada segun la cabecera, pero sin los 96 bytes detras.
    let s = seccion(3, ALGO_ED25519, None);
    assert_eq!(examinar(&s, &CADENA, &[]), Veredicto::SeccionRota);
}

/// **Nada de lo que llegue puede hacer estallar el gate.** En Ring 0 un panico
/// no es un test rojo: es la maquina parada.
#[test]
fn ningun_byte_corrompido_tumba_el_gate() {
    let (base, pk) = firmada_por(&SEMILLA_A, &CADENA, 2);
    for i in 0..base.len() {
        for v in [0x00u8, 0xFF] {
            let mut s = base.clone();
            s[i] = v;
            let _ = examinar(&s, &CADENA, &[pk]);
        }
    }
}

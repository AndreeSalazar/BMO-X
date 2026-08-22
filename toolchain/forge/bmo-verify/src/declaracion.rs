//! **Lo que el binario dice de si mismo, y como exigirlo.**
//!
//! ## Por que es un fichero aparte
//!
//! `verify()` contesta *"es admisible el envase?"*. `ram` contesta *"como va a
//! viajar?"*. Esto contesta una tercera: **"declara este binario lo que es?"**
//!
//! Son tres preguntas y la regla de esta casa es que no comparten cajon.
//!
//! ## ** Y por que es OPCIONAL, y tiene que serlo
//!
//! Exigir el manifiesto dentro de `verify()` rechazaria hoy todos los `.bex` de
//! BMO C, COBOL y Ada, que no lo escriben. Eso no seria un gate mas estricto:
//! seria el toolchain dejando de compilar.
//!
//! Asi que es una politica que el productor **elige**, y el primero que la
//! elige es INTI sobre si mismo. Ese es el punto entero: un compilador que
//! escribe el manifiesto y ademas se obliga a comprobarlo **no puede volver a
//! escribir un binario mudo por accidente**. El dia que alguien rompa el
//! cableado, el compilador se niega a escribir el fichero en vez de sacar un
//! `.bex` correcto por dentro y sin nombre por fuera.
//!
//! Es lo mismo que `perfil/mod.rs` llevaba escrito desde antes y no era verdad:
//! *"va al informe del `.bex` para que `bmo-verify` pueda exigirlo firmado"*.
//! Esta es la mitad que faltaba para que lo sea.

use bmo_abi::bef::katanas;
use bmo_abi::bef::paquete;
use bmo_abi::bef::sections::SectionKind;

use crate::Verdict;

/// Los bytes del manifiesto, si el binario lo trae.
///
/// No lo interpreta: **este crate no sabe TOML y no tiene por que**. Quien
/// escribio el manifiesto sabe leerlo; aqui solo se comprueba que exista y que
/// sea texto.
pub fn manifiesto(bef: &[u8]) -> Option<&[u8]> {
    paquete::seccion(bef, SectionKind::Manifest)
}

/// **`verify()` mas una exigencia: que el binario declare lo que es.**
///
/// Estrictamente mas fuerte que `verify()` -- lo llama primero, asi que nada
/// que pase por aqui deja de pasar por el gate normal.
pub fn exige_manifiesto(bef: &[u8]) -> Verdict {
    let base = crate::verify(bef);
    if !base.is_ok() {
        return base;
    }
    let datos = match manifiesto(bef) {
        Some(d) => d,
        None => {
            return Verdict::Rejected(vec![String::from(
                "el binario no declara lo que es: falta la seccion Manifest (0x09)",
            )])
        }
    };
    if datos.is_empty() {
        return Verdict::Rejected(vec![String::from(
            "la seccion Manifest esta vacia: declarar nada no es declarar",
        )]);
    }
    if core::str::from_utf8(datos).is_err() {
        return Verdict::Rejected(vec![String::from(
            "la seccion Manifest no es UTF-8, y el manifiesto es texto",
        )]);
    }
    Verdict::Ok
}

/// **QUE LA MESA DE KATANAS NO MIENTA SOBRE EL CODIGO QUE TIENE AL LADO.**
///
/// ## Las tres capas, y lo que prueba cada una
///
/// ```text
///    katanas::revisar        la FORMA      cada bloque cae dentro del codigo
///    lleva_su_codigo         lo ARITMETICO el bloque materializa ese numero
///    (el emisor, en x86)     el PATRON     y es de verdad un `mov` + retorno
/// ```
///
/// ** Aqui viven las dos primeras. La tercera **no puede vivir aqui**: sabe
/// x86, y este crate verifica el ENVASE. La cabecera del modulo lo dice desde
/// 2026-08-12 y sigue siendo verdad.
///
/// ## Y por que esto NO contradice aquella cabecera
///
/// Aquella dice que este gate no comprueba *"lo que hacen las instrucciones"*, y
/// no lo comprueba: **esto no dice que el codigo sea seguro**. Dice que **la
/// DECLARACION cuadra con los bytes**. Un binario que promete una trampa en el
/// byte 3.351 y tiene otra cosa ahi no es inseguro: es **mal formado como
/// promesa**, y la forma sigue siendo el trabajo de esta casa.
///
/// > Verificar una declaracion es verificar la forma de una promesa, no la
/// > seguridad del codigo.
///
/// ## Lo que NO comprueba, dicho por delante
///
/// Que no **falte** ninguna katana. Una tabla vacia es la tabla honesta de un
/// binario sin reglas, y desde aqui no se distingue de uno al que se las
/// quitaron. Eso pide recorrer el codigo entero con un decodificador y exigir
/// que cada operacion que pide regla traiga la suya al lado.
///
/// **Esto cierra la mentira facil. La dificil sigue abierta, y esta escrito.**
pub fn exige_katanas(bef: &[u8]) -> Verdict {
    let base = crate::verify(bef);
    if !base.is_ok() {
        return base;
    }
    let tabla = match paquete::seccion(bef, SectionKind::Katanas) {
        Some(t) => t,
        None => {
            return Verdict::Rejected(vec![String::from(
                "el binario no declara sus reglas: falta la seccion Katanas (0x16)",
            )])
        }
    };
    let codigo = match paquete::seccion(bef, SectionKind::Code) {
        Some(c) => c,
        None => {
            return Verdict::Rejected(vec![String::from(
                "no hay seccion de codigo contra la que comprobar las katanas",
            )])
        }
    };

    let n = match katanas::revisar(tabla, codigo.len()) {
        Ok(n) => n,
        Err(f) => return Verdict::Rejected(vec![String::from(f.nombre())]),
    };

    let mut motivos = Vec::new();
    for i in 0..n {
        let k = match katanas::katana(tabla, i) {
            Some(k) => k,
            None => {
                motivos.push(format!("la katana {} no se puede leer", i));
                continue;
            }
        };
        let bloque = &codigo[k.offset as usize..(k.offset + k.longitud) as usize];
        if !katanas::lleva_su_codigo(bloque, k.codigo) {
            // ** El motivo lleva el numero y el sitio, no "la tabla esta mal".
            // Quien lea esto tiene que poder ir al byte.
            motivos.push(format!(
                "la katana {} dice devolver E{} desde el byte {} y ese bloque no lleva \
                 ese numero dentro",
                i, k.codigo, k.offset
            ));
        }
    }
    if motivos.is_empty() {
        Verdict::Ok
    } else {
        Verdict::Rejected(motivos)
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use bmo_abi::bef::writer::{BefBuilder, BefSection};

    fn con(secciones: Vec<BefSection>) -> Vec<u8> {
        let mut b = BefBuilder::new();
        for s in secciones {
            b.add_section(s);
        }
        b.build().expect("no se escribe")
    }

    /// Sin manifiesto pasa el gate normal y **no** pasa la exigencia. Las dos
    /// mitades: si pasara las dos, la exigencia no exige nada.
    #[test]
    fn un_binario_mudo_pasa_verify_y_no_pasa_la_exigencia() {
        let bytes = con(vec![BefSection::code(vec![0xC3; 16])]);
        assert!(crate::verify(&bytes).is_ok(), "el gate normal no lo rechaza");
        match exige_manifiesto(&bytes) {
            Verdict::Rejected(r) => assert!(
                r.iter().any(|x| x.contains("Manifest")),
                "la razon no nombra lo que falta: {:?}",
                r
            ),
            Verdict::Ok => panic!("un binario sin manifiesto no puede pasar la exigencia"),
        }
    }

    #[test]
    fn un_binario_que_se_declara_pasa_las_dos() {
        let bytes = con(vec![
            BefSection::code(vec![0xC3; 16]),
            BefSection::manifest_toml(b"[modulo]\nlenguaje = \"inti\"\n".to_vec()),
        ]);
        assert!(exige_manifiesto(&bytes).is_ok());
        assert_eq!(
            manifiesto(&bytes).map(|d| d.starts_with(b"[modulo]")),
            Some(true)
        );
    }

    /// Una seccion vacia es peor que ninguna: parece que declara.
    #[test]
    fn declarar_nada_no_es_declarar() {
        let bytes = con(vec![
            BefSection::code(vec![0xC3; 16]),
            BefSection::manifest_toml(Vec::new()),
        ]);
        assert!(!exige_manifiesto(&bytes).is_ok());
    }
}

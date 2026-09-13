//! `ocho_bytes("...")`: un texto corto hecho numero, AL COMPILAR.
//!
//! ** Quita un parche: las sondas llevaban sus etiquetas como numeros de 64
//! bits calculados con Python fuera del arbol. Estas pruebas fijan que el
//! compilador da EL MISMO numero que aquel programa --los de `pulso.inti` y
//! `cpu.inti` salieron de el-- y que lo que no se puede plegar no compila.

use super::*;

fn da(texto: &str) -> u64 {
    let f = format!(
        "perfil llano\n\nfuncion prueba(a es natural64, b es natural64) devuelve natural64\n    devuelve ocho_bytes(\"{}\")\n",
        texto
    );
    ejecuta_en(&f, "prueba", 0, 0)
}

/// ** Los numeros que ya corrieron en el Ryzen, calculados con Python.
#[test]
fn da_los_mismos_numeros_que_se_escribian_a_mano() {
    assert_eq!(da("hz real "), 2336349395032767080);
    assert_eq!(da("-- fin -"), 3251720329925504301);
    assert_eq!(da("datos/fo"), 8027155558672130404);
    assert_eq!(da("to.bmp"), 123615100956532, "uno corto deja ceros arriba");
}

/// El primero en el byte BAJO: la Regla 10, no una maquina.
#[test]
fn el_primer_byte_va_abajo() {
    assert_eq!(bmo_inti_front::tablas::ocho_bytes_de("A"), Some(0x41));
    assert_eq!(bmo_inti_front::tablas::ocho_bytes_de("AB"), Some(0x4241));
}

/// ** No es una llamada: ni un `call` en los bytes.
#[test]
fn no_emite_una_llamada() {
    let f = "perfil llano\n\nfuncion prueba devuelve natural64\n    devuelve ocho_bytes(\"hola\")\n";
    let e = emitido(f);
    assert!(e.sin_emitir.is_empty(), "{:?}", e.sin_emitir);
    assert!(!e.codigo.contains(&0xE8), "hay un `call` donde tenia que haber una constante");
}

/// Los codigos del analisis de PERFIL, que es quien acusa `E0124`.
///
/// ** Con las tablas INCRUSTADAS y no las del disco, por lo mismo que `png.rs`:
/// una prueba que leyera `$BMO_MODS` diria cosas distintas segun quien la corra.
fn codigos_de(f: &str) -> Vec<String> {
    let arbol = bmo_inti_front::armar(f);
    let informe = bmo_inti_front::perfil::comprobar(
        &arbol.valor,
        &bmo_inti_front::perfil::Catalogo::por_defecto(),
        &[],
        &bmo_inti_front::tablas::Modulos::por_defecto(),
    );
    informe.codigos().iter().map(|c| c.to_string()).collect()
}

/// *** Lo que no se puede plegar NO COMPILA, y dice por que.
#[test]
fn lo_que_no_cabe_da_e0124() {
    let malo = |arg: &str| {
        let f = format!(
            "perfil llano\n\nfuncion prueba(x es natural64) devuelve natural64\n    devuelve ocho_bytes({})\n",
            arg
        );
        let c = codigos_de(&f);
        assert!(c.iter().any(|x| x == "E0124"), "`ocho_bytes({})` tenia que dar E0124: {:?}", arg, c);
    };
    malo("\"nueve let\"");
    malo("\"\"");
    malo("\"a\u{f1}o\"");
    malo("x");
    malo("");
}

/// Y el bueno no acusa nada.
#[test]
fn el_bueno_no_acusa() {
    let c = codigos_de("perfil llano\n\nfuncion prueba devuelve natural64\n    devuelve ocho_bytes(\"12345678\")\n");
    assert!(!c.iter().any(|x| x == "E0124"), "{:?}", c);
}

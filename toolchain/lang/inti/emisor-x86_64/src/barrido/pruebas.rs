//! Pruebas del barrido lineal.
//!
//! ** La que decide es `el_barrido_recorre_la_sonda_entera`: la sonda del Ryzen
//! son 8.050 bytes de codigo de verdad, con las tres reglas, llamadas, flotantes
//! y la puerta. Si se recorre entera, el lector conoce lo que este emisor emite.

use super::*;
use crate::{emitir, Taller};

/// Las secuencias de la tabla de la maquina, que es la que lee el emisor.
fn maquina() -> Vec<Vec<u8>> {
    match Taller::nuevo().intrinsecos {
        Some(t) => t
            .names()
            .iter()
            .filter_map(|n| t.get(n).map(|d| d.bytes.clone()))
            .collect(),
        None => Vec::new(),
    }
}

fn emitido(fuente: &str) -> crate::Emitido {
    let arbol = bmo_inti_front::armar(fuente);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    let raices = bmo_mods::Roots::find();
    let modulos = bmo_inti_front::tablas::Modulos::cargar(&raices);
    let plano = bmo_inti_front::disposicion::comprobar(
        &arbol.valor,
        bmo_inti_front::disposicion::Medidas::cargar(&raices),
    );
    let metal = bmo_inti_front::ir::metal_que_declara(&arbol.valor, &raices, &modulos);
    let ir = bmo_inti_front::ir::bajar_con(&arbol.valor, &modulos, &plano.valor, &metal).valor;
    let _ = Taller::nuevo();
    emitir(&ir)
}

const LAS_TRES: &str = "\
perfil llano

funcion desborda devuelve natural64
    cambiante x es entero64 = 4000000000
    devuelve x * x

funcion entre_cero devuelve natural64
    cambiante c es entero64 = 0
    devuelve 10 entre c

funcion convierte devuelve natural64
    devuelve entero32(1e30)

funcion principal devuelve entero32
    devuelve 0
";

/// **EL BARRIDO RECORRE LA SONDA DEL RYZEN ENTERA.**
///
/// *** Es la prueba que decide si esto sirve. `cpu.inti` son 8.050 bytes de
/// codigo real: las tres reglas, llamadas entre funciones, flotantes, bucles, la
/// puerta del sistema y el monton. Si se lee de principio a fin sin atascarse,
/// **este lector conoce lo que este emisor emite**.
///
/// Y si un dia el emisor aprende una instruccion nueva, esta prueba se cae antes
/// que nadie -- que es exactamente lo que tiene que pasar: el que anade la
/// instruccion es el que sabe como se lee.
#[test]
fn el_barrido_recorre_la_sonda_entera() {
    let texto = std::fs::read_to_string("../sondas/cpu.inti").expect("no encuentro la sonda");
    let e = emitido(&texto);
    let b = recorrer_con(&e.codigo, &maquina());
    match &b {
        Barrido::Completo(pasos) => {
            assert!(pasos.len() > 500, "solo {} instrucciones?", pasos.len());
            // Las longitudes tienen que sumar EXACTAMENTE el codigo. Si sumaran
            // menos, el recorrido se habria saltado bytes sin decirlo.
            let suma: usize = pasos.iter().map(|p| p.len).sum();
            assert_eq!(suma, e.codigo.len(), "las longitudes no cuadran");
        }
        Barrido::Atascado { off, byte, leido } => panic!(
            "atascado en el byte {} (0x{:02X}) tras leer {} instrucciones.\n\
             Contexto: {:02X?}",
            off,
            byte,
            leido.len(),
            &e.codigo[off.saturating_sub(8)..(*off + 8).min(e.codigo.len())]
        ),
    }
}

/// **Y encuentra las tres operaciones que piden regla.**
#[test]
fn encuentra_las_operaciones_que_piden_regla() {
    let e = emitido(LAS_TRES);
    let b = recorrer_con(&e.codigo, &maquina());
    assert!(b.completo(), "no se recorrio: {:?}", b);

    let mut pedidas: Vec<&'static str> = b
        .pasos()
        .iter()
        .filter_map(|p| match p.que {
            Que::Pide(c) => Some(c.codigo()),
            _ => None,
        })
        .collect();
    pedidas.sort_unstable();
    pedidas.dedup();
    for esperada in ["E1001", "E1003", "E1012"] {
        assert!(
            pedidas.contains(&esperada),
            "el barrido no vio la operacion que pide {}: {:?}",
            esperada,
            pedidas
        );
    }
}

/// ***NINGUNA OPERACION SE QUEDA SIN SU REGLA.***
///
/// Es el criterio de aprobado de S5, escrito como una prueba: se recorre el
/// codigo, se cruzan las operaciones que piden regla con la mesa de katanas, y
/// **no puede sobrar ninguna**.
#[test]
fn ninguna_operacion_se_queda_descubierta() {
    for fuente in [LAS_TRES, &std::fs::read_to_string("../sondas/cpu.inti").unwrap()] {
        let e = emitido(fuente);
        let b = recorrer_con(&e.codigo, &maquina());
        assert!(b.completo(), "no se recorrio");
        let trampas: Vec<(u64, usize)> =
            e.katanas.iter().map(|(c, o, _)| (*c, *o)).collect();
        let malas = descubiertas(&b, &e.inicios, &trampas);
        assert!(
            malas.is_empty(),
            "hay operaciones sin su regla: {:?}",
            malas
        );
    }
}

/// **Y LA AUDITORIA SABE DECIR QUE NO.**
///
/// ** Sin esta prueba, `descubiertas` podria estar devolviendo la lista vacia
/// siempre y las tres de arriba seguirian en verde. Se le quita a la mesa el
/// bloque de la Regla 1 -- como si el emisor no la hubiera emitido -- y se exige
/// que lo diga.
#[test]
fn si_falta_una_regla_la_auditoria_lo_dice() {
    let e = emitido(LAS_TRES);
    let b = recorrer_con(&e.codigo, &maquina());
    assert!(b.completo());

    // La mesa, MENOS los bloques de E1001.
    let mutilada: Vec<(u64, usize)> = e
        .katanas
        .iter()
        .filter(|(c, _, _)| *c != 1001)
        .map(|(c, o, _)| (*c, *o))
        .collect();
    let malas = descubiertas(&b, &e.inicios, &mutilada);
    assert!(
        malas.iter().any(|d| d.regla == "E1001"),
        "quitando los bloques de E1001, la auditoria tenia que echarlos de menos: {:?}",
        malas
    );
}

/// **Ante lo que no conoce se para y dice donde. No rechaza.**
///
/// *** Son tres respuestas distintas y confundirlas es lo que convierte un
/// verificador en un estorbo. `0xD6` no lo emite nadie de esta casa.
#[test]
fn ante_lo_desconocido_se_para_y_no_opina() {
    let basura = [0x55u8, 0x48, 0x89, 0xEC, 0xD6, 0xC3];
    match recorrer(&basura) {
        Barrido::Atascado { off, byte, leido } => {
            assert_eq!(off, 4, "se paro en el sitio que no es");
            assert_eq!(byte, 0xD6);
            assert_eq!(leido.len(), 2, "lo de antes se leyo y sigue valiendo");
        }
        Barrido::Completo(p) => panic!("leyo lo que no existe: {:?}", p),
    }
}

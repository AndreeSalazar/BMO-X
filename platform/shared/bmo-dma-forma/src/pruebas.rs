//! **Las filas del que elige la forma, y cada motivo trae su vecina.**
//!
//! ** La regla de las hojas de metal: veinte pruebas que solo pueden salir bien
//! no prueban nada. Aqui la forma concreta es que **por cada motivo hay una
//! fila que lo produce y otra que pasa JUSTO AL LADO sin producirlo** -- un
//! byte mas alla del espejo y un byte mas aca, impar y par, el techo y el techo
//! menos uno.
//!
//! *** Un juez que dijera `Rebote` siempre pasaria la mitad de estas filas. Es
//! la otra mitad la que lo obliga a acertar.

use super::*;

/// La forma de una lectura de sector del AHCI en esta placa.
///
/// Numeros de verdad: el espejo del kernel en `0xFFFF_8000_0000_0000` con 16
/// GiB, un bufer a 1 MiB dentro de el, 4 KiB pedidos, sector de 512, y el PRDT
/// de 32 bits.
fn buena() -> Peticion {
    Peticion {
        virt: 0xFFFF_8000_0010_0000,
        bytes: 4096,
        espejo_base: 0xFFFF_8000_0000_0000,
        espejo_bytes: 16 << 30,
        alineacion: 2,
        minimo: 512,
        bits_de_cuenta: 32,
        es_su_corral: false,
    }
}

#[test]
fn el_caso_normal_es_prestado() {
    let e = elegir(&buena());
    assert_eq!(e.forma, Forma::Prestado);
    assert_eq!(e.por_que, PorQue::ElDestinoYaSirve);
    assert_eq!(e.fisica, 0x0010_0000, "la fisica del espejo es una RESTA");
    assert_eq!(e.bytes, 4096);
}

// == EL CORRAL: la unica que hace innecesarias a las demas ==================

#[test]
fn su_corral_gana_a_todo_lo_demas() {
    // *** Adrede: esta peticion esta desalineada, fuera del espejo Y no cabe.
    // Y aun asi sale CORRAL, porque una direccion que se calcula dentro de una
    // arena propia **no puede estar mal**. Si esta fila fallara, el orden de
    // `elegir` se habria roto sin que nada mas lo notara.
    let p = Peticion { virt: 7, bytes: 1, es_su_corral: true, ..buena() };
    let e = elegir(&p);
    assert_eq!(e.forma, Forma::Corral);
    assert_eq!(e.por_que, PorQue::EsSuyo);
}

#[test]
fn sin_corral_esa_misma_peticion_rebota() {
    // La vecina de la de arriba: el MISMO disparate, sin la arena.
    let p = Peticion { virt: 7, bytes: 1, es_su_corral: false, ..buena() };
    assert_eq!(elegir(&p).forma, Forma::Rebote);
}

// == EL TAMANO: y va ANTES que el sitio, a proposito ========================

#[test]
fn menos_de_un_sector_no_cabe() {
    let p = Peticion { bytes: 511, ..buena() };
    assert_eq!(elegir(&p).por_que, PorQue::NoCabeElTramo);
}

#[test]
fn un_sector_justo_si_cabe() {
    let p = Peticion { bytes: 512, ..buena() };
    assert_eq!(elegir(&p).forma, Forma::Prestado);
}

#[test]
fn lo_pequenio_se_queja_del_tamanio_aunque_este_fuera() {
    // *** ESTA ES LA FILA QUE FIJA EL ORDEN DE LAS PREGUNTAS.
    //
    // La peticion es pequena Y esta fuera del espejo. Si el sitio se preguntara
    // primero saldria `FueraDelEspejo`, y la cuenta de motivos diria "hay
    // bufers mal colocados" cuando el problema es que se pide de poco en poco.
    // Los dos se arreglan de formas que no se parecen en nada.
    let p = Peticion { virt: 0x1000, bytes: 8, ..buena() };
    assert_eq!(elegir(&p).por_que, PorQue::NoCabeElTramo);
}

// == EL ESPEJO: un byte a cada lado =========================================

#[test]
fn justo_debajo_del_espejo_rebota() {
    let p = Peticion { virt: 0xFFFF_7FFF_FFFF_FFFE, ..buena() };
    assert_eq!(elegir(&p).por_que, PorQue::FueraDelEspejo);
}

#[test]
fn el_primer_byte_del_espejo_no_rebota() {
    let p = Peticion { virt: 0xFFFF_8000_0000_0000, ..buena() };
    let e = elegir(&p);
    assert_eq!(e.forma, Forma::Prestado);
    assert_eq!(e.fisica, 0, "la fisica cero es una fisica valida");
}

// [!] LAS DOS DE ABAJO LLEVAN `bits_de_cuenta: 64` A PROPOSITO, y no es un
// atajo: la primera version no lo llevaba y las dos salieron ROJAS con
// `NoLoDireccionaElAparato`. El fallo era de las FILAS, no de `elegir` -- el
// final de un espejo de 16 GiB esta cuatro veces por encima del techo de 32
// bits, asi que el aparato lo rechaza antes de que el borde importe.
//
// *** Y esa es la leccion que estas dos filas valen mas que su assert: **una
// fila que mezcla dos condiciones no prueba ninguna de las dos**. Si el orden
// de `elegir` cambiara, una fila asi seguiria verde por el motivo equivocado.

#[test]
fn el_final_del_espejo_acota_lo_que_se_da() {
    // ** Un bufer que empieza dentro y pediria mas de lo que queda: se le da lo
    // que hay, no se le rechaza. Esto es lo que impide que el ultimo tramo del
    // espejo sea un rebote gratis.
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (16u64 << 30) - 1024,
        bytes: 4096,
        bits_de_cuenta: 64,
        ..buena()
    };
    let e = elegir(&p);
    assert_eq!(e.forma, Forma::Prestado);
    assert_eq!(e.bytes, 1024, "se da lo que queda de ventana");
}

#[test]
fn el_rabo_del_espejo_mas_corto_que_un_sector_rebota() {
    // La vecina de la de arriba: si lo que queda no llega a un sector, no vale.
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (16u64 << 30) - 8,
        bytes: 4096,
        bits_de_cuenta: 64,
        ..buena()
    };
    assert_eq!(elegir(&p).por_que, PorQue::NoCabeElTramo);
}

#[test]
fn y_el_techo_gana_al_borde_del_espejo_cuando_los_dos_aplican() {
    // *** La fila que deja escrito lo que las dos de arriba descubrieron: con
    // un aparato de 32 bits, el final de un espejo de 16 GiB se rechaza por el
    // TECHO y no por el borde. Los dos motivos son ciertos; el que se cuenta
    // es el que se pregunta primero, y esta fila fija cual es.
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (16u64 << 30) - 1024,
        bytes: 4096,
        ..buena()
    };
    assert_eq!(elegir(&p).por_que, PorQue::NoLoDireccionaElAparato);
}

// == LA ALINEACION: la que costo una lectura corta el 2026-08-11 ===========

#[test]
fn impar_rebota_con_alineacion_de_dos() {
    let p = Peticion { virt: 0xFFFF_8000_0010_0001, ..buena() };
    assert_eq!(elegir(&p).por_que, PorQue::Desalineado);
}

#[test]
fn esa_misma_impar_pasa_si_al_aparato_le_da_igual() {
    // ** La alineacion es del APARATO, no de la direccion. La misma direccion
    // con `alineacion: 1` es perfectamente buena, y esta fila es lo que impide
    // que alguien meta el 2 dentro de `elegir` como si fuera una ley.
    let p = Peticion { virt: 0xFFFF_8000_0010_0001, alineacion: 1, ..buena() };
    assert_eq!(elegir(&p).forma, Forma::Prestado);
}

#[test]
fn una_alineacion_mas_dura_rechaza_lo_que_la_blanda_acepta() {
    let dir = 0xFFFF_8000_0010_0002;
    assert_eq!(elegir(&Peticion { virt: dir, alineacion: 2, ..buena() }).forma, Forma::Prestado);
    assert_eq!(
        elegir(&Peticion { virt: dir, alineacion: 4096, ..buena() }).por_que,
        PorQue::Desalineado
    );
}

// == EL TECHO DEL APARATO: y se mira el FINAL, no la base ==================

#[test]
fn por_encima_de_los_32_bits_rebota() {
    let p = Peticion { virt: 0xFFFF_8000_0000_0000 + (5u64 << 30), ..buena() };
    assert_eq!(elegir(&p).por_que, PorQue::NoLoDireccionaElAparato);
}

#[test]
fn un_tramo_que_CRUZA_el_techo_rebota_aunque_empiece_debajo() {
    // *** LA FILA MAS IMPORTANTE DE ESTE FICHERO.
    //
    // Empieza en 4 GiB - 512 y pide 4 KiB: la BASE cabe en 32 bits y el FINAL
    // no. Un juez que solo mirara la base lo daria por bueno, el aparato le
    // daria la vuelta al contador, y escribiria en la direccion baja -- una
    // corrupcion en un sitio que nadie relaciona con el disco.
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (1u64 << 32) - 512,
        bytes: 4096,
        ..buena()
    };
    assert_eq!(elegir(&p).por_que, PorQue::NoLoDireccionaElAparato);
}

#[test]
fn ese_mismo_tramo_pasa_si_el_aparato_direcciona_64() {
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (1u64 << 32) - 512,
        bytes: 4096,
        bits_de_cuenta: 64,
        ..buena()
    };
    assert_eq!(elegir(&p).forma, Forma::Prestado);
}

#[test]
fn justo_debajo_del_techo_y_cabiendo_entero_pasa() {
    let p = Peticion {
        virt: 0xFFFF_8000_0000_0000 + (1u64 << 32) - 4096,
        bytes: 4096,
        ..buena()
    };
    assert_eq!(elegir(&p).forma, Forma::Prestado);
}

// == EL VOCABULARIO: cerrado, y con su cuenta ==============================

#[test]
fn los_seis_motivos_tienen_indice_propio() {
    // ** Si dos motivos compartieran indice, la tabla de cuentas del que llama
    // sumaria dos causas distintas en la misma casilla -- y ese es exactamente
    // el problema que este crate existe para no tener.
    let todos = [
        PorQue::EsSuyo,
        PorQue::ElDestinoYaSirve,
        PorQue::FueraDelEspejo,
        PorQue::Desalineado,
        PorQue::NoCabeElTramo,
        PorQue::NoLoDireccionaElAparato,
    ];
    assert_eq!(todos.len(), PorQue::CUANTOS);
    for (i, a) in todos.iter().enumerate() {
        assert!(a.indice() < PorQue::CUANTOS, "un indice fuera de la tabla");
        assert!(!a.nombre().is_empty(), "un motivo sin nombre no se puede ensenar");
        for b in todos.iter().skip(i + 1) {
            assert_ne!(a.indice(), b.indice(), "dos motivos con el mismo indice");
        }
    }
}

#[test]
fn un_rebote_no_trae_direccion() {
    // [!] La direccion de un rebote la pone el que rebota. Devolver aqui una
    // que parezca util seria invitar a usarla, y es justo la que no vale.
    let e = elegir(&Peticion { virt: 0x1000, ..buena() });
    assert_eq!(e.forma, Forma::Rebote);
    assert_eq!(e.fisica, 0);
    assert_eq!(e.bytes, 0);
}

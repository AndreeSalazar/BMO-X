//! Las pruebas del juez del prestamo.
//!
//! # ** LO QUE ESTE FICHERO TIENE QUE PODER VER
//!
//! Lo mismo que `bmo-mmio-juicio`: **cada NO por separado**, y el caso real que
//! lo trajo con sus numeros de verdad. Un juez que devuelve siempre `None`
//! pasaria una suite que solo prueba rechazos.

use super::*;

/// El caso legitimo mas simple: alineado, una pagina justa.
#[test]
fn alineado_una_pagina_justa() {
    let t = tramo(0x1000, PAGINA).unwrap();
    assert_eq!(t, Tramo { pagina: 0x1000, dentro: 0, mapeado: PAGINA });
    assert_eq!(t.base_para(0x5000), 0x5000);
}

/// *** LA FILA DEL FALLO, con los numeros del cubo del 2026-09-12.
///
/// 360x360 con buzon de 32 ranuras son 518.704 bytes. Y la imagen no empieza en
/// una pagina: va detras de un `malloc(48)` y de una cabecera de 16.
#[test]
fn el_cubo_no_empieza_en_una_pagina_y_la_base_lo_cuenta() {
    let origen = 0x0000_0000_4010_0050;
    let t = tramo(origen, 518_704).unwrap();
    assert_eq!(t.dentro, 0x50);
    assert_eq!(t.pagina, 0x0000_0000_4010_0000);
    // Lo que el DIRECTOR lee tiene que ser la cabecera, no el principio de la
    // pagina. Esta es la linea que antes valia `va` a secas.
    assert_eq!(t.base_para(0x1_0000_0000), 0x1_0000_0050);
}

/// ** EL SEGUNDO FALLO: con desplazamiento, `bytes.div_ceil(PAGINA)` se queda
/// corto por el final. Aqui: 4.096 bytes que empiezan 16 dentro necesitan DOS
/// paginas, y la cuenta vieja daba una.
#[test]
fn el_final_no_se_queda_sin_mapear() {
    let t = tramo(0x2010, PAGINA).unwrap();
    assert_eq!(t.dentro, 16);
    assert_eq!(t.mapeado, 2 * PAGINA);
    let viejo = PAGINA.div_ceil(PAGINA) * PAGINA;
    assert!(viejo < t.dentro + PAGINA, "la cuenta vieja dejaba bytes sin mapear");
}

/// Lo mapeado siempre cubre hasta el ultimo byte, y siempre en paginas enteras.
#[test]
fn lo_mapeado_cubre_siempre_el_ultimo_byte() {
    for dentro in [0u64, 1, 15, 16, 4095] {
        for bytes in [1u64, 15, 4095, 4096, 4097, 518_704, 2_304_528] {
            let t = tramo(0x7000 + dentro, bytes).unwrap();
            assert_eq!(t.mapeado % PAGINA, 0);
            assert!(t.mapeado >= t.dentro + bytes);
            assert!(t.mapeado < t.dentro + bytes + PAGINA, "no mapea una pagina de mas");
        }
    }
}

/// NO 1: cero bytes no tienen tramo. Nada de "una pagina por si acaso".
#[test]
fn no_cero_bytes() {
    assert_eq!(tramo(0x1000, 0), None);
}

/// NO 2: un final que no existe en 64 bits no se redondea a algo pequeno.
#[test]
fn no_desborda() {
    assert_eq!(tramo(u64::MAX - 10, 100), None);
    // *** Esta fila cazo un hueco de la primera version: lo PEDIDO cabe en 64
    // bits, pero la pagina redondeada termina en 2^64 exacto. El desborde no
    // esta en el tamano, esta en el FINAL DE LA PAGINA.
    assert_eq!(tramo(u64::MAX - PAGINA / 2, PAGINA / 4), None,
               "lo pedido cabe, pero el final de su pagina no existe en 64 bits");
}

/// NO 3: la ventana se juzga con lo MAPEADO. Una ventana exacta que empieza 16
/// bytes dentro de su pagina ya no cabe: invadiria la ventana de al lado.
#[test]
fn no_cabe_si_lo_mapeado_pasa_la_ventana() {
    let ventana = 64 * 1024 * 1024;
    assert!(tramo(0x1000, ventana).unwrap().cabe_en(ventana));
    assert!(!tramo(0x1010, ventana).unwrap().cabe_en(ventana));
}

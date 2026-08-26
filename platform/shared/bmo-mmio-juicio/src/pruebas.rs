//! Las pruebas del juez.
//!
//! # ** LO QUE ESTE FICHERO TIENE QUE PODER VER
//!
//! No que el juez diga que si a lo bueno -- eso lo hace cualquier funcion que
//! devuelva `Ok`. Lo que hay que poder ver es **cada NO por separado**, porque
//! un juez que rechaza todo tambien pasaria una suite que solo prueba rechazos,
//! y uno que acepta todo pasaria una que solo prueba aceptaciones.
//!
//! Por eso cada veto tiene su prueba con su nombre, y hay una que exige que el
//! caso legitimo entre.

use super::*;

/// Un mapa de mentira con la forma del de verdad: RAM baja, un agujero, y RAM
/// alta. Los BAR de PCI viven por encima, hacia los 4 GiB.
fn mapa() -> [Tramo; 3] {
    [
        Tramo { base: 0x0010_0000, bytes: 0x7FF0_0000, es_ram: true },   // 1 MiB .. 2 GiB
        Tramo { base: 0x8000_0000, bytes: 0x1000_0000, es_ram: false },  // agujero MMIO
        Tramo { base: 0x1_0000_0000, bytes: 0x2_0000_0000, es_ram: true },// 4 GiB .. 12 GiB
    ]
}

fn pantalla() -> [Reserva; 1] {
    [Reserva { base: 0x9000_0000, bytes: 0x0080_0000, nombre: "la pantalla" }]
}

// -- Lo que SI entra ---------------------------------------------------------

/// ** La que hace que todo lo demas signifique algo.
///
/// Un BAR de verdad --en el agujero MMIO, alineado, de una pagina para arriba--
/// tiene que entrar. Sin esta, un juez que devolviera `Err` siempre pasaria
/// todas las otras.
#[test]
fn un_bar_legitimo_entra() {
    assert_eq!(cedible(0x8000_0000, 0x2000, &mapa(), &pantalla()), Ok(()));
}

#[test]
fn una_sola_pagina_entra() {
    assert_eq!(cedible(0x8010_0000, PAGINA, &mapa(), &pantalla()), Ok(()));
}

#[test]
fn pegado_al_final_de_la_ram_baja_pero_sin_tocarla() {
    // La RAM baja acaba justo en 0x8000_0000. Empezar ahi no la pisa.
    assert_eq!(cedible(0x8000_0000, PAGINA, &mapa(), &[]), Ok(()));
}

// -- Lo que NO entra, uno por uno --------------------------------------------

#[test]
fn cero_bytes_no_es_un_rango() {
    assert_eq!(cedible(0x8000_0000, 0, &mapa(), &[]), Err(Veto::Vacio));
}

#[test]
fn la_base_tiene_que_empezar_en_una_pagina() {
    assert_eq!(
        cedible(0x8000_0800, PAGINA, &mapa(), &[]),
        Err(Veto::NoAlineado { base: 0x8000_0800 })
    );
}

#[test]
fn el_largo_tiene_que_ser_multiplo_de_pagina() {
    assert_eq!(
        cedible(0x8000_0000, 0x1800, &mapa(), &[]),
        Err(Veto::LargoNoAlineado { bytes: 0x1800 })
    );
}

/// *** El que no es obvio, y por eso lleva la explicacion en el `Veto`.
///
/// Un BAR de 256 bytes es legitimo. Cederlo no lo es: la unidad de la MMU es la
/// pagina, asi que cederlo cede los 4.096 bytes que lo rodean -- y ahi pueden
/// vivir los registros de otro aparato.
#[test]
fn un_bar_mas_pequeno_que_una_pagina_no_se_cede() {
    assert_eq!(
        cedible(0x8000_0000, 256, &mapa(), &[]),
        Err(Veto::MasPequenoQueUnaPagina { bytes: 256 })
    );
}

/// ** Y que los dos vetos del largo sean DISTINGUIBLES, que es lo que decide si
/// uno de ellos existe.
///
/// 256 bytes no se arregla redondeando -- ceder ese BAR cede la pagina entera.
/// 0x1800 si: se redondea a 0x2000 y ya. Si el juez los mezclara, el primero se
/// leeria como un error de calculo del que llama.
#[test]
fn el_largo_pequeno_y_el_largo_torcido_no_dicen_lo_mismo() {
    assert_eq!(
        cedible(0x8000_0000, 256, &mapa(), &[]),
        Err(Veto::MasPequenoQueUnaPagina { bytes: 256 })
    );
    assert_eq!(
        cedible(0x8000_0000, 0x1800, &mapa(), &[]),
        Err(Veto::LargoNoAlineado { bytes: 0x1800 })
    );
}

#[test]
fn el_megabyte_legacy_no_se_cede() {
    assert_eq!(
        cedible(0x0009_0000, PAGINA, &[], &[]),
        Err(Veto::DebajoDeUnMega { base: 0x0009_0000 })
    );
}

// -- El veto que sostiene todo lo demas --------------------------------------

#[test]
fn pisar_ram_por_dentro() {
    let v = cedible(0x1000_0000, PAGINA, &mapa(), &[]);
    assert!(matches!(v, Err(Veto::PisaRam { .. })), "{:?}", v);
}

/// Cruzar el borde DE ARRIBA de la RAM alta.
///
/// [!] El borde de abajo no sirve para esta prueba: el APIC vive en
/// `0xFEC0_0000 .. 0x1_0000_0000`, o sea **pegado por debajo a los 4 GiB**, asi
/// que cualquier rango que se acerque a la RAM alta por abajo choca antes con
/// el APIC. Se descubrio escribiendo esta prueba, y es exactamente el tipo de
/// dato que un mapa dibujado a mano no da.
#[test]
fn pisar_ram_cruzando_su_borde_de_arriba() {
    let v = cedible(0x2_FFFF_F000, 0x2000, &mapa(), &[]);
    assert!(matches!(v, Err(Veto::PisaRam { .. })), "{:?}", v);
}

#[test]
fn pisar_ram_por_el_final() {
    // Acaba justo despues del principio de la RAM baja.
    let v = cedible(0x000F_F000, 0x2000, &mapa(), &[]);
    // Cae antes por el megabyte legacy, que tambien es un NO. Se comprueba el
    // caso limpio justo encima del mega.
    assert!(v.is_err(), "{:?}", v);
    let v2 = cedible(0x0010_0000, PAGINA, &mapa(), &[]);
    assert!(matches!(v2, Err(Veto::PisaRam { .. })), "{:?}", v2);
}

#[test]
fn un_rango_que_contiene_un_tramo_de_ram_entero_tambien_lo_pisa() {
    // Desde el mega justo hasta pasado el final de la RAM baja.
    let v = cedible(0x0010_0000, 0x8000_0000, &mapa(), &[]);
    assert!(matches!(v, Err(Veto::PisaRam { .. })), "{:?}", v);
}

#[test]
fn el_veto_de_ram_dice_contra_que_choco() {
    match cedible(0x1000_0000, PAGINA, &mapa(), &[]) {
        Err(Veto::PisaRam { base, tramo_base, tramo_bytes }) => {
            assert_eq!(base, 0x1000_0000);
            assert_eq!(tramo_base, 0x0010_0000);
            assert_eq!(tramo_bytes, 0x7FF0_0000);
        }
        otro => panic!("tenia que ser PisaRam con los dos lados: {:?}", otro),
    }
}

// -- El APIC -----------------------------------------------------------------

#[test]
fn el_apic_entero_no_se_cede() {
    assert_eq!(
        cedible(APIC_BASE, PAGINA, &[], &[]),
        Err(Veto::EsElApic { base: APIC_BASE })
    );
}

#[test]
fn una_pagina_cualquiera_de_dentro_del_apic_tampoco() {
    let dentro = APIC_BASE + 0x10_0000;
    assert_eq!(cedible(dentro, PAGINA, &[], &[]), Err(Veto::EsElApic { base: dentro }));
}

/// *** Y esta es la que importa: **el APIC se niega sin que nadie lo pase**.
///
/// Si viajara en `reservas`, olvidarlo seria posible -- y olvidarlo una vez es
/// ceder el control de las interrupciones para siempre. Lo que puede olvidarse,
/// se olvida.
#[test]
fn el_apic_se_niega_con_el_mapa_y_las_reservas_vacios() {
    assert!(cedible(APIC_BASE, PAGINA, &[], &[]).is_err());
}

#[test]
fn justo_debajo_del_apic_si_entra() {
    assert_eq!(cedible(APIC_BASE - PAGINA, PAGINA, &[], &[]), Ok(()));
}

#[test]
fn justo_encima_del_apic_si_entra() {
    assert_eq!(cedible(APIC_BASE + APIC_BYTES, PAGINA, &[], &[]), Ok(()));
}

// -- Las reservas de la casa -------------------------------------------------

#[test]
fn la_pantalla_se_reparte_por_su_propia_puerta() {
    match cedible(0x9000_0000, PAGINA, &mapa(), &pantalla()) {
        Err(Veto::EsDeLaCasa { nombre, .. }) => assert_eq!(nombre, "la pantalla"),
        otro => panic!("tenia que decir de quien es: {:?}", otro),
    }
}

// -- Aritmetica que viene de fuera -------------------------------------------

#[test]
fn un_largo_que_da_la_vuelta_no_entra() {
    // Alineado, por encima del mega, y `base + bytes` desborda.
    let v = cedible(0xFFFF_FFFF_FFFF_F000, PAGINA * 2, &[], &[]);
    assert!(v.is_err(), "{:?}", v);
}

/// El orden de los vetos: un `bytes = 0` tiene que salir como `Vacio` y no como
/// *"no pisa nada"*, que es un SI con otra cara.
#[test]
fn el_vacio_se_juzga_antes_que_los_solapes() {
    assert_eq!(cedible(0x1000_0000, 0, &mapa(), &pantalla()), Err(Veto::Vacio));
}

// -- Lo que CABINA va a pintar -----------------------------------------------

#[test]
fn cada_veto_tiene_nombre_y_ninguno_esta_vacio() {
    let todos = [
        Veto::Vacio,
        Veto::NoAlineado { base: 1 },
        Veto::LargoNoAlineado { bytes: 1 },
        Veto::SeSaleDelEspacio { base: 1, bytes: 1 },
        Veto::MasPequenoQueUnaPagina { bytes: 1 },
        Veto::DebajoDeUnMega { base: 1 },
        Veto::PisaRam { base: 1, tramo_base: 2, tramo_bytes: 3 },
        Veto::EsElApic { base: 1 },
        Veto::EsDeLaCasa { base: 1, nombre: "x" },
    ];
    for v in todos {
        assert!(!v.nombre().is_empty(), "{:?} sin nombre", v);
        // La fila de CABINA son 80 columnas y el prefijo gasta 26. Un nombre
        // que no quepa saldria recortado justo el dia que se lee.
        // 80 columnas - 27 de prefijo - 2 del " =" - 16 de la direccion = 35.
        assert!(v.nombre().len() <= 35, "{:?} no cabe en la fila: {}", v, v.nombre().len());
    }
}

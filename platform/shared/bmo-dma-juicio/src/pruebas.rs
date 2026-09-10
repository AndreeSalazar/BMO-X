//! **Las filas del juez del DMA, y la mitad estan para decir que NO.**
//!
//! ** La regla de las hojas de metal: veinte pruebas que solo pueden salir bien
//! no prueban nada. Por cada pregunta de `juzgar` hay al menos una fila que la
//! ROMPE, y una que pasa justo al lado -- porque un juez que rechaza SIEMPRE
//! tambien pasa por guardian.

use super::*;

/// El caso normal, contra el que se compara todo lo demas.
///
/// Numeros de verdad: un marco de 4 KiB en `0x0010_0000`, del aparato 1, y una
/// peticion de 512 bytes alineada a 2 en mitad del marco. Es la forma de una
/// lectura de sector del AHCI.
fn marco_bueno() -> Marco {
    Marco {
        es_neutro: true,
        en_vuelo_para: None,
        base: 0x0010_0000,
        bytes: 4096,
        aparato: 1,
    }
}

fn peticion_buena() -> Peticion {
    Peticion {
        fisica: 0x0010_0200,
        bytes: 512,
        aparato: 1,
        alineacion: 2,
        bits_de_cuenta: 22,
    }
}

#[test]
fn el_caso_normal_pasa_y_devuelve_la_misma_direccion() {
    let prenda = juzgar(peticion_buena(), marco_bueno()).expect("esto tiene que pasar");
    // ** Que devuelva la MISMA direccion importa: un juez que "arregla" la
    // direccion en vez de rechazarla dejaria al aparato escribiendo en un sitio
    // que nadie pidio.
    assert_eq!(prenda.cruda(), 0x0010_0200);
}

// -- 1. de quien es el marco --------------------------------------------

#[test]
fn un_marco_que_no_es_de_ningun_aparato_se_rechaza() {
    let mut m = marco_bueno();
    m.es_neutro = false;
    assert_eq!(juzgar(peticion_buena(), m), Err(Veto::NoEsDeUnAparato));
}

// -- 2. y es del que va a escribir --------------------------------------

#[test]
fn un_aparato_no_puede_escribir_en_el_marco_de_otro() {
    let mut p = peticion_buena();
    p.aparato = 2;
    assert_eq!(
        juzgar(p, marco_bueno()),
        Err(Veto::DeOtroAparato { suyo: 1, pide: 2 })
    );
}

// -- 3. pide algo -------------------------------------------------------

#[test]
fn una_peticion_de_cero_bytes_se_rechaza() {
    // *** ESTA FILA NO ESTABA EN EL PLAN. Salio al escribir las pruebas: con
    // `bytes == 0` las comprobaciones de "cabe" dan que SI, asi que un
    // descriptor vacio pasaria las otras cinco preguntas.
    let mut p = peticion_buena();
    p.bytes = 0;
    assert_eq!(juzgar(p, marco_bueno()), Err(Veto::NoPideNada));
}

// -- 4. cabe dentro del marco -------------------------------------------

#[test]
fn una_direccion_por_debajo_del_marco_se_rechaza() {
    let mut p = peticion_buena();
    p.fisica = 0x000F_F000;
    assert_eq!(juzgar(p, marco_bueno()), Err(Veto::SeSaleDelMarco));
}

#[test]
fn una_peticion_que_se_pasa_por_el_final_se_rechaza() {
    let mut p = peticion_buena();
    // Empieza dentro y acaba 512 bytes despues del final.
    p.fisica = 0x0010_0E00;
    p.bytes = 1024;
    assert_eq!(juzgar(p, marco_bueno()), Err(Veto::SeSaleDelMarco));
}

#[test]
fn la_peticion_que_llena_el_marco_exacto_pasa() {
    // ** La de al lado de la anterior. Sin esta, un juez que rechazara todo lo
    // que toque el ultimo byte pasaria las dos.
    let p = Peticion { fisica: 0x0010_0000, bytes: 4096, ..peticion_buena() };
    assert!(juzgar(p, marco_bueno()).is_ok());
}

#[test]
fn un_desbordamiento_de_u64_no_se_lee_como_que_cabe() {
    // *** El caso que un `<=` a secas dejaria pasar: `fisica + bytes` da la
    // vuelta y el resultado queda por debajo del final del marco.
    let p = Peticion { fisica: u64::MAX - 16, bytes: 64, ..peticion_buena() };
    assert_eq!(juzgar(p, marco_bueno()), Err(Veto::SeSaleDelMarco));
}

// -- 5. alineacion ------------------------------------------------------

#[test]
fn una_direccion_impar_donde_se_exige_par_se_rechaza() {
    // Es LA comprobacion que el AHCI ya hacia --`if buf_phys & 1 != 0`-- y la
    // unica de las seis que existia antes de este crate.
    let mut p = peticion_buena();
    p.fisica = 0x0010_0201;
    assert_eq!(juzgar(p, marco_bueno()), Err(Veto::MalAlineada { pide: 2 }));
}

#[test]
fn alineacion_1_deja_pasar_una_direccion_impar() {
    // ** La de al lado: `1` quiere decir "me da igual", no "alineado a 1 byte
    // y ademas comprueba". Sin esta fila, un aparato sin exigencias quedaria
    // bloqueado por una comprobacion que no pidio.
    let mut p = peticion_buena();
    p.fisica = 0x0010_0201;
    p.alineacion = 1;
    assert!(juzgar(p, marco_bueno()).is_ok());
}

#[test]
fn alineacion_de_64_para_un_anillo_de_xhci() {
    // El DCBAA del xHC se alinea a 64. Forma real, no inventada.
    let m = Marco { aparato: 3, ..marco_bueno() };
    let base = Peticion { aparato: 3, alineacion: 64, ..peticion_buena() };
    assert!(juzgar(Peticion { fisica: 0x0010_0240, ..base }, m).is_ok());
    assert_eq!(
        juzgar(Peticion { fisica: 0x0010_0220, ..base }, m),
        Err(Veto::MalAlineada { pide: 64 })
    );
}

// -- 6. la CUENTA cabe en su campo --------------------------------------

#[test]
fn una_cuenta_que_no_cabe_en_22_bits_se_rechaza() {
    // *** El campo de longitud del PRDT de AHCI mide 22 bits: 4 MiB. Pedir mas
    // no da error en el aparato -- **da una cuenta truncada**, que es una
    // transferencia mas corta de la que el driver cree. Silencio puro.
    let m = Marco { bytes: 64 * 1024 * 1024, ..marco_bueno() };
    let p = Peticion { bytes: 8 * 1024 * 1024, ..peticion_buena() };
    assert_eq!(juzgar(p, m), Err(Veto::NoCabeLaCuenta { bits: 22 }));
}

#[test]
fn la_cuenta_que_llena_los_22_bits_exactos_pasa() {
    let m = Marco { bytes: 64 * 1024 * 1024, ..marco_bueno() };
    let p = Peticion { bytes: 4 * 1024 * 1024, ..peticion_buena() };
    assert!(juzgar(p, m).is_ok());
}

#[test]
fn bits_de_cuenta_0_quiere_decir_sin_limite() {
    // ** Un aparato que no declara el ancho de su campo no se bloquea: se le
    // cree. Es la misma decision que `Anonimo` en `vmm::es_tabla` -- rechazar
    // lo que no se sabe rompe lo que funcionaba a cambio de una sospecha.
    let m = Marco { bytes: 64 * 1024 * 1024, ..marco_bueno() };
    let p = Peticion { bytes: 8 * 1024 * 1024, bits_de_cuenta: 0, ..peticion_buena() };
    assert!(juzgar(p, m).is_ok());
}

// -- El orden de los vetos, que tambien es un contrato ------------------

#[test]
fn un_marco_ajeno_se_rechaza_antes_de_mirar_la_aritmetica() {
    // *** Si un marco no es de nadie, da igual que ademas la direccion se salga
    // y este mal alineada: el motivo que se informa tiene que ser el PRIMERO,
    // porque es el que explica el fallo. Un veto de alineacion sobre memoria
    // ajena manda a arreglar lo que no era.
    let mut m = marco_bueno();
    m.es_neutro = false;
    let p = Peticion {
        fisica: 0xDEAD_BEEF,
        bytes: 0,
        aparato: 9,
        alineacion: 4096,
        bits_de_cuenta: 8,
    };
    assert_eq!(juzgar(p, m), Err(Veto::NoEsDeUnAparato));
}

// -- CASO 2: el marco PRESTADO, y por que existe ------------------------
//
// *** Estas cuatro filas nacieron de un fallo. La primera version del juez solo
// tenia `es_neutro`, y al ir a cablearlo en el AHCI aparecio el camino DIRECTO
// de una lectura (`dev/disk/transfer.rs:64`): el disco escribe en el bufer del
// que llamo, que **no es del aparato y tiene todo el derecho a no serlo**.
//
// Un juez con una sola regla habria rechazado una lectura legitima y dejado el
// disco sin funcionar. Y no lo encontro una lectura del codigo: lo encontro el
// INTENTO de cablearlo.

#[test]
fn un_marco_prestado_a_este_aparato_pasa_aunque_no_sea_suyo() {
    let m = Marco { es_neutro: false, en_vuelo_para: Some(1), ..marco_bueno() };
    assert!(juzgar(peticion_buena(), m).is_ok());
}

#[test]
fn un_marco_prestado_a_otro_se_rechaza_con_nombre() {
    // ** El fallo mas peligroso de los seis: dos aparatos sobre el mismo bufer.
    // Se informa como `DeOtroAparato` y no como `NoEsDeUnAparato` porque el
    // segundo mandaria a mirar el sitio equivocado.
    let m = Marco { es_neutro: false, en_vuelo_para: Some(7), ..marco_bueno() };
    assert_eq!(
        juzgar(peticion_buena(), m),
        Err(Veto::DeOtroAparato { suyo: 7, pide: 1 })
    );
}

#[test]
fn el_prestamo_manda_sobre_el_duenno_del_corral() {
    // ** Un marco que es del aparato 1 por corral pero esta prestado al 2: el
    // que vale AHORA es el prestamo. Sin esta fila, un juez que mirara primero
    // `es_neutro` dejaria pasar al 1 sobre un bufer que ya no es suyo.
    let m = Marco { es_neutro: true, en_vuelo_para: Some(2), aparato: 1, ..marco_bueno() };
    assert_eq!(
        juzgar(peticion_buena(), m),
        Err(Veto::DeOtroAparato { suyo: 2, pide: 1 })
    );
}

#[test]
fn sin_prestamo_y_sin_corral_no_hay_forma_de_pasar() {
    // La de al lado de las tres anteriores: `None` + `es_neutro: false` es
    // memoria de otro, y punto.
    let m = Marco { es_neutro: false, en_vuelo_para: None, ..marco_bueno() };
    assert_eq!(juzgar(peticion_buena(), m), Err(Veto::NoEsDeUnAparato));
}

// -- Y EL EMBUDO, que es el paso N0 -------------------------------------

#[test]
fn una_prenda_solo_sale_de_juzgar() {
    // ** Esta prueba no comprueba un valor: comprueba una FORMA, y por eso su
    // valor esta en lo que NO compila. `Prenda` no tiene constructor publico,
    // asi que `Prenda(0x1000)` desde fuera de este crate es un error de tipos.
    //
    // *** Eso es el paso N0 --*"que solo haya UN sitio por donde salga una
    // direccion fisica"*-- cumplido por el COMPILADOR en vez de por un grep.
    // Aqui dentro si se puede construir, y la fila existe para dejar dicho que
    // esa es la unica excepcion.
    let a = juzgar(peticion_buena(), marco_bueno()).unwrap();
    let b = Prenda(0x0010_0200);
    assert_eq!(a, b, "la prenda tiene que llevar la direccion que se juzgo");
}

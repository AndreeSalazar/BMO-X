//! LA SONDA DEL PULSO, calibrada antes de ir al metal.
//!
//! `sondas/pulso.inti` pregunta al kernel lo que el silicio solo le cuenta a
//! el: frecuencia real, milivatios, obreros en pie. En el emulador no hay
//! silicio, y la puerta de `INFO` contesta 0 -- que es "no se sabe".
//!
//! ** Asi que aqui no se prueban los numeros del Ryzen: se prueba que la sonda
//! PREGUNTA lo que dice preguntar y ESCRIBE lo que le contestan. Si el
//! formateador estuviera mal, un 4.600.000.000 Hz saldria como otra cosa y la
//! culpa pareceria del procesador.
//!
//! Igual que `sonda.rs`: con EL FICHERO DE VERDAD, cortado antes de su
//! `principal`, y no con una copia de sus funciones.

use super::*;

fn maquinaria_de_pulso() -> String {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sondas")
        .join("pulso.inti");
    let texto = std::fs::read_to_string(&p)
        .unwrap_or_else(|e| panic!("no puedo leer {}: {}", p.display(), e));
    let corte = texto
        .find("funcion principal")
        .expect("pulso.inti tiene que tener un `principal`");
    texto[..corte].to_string()
}

fn con_principal(cuerpo: &str) -> String {
    format!(
        "{}funcion principal devuelve entero32\n{}    devuelve 0\n",
        maquinaria_de_pulso(),
        cuerpo
    )
}

/// **LA CALIBRACION**: cuatro mil seiscientos millones de hercios salen en
/// decimal, alineados a la derecha, con espacios delante.
#[test]
fn el_pulso_escribe_hercios_en_decimal() {
    let m = arranca(&con_principal("    linea(et_hz(), 4600000000)\n"));
    assert_eq!(
        lo_escrito(&m),
        vec!["hz real ", "      46", "00000000"],
        "diez cifras en dieciseis: seis espacios delante"
    );
}

/// ** El cero se VE. Una linea en blanco no se distingue de una que no llego,
/// y aqui el cero significa algo concreto: "no se sabe".
#[test]
fn el_cero_sale_como_cero_y_no_como_nada() {
    let m = arranca(&con_principal("    linea(et_pkg(), 0)\n"));
    assert_eq!(lo_escrito(&m), vec!["mW pkg  ", "        ", "       0"]);
}

/// Las dieciseis cifras llenas, sin un espacio: el borde de arriba.
#[test]
fn dieciseis_cifras_no_dejan_hueco() {
    let m = arranca(&con_principal("    linea(et_tick(), 1234567890123456)\n"));
    assert_eq!(lo_escrito(&m), vec!["tick    ", "12345678", "90123456"]);
}

/// ** PREGUNTA POR LA PUERTA, Y POR LO QUE DICE.
///
/// `muestra` tiene que cruzar `INFO` con cada selector de la tabla, sobre
/// `mi_tarea`. Un selector cambiado no falla al compilar: pregunta por el campo
/// de al lado y recibe un numero que parece bueno.
#[test]
fn una_muestra_pregunta_los_seis_campos_por_info() {
    let m = arranca(&con_principal("    muestra()\n"));
    let preguntas: Vec<u64> = m
        .syscalls
        .iter()
        .filter(|s| s.capability == 0xFFFF_FFFF_FFFF_FFFE && s.operation == 0x13)
        .map(|s| s.arg0)
        .collect();
    assert_eq!(
        preguntas,
        vec![0x0B, 0x20, 0x21, 0x22, 0x1B, 0x2F],
        "tick, hz real, mW paquete, mW nucleo, vivos, puertas -- en ese orden"
    );
    // Y como el emulador no tiene silicio, cada respuesta es "no se sabe".
    let dice = lo_escrito(&m);
    assert_eq!(dice.len(), 18, "seis lineas de tres palabras");
    for fila in dice.chunks(3) {
        assert_eq!(fila[2], "       0", "{} deberia decir 0 en el emulador", fila[0]);
    }
}

/// ** La sonda entera compila, arranca, y no deja nada mudo.
///
/// No se EJECUTA entera aqui a proposito: espera medio segundo veinte veces, y
/// en el emulador el reloj no avanza -- la espera se sale por su tope de
/// vueltas, que es justo para lo que esta, pero son millones de instrucciones.
#[test]
fn la_sonda_del_pulso_compila_entera() {
    let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("sondas")
        .join("pulso.inti");
    let fuente = std::fs::read_to_string(&p).expect("no puedo leer pulso.inti");
    let e = emitido(&fuente);
    assert!(e.arranca, "pulso.inti tiene `principal` y tiene que arrancar solo");
    assert!(
        e.sin_emitir.is_empty(),
        "hay nombres que no llegaron a bytes: {:?}",
        e.sin_emitir
    );
}

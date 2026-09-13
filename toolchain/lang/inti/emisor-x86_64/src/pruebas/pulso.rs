//! LA SONDA DEL PULSO, calibrada antes de ir al metal.
//!
//! `sondas/pulso.inti` pregunta al kernel lo que el silicio solo le cuenta a
//! el: frecuencia real, milivatios, obreros en pie. En el emulador no hay
//! silicio, y la puerta de `INFO` contesta 0 -- que es "no se sabe".
//!
//! ** Asi que aqui no se prueban los numeros del Ryzen: se prueba que la sonda
//! PREGUNTA lo que dice preguntar, ESCRIBE lo que le contestan, y DUERME en vez
//! de girar. Si el formateador estuviera mal, un 4.600.000.000 Hz saldria como
//! otra cosa y la culpa pareceria del procesador.
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
/// decimal, alineados a la derecha, con espacios delante -- y la linea TERMINA.
#[test]
fn el_pulso_escribe_hercios_en_decimal() {
    let m = arranca(&con_principal("    linea(et_hz(), 4600000000)\n"));
    assert_eq!(
        lo_escrito(&m),
        vec!["hz real ", "      46", "00000000", "\n"],
        "diez cifras en dieciseis: seis espacios delante, y un salto detras"
    );
}

/// ** El cero se VE. Una linea en blanco no se distingue de una que no llego,
/// y aqui el cero significa algo concreto: "no se sabe".
#[test]
fn el_cero_sale_como_cero_y_no_como_nada() {
    let m = arranca(&con_principal("    linea(et_pkg(), 0)\n"));
    assert_eq!(lo_escrito(&m), vec!["mW pkg  ", "        ", "       0", "\n"]);
}

/// Las dieciseis cifras llenas, sin un espacio: el borde de arriba.
#[test]
fn dieciseis_cifras_no_dejan_hueco() {
    let m = arranca(&con_principal("    linea(et_tick(), 1234567890123456)\n"));
    assert_eq!(lo_escrito(&m), vec!["tick    ", "12345678", "90123456", "\n"]);
}

/// ** ENSENA LO QUE LE DAN, y pregunta solo lo que no se resta.
///
/// Desde el 2026-09-12 la frecuencia y los vatios los calcula la sonda restando
/// sus propias lecturas; `ensena` recibe los numeros hechos y solo pregunta los
/// dos que no son contadores: obreros vivos y puertas.
#[test]
fn ensena_escribe_lo_que_le_dan_y_pregunta_vivos_y_puertas() {
    let m = arranca(&con_principal("    ensena(1, 2, 3, 4)\n"));
    let preguntas: Vec<u64> = m
        .syscalls
        .iter()
        .filter(|s| s.capability == 0xFFFF_FFFF_FFFF_FFFE && s.operation == 0x13)
        .map(|s| s.arg0)
        .collect();
    assert_eq!(preguntas, vec![0x1B, 0x2F], "vivos y puertas, en ese orden");
    let dice = lo_escrito(&m);
    assert_eq!(dice.len(), 6 * 4 + 1);
    for (fila, esperado) in dice[..16].chunks(4).zip(["       1", "       2", "       3", "       4"]) {
        assert_eq!(fila[2], esperado, "{}", fila[0]);
        assert_eq!(fila[3], "\n");
    }
}

/// ** LA RESTA DE LA SONDA, con los numeros del Ryzen: medio segundo a 4,52 GHz
/// y 58 W. Es la cuenta que antes hacia el kernel para todos a la vez.
#[test]
fn la_sonda_resta_sus_propias_lecturas() {
    let m = arranca(&con_principal(
        "    linea(et_hz(), hz_entre(3700000000, 2260000000, 1850000000))\n    linea(et_pkg(), mw_entre(29000000, 500))\n    linea(et_nucl(), hz_entre(3700000000, 5, 99999))\n",
    ));
    assert_eq!(
        lo_escrito(&m),
        vec![
            "hz real ", "      45", "20000000", "\n",
            "mW pkg  ", "        ", "   58000", "\n",
            "mW nucl ", "        ", "       0", "\n",
        ],
        "4,52 GHz, 58 W, y una ventana de MPERF demasiado corta dice 0"
    );
}

/// *** ESPERAR ES DORMIR, NO GIRAR -- lo que el Ryzen ensenyo la primera vez.
///
/// Con `ceder`, cada muestra cruzaba ~200.000 puertas. Aqui el emulador no tiene
/// reloj: `INFO_TICKS` contesta 0 siempre, asi que `espera_ms` agota su tope de
/// veinte vueltas -- y lo que se comprueba es que cada vuelta es UN `WAIT` con
/// el plazo en nanosegundos, y no un giro.
#[test]
fn esperar_medio_segundo_es_un_wait_con_su_plazo() {
    let m = arranca(&con_principal("    espera_ms(500)\n"));
    let esperas: Vec<_> = m.syscalls.iter().filter(|s| s.nr == 2).collect();
    assert_eq!(esperas.len(), 20, "el tope de vueltas, con el reloj parado");
    for w in &esperas {
        assert_eq!(w.capability, 0, "sin nada que esperar: solo el plazo");
        assert_eq!(w.arg0, 500_000_000, "500 ms en nanosegundos");
    }
    let todas = m.syscalls.len();
    assert!(todas < 50, "{} puertas para medio segundo: eso es girar", todas);
}

/// ** EL NO SE LEE DEL CODIGO: motivo abajo, banderas arriba.
///
/// El emulador no sabe negar un campo --contesta exito a todo `INFO`-- asi que
/// aqui se le da a la sonda la palabra que devolveria el kernel al negar la
/// memoria de otros: `(16 << 32) | 3`. Si `codigo_de` o `banderas_de` cortaran
/// mal, el usuario leeria un motivo que no es.
#[test]
fn el_no_del_jefe_se_parte_en_motivo_y_bandera() {
    let r = (16u64 << 32) | 3;
    let m = arranca(&con_principal(&format!(
        "    linea(et_sin_perm(), codigo_de({r}))\n    linea(et_bandera(), banderas_de({r}))\n"
    )));
    assert_eq!(
        lo_escrito(&m),
        vec!["sin perm", "        ", "       3", "\n", "bandera ", "        ", "      16", "\n"]
    );
}

/// Y `el_jefe_dice` pregunta lo que dice preguntar, por `invoca` -- que recoge
/// el CODIGO. Con `invoca_valor` el NO llegaria como un 0 mudo.
#[test]
fn el_jefe_dice_pregunta_un_campo_que_no_existe_y_uno_de_otros() {
    let m = arranca(&con_principal("    el_jefe_dice()\n"));
    let preguntas: Vec<u64> = m
        .syscalls
        .iter()
        .filter(|s| s.operation == 0x13)
        .map(|s| s.arg0)
        .collect();
    assert_eq!(preguntas, vec![254, 0x24]);
}

/// ** La sonda entera compila, arranca, y no deja nada mudo.
///
/// No se EJECUTA entera aqui a proposito: son veinte muestras de seis numeros de
/// dieciseis cifras, y el formateo solo ya se prueba arriba.
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

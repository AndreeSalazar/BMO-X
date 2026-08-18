//! **El motor de la calculadora del escritorio**, ejecutado de verdad.
//!
//! `cobol/calcgui.bex` es el unico ejemplo de COBOL que tiene un programa al
//! otro lado en vez de una persona: lo lanza el compositor cuando se pulsa `=`,
//! le pasa tres lineas por la consola y le lee dos. Y hasta hoy **era el unico
//! que nadie habia ejecutado en el banco**, porque el `ACCEPT` no se podia
//! alimentar desde una prueba.
//!
//! Escribir esto destapo por que: la lectura de linea perdia los bytes que
//! vinieran detras del `\n` en el mismo paquete, asi que el motor recibia mal
//! su operacion y contestaba una cuenta que nadie habia pedido. El arreglo esta
//! en el contrato de `TASK_OP_CONSOLE_READ`; estas pruebas son lo que impide
//! que vuelva.
//!
//! ## Lo que se comprueba, y por que en este orden
//!
//! Primero que las cuentas SALEN, que es de lo que va COBOL. Despues los dos
//! caminos donde un motor mediocre inventa un numero en vez de decir que no
//! sabe -- que en una calculadora de dinero son el caso importante.

use crate::tests::comun::run_cobol_con_entrada;

/// La FUENTE, no una copia. Si alguien edita el `.cob`, estas pruebas hablan de
/// lo que edito.
const MOTOR: &str = include_str!("../../examples/2-decimal/calcgui.cob");

/// Las tres lineas que el compositor escribe en la consola del hijo.
/// Ver `desktop/calc.rs`, funcion `lanzar`.
fn preguntar(izq: &str, cod: u8, der: &str) -> String {
    run_cobol_con_entrada(MOTOR, &format!("{izq}\n{cod}\n{der}\n"))
}

// ------------------------------------------------------------------------
//  Las cuentas
// ------------------------------------------------------------------------

#[test]
fn las_cuatro_operaciones_de_siempre_salen_exactas() {
    assert_eq!(preguntar("2.50", 1, "3.25"), "0\n5.75\n");
    assert_eq!(preguntar("10.00", 2, "25.50"), "0\n-15.50\n", "y el signo se mantiene");
    assert_eq!(preguntar("10", 4, "4"), "0\n2.50\n");
}

#[test]
fn tres_por_diecinueve_noventa_y_nueve_da_cincuenta_y_nueve_noventa_y_siete() {
    // ** El mismo numero que sostiene el escalon 2 del lenguaje entero: en
    // coma flotante binaria esto da 59.969999999999999, y por eso el dinero no
    // se guarda asi. Aqui son centavos enteros y sale clavado.
    assert_eq!(preguntar("3", 3, "19.99"), "0\n59.97\n");
}

#[test]
fn el_tanto_por_ciento_tambien_es_decimal_exacto() {
    // El segundo por ciento del primero: el 10% de 200.
    assert_eq!(preguntar("200", 5, "10"), "0\n20.00\n");
}

// ------------------------------------------------------------------------
//  La tecla `$`, que no calcula: PRESENTA
// ------------------------------------------------------------------------

#[test]
fn la_tecla_del_dinero_devuelve_la_mascara_de_un_banco() {
    // Un solo operando: el segundo va como relleno y esta rama no lo mira.
    //
    // ** La plantilla se gasta AL COMPILAR: en el `.bex` no queda ni la mascara
    // ni un interprete que la lea. Eso es lo que ninguna calculadora de
    // escritorio puede ensenar.
    assert_eq!(preguntar("12345.67", 6, "0"), "0\n    $12,345.67\n");
}

#[test]
fn la_mascara_es_ancha_y_un_importe_grande_no_se_pierde_por_arriba() {
    // [!] Con `$$$,$$9.99` --cinco digitos enteros-- esto habria salido como
    // `$4,567.89`: un importe equivocado por un factor de cien, en silencio,
    // porque lo que no cabe en una PICTURE se pierde POR ARRIBA. La prueba
    // existe para que nadie estreche la mascara sin enterarse.
    assert_eq!(preguntar("1234567.89", 6, "0"), "0\n $1,234,567.89\n");
}

// ------------------------------------------------------------------------
//  Donde un motor mediocre se inventa un numero
// ------------------------------------------------------------------------

#[test]
fn dividir_entre_cero_no_contesta_cero_contesta_que_no_sabe() {
    // ** "No se" y "da cero" son cosas MUY distintas en una calculadora de
    // dinero, y la que sale por el mismo sitio que un importe es la peligrosa.
    let salida = preguntar("5", 4, "0");
    assert!(salida.starts_with("1\n"), "el estado dice que no hay resultado");
    assert!(salida.contains("no se divide"), "y dice por que: {salida:?}");
}

#[test]
fn un_codigo_que_el_motor_no_conoce_se_dice_en_vez_de_inventarse() {
    let salida = preguntar("5", 9, "2");
    assert!(salida.starts_with("1\n"), "{salida:?}");
}

// ------------------------------------------------------------------------
//  El contrato con el escritorio
// ------------------------------------------------------------------------

#[test]
fn el_estado_va_en_su_propia_linea_y_va_primero() {
    // Es lo que impide que el escritorio pinte un motivo de error en la
    // pantallita como si fuera una cifra. Y con la tecla `$` deja de poder
    // adivinarse mirando: `$12,345.67` es una respuesta BUENA que no parece un
    // numero, asi que quien sabe si contesto tiene que ser el motor.
    for (izq, cod, der, estado) in [
        ("2.50", 1u8, "3.25", "0"),
        ("12345.67", 6, "0", "0"),
        ("5", 4, "0", "1"),
        ("5", 9, "2", "1"),
    ] {
        let salida = preguntar(izq, cod, der);
        let mut lineas = salida.lines();
        assert_eq!(lineas.next(), Some(estado), "codigo {cod}");
        assert!(lineas.next().is_some(), "y detras SIEMPRE viene una linea mas");
        assert_eq!(lineas.next(), None, "dos lineas, ni una mas");
    }
}

// ------------------------------------------------------------------------
//  El fallo que destapo todo esto
// ------------------------------------------------------------------------

#[test]
fn tres_lineas_escritas_de_golpe_llegan_enteras() {
    // ** ESTA ES LA PRUEBA DE LA REGRESION, y merece decirse entera.
    //
    // El compositor escribe las tres lineas SEGUIDAS y despues lanza el motor,
    // asi que los diez bytes ya estan en el anillo antes de la primera lectura.
    // `CONSOLE_READ` entrega hasta siete de una vez.
    //
    //     sin la regla   paquete 1 = "12.50\n3"  -> el ACCEPT se queda "12.50"
    //                                               y TIRA el 3, que era la
    //                                               operacion que se pedia
    //
    // El motor contestaba `0.00` con estado 0: una cuenta que nadie habia
    // pedido, sin un solo error. Lo que lo arregla es que un paquete no cruce
    // nunca un salto de linea -- ver `TASK_OP_CONSOLE_READ`.
    //
    // Diez bytes en total, que es exactamente el caso que se rompia.
    assert_eq!(preguntar("12.50", 3, "4"), "0\n50.00\n");
}

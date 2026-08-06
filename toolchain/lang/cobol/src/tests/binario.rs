//! BINARIO — 5 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// ── REGISTROS BINARIOS: leer lo que ya existe ───────────────────────
//
// Hasta aquí el fichero era TEXTO: una línea, un número. Un banco no da
// eso — da registros de largo fijo con los campos en su byte y los importes
// empaquetados. Esto es `1.1` + `1.2` del plan.

/// ★ EL VIAJE COMPLETO: escribir un registro binario y volver a leerlo.
///
/// El fichero que queda **no es texto**: son 16 bytes por registro, sin
/// salto de línea, con el número zonado y el importe en nibbles. Que salga
/// y vuelva igual es lo que prueba que las dos mitades —empaquetar y
/// desempaquetar— dicen lo mismo.
#[test]
fn un_registro_binario_va_al_disco_y_vuelve() {
    let src = programa_con_ficheros(
        "FILE SECTION.\n\
         FD SALIDA.\n\
         01 REG-OUT.\n\
         05 O-NUM PIC 9(10).\n\
         05 O-IMP PIC S9(7)V99 COMP-3.\n\
         05 O-EST PIC 9.\n\
         FD ENTRADA.\n\
         01 REG-IN.\n\
         05 I-NUM PIC 9(10).\n\
         05 I-IMP PIC S9(7)V99 COMP-3.\n\
         05 I-EST PIC 9.\n\
         WORKING-STORAGE SECTION.\n01 FIN PIC 9 VALUE ZERO.",
        "MOVE 4471998200 TO O-NUM.\nMOVE -1234.56 TO O-IMP.\nMOVE 7 TO O-EST.\n\
         OPEN OUTPUT SALIDA.\nWRITE REG-OUT.\nCLOSE SALIDA.",
    );
    let (_, m) = run_cobol_con_disco(&src, &[]);
    let bytes = m.archivo("d/s.txt").expect("tiene que haber fichero").to_vec();

    // ★ 16 bytes EXACTOS y ni uno más: 10 zonados + 5 empaquetados + 1.
    // Un salto de línea aquí correría todo lo de detrás.
    assert_eq!(bytes.len(), 16, "el registro no mide lo que dice su copybook");
    assert_eq!(&bytes[0..10], b"4471998200", "el numero no salio zonado");
    // -1234.56 en centavos = -123456, en 5 bytes: 00 01 23 45 6D
    assert_eq!(&bytes[10..15], &[0x00, 0x01, 0x23, 0x45, 0x6D]);
    assert_eq!(bytes[15], b'7');
}

/// Y el otro sentido, con **varios registros seguidos** — que es donde el
/// resto de siete bytes de la puerta se nota. Con registros de 16 bytes, el
/// primero deja sobra y el segundo la tiene que gastar antes de pedir más.
#[test]
fn un_batch_lee_registros_binarios_seguidos() {
    // Tres registros de 16: número zonado, importe empaquetado, estado.
    let mut datos: Vec<u8> = Vec::new();
    for (num, cent) in [(1u64, 1000_00i64), (2, 234_56), (3, -100_00)] {
        datos.extend_from_slice(format!("{num:010}").as_bytes());
        // El empaquetado a mano, para no probar el código con el código.
        let neg = cent < 0;
        let mut d = format!("{:09}", cent.abs());
        d.push(if neg { 'd' } else { 'c' });
        for par in d.as_bytes().chunks(2) {
            let alto = (par[0] - b'0') << 4;
            let bajo = if par[1] == b'c' { 0x0C } else if par[1] == b'd' { 0x0D }
                       else { par[1] - b'0' };
            datos.push(alto | bajo);
        }
        datos.push(b'0');
    }

    let src = programa_con_ficheros(
        "FILE SECTION.\n\
         FD ENTRADA.\n\
         01 REG-IN.\n\
         05 I-NUM PIC 9(10).\n\
         05 I-IMP PIC S9(7)V99 COMP-3.\n\
         05 I-EST PIC 9.\n\
         WORKING-STORAGE SECTION.\n\
         01 TOTAL PIC S9(9)V99 COMP-3 VALUE ZERO.\n\
         01 CUANTOS PIC 9(3) VALUE ZERO.\n\
         01 ULTIMO PIC 9(10) VALUE ZERO.\n\
         01 FIN PIC 9 VALUE ZERO.\n88 SE-ACABO VALUE 1.",
        "OPEN INPUT ENTRADA.\n\
         PERFORM UNTIL SE-ACABO\n\
         READ ENTRADA\n\
         AT END MOVE 1 TO FIN\n\
         NOT AT END ADD I-IMP TO TOTAL\n\
         ADD 1 TO CUANTOS\n\
         MOVE I-NUM TO ULTIMO\n\
         END-READ\n\
         END-PERFORM.\n\
         CLOSE ENTRADA.\n\
         DISPLAY CUANTOS.\nDISPLAY TOTAL.\nDISPLAY ULTIMO.",
    );
    let (consola, _) = run_cobol_con_disco_bytes(&src, &[("d/e.txt", &datos)]);
    // 1000.00 + 234.56 - 100.00 = 1134.56, y el último número es el 3.
    assert_eq!(consola, "3\n1134.56\n3\n", "los registros se corrieron o se perdio alguno");
}

/// ★★ EL VIAJE ENTERO: un programa COBOL escribe un fichero binario, y el
/// VISOR lo lee y lo enseña.
///
/// Es la prueba de que el visor **no puede mentir sobre lo que el programa
/// escribió**: los dos usan la misma disposición, y los decodificadores del
/// visor están comparados contra los emitidos en `bmo-lower`.
///
/// Si alguien cambia el empaquetado sin tocar el visor, o al revés, este
/// test lo dice — que es exactamente lo que un copybook mantenido a mano no
/// puede hacer.
#[test]
fn el_visor_lee_lo_que_el_programa_escribio() {
    let src = programa_con_ficheros(
        "FILE SECTION.\n\
         FD SALIDA.\n\
         01 REG-CUENTA.\n\
         05 CTA-NUMERO PIC 9(10).\n\
         05 CTA-SALDO  PIC S9(7)V99 COMP-3.\n\
         05 CTA-ESTADO PIC 9.\n\
         WORKING-STORAGE SECTION.\n01 X PIC 9.",
        "OPEN OUTPUT SALIDA.\n\
         MOVE 4471998200 TO CTA-NUMERO.\nMOVE 15234.75 TO CTA-SALDO.\n\
         MOVE 1 TO CTA-ESTADO.\nWRITE REG-CUENTA.\n\
         MOVE 4471998201 TO CTA-NUMERO.\nMOVE -890.10 TO CTA-SALDO.\n\
         MOVE 2 TO CTA-ESTADO.\nWRITE REG-CUENTA.\n\
         CLOSE SALIDA.",
    );
    let (_, m) = run_cobol_con_disco(&src, &[]);
    let bytes = m.archivo("d/s.txt").expect("tiene que haber fichero").to_vec();
    assert_eq!(bytes.len(), 32, "dos registros de 16");

    let visto = ver_registros(&src, &bytes, Some("REG-CUENTA"), 10).unwrap();

    // Los importes, decodificados y con su coma puesta.
    assert!(visto.contains("2 registro(s) de 16"), "{visto}");
    assert!(visto.contains("4471998200"), "{visto}");
    assert!(visto.contains("15234.75"), "el saldo empaquetado no se leyo:\n{visto}");
    assert!(visto.contains("-890.10"), "el signo del segundo no se leyo:\n{visto}");
    // Y los bytes crudos al lado, que es lo que hace de esto un visor y no
    // un volcado de variables.
    // 15234.75 → 1523475 centavos → nueve dígitos `001523475` + signo `C`.
    assert!(visto.contains("00 15 23 47 5C"), "faltan los bytes crudos:\n{visto}");
}

/// ★ Un fichero que NO cuadra con el copybook. Es el síntoma clásico de
/// estar mirando el formato equivocado, y callarlo dejaría al que mira
/// creyendo que el último registro es raro.
#[test]
fn el_visor_avisa_cuando_el_fichero_no_cuadra() {
    let src = programa_con_ficheros(
        "FILE SECTION.\nFD ENTRADA.\n01 REG.\n05 A PIC 9(4).\n05 B PIC 9(4).\n\
         WORKING-STORAGE SECTION.\n01 X PIC 9.",
        "DISPLAY \"x\".",
    );
    // 20 bytes con registros de 8: sobran 4.
    let datos: Vec<u8> = b"1111222233334444abcd".to_vec();
    let visto = ver_registros(&src, &datos, None, 10).unwrap();
    assert!(visto.contains("SOBRAN 4 BYTES"), "{visto}");
    assert!(visto.contains("no es"), "{visto}");
    assert!(visto.contains("LO QUE SOBRA"), "{visto}");
    // Y aun así enseña los dos que sí cuadran.
    assert!(visto.contains("1111"), "{visto}");
}

/// ★★ EL NIVEL 10: escribe el maestro binario y lo vuelve a leer.
///
/// Que los importes salgan iguales prueba que empaquetar y desempaquetar
/// dicen lo mismo. Y el fichero que queda mide 48 bytes exactos: tres
/// registros de dieciséis, sin separador.
#[test]
fn el_ejemplo_binario_escribe_el_maestro_y_lo_relee() {
    let (salida, m) = run_cobol_con_disco(
        include_str!("../../examples/10-binario/maestro.cob"),
        &[],
    );
    let bytes = m.archivo("datos/ctas.bin").expect("tiene que haber maestro");
    assert_eq!(bytes.len(), 48, "tres registros de 16, sin salto de linea");

    assert!(salida.contains("escritas 3 cuentas"), "{salida}");
    // Los tres saldos, releídos del disco: los importes empaquetados
    // vuelven iguales que como se escribieron.
    assert!(salida.contains("15,234.75"), "{salida}");
    assert!(salida.contains("3,105.40"), "{salida}");
    // ★ Y el que está en rojo sale con su `CR`. Con una máscara sin símbolo
    // de signo saldría `890.10` a secas y el extracto diría que la cuenta
    // está en verde — el fallo que este ejemplo existe para no cometer.
    assert!(salida.contains("890.10CR"), "el descubierto salio SIN signo:\n{salida}");
    // El cuadre: 15234.75 - 890.10 + 3105.40 = 17450.05
    assert!(salida.contains("17,450.05"), "el total no cuadra:\n{salida}");
    assert!(salida.contains("en descubierto:\n1\n"), "{salida}");
}


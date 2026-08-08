//! COMP3 -- 12 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

/// * Un `VALUE` sobre un `COMP-3` tiene que quedar EMPAQUETADO, no como un
/// entero crudo en el hueco. Se ve porque el campo trunca a su PICTURE: si
/// la inicializacion se hubiera saltado el empaquetado, saldrian los cinco
/// digitos.
#[test]
fn value_sobre_comp3_queda_empaquetado() {
    let src = program("01 A PIC 9(3) COMP-3 VALUE 12345.", "DISPLAY A.");
    assert_eq!(run_cobol(&src), "345\n");
}

// -- COMP-3: el formato en el que estan los datos de un banco --------
//
// La trampa de esta caracteristica es que se puede fingir entera: guardar
// el mismo entero de siempre, no empaquetar nada, y todos los programas
// seguirian dando el mismo resultado. Compilaria, validaria, y el dia que
// alguien leyera un fichero de verdad no habria nibbles donde tocaba.
//
// Por eso estas pruebas no comprueban que "el COMP-3 no rompe": comprueban
// lo que SOLO puede pasar si el dato de verdad vive empaquetado en un campo
// del ancho que dice su PICTURE. Los bytes exactos estan probados aparte,
// en `bmo_lower::packed`.

/// El decimal exacto sobrevive al empaquetado. Es lo minimo: si tres cuotas
/// de 19.99 dejaran de dar 59.97 al pasar por nibbles, el formato no
/// serviria para lo unico para lo que existe.
#[test]
fn comp3_mantiene_el_decimal_exacto() {
    let src = program(
        "01 SALDO PIC S9(7)V99 COMP-3.\n01 CUOTA PIC S9(5)V99 COMP-3.",
        "MOVE 0 TO SALDO.\nMOVE 19.99 TO CUOTA.\n\
         PERFORM 3 TIMES\nADD CUOTA TO SALDO\nEND-PERFORM.\n\
         DISPLAY SALDO.\n\
         IF SALDO = 59.97\nDISPLAY \"cuadra\"\nELSE\nDISPLAY \"se perdio\"\nEND-IF.",
    );
    assert_eq!(run_cobol(&src), "59.97\ncuadra\n");
}

/// * La prueba de que el almacenamiento es DE VERDAD el del PICTURE.
///
/// Un `PIC 9(3)` COMP-3 son dos bytes: tres huecos de digito y el signo. Lo
/// que no cabe se pierde por arriba, que es lo que manda el estandar. El
/// mismo dato en DISPLAY hoy sigue siendo un registro de 64 bits y guarda
/// los cinco digitos -- asi que este test falla en cuanto alguien convierta
/// el COMP-3 en decoracion.
#[test]
fn comp3_ocupa_lo_que_dice_su_picture_y_trunca() {
    let empaquetado = program("01 A PIC 9(3) COMP-3.", "MOVE 12345 TO A.\nDISPLAY A.");
    assert_eq!(run_cobol(&empaquetado), "345\n", "un COMP-3 de 3 digitos guardo mas de 3");

    let suelto = program("01 A PIC 9(3).", "MOVE 12345 TO A.\nDISPLAY A.");
    assert_eq!(run_cobol(&suelto), "12345\n", "el DISPLAY dejo de ser un entero de 64 bits");
}

/// El signo va en el ultimo nibble, y tiene que volver. Un campo con `S`
/// que perdiera el signo convertiria un cargo en un abono.
#[test]
fn comp3_conserva_el_signo() {
    let src = program(
        "01 A PIC S9(5)V99 COMP-3.",
        "MOVE 0 TO A.\nSUBTRACT 123.45 FROM A.\nDISPLAY A.",
    );
    assert_eq!(run_cobol(&src), "-123.45\n");
}

/// Y un campo SIN `S` guarda el valor absoluto, que es lo que dice el
/// estandar. No es un detalle: es la diferencia entre un campo que puede
/// estar en rojo y uno que no, y el fichero de al lado lo lee por el nibble.
#[test]
fn comp3_sin_signo_guarda_el_valor_absoluto() {
    let src = program(
        "01 A PIC 9(5)V99 COMP-3.",
        "MOVE 0 TO A.\nSUBTRACT 123.45 FROM A.\nDISPLAY A.",
    );
    assert_eq!(run_cobol(&src), "123.45\n");
}

/// Dos campos empaquetados seguidos no se pisan. Un `off by one` al
/// escribir nibbles mete el importe de uno en el otro, y eso en un batch
/// aparece como un descuadre semanas despues.
#[test]
fn dos_comp3_seguidos_no_se_pisan() {
    let src = program(
        "01 A PIC S9(7)V99 COMP-3.\n01 B PIC S9(7)V99 COMP-3.\n01 C PIC S9(3) COMP-3.",
        "MOVE 11111.11 TO A.\nMOVE 22222.22 TO B.\nMOVE 333 TO C.\n\
         DISPLAY A.\nDISPLAY B.\nDISPLAY C.",
    );
    assert_eq!(run_cobol(&src), "11111.11\n22222.22\n333\n");
}

/// El empaquetado convive con lo que ya habia: se puede mezclar un COMP-3
/// con un DISPLAY en la misma cuenta, porque la aritmetica sigue viendo el
/// entero escalado y no la representacion.
#[test]
fn comp3_y_display_se_mezclan_en_la_misma_cuenta() {
    let src = program(
        "01 P PIC S9(7)V99 COMP-3.\n01 D PIC 9(5)V99.\n01 R PIC S9(7)V99 COMP-3.",
        "MOVE 100.50 TO P.\nMOVE 25.25 TO D.\nCOMPUTE R = P + D.\nDISPLAY R.",
    );
    assert_eq!(run_cobol(&src), "125.75\n");
}

/// Una TABLA de empaquetados: cada elemento tiene sus propios nibbles.
#[test]
fn una_tabla_de_comp3_guarda_cada_casilla_aparte() {
    let src = program(
        "01 TABLA.\n05 T PIC S9(5)V99 COMP-3 OCCURS 3 TIMES.\n01 I PIC 9(2).",
        "MOVE 10.01 TO T(1).\nMOVE 20.02 TO T(2).\nMOVE 3 TO I.\nMOVE 30.03 TO T(I).\n\
         DISPLAY T(1).\nDISPLAY T(2).\nDISPLAY T(3).",
    );
    assert_eq!(run_cobol(&src), "10.01\n20.02\n30.03\n");
}

/// Un COMP-3 en el REGISTRO de un fichero: se lee del disco como texto y se
/// guarda empaquetado. El fichero sigue siendo texto --los registros
/// binarios son otro paso-- pero el campo en memoria ya es el de un banco.
#[test]
fn el_registro_de_un_fichero_puede_ser_comp3() {
    let src = programa_con_ficheros(
        "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99 COMP-3.\n\
         WORKING-STORAGE SECTION.\n01 TOTAL PIC S9(9)V99 COMP-3.\n01 FIN PIC 9.",
        "MOVE 0 TO TOTAL.\nMOVE 0 TO FIN.\nOPEN INPUT ENTRADA.\n\
         PERFORM UNTIL FIN = 1\n\
         READ ENTRADA\nAT END MOVE 1 TO FIN\nNOT AT END ADD R TO TOTAL\nEND-READ\n\
         END-PERFORM.\nCLOSE ENTRADA.\nDISPLAY TOTAL.",
    );
    let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "19.99\n25.01\n0.50\n")]);
    assert_eq!(consola, "45.50\n");
}

/// * EL EJEMPLO DE NIVEL 7, ejecutado entero.
///
/// Las dos primeras lineas de numeros son la prueba que no se puede
/// fingir: el mismo `12345` en un campo empaquetado de tres digitos y en
/// uno sin empaquetar. Salen distintos porque el empaquetado mide lo que
/// dice su PICTURE. El dia que salgan iguales, el COMP-3 volvio a ser un
/// entero con otro nombre.
#[test]
fn el_ejemplo_de_empaquetado_hace_lo_que_dice() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/7-empaquetado/cuentas.cob"),
        &[("datos/movim.txt", "1000.00\n234.56\n0.44\n-100.00\n")],
    );
    let esperado = [
        "CUENTAS - DECIMAL EMPAQUETADO",
        "empaquetado de 3 digitos:",
        "345",
        "el mismo dato sin empaquetar:",
        "12345",
        "una cuenta en rojo:",
        "-1234.56",
        "el mismo importe en un campo sin signo:",
        "1234.56",
        "saldo tras el cierre, menos comision:",
        " $1,133.50",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(salida, esperado);
}

/// Las cuatro formas de escribirlo son la misma cosa.
#[test]
fn comp3_se_escribe_de_cuatro_maneras() {
    for forma in ["COMP-3", "COMPUTATIONAL-3", "USAGE COMP-3", "USAGE IS PACKED-DECIMAL"] {
        let src = program(
            &format!("01 A PIC 9(3) {forma}."),
            "MOVE 12345 TO A.\nDISPLAY A.",
        );
        assert_eq!(run_cobol(&src), "345\n", "forma {forma}");
    }
}

/// Lo que NO se compila se dice CON SU MOTIVO. Aceptar `COMP` y guardar un
/// entero de 64 bits seria compilar una palabra que promete un formato y no
/// lo da -- que es exactamente el fallo del que este compilador huye.
#[test]
fn los_usage_que_no_estan_se_rechazan_diciendo_por_que() {
    let casos: &[(&str, &str)] = &[
        ("01 A PIC 9(3) COMP.", "binario"),
        ("01 A PIC 9(3) BINARY.", "binario"),
        ("01 A PIC 9(3) COMP-5.", "binario"),
        ("01 A COMP-2.", "FLOTANTE"),
        ("01 A PIC 9(3)V99 COMP-1.", "FLOTANTE"),
        ("01 A COMP-3.", "sin PIC"),
        ("01 A PIC X(10) COMP-3.", "solo se empaqueta lo numerico"),
        ("01 A PIC $$$,$$9.99 COMP-3.", "es para ENSENAR"),
    ];
    for (decl, pista) in casos {
        let src = program(decl, "DISPLAY \"x\".");
        let err = compile_source_to_bef(&src)
            .expect_err(&format!("{decl} deberia rechazarse"))
            .to_string();
        assert!(err.contains(pista), "{decl} => {err:?}\n  (se esperaba que dijera {pista:?})");
    }
}


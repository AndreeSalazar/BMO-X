//! PARRAFOS — 12 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

/// La forma FUERA DE LÍNEA: el cuerpo es un párrafo.
#[test]
fn perform_varying_de_parrafo() {
    let src = programa_con_parrafos(
        "01 I PIC 9(3).\n01 SUMA PIC 9(5) VALUE ZERO.",
        "PERFORM 1000-SUMA VARYING I FROM 1 BY 1 UNTIL I > 4.\n\
         DISPLAY SUMA.\n\
         STOP RUN.\n\
         1000-SUMA.\n\
         ADD I TO SUMA.",
    );
    assert_eq!(run_cobol(&src), "10\n"); // 1+2+3+4
}

// ── PÁRRAFOS: la estructura de todo COBOL real ──────────────────────
//
// Un programa era una lista plana de sentencias. Un batch de banca no se
// escribe así: se escribe con un cuerpo principal de cinco `PERFORM`
// legibles y el trabajo repartido en párrafos con nombre.

/// La forma corriente: cuerpo principal con `PERFORM`, `STOP RUN`, y los
/// párrafos detrás.
#[test]
fn el_perform_de_parrafo_llama_y_vuelve() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "MOVE 0 TO A.\n\
         PERFORM 1000-SUMA.\n\
         DISPLAY A.\n\
         STOP RUN.\n\
         1000-SUMA.\n\
         ADD 5 TO A.",
    );
    assert_eq!(run_cobol(&src), "5\n");
}

/// ★ `PERFORM A THRU C` ejecuta A, B y C — **todo lo que hay entre los
/// dos**, porque están seguidos en el código.
///
/// Es la prueba de que el epílogo de cada párrafo pregunta en ejecución en
/// vez de retornar siempre. Un emisor que pusiera un `ret` fijo al final de
/// cada párrafo pasaría el test de arriba y fallaría éste.
#[test]
fn un_perform_thru_recorre_todos_los_parrafos_del_rango() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "PERFORM 1000-A THRU 3000-C.\n\
         DISPLAY \"fin\".\n\
         STOP RUN.\n\
         1000-A.\n\
         DISPLAY \"a\".\n\
         2000-B.\n\
         DISPLAY \"b\".\n\
         3000-C.\n\
         DISPLAY \"c\".",
    );
    assert_eq!(run_cobol(&src), "a\nb\nc\nfin\n");
}

/// Y el MISMO párrafo, llamado solo, no arrastra al siguiente. Es la otra
/// mitad de lo mismo: si el rango se decidiera al compilar, uno de los dos
/// tests tendría que fallar.
#[test]
fn el_mismo_parrafo_llamado_solo_no_arrastra_al_siguiente() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "PERFORM 1000-A.\n\
         DISPLAY \"fin\".\n\
         STOP RUN.\n\
         1000-A.\n\
         DISPLAY \"a\".\n\
         2000-B.\n\
         DISPLAY \"b\".",
    );
    assert_eq!(run_cobol(&src), "a\nfin\n", "1000-A se llevo por delante a 2000-B");
}

/// ★ `PERFORM <párrafo> UNTIL <cond>` — **el bucle de un batch**: el
/// párrafo lee y el `UNTIL` mira si se acabó.
#[test]
fn un_perform_de_parrafo_until_repite() {
    let src = programa_con_parrafos(
        "01 I PIC 9(3).",
        "MOVE 0 TO I.\n\
         PERFORM 1000-CUENTA UNTIL I = 4.\n\
         DISPLAY I.\n\
         STOP RUN.\n\
         1000-CUENTA.\n\
         ADD 1 TO I.",
    );
    assert_eq!(run_cobol(&src), "4\n");
}

/// `PERFORM <párrafo> <n> TIMES`.
#[test]
fn un_perform_de_parrafo_n_veces() {
    let src = programa_con_parrafos(
        "01 A PIC S9(7)V99.",
        "MOVE 0 TO A.\n\
         PERFORM 1000-CUOTA 3 TIMES.\n\
         DISPLAY A.\n\
         STOP RUN.\n\
         1000-CUOTA.\n\
         ADD 19.99 TO A.",
    );
    assert_eq!(run_cobol(&src), "59.97\n");
}

/// La otra forma corriente de escribirlo: **todo** en párrafos, sin cuerpo
/// principal. Entonces el programa empieza por el primero.
#[test]
fn un_programa_que_empieza_por_un_parrafo() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "1000-PRINCIPAL.\n\
         MOVE 7 TO A.\n\
         PERFORM 2000-ENSENA.\n\
         STOP RUN.\n\
         2000-ENSENA.\n\
         DISPLAY A.",
    );
    assert_eq!(run_cobol(&src), "7\n");
}

/// `EXIT` no hace nada, y ese es su trabajo: ser el destino de un
/// `PERFORM … THRU X-SALIR`.
#[test]
fn exit_es_el_final_de_un_rango_y_no_hace_nada() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "PERFORM 1000-A THRU 1000-SALIR.\n\
         DISPLAY \"fin\".\n\
         STOP RUN.\n\
         1000-A.\n\
         DISPLAY \"trabajo\".\n\
         1000-SALIR.\n\
         EXIT.",
    );
    assert_eq!(run_cobol(&src), "trabajo\nfin\n");
}

/// ★ UN BATCH ENTERO escrito como se escribe de verdad: cuerpo principal
/// legible de tres `PERFORM`, y cada paso en su párrafo.
///
/// Es la forma del 90 % del COBOL que hay escrito, y hasta hoy no compilaba
/// ni una línea de ella.
#[test]
fn el_batch_con_parrafos_es_legible_y_cuadra() {
    let src = ficheros_con_parrafos(
        "FILE SECTION.\nFD ENTRADA.\n01 IMPORTE PIC S9(7)V99 COMP-3.\n\
         WORKING-STORAGE SECTION.\n\
         01 TOTAL PIC S9(9)V99 COMP-3 VALUE ZERO.\n\
         01 CUANTOS PIC 9(5) VALUE ZERO.\n\
         01 FIN PIC 9 VALUE ZERO.\n\
         88 SE-ACABO VALUE 1.",
        "PERFORM 1000-INICIO.\n\
         PERFORM 2000-PROCESO UNTIL SE-ACABO.\n\
         PERFORM 3000-CIERRE.\n\
         STOP RUN.\n\
         1000-INICIO.\n\
         DISPLAY \"CIERRE DEL DIA\".\n\
         OPEN INPUT ENTRADA.\n\
         2000-PROCESO.\n\
         READ ENTRADA\n\
         AT END MOVE 1 TO FIN\n\
         NOT AT END ADD IMPORTE TO TOTAL\n\
         END-READ.\n\
         3000-CIERRE.\n\
         CLOSE ENTRADA.\n\
         DISPLAY TOTAL.",
    );
    let (consola, _) =
        run_cobol_con_disco(&src, &[("d/e.txt", "1000.00\n234.56\n0.44\n-100.00\n")]);
    assert_eq!(consola, "CIERRE DEL DIA\n1135.00\n");
}

/// ★ EL EJEMPLO DE NIVEL 8, ejecutado entero: el batch escrito con
/// párrafos, que es como está escrito el 90 % del COBOL que hay.
///
/// Junta todo lo de la fase 0: `VALUE` que inicializa, `OR` en las
/// condiciones, `88` colgando de un dato, `PERFORM … UNTIL <88>`,
/// `PERFORM … THRU` sobre tres párrafos, `COMP-3` y una PIC editada.
#[test]
fn el_ejemplo_de_parrafos_cierra_el_dia() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/8-parrafos/cierre.cob"),
        // El 0.00 de en medio es el que ejercita el descarte.
        &[("datos/movim.txt", "1000.00\n234.56\n0.00\n0.44\n-100.00\n")],
    );
    let esperado = [
        "BANCO BMO - CIERRE DEL DIA",
        "--------------------------",
        "movimientos contados:",
        "4", // los cinco menos el de cero
        "de mas de 500:",
        "1",
        "abonos:",
        "1",
        "total del dia:",
        " $1,135.00",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(salida, esperado);
}

/// Lo que no cuadra se dice, en vez de saltar a cualquier parte.
#[test]
fn los_perform_de_parrafo_imposibles_se_rechazan() {
    let casos: &[(&str, &str)] = &[
        (
            "PERFORM 9000-NO-EXISTE.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
            "no hay ningun parrafo con ese nombre",
        ),
        (
            "PERFORM 2000-B THRU 1000-A.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".\n2000-B.\nDISPLAY \"b\".",
            "el final esta ANTES del principio",
        ),
        (
            "PERFORM 1000-A 3 TIMES UNTIL 1 = 1.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
            "hay que elegir una",
        ),
    ];
    for (body, pista) in casos {
        let src = programa_con_parrafos("01 A PIC 9(3).", body);
        let err = compile_source_to_bef(&src)
            .expect_err(&format!("deberia rechazarse: {body}"))
            .to_string();
        assert!(err.contains(pista), "{body}\n => {err:?}\n  (se esperaba {pista:?})");
    }
}

/// Dos párrafos con el mismo nombre hacen que un `PERFORM` no sepa a cuál
/// va. Se dice al declararlos, no al llamarlos.
#[test]
fn dos_parrafos_con_el_mismo_nombre_se_rechazan() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "STOP RUN.\n1000-A.\nDISPLAY \"a\".\n1000-A.\nDISPLAY \"otra\".",
    );
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("ya existe"), "{err}");
}

/// El orden importa y se ve: si el `PERFORM` no volviera, la segunda línea
/// no saldría.
#[test]
fn despues_del_perform_sigue_el_cuerpo_principal() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "PERFORM 1000-UNO.\n\
         DISPLAY \"vuelvo\".\n\
         STOP RUN.\n\
         1000-UNO.\n\
         DISPLAY \"dentro\".",
    );
    assert_eq!(run_cobol(&src), "dentro\nvuelvo\n");
}

/// Un `PERFORM` DENTRO de un párrafo. La salida del de fuera se guarda en
/// la pila; sin eso, el de fuera no volvería nunca.
#[test]
fn los_perform_se_anidan() {
    let src = programa_con_parrafos(
        "01 A PIC 9(3).",
        "PERFORM 1000-FUERA.\n\
         DISPLAY \"raiz\".\n\
         STOP RUN.\n\
         1000-FUERA.\n\
         DISPLAY \"fuera-antes\".\n\
         PERFORM 2000-DENTRO.\n\
         DISPLAY \"fuera-despues\".\n\
         2000-DENTRO.\n\
         DISPLAY \"dentro\".",
    );
    assert_eq!(
        run_cobol(&src),
        "fuera-antes\ndentro\nfuera-despues\nraiz\n",
        "un PERFORM anidado se comio la salida del de fuera"
    );
}


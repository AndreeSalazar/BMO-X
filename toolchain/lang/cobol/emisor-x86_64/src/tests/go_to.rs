//! GO TO -- 5 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// -- GO TO: el descarte dentro de un rango ---------------------------

/// * EL CASO POR EL QUE EXISTE: descartar un registro dentro de un
/// `PERFORM ... THRU`, saltando al parrafo de salida.
///
/// Es lo que el ejemplo del nivel 8 escribia con un interruptor porque esto
/// no existia -- y lo decia ahi mismo, porque fingirlo con un `PERFORM` del
/// parrafo de salida no vale: aquel lo ejecuta y **vuelve**, asi que el
/// trabajo de debajo se hace igual.
#[test]
fn go_to_descarta_dentro_de_un_rango() {
    let src = programa_con_parrafos(
        "01 I PIC 9(3) VALUE ZERO.\n01 CONTADOS PIC 9(3) VALUE ZERO.",
        "PERFORM 1000-VALIDA THRU 1000-SALIR.\n\
         MOVE 5 TO I.\n\
         PERFORM 1000-VALIDA THRU 1000-SALIR.\n\
         DISPLAY CONTADOS.\n\
         STOP RUN.\n\
         1000-VALIDA.\n\
         IF I = 0\n\
         GO TO 1000-SALIR\n\
         END-IF.\n\
         1100-CUENTA.\n\
         ADD 1 TO CONTADOS.\n\
         1000-SALIR.\n\
         EXIT.",
    );
    // La primera vuelta descarta (I = 0), la segunda cuenta.
    assert_eq!(run_cobol(&src), "1\n", "el GO TO no salto el trabajo de en medio");
}

/// Y **vuelve al PERFORM que lo llamo**: despues del rango, el cuerpo
/// principal sigue. Un salto que se comiera el retorno dejaria el programa
/// en cualquier parte.
#[test]
fn despues_de_un_go_to_el_perform_vuelve() {
    let src = programa_con_parrafos(
        "01 X PIC 9.",
        "PERFORM 1000-A THRU 1000-FIN.\n\
         DISPLAY \"volvi\".\n\
         STOP RUN.\n\
         1000-A.\n\
         DISPLAY \"a\".\n\
         GO TO 1000-FIN.\n\
         1000-B.\n\
         DISPLAY \"b\".\n\
         1000-FIN.\n\
         EXIT.",
    );
    assert_eq!(run_cobol(&src), "a\nvolvi\n", "o no salto, o no volvio");
}

/// Un `GO TO` hacia ATRAS es un bucle, y es COBOL legitimo del de siempre.
#[test]
fn un_go_to_hacia_atras_es_un_bucle() {
    let src = programa_con_parrafos(
        "01 I PIC 9(3) VALUE ZERO.",
        "PERFORM 1000-BUCLE THRU 1000-FIN.\n\
         DISPLAY I.\n\
         STOP RUN.\n\
         1000-BUCLE.\n\
         ADD 1 TO I.\n\
         IF I < 4\n\
         GO TO 1000-BUCLE\n\
         END-IF.\n\
         1000-FIN.\n\
         EXIT.",
    );
    assert_eq!(run_cobol(&src), "4\n");
}

/// Desde el cuerpo principal NO: aqui un parrafo es una subrutina a la que
/// se entra por `call`, y saltar dentro sin haber entrado por su `PERFORM`
/// dejaria el `ret` del final sin direccion a la que volver.
#[test]
fn un_go_to_desde_el_cuerpo_principal_se_rechaza() {
    let src = programa_con_parrafos(
        "01 X PIC 9.",
        "GO TO 1000-A.\nSTOP RUN.\n1000-A.\nDISPLAY \"a\".",
    );
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("cuerpo principal"), "{err}");
}

/// Y a un parrafo que no existe, tampoco.
#[test]
fn un_go_to_a_la_nada_se_rechaza() {
    let src = programa_con_parrafos(
        "01 X PIC 9.",
        "PERFORM 1000-A.\nSTOP RUN.\n1000-A.\nGO TO 9000-NO-EXISTE.",
    );
    let err = compile_source_to_bef(&src).unwrap_err().to_string();
    assert!(err.contains("no hay ningun parrafo"), "{err}");
}


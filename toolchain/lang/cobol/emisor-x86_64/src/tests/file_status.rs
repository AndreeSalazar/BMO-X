//! FILE STATUS -- 6 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

/// * `35` -- el fichero no existe. Es el caso que mas se da y el unico
/// motivo que la puerta permite distinguir hoy.
#[test]
fn file_status_dice_35_cuando_el_fichero_no_esta() {
    let src = programa_con_estado(
        "FILE SECTION.
FD ENTRADA.
01 R PIC 9(4).
         WORKING-STORAGE SECTION.
01 ST PIC XX VALUE \"??\".",
        "OPEN INPUT ENTRADA.
         IF ST = \"00\"
DISPLAY \"abierto\"
ELSE
DISPLAY ST
END-IF.",
    );
    // Sin sembrar el fichero: no existe.
    let (consola, _) = run_cobol_con_disco(&src, &[]);
    assert_eq!(consola, "35
");
}

/// Y `00` cuando si esta.
#[test]
fn file_status_dice_00_cuando_abre() {
    let src = programa_con_estado(
        "FILE SECTION.
FD ENTRADA.
01 R PIC 9(4).
         WORKING-STORAGE SECTION.
01 ST PIC XX VALUE \"??\".",
        "OPEN INPUT ENTRADA.
DISPLAY ST.",
    );
    let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "1234
")]);
    assert_eq!(consola, "00
");
}

/// * `10` -- fin de fichero. Es la forma del estandar de escribir un bucle
/// de batch: se lee hasta que el estado deja de ser `00`.
#[test]
fn file_status_dice_10_al_acabarse_el_fichero() {
    let src = programa_con_estado(
        "FILE SECTION.
FD ENTRADA.
01 IMPORTE PIC S9(7)V99.
         WORKING-STORAGE SECTION.
         01 ST PIC XX VALUE \"??\".
         01 TOTAL PIC S9(9)V99 VALUE ZERO.
         01 CUANTOS PIC 9(3) VALUE ZERO.",
        "OPEN INPUT ENTRADA.
         PERFORM UNTIL ST NOT = \"00\"
         READ ENTRADA
         AT END CONTINUE
         NOT AT END ADD IMPORTE TO TOTAL
         ADD 1 TO CUANTOS
         END-READ
         END-PERFORM.
         CLOSE ENTRADA.
         DISPLAY CUANTOS.
DISPLAY TOTAL.
DISPLAY ST.",
    );
    let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "100.00
25.50
0.50
")]);
    // Tres registros, y el bucle paro POR EL ESTADO y no por una bandera
    // puesta a mano. El CLOSE lo devuelve a `00`.
    assert_eq!(consola, "3
126.00
00
");
}

/// ** `30` -- **el `CLOSE` que no guardo**, que es el estado que mas
/// importa de todos.
///
/// Hasta el `CLOSE` no hay nada en el disco: escribir es un acto de dos
/// pasos y el segundo es este. `emit_close` ponia `"00"` a pelo sin mirar
/// lo que contestaba la puerta, asi que un programa que se habia molestado
/// en declarar `FILE STATUS` --o sea, uno que preguntaba-- recibia "todo
/// bien" con el fichero sin escribir.
///
/// Y no es un caso de laboratorio: hoy `TASK_OP_ARCHIVO_CREAR` **no puede
/// reemplazar un fichero que ya existe**, asi que la SEGUNDA corrida de
/// cualquier programa que escriba su salida cae exactamente aqui.
#[test]
fn file_status_dice_30_cuando_el_close_no_guarda() {
    let src = programa_que_guarda(
        "OPEN OUTPUT SALIDA.\nMOVE 1234 TO R.\nWRITE R.\nCLOSE SALIDA.\nDISPLAY ST.",
    );
    let (consola, m) = run_cobol_sin_poder_guardar(&src, &[], &["d/s.txt"]);
    assert_eq!(consola, "30\n", "el programa tiene que ENTERARSE de que no se guardo");
    // Y que no quede a medias: o entero o nada.
    assert_eq!(m.archivo("d/s.txt"), None, "no se puede guardar un trozo");
}

/// Y `00` con el mismo programa cuando el disco si acepta.
///
/// Es la mitad que impide que el arreglo de arriba sea "poner `30` siempre":
/// las dos pruebas juntas dicen que el estado **depende de lo que paso**.
#[test]
fn file_status_dice_00_cuando_el_close_guarda() {
    let src = programa_que_guarda(
        "OPEN OUTPUT SALIDA.\nMOVE 1234 TO R.\nWRITE R.\nCLOSE SALIDA.\nDISPLAY ST.",
    );
    let (consola, m) = run_cobol_sin_poder_guardar(&src, &[], &[]);
    assert_eq!(consola, "00\n");
    assert!(m.archivo("d/s.txt").is_some(), "esta vez si tiene que estar en el disco");
}

/// El campo tiene que existir y medir DOS letras. Si no, el programa
/// compararia contra basura y decidiria por ella -- `IF ST = "00"` daria
/// falso siempre y el batch se pararia cada noche sin motivo.
#[test]
fn un_file_status_mal_declarado_se_rechaza() {
    let casos: &[(&str, &str)] = &[
        ("WORKING-STORAGE SECTION.\n01 OTRO PIC XX.", "no esta declarado"),
        ("WORKING-STORAGE SECTION.\n01 ST PIC X(5).", "tiene que ser `PIC XX`"),
        ("WORKING-STORAGE SECTION.\n01 ST PIC 99.", "tiene que ser `PIC XX`"),
    ];
    for (decls, pista) in casos {
        let src = format!(
            "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
             ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
             SELECT ENTRADA ASSIGN TO \"d/e.txt\" FILE STATUS IS ST.\n\
             DATA DIVISION.\nFILE SECTION.\nFD ENTRADA.\n01 R PIC 9(4).\n\
             {decls}\nPROCEDURE DIVISION.\nOPEN INPUT ENTRADA.\nSTOP RUN.\n"
        );
        let err = compile_source_to_bef(&src)
            .expect_err(&format!("deberia rechazarse: {decls}"))
            .to_string();
        assert!(err.contains(pista), "{decls}\n => {err:?}");
    }
}

/// * Cerrar una ENTRADA no puede dar `30` por accidente.
///
/// La puerta contesta `1` al cerrar un fichero de lectura porque no hay
/// nada que guardar. Si eso se leyera como fallo, todo batch que cierre su
/// fichero de entrada --o sea, todos-- se pararia creyendo que el disco esta
/// roto. Esta es la prueba de que el arreglo no se paso de listo.
#[test]
fn cerrar_una_entrada_sigue_dando_00() {
    let src = programa_con_estado(
        "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(4).\n\
         WORKING-STORAGE SECTION.\n01 ST PIC XX VALUE \"??\".",
        "OPEN INPUT ENTRADA.\nCLOSE ENTRADA.\nDISPLAY ST.",
    );
    let (consola, _) = run_cobol_con_disco(&src, &[("d/e.txt", "1234\n")]);
    assert_eq!(consola, "00\n");
}


//! FICHEROS -- 6 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

/// * EL BATCH. Lee un fichero de movimientos, los totaliza en decimal
/// exacto y escribe el cierre en otro fichero.
///
/// Es el programa que justifica todo lo demas: hasta ahora BMO COBOL sabia
/// calcular y sabia presentar, y no tenia de donde sacar los datos.
#[test]
fn el_batch_totaliza_un_fichero_y_escribe_el_cierre() {
    let (salida, m) = run_cobol_con_disco(
        include_str!("../../examples/4-ficheros/batch.cob"),
        // Cuatro movimientos. 1000.00 + 234.56 + 0.44 + (-100.00).
        &[("datos/movim.txt", "1000.00\n234.56\n0.44\n-100.00\n")],
    );
    let esperado = [
        "BATCH DE CIERRE - BANCO BMO",
        "total del dia:",
        " $1,135.00",
        "cierre escrito en datos/cierre.txt",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(salida, esperado);
    // Y en el disco queda el total, no un fichero vacio ni a medias.
    assert_eq!(m.archivo_texto("datos/cierre.txt").as_deref(), Some("1135.00\n"));
}

/// Un fichero que no existe NO es un fichero vacio: el `AT END` salta a la
/// primera y el total es cero, sin reventar. En un batch nocturno eso es
/// la diferencia entre "hoy no hubo movimientos" y una caida.
#[test]
fn un_fichero_que_falta_da_cero_y_no_revienta() {
    let (salida, m) = run_cobol_con_disco(include_str!("../../examples/4-ficheros/batch.cob"), &[]);
    assert!(salida.contains("total del dia:"), "{salida}");
    assert!(salida.contains("     $0.00"), "{salida}");
    assert_eq!(m.archivo_texto("datos/cierre.txt").as_deref(), Some("0.00\n"));
}

/// Un `READ` sin `AT END` se RECHAZA. Compilaria a un `PERFORM UNTIL` que
/// no termina nunca, y eso es peor que no compilar.
#[test]
fn un_read_sin_at_end_se_rechaza() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
               ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
               SELECT F ASSIGN TO \"a.txt\".\n\
               DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
               PROCEDURE DIVISION.\nOPEN INPUT F.\nREAD F END-READ.\nCLOSE F.\nSTOP RUN.\n";
    let e = compile_source_to_bef(src).unwrap_err();
    assert!(format!("{e:?}").contains("AT END"), "{e:?}");
}

/// Una ruta que no cabe en 8.3 se rechaza AL COMPILAR.
///
/// En la maquina, `apps/movimientos.txt` daria handle nulo, y COBOL lee un
/// handle nulo como "fin de fichero desde el principio": un cierre a cero
/// sin una sola queja. El nombre se sabe al compilar, asi que se dice al
/// compilar.
#[test]
fn una_ruta_que_no_cabe_en_8_3_se_rechaza_al_compilar() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
               ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
               SELECT F ASSIGN TO \"apps/movimientos.txt\".\n\
               DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
               PROCEDURE DIVISION.\nSTOP RUN.\n";
    let t = format!("{:?}", compile_source_to_bef(src).unwrap_err());
    assert!(t.contains("no cabe en 8.3") && t.contains("movimientos.txt"), "{t}");
}

/// Y las rutas que si caben siguen pasando, incluida la letra de unidad.
#[test]
fn las_rutas_de_8_3_con_letra_de_unidad_pasan() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
               ENVIRONMENT DIVISION.\nINPUT-OUTPUT SECTION.\nFILE-CONTROL.\n\
               SELECT F ASSIGN TO \"A:/apps/movim.txt\".\n\
               DATA DIVISION.\nFILE SECTION.\nFD F.\n01 R PIC 9(3).\n\
               PROCEDURE DIVISION.\nSTOP RUN.\n";
    assert!(compile_source_to_bef(src).is_ok(), "A:/apps/movim.txt tiene que valer");
}

/// Usar un fichero que nadie declaro se rechaza con el `SELECT` que falta,
/// no con un "no se pudo".
#[test]
fn un_fichero_sin_select_se_rechaza_diciendo_cual() {
    let src = "IDENTIFICATION DIVISION.\nPROGRAM-ID. T.\n\
               DATA DIVISION.\nWORKING-STORAGE SECTION.\n01 A PIC 9(3).\n\
               PROCEDURE DIVISION.\nOPEN INPUT NADIE.\nSTOP RUN.\n";
    let e = compile_source_to_bef(src).unwrap_err();
    let t = format!("{e:?}");
    assert!(t.contains("NADIE") && t.contains("SELECT"), "{t}");
}


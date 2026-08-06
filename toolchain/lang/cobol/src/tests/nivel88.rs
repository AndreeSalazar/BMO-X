//! NIVEL88 — 9 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

// ── NIVEL 88: lo que se RECHAZA ─────────────────────────────────────

/// Un `88` con `PIC` no es un 88: es alguien que cree estar declarando un
/// dato. Se dice qué es un nombre de condición.
#[test]
fn un_88_con_pic_se_rechaza() {
    let src = program("01 F PIC 9.\n88 FIN PIC 9 VALUE 1.", "MOVE 1 TO F.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("nombre de condicion") && t.contains("no lleva PIC"), "{t}");
}

/// Un `88` es el apodo de una comparación sobre el dato de arriba. Si no
/// hay nadie arriba, no hay de qué colgarlo.
#[test]
fn un_88_sin_dato_encima_se_rechaza() {
    let src = program("88 FIN VALUE 1.", "STOP RUN.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("no hay ningun dato encima"), "{t}");
}

/// ★ `88 … VALUE 1 THRU 5` — los dos extremos INCLUIDOS.
///
/// Estaba rechazado porque expandirlo pide un `OR`. Ya hay `OR`, así que se
/// expande a `DIA >= 1 AND DIA <= 5` y baja por el mismo emisor de árboles
/// que una condición escrita a mano.
///
/// Se recorre el rango entero y **los dos vecinos de fuera**: un `>` donde
/// va un `>=` sólo se ve en el extremo, y ahí es donde vive el error de
/// "el día 1 no era laborable".
#[test]
fn un_88_con_rango_compara_el_rango_entero() {
    for dia in 0..=7 {
        let esperado = if (1..=5).contains(&dia) { "labor\n" } else { "fiesta\n" };
        let src = program(
            "01 DIA PIC 9.\n88 LABORABLE VALUE 1 THRU 5.",
            &format!(
                "MOVE {dia} TO DIA.\n\
                 IF LABORABLE\nDISPLAY \"labor\"\nELSE\nDISPLAY \"fiesta\"\nEND-IF."
            ),
        );
        assert_eq!(run_cobol(&src), esperado, "dia {dia}");
    }
}

/// Y varios valores sueltos, que es un `OR`. `THROUGH` es el sinónimo largo
/// de `THRU` y tiene que valer igual.
#[test]
fn un_88_con_varios_valores_es_un_or() {
    for dia in 1..=7 {
        let esperado = if dia == 6 || dia == 7 { "fin\n" } else { "no\n" };
        let src = program(
            "01 DIA PIC 9.\n88 FIN-DE-SEMANA VALUE 6, 7.",
            &format!(
                "MOVE {dia} TO DIA.\n\
                 IF FIN-DE-SEMANA\nDISPLAY \"fin\"\nELSE\nDISPLAY \"no\"\nEND-IF."
            ),
        );
        assert_eq!(run_cobol(&src), esperado, "dia {dia}");
    }

    let src = program(
        "01 D PIC 9.\n88 R VALUE 2 THROUGH 4.",
        "MOVE 3 TO D.\nIF R\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF.",
    );
    assert_eq!(run_cobol(&src), "si\n", "THROUGH no vale lo mismo que THRU");
}

/// Mezclando las dos formas, que es como se escribe una tabla de códigos de
/// verdad: unos sueltos y un tramo.
#[test]
fn un_88_mezcla_rangos_y_valores_sueltos() {
    for c in 0..=9 {
        let esperado = if c == 0 || (3..=5).contains(&c) || c == 9 { "si\n" } else { "no\n" };
        let src = program(
            "01 C PIC 9.\n88 VALIDO VALUE 0, 3 THRU 5, 9.",
            &format!("MOVE {c} TO C.\nIF VALIDO\nDISPLAY \"si\"\nELSE\nDISPLAY \"no\"\nEND-IF."),
        );
        assert_eq!(run_cobol(&src), esperado, "codigo {c}");
    }
}

/// Un `88` con decimales: el rango se compara en la escala del padre, no en
/// enteros. Un `9.99` que se leyera como `9` daría un rango de más.
#[test]
fn un_88_con_rango_respeta_la_escala_del_padre() {
    let casos: &[(&str, &str)] = &[
        ("9.98", "fuera\n"),
        ("9.99", "dentro\n"),
        ("15.00", "dentro\n"),
        ("20.00", "dentro\n"),
        ("20.01", "fuera\n"),
    ];
    for (importe, esperado) in casos {
        let src = program(
            "01 IMPORTE PIC S9(5)V99.\n88 EN-TRAMO VALUE 9.99 THRU 20.00.",
            &format!(
                "MOVE {importe} TO IMPORTE.\n\
                 IF EN-TRAMO\nDISPLAY \"dentro\"\nELSE\nDISPLAY \"fuera\"\nEND-IF."
            ),
        );
        assert_eq!(run_cobol(&src), *esperado, "importe {importe}");
    }
}

/// Un `88` dentro de una condición compuesta: se combina con lo demás como
/// cualquier comparación, porque baja por el mismo árbol.
#[test]
fn un_88_se_combina_con_otras_condiciones() {
    let src = program(
        "01 D PIC 9.\n88 LABORABLE VALUE 1 THRU 5.\n01 SALDO PIC S9(5)V99.",
        "MOVE 3 TO D.\nMOVE 100.00 TO SALDO.\n\
         IF LABORABLE AND SALDO > 50.00\nDISPLAY \"abre\"\nELSE\nDISPLAY \"cierra\"\nEND-IF.",
    );
    assert_eq!(run_cobol(&src), "abre\n");
}

/// Una palabra suelta en un `IF` que no es ningún 88 se rechaza diciendo
/// las dos salidas. Antes, `IF LO-QUE-SEA` no encontraba operador y el
/// mensaje mandaba a buscar un `=` que nadie quería escribir.
#[test]
fn un_nombre_de_condicion_que_no_existe_se_rechaza() {
    let src = program("01 F PIC 9.", "MOVE 1 TO F.\nIF PEPE\nDISPLAY \"x\"\nEND-IF.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("PEPE") && t.contains("nivel 88"), "{t}");
}

/// Y un `88` no ocupa memoria: declarar veinte no mueve ni un byte el marco
/// de pila. Es la prueba de que es un apodo y no un dato.
#[test]
fn los_88_no_ocupan_memoria() {
    let sin = compile_source_to_bef(&program("01 F PIC 9.", "MOVE 1 TO F.")).unwrap();
    let con = compile_source_to_bef(&program(
        "01 F PIC 9.\n88 A VALUE 1.\n88 B VALUE 2.\n88 C VALUE 3.",
        "MOVE 1 TO F.",
    ))
    .unwrap();
    assert_eq!(
        code_section(&sin).len(),
        code_section(&con).len(),
        "tres nombres de condicion no deben cambiar ni un byte del codigo"
    );
}


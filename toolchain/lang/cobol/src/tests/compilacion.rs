//! COMPILACION -- 11 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

#[test]
fn parses_display_program() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BMO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NAME PIC X(20).
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
    let program = parse(src).unwrap();
    assert_eq!(program.program_id, "HELLO-BMO");
    assert_eq!(program.data_items.len(), 1);
    assert_eq!(program.data_items[0].name, "WS-NAME");
}

#[test]
fn emits_bef() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
    assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
    let validation = bmo_abi::bef::validate(&bef);
    assert!(validation.is_valid, "generated BEF must validate: {:?}", validation.issues);
    let loaded = bmo_abi::bef::load(&bef, 0, bmo_abi::bef::loader::no_imports).unwrap();
    assert_ne!(loaded.entry_point, 0);
    assert!(loaded.sections.iter().any(|section| section.kind == bmo_abi::bef::SectionKind::Code));
}

/// Matriz de conformidad de COBOL: ejecuta cada verbo y compara.
///
/// Misma idea que la de C. Antes de existir, `IF` ejecutaba las dos
/// ramas y `PERFORM` no repetia nada -- y el BEF validaba.
#[test]
fn cobol_feature_matrix_runs_correctly() {
    let cases: &[(&str, &str, &str, &str)] = &[
        ("MOVE literal", "01 A PIC 9(3).", "MOVE 7 TO A.\nIF A = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("MOVE variable", "01 A PIC 9(3).\n01 B PIC 9(3).", "MOVE 5 TO A.\nMOVE A TO B.\nIF B = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("ADD", "01 A PIC 9(3).", "MOVE 2 TO A.\nADD 3 TO A.\nIF A = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("SUBTRACT", "01 A PIC 9(3).", "MOVE 9 TO A.\nSUBTRACT 4 FROM A.\nIF A = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("MULTIPLY", "01 A PIC 9(3).", "MOVE 3 TO A.\nMULTIPLY 4 BY A.\nIF A = 12\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("DIVIDE", "01 A PIC 9(3).", "MOVE 12 TO A.\nDIVIDE 4 BY A.\nIF A = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("COMPUTE", "01 A PIC 9(3).", "COMPUTE A = 2 + 3 * 4.\nIF A = 14\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("COMPUTE parens", "01 A PIC 9(3).", "COMPUTE A = (2 + 3) * 4.\nIF A = 20\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("COMPUTE vars", "01 A PIC 9(3).\n01 B PIC 9(3).", "MOVE 6 TO A.\nMOVE 7 TO B.\nCOMPUTE A = A * B.\nIF A = 42\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("IF/ELSE", "01 A PIC 9(3).", "MOVE 1 TO A.\nIF A > 5\nDISPLAY \"no\"\nELSE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("IF anidado", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A > 1\nIF A < 9\nDISPLAY \"ok\"\nEND-IF\nEND-IF.", "ok\n"),
        ("IF con AND", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A > 1 AND A < 9\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("PERFORM TIMES", "01 A PIC 9(3).", "PERFORM 2 TIMES\nDISPLAY \"ok\"\nEND-PERFORM.", "ok\nok\n"),
        ("PERFORM UNTIL", "01 I PIC 9(3).", "MOVE 0 TO I.\nPERFORM UNTIL I >= 2\nDISPLAY \"ok\"\nADD 1 TO I\nEND-PERFORM.", "ok\nok\n"),
        ("EVALUATE sujeto", "01 T PIC 9.", "MOVE 2 TO T.\nEVALUATE T\nWHEN 1\nDISPLAY \"no\"\nWHEN 2\nDISPLAY \"ok\"\nEND-EVALUATE.", "ok\n"),
        ("EVALUATE OTHER", "01 T PIC 9.", "MOVE 9 TO T.\nEVALUATE T\nWHEN 1\nDISPLAY \"no\"\nWHEN OTHER\nDISPLAY \"ok\"\nEND-EVALUATE.", "ok\n"),
        ("EVALUATE THRU", "01 T PIC 9.", "MOVE 4 TO T.\nEVALUATE T\nWHEN 2 THRU 5\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
        ("EVALUATE lista", "01 T PIC 9.", "MOVE 7 TO T.\nEVALUATE T\nWHEN 6, 7\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
        ("EVALUATE TRUE", "01 S PIC S9(5)V99.", "MOVE 500.00 TO S.\nEVALUATE TRUE\nWHEN S > 1000.00\nDISPLAY \"no\"\nWHEN S > 100.00\nDISPLAY \"ok\"\nWHEN OTHER\nDISPLAY \"no\"\nEND-EVALUATE.", "ok\n"),
        ("OR en IF", "01 A PIC 9(3).", "MOVE 0 TO A.\nIF A = 9 OR A = 0\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("88 con THRU", "01 D PIC 9.\n88 LABORABLE VALUE 1 THRU 5.", "MOVE 3 TO D.\nIF LABORABLE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("VALUE inicial", "01 A PIC S9(5)V99 VALUE 12.34.", "DISPLAY A.", "12.34\n"),
        ("PERFORM VARYING", "01 I PIC 9(3).\n01 S PIC 9(5) VALUE ZERO.", "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 4\nADD I TO S\nEND-PERFORM.\nDISPLAY S.", "10\n"),
        ("VARYING AFTER", "01 I PIC 9(3).\n01 J PIC 9(3).\n01 N PIC 9(4) VALUE ZERO.", "PERFORM VARYING I FROM 1 BY 1 UNTIL I > 2\nAFTER J FROM 1 BY 1 UNTIL J > 3\nADD 1 TO N\nEND-PERFORM.\nDISPLAY N.", "6\n"),
        ("ROUNDED", "01 A PIC S9(5)V99 VALUE 10.00.", "DIVIDE 7 BY A ROUNDED.\nDISPLAY A.", "1.43\n"),
        ("ON SIZE ERROR", "01 A PIC 9(3) VALUE 999.", "ADD 999 TO A ON SIZE ERROR\nDISPLAY \"no cabe\"\nEND-ADD.\nDISPLAY A.", "no cabe\n999\n"),
        ("PIC X", "01 T PIC X(6) VALUE \"HOLA\".", "DISPLAY T.", "HOLA  \n"),
        ("texto compara", "01 T PIC XX VALUE \"00\".", "IF T = \"00\"\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("INSPECT", "01 T PIC X(7) VALUE \"  12 34\".", "INSPECT T REPLACING LEADING SPACE BY ZERO.\nDISPLAY T.", "0012 34\n"),
        ("STRING", "01 A PIC X(2) VALUE \"AB\".\n01 C PIC X(5).", "STRING A DELIMITED BY SIZE \"-\" DELIMITED BY SIZE A DELIMITED BY SIZE INTO C.\nDISPLAY C.", "AB-AB\n"),
        ("PERFORM anidado", "01 I PIC 9(3).", "PERFORM 2 TIMES\nPERFORM 2 TIMES\nDISPLAY \"ok\"\nEND-PERFORM\nEND-PERFORM.", "ok\nok\nok\nok\n"),
        ("decimal exacto", "01 S PIC 9(5)V99.", "MOVE 10.05 TO S.\nADD 0.20 TO S.\nIF S = 10.25\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("escalas mixtas", "01 S PIC 9(5)V99.\n01 N PIC 9(3).", "MOVE 2 TO N.\nMOVE 1.50 TO S.\nADD N TO S.\nIF S = 3.50\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("cond en palabras", "01 A PIC 9(3).", "MOVE 5 TO A.\nIF A IS EQUAL TO 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // -- PICTURE de edicion, EN EJECUCION --
        //
        // La linea del extracto. El dato sigue siendo un entero de
        // centavos; lo que cambia es que al ensenarlo se recorre la
        // mascara, y ese recorrido son instrucciones dentro del .bex.
        ("PIC moneda", "01 L PIC $$$,$$9.99.", "MOVE 12345.67 TO L.\nDISPLAY L.", "$12,345.67\n"),
        ("PIC moneda pequena", "01 L PIC $$$,$$9.99.", "MOVE 0.45 TO L.\nDISPLAY L.", "     $0.45\n"),
        // * El simbolo flotante cuando la supresion muere JUSTO tras la
        // coma: el `$` va en la casilla de la coma, porque los separadores
        // de dentro del grupo flotante son parte del grupo. Daba
        // `  $ 105.00` --con un hueco en medio-- y los 238 casos de
        // `edicion.rs` no lo veian porque comparan las dos
        // implementaciones entre si, y las dos se equivocaban igual.
        ("PIC moneda tras coma", "01 L PIC $$$,$$9.99.", "MOVE 105.00 TO L.\nDISPLAY L.", "   $105.00\n"),
        ("PIC cheque", "01 L PIC **,**9.99.", "MOVE 0.45 TO L.\nDISPLAY L.", "*****0.45\n"),
        ("PIC saldo en rojo", "01 L PIC Z,ZZ9.99CR.", "MOVE -120.00 TO L.\nDISPLAY L.", "  120.00CR\n"),
        ("PIC saldo en verde", "01 L PIC Z,ZZ9.99CR.", "MOVE 120.00 TO L.\nDISPLAY L.", "  120.00  \n"),
        ("PIC supresion", "01 L PIC Z,ZZ9.", "MOVE 7 TO L.\nDISPLAY L.", "    7\n"),
        ("PIC signo flotante", "01 L PIC ---9.", "MOVE -7 TO L.\nDISPLAY L.", "  -7\n"),
        // La edicion no toca la aritmetica: el campo se totaliza como
        // cualquier otro y solo al final se ensena con su mascara.
        ("PIC se puede sumar", "01 L PIC $$$,$$9.99.", "MOVE 10.05 TO L.\nADD 0.20 TO L.\nDISPLAY L.", "    $10.25\n"),
        // Y el signo del literal sobrevive al camino entero.
        ("literal negativo", "01 A PIC S9(3)V99.", "MOVE -1.50 TO A.\nIF A < 0\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // -- NIVEL 88: nombres de condicion --
        //
        // Un 88 no ocupa memoria: le pone nombre a una comparacion. Es lo
        // que convierte `PERFORM UNTIL FIN = 1` en
        // `PERFORM UNTIL FIN-DE-FICHERO`, que es COBOL bancario del que se
        // lee en voz alta.
        ("88 verdadero", "01 F PIC 9.\n88 FIN VALUE 1.", "MOVE 1 TO F.\nIF FIN\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("88 falso", "01 F PIC 9.\n88 FIN VALUE 1.", "MOVE 0 TO F.\nIF FIN\nDISPLAY \"no\"\nELSE\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // * Para lo que existe: el bucle del batch, legible.
        ("88 en PERFORM UNTIL", "01 F PIC 9.\n88 SE-ACABO VALUE 1.\n01 I PIC 9(3).", "MOVE 0 TO F.\nMOVE 0 TO I.\nPERFORM UNTIL SE-ACABO\nADD 1 TO I\nIF I >= 3\nMOVE 1 TO F\nEND-IF\nEND-PERFORM.\nIF I = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // Varios 88 sobre el MISMO dato, que es como se usa de verdad.
        ("88 varios sobre uno", "01 E PIC 9.\n88 ACTIVO VALUE 1.\n88 CERRADO VALUE 2.", "MOVE 2 TO E.\nIF CERRADO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // El 88 cuelga del dato de ARRIBA, no del primero del programa.
        ("88 cuelga del de arriba", "01 A PIC 9.\n01 B PIC 9.\n88 B-ES-CINCO VALUE 5.", "MOVE 5 TO A.\nMOVE 5 TO B.\nIF B-ES-CINCO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // Con decimales: el valor se escala como cualquier literal.
        ("88 con decimales", "01 S PIC S9(5)V99.\n88 SALDADO VALUE 0.00.", "MOVE 0 TO S.\nIF SALDADO\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("88 y AND", "01 F PIC 9.\n88 FIN VALUE 1.\n01 N PIC 9(3).", "MOVE 1 TO F.\nMOVE 7 TO N.\nIF FIN AND N = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // -- OCCURS: tablas --
        //
        // El subindice literal se resuelve al compilar (sin multiplicar y
        // sin comprobar nada en ejecucion); el variable, con su guarda.
        ("OCCURS literal", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 5 TO E(1).\nIF E(1) = 5\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        ("OCCURS variable", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 2 TO I.\nMOVE 7 TO E(I).\nIF E(I) = 7\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // Cada elemento es SUYO. Si el paso estuviera mal, escribir en el
        // segundo se veria en el primero -- y un total por concepto saldria
        // sumado en la casilla del vecino.
        ("OCCURS no se pisan", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.", "MOVE 1 TO E(1).\nMOVE 2 TO E(2).\nMOVE 3 TO E(3).\nIF E(1) = 1 AND E(2) = 2 AND E(3) = 3\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // * Para lo que existe OCCURS: recorrer la tabla y totalizar.
        ("OCCURS totaliza", "01 T.\n05 E PIC S9(7)V99 OCCURS 3 TIMES.\n01 I PIC 9(3).\n01 TOT PIC S9(7)V99.", "MOVE 10.05 TO E(1).\nMOVE 0.20 TO E(2).\nMOVE 1.75 TO E(3).\nMOVE 0 TO TOT.\nMOVE 1 TO I.\nPERFORM UNTIL I > 3\nADD E(I) TO TOT\nADD 1 TO I\nEND-PERFORM.\nIF TOT = 12.00\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // Un elemento con PIC editada ensena su mascara, como cualquier
        // otro dato: la edicion es de la tabla, no de la casilla.
        ("OCCURS PIC editada", "01 T.\n05 L PIC $$$,$$9.99 OCCURS 2 TIMES.", "MOVE 10.05 TO L(2).\nDISPLAY L(2).", "    $10.05\n"),
        // El subindice puede ser OTRO elemento de tabla. Es lo que prueba
        // que el valor a guardar sobrevive al calculo de la direccion.
        ("OCCURS subindice de tabla", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 X.\n05 IDX PIC 9(3) OCCURS 2 TIMES.", "MOVE 3 TO IDX(1).\nMOVE 9 TO E(IDX(1)).\nIF E(3) = 9\nDISPLAY \"ok\"\nEND-IF.", "ok\n"),
        // * Y el subindice que se sale PARA el programa diciendo cual.
        // Seguir con una direccion inventada escribiria encima del campo de
        // al lado, y el descuadre apareceria semanas despues en otro sitio.
        ("OCCURS fuera de rango", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 4 TO I.\nMOVE 1 TO E(I).\nDISPLAY \"no deberia llegar\".", "SUBINDICE FUERA DE RANGO EN E (1..3)\n"),
        ("OCCURS subindice cero", "01 T.\n05 E PIC 9(3) OCCURS 3 TIMES.\n01 I PIC 9(3).", "MOVE 0 TO I.\nMOVE 1 TO E(I).\nDISPLAY \"no deberia llegar\".", "SUBINDICE FUERA DE RANGO EN E (1..3)\n"),
    ];
    let mut broken = Vec::new();
    for (name, data, body, expected) in cases {
        let src = program(data, body);
        let got = std::panic::catch_unwind(|| run_cobol(&src))
            .unwrap_or_else(|_| "<no ejecuta>".into());
        if got != *expected {
            broken.push(format!("  {name:<18} => {got:?}  (esperado {expected:?})"));
        }
    }

    // -- E/S DE FICHEROS ---------------------------------------------
    //
    // Estos casos necesitan DISCO, asi que van con su propio banco: se
    // siembra `d/e.txt`, se ejecuta, y se mira la consola Y lo que quedo
    // en `d/s.txt`. Mirar solo la consola dejaria pasar un `WRITE` que no
    // escribe, y mirar solo el fichero dejaria pasar un `AT END` que nunca
    // salta.
    //
    // Campos: nombre, declaraciones, cuerpo, lo sembrado, consola
    // esperada, y el fichero esperado (`None` = no debe existir).
    let discos: &[(&str, &str, &str, &str, &str, Option<&str>)] = &[
        (
            "OPEN/READ/CLOSE",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"vacio\"\nNOT AT END DISPLAY R\nEND-READ.\nCLOSE ENTRADA.",
            "42\n",
            "42\n",
            None,
        ),
        (
            "AT END salta",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"ok\"\nEND-READ.\nCLOSE ENTRADA.",
            "",
            "ok\n",
            None,
        ),
        // El bucle del batch: leer hasta el final y totalizar. Es LA forma
        // del proceso por lotes, y sin `AT END` no terminaria nunca.
        (
            "PERFORM sobre fichero",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.\nWORKING-STORAGE SECTION.\n01 T PIC S9(7)V99.\n01 F PIC 9.",
            "MOVE 0 TO T.\nMOVE 0 TO F.\nOPEN INPUT ENTRADA.\nPERFORM UNTIL F = 1\nREAD ENTRADA\nAT END MOVE 1 TO F\nNOT AT END ADD R TO T\nEND-READ\nEND-PERFORM.\nCLOSE ENTRADA.\nIF T = 1235.00\nDISPLAY \"ok\"\nEND-IF.",
            "1000.00\n234.56\n0.44\n",
            "ok\n",
            None,
        ),
        // Un registro leido es un decimal EXACTO, no un float: cinco
        // centimos no pueden convertirse en cincuenta al cruzar el disco.
        (
            "READ decimal exacto",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 0.05\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
            "0.05\n",
            "ok\n",
            None,
        ),
        (
            "READ negativo",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC S9(7)V99.",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R < 0\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
            "-100.00\n",
            "ok\n",
            None,
        ),
        // El fichero viene del anfitrion con `\r\n`. Ese `\r` dentro del
        // numero lo convertiria en otro.
        (
            "READ con CRLF",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 77\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
            "77\r\n",
            "ok\n",
            None,
        ),
        // El clasico que se come el movimiento de mas valor: el ultimo.
        (
            "READ sin salto final",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).",
            "OPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 9\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
            "9",
            "ok\n",
            None,
        ),
        (
            "OPEN OUTPUT/WRITE",
            "FILE SECTION.\nFD SALIDA.\n01 S PIC S9(7)V99.",
            "MOVE 1135.00 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.",
            "",
            "",
            Some("1135.00\n"),
        ),
        // * El registro con PIC editada escribe su LINEA, no su numero:
        // eso es un informe bancario. Antes de `emitir_en_buffer` esto
        // habria escrito `10.25` callando que habia mascara.
        (
            "WRITE PIC editada",
            "FILE SECTION.\nFD SALIDA.\n01 S PIC $$$,$$9.99.",
            "MOVE 10.05 TO S.\nADD 0.20 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.",
            "",
            "",
            Some("    $10.25\n"),
        ),
        (
            "WRITE varias lineas",
            "FILE SECTION.\nFD SALIDA.\n01 S PIC 9(3).",
            "OPEN OUTPUT SALIDA.\nMOVE 1 TO S.\nWRITE S.\nMOVE 2 TO S.\nWRITE S.\nCLOSE SALIDA.",
            "",
            "",
            Some("1\n2\n"),
        ),
        // Sin CLOSE no se guarda NADA. No medio fichero: ninguno. Un
        // extracto truncado se parece demasiado a uno completo.
        (
            "sin CLOSE no se guarda",
            "FILE SECTION.\nFD SALIDA.\n01 S PIC 9(3).",
            "MOVE 7 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.",
            "",
            "",
            None,
        ),
        // Y la vuelta entera: lo escrito se puede volver a leer. Es el
        // contrato entre `WRITE` y `read_line` -- un registro por linea.
        (
            "lo escrito se relee",
            "FILE SECTION.\nFD ENTRADA.\n01 R PIC 9(5).\nFD SALIDA.\n01 S PIC 9(5).",
            "MOVE 314 TO S.\nOPEN OUTPUT SALIDA.\nWRITE S.\nCLOSE SALIDA.\nOPEN INPUT ENTRADA.\nREAD ENTRADA\nAT END DISPLAY \"no\"\nNOT AT END IF R = 314\nDISPLAY \"ok\"\nEND-IF\nEND-READ.\nCLOSE ENTRADA.",
            "314\n",
            "ok\n",
            Some("314\n"),
        ),
    ];
    for (name, decls, body, entrada, esperado, fichero) in discos {
        let src = programa_con_ficheros(decls, body);
        let sembrado: Vec<(&str, &str)> =
            if entrada.is_empty() { vec![] } else { vec![("d/e.txt", entrada)] };
        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let (consola, m) = run_cobol_con_disco(&src, &sembrado);
            (consola, m.archivo_texto("d/s.txt"))
        }));
        match got {
            Err(_) => broken.push(format!("  {name:<22} => <no ejecuta>")),
            Ok((consola, en_disco)) => {
                if consola != *esperado {
                    broken.push(format!(
                        "  {name:<22} => consola {consola:?}  (esperado {esperado:?})"
                    ));
                }
                if en_disco.as_deref() != *fichero {
                    broken.push(format!(
                        "  {name:<22} => disco {en_disco:?}  (esperado {fichero:?})"
                    ));
                }
            }
        }
    }

    let total = cases.len() + discos.len();
    assert!(broken.is_empty(), "\n{}/{} FUNCIONAN. ROTOS:\n{}", total - broken.len(), total, broken.join("\n"));
}

/// Un dato que nadie declaro se rechaza. Antes `load_var`/`store_var` no
/// emitian NADA: `DISPLAY PEPE` imprimia lo que hubiera en `rax` y
/// `MOVE 1 TO PEPE` se perdia sin una palabra.
#[test]
fn un_dato_sin_declarar_se_rechaza() {
    let src = program("01 A PIC 9(3).", "MOVE 1 TO PEPE.");
    let t = format!("{:?}", compile_source_to_bef(&src).unwrap_err());
    assert!(t.contains("PEPE") && t.contains("no esta declarado"), "{t}");
}

/// El puente L2->L1: `DISPLAY "texto"` debe bajar a la puerta de consola
/// del ABI, con el salto de linea que COBOL exige al final (cada DISPLAY
/// ocupa su propia fila porque `\n` dispara el flush del kernel).
///
/// Antes de esto, COBOL emitia `syscall NR_DEBUG_PRINT` con un puntero --
/// numero que el kernel no despacha y forma que la superficie congelada
/// rechaza. En hardware no imprimia nada.
#[test]
fn display_lowers_to_the_console_door() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
    let bef = compile_source_to_bef(src).unwrap();
    let mut door = Vec::new();
    bmo_lower::console::write_const(&mut door, b"HOLA COBOL\n");
    assert!(
        !door.is_empty() && bef.windows(door.len()).any(|w| w == door),
        "el BEF debe contener la secuencia INVOKE/CONSOLE_WRITE de la puerta"
    );
}

/// El cierre del programa no puede usar `hlt`: es privilegiada, y en
/// Ring 3 provoca el #GP del que pretendia proteger.
#[test]
fn program_epilogue_has_no_privileged_instruction() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "X".
STOP RUN.
"#;
    let bef = compile_source_to_bef(src).unwrap();
    let mut net = Vec::new();
    bmo_lower::task::exit(&mut net);
    assert!(
        bef.windows(net.len()).any(|w| w == net),
        "el epílogo debe ser INVOKE(EXIT) + red de pause/jmp"
    );
}

#[test]
fn emits_valid_bex_image() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BEX.
PROCEDURE DIVISION.
DISPLAY "HOLA BMO".
STOP RUN.
"#;
    let bex = compile_source_to_bex(src).unwrap();
    assert!(bmo_abi::bex::validate(&bex).is_valid);
    assert_eq!(bmo_abi::bex::BEX_WIRE_MAGIC, bmo_abi::bef::BEF_MAGIC);
}

/// La E/S de ficheros, tal y como se escribe de verdad: el `SELECT` le da
/// la ruta, el `FD` le da el registro y el `READ` lleva su `AT END`.
///
/// Este test decia antes `READ INFILE INTO WS-REC.` sin `SELECT`, sin `FD`
/// y sin `AT END`, y pasaba -- porque el parser guardaba dos cadenas y el
/// codegen las tiraba. Ahora un fichero es un fichero: si le falta la ruta
/// o el registro, no compila.
#[test]
fn parses_open_read_write_close() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FILEIO.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
SELECT INFILE ASSIGN TO "datos/mov.txt".
DATA DIVISION.
FILE SECTION.
FD INFILE.
01 WS-REC PIC 9(5).
PROCEDURE DIVISION.
OPEN INPUT INFILE.
READ INFILE
AT END DISPLAY "fin"
NOT AT END DISPLAY WS-REC
END-READ.
WRITE WS-REC.
CLOSE INFILE.
STOP RUN.
"#;
    let program = parse(src).unwrap();
    assert_eq!(program.statements.len(), 5);
    // La ruta y el registro llegan al AST: sin los dos no hay E/S.
    let f = program.file("INFILE").expect("el SELECT declara INFILE");
    assert_eq!(f.path, "datos/mov.txt");
    assert_eq!(f.record, "WS-REC");
    // Y el READ se queda con sus DOS ramas, no con una cadena.
    match &program.statements[1] {
        CobolStatement::Read(name, al_final, si_hay) => {
            assert_eq!(name, "INFILE");
            assert_eq!(al_final.len(), 1);
            assert_eq!(si_hay.len(), 1);
        }
        otro => panic!("se esperaba un READ, no {otro:?}"),
    }
}

/// `PERFORM` ahora exige cuerpo y cierre: sin ellos no habia nada que
/// repetir, y la version anterior lo aceptaba emitiendo un no-op.
#[test]
fn parses_perform() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LOOP.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-COUNT PIC 9(3).
PROCEDURE DIVISION.
PERFORM 5 TIMES
  ADD 1 TO WS-COUNT
END-PERFORM.
PERFORM UNTIL WS-COUNT > 10
  ADD 1 TO WS-COUNT
END-PERFORM.
STOP RUN.
"#;
    let program = parse(src).unwrap();
    assert!(program.statements.len() >= 2);
    assert!(matches!(program.statements[0], CobolStatement::PerformTimes(5, _)));
    assert!(matches!(program.statements[1], CobolStatement::PerformUntil(_, _)));
}

#[test]
fn cobol_syscall_with_asm_path() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TEST.
USE "bmo/proc".
PROCEDURE DIVISION.
SYSCALL bmo_exit 42.
"#;
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
    let bef = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
    assert!(bef.len() > 48);
    let nr = bmo_abi::syscalls::surface::NR_INVOKE;
    let mov_eax = &nr.to_le_bytes();
    let mut expected = vec![0xB8u8];
    expected.extend_from_slice(mov_eax);
    assert!(bef.windows(5).any(|w| w == &expected[..]), "BEF should lower bmo_exit to BMO_INVOKE");

    let mut current_task = vec![0x48, 0xB8];
    current_task.extend_from_slice(&bmo_abi::syscalls::surface::CURRENT_TASK.to_le_bytes());
    current_task.extend_from_slice(&[0x48, 0x89, 0xC7]); // mov rdi, rax
    assert!(bef.windows(current_task.len()).any(|w| w == &current_task[..]));

    let mut exit_operation = vec![0x48, 0xB8];
    exit_operation.extend_from_slice(&bmo_abi::syscalls::surface::task_op::EXIT.to_le_bytes());
    exit_operation.extend_from_slice(&[0x48, 0x89, 0xC6]); // mov rsi, rax
    assert!(bef.windows(exit_operation.len()).any(|w| w == &exit_operation[..]));

    let mut exit_code = vec![0x48, 0xB8];
    exit_code.extend_from_slice(&42_u64.to_le_bytes());
    exit_code.extend_from_slice(&[0x48, 0x89, 0xC2]); // mov rdx, rax
    assert!(bef.windows(exit_code.len()).any(|w| w == &exit_code[..]));
}

#[test]
fn cobol_syscall_uses_r10_for_fourth_argument() {
    let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. SYSCALL-ABI.
PROCEDURE DIVISION.
SYSCALL bmo_wm_create_window 17, 34, 51, 68.
"#;
    let bef = compile_source_to_bef(src).unwrap();
    let mut expected = vec![0x48, 0xB8]; // mov rax, 68
    expected.extend_from_slice(&68_u64.to_le_bytes());
    expected.extend_from_slice(&[0x49, 0x89, 0xC2]); // mov r10, rax
    assert!(bef.windows(expected.len()).any(|window| window == expected));
}


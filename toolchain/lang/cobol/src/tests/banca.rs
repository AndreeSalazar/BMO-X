//! BANCA — 9 pruebas.

#[allow(unused_imports)]
use crate::*;
#[allow(unused_imports)]
use std::path::PathBuf;
#[allow(unused_imports)]
use super::comun::*;

/// ★ EL NIVEL 9, ejecutado: la tabla de decisión y el redondeo legal.
#[test]
fn el_ejemplo_de_decision_calcula_las_comisiones() {
    let salida = run_cobol(include_str!("../../examples/9-decision/comision.cob"));
    // Tres clientes, tres tramos. 1500 × 0,25 % = 3,75 exacto.
    assert!(salida.contains(" $1,500.00"), "{salida}");
    assert!(salida.contains("     $3.75"), "el tramo preferente:\n{salida}");
    // 500 × 0,50 % = 2,50 exacto.
    assert!(salida.contains("     $2.50"), "{salida}");
    // 50 × 0,75 % = 0,375 → redondeado, 0,38. Truncado sería 0,37.
    assert!(salida.contains("     $0.38"), "el ROUNDED del tercer tramo:\n{salida}");
    // ★ Y el sesgo: el clásico inventa dos céntimos, el banquero cuadra.
    assert!(salida.contains("0.10"), "el clasico tenia que subir los cuatro:\n{salida}");
    assert!(salida.contains("0.08"), "el del banquero tenia que cuadrar:\n{salida}");
}

/// El payload `hola_COBOL.bex` que el kernel EMBEBE, ejecutado.
///
/// Regenerar tras tocar el codegen:
///   cargo run -p bmo-cobol-front --     ///     toolchain/lang/cobol/examples/2-decimal/hola_COBOL.cob     ///     -o Ultra_kernel_x86-64/kernel/src/ring0/hola_COBOL.bex
#[test]
fn hola_cobol_payload_output_is_what_the_kernel_will_show() {
    let out = run_cobol(include_str!("../../examples/2-decimal/hola_COBOL.cob"));
    let esperado = [
        "hola desde COBOL en el Ryzen",
        "3 x 19.99 = 59.97 exacto",
        "cargo entero aplicado bien",
        "recibo emitido",
        "recibo emitido",
        "dos devoluciones aplicadas",
        "COBOL termino ok",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(out, esperado);
}

/// El extracto entero, ejecutado. Es la prueba de que la cadena completa
/// —fuente COBOL, parser, codegen, BEF, CPU— produce la linea que un
/// banco imprimiria, y no una aproximacion.
///
/// Cada columna esta alineada porque cada campo mide lo que su PIC
/// declara. Si alguien rompe el ancho, este test lo dice antes de que un
/// informe salga descuadrado.
#[test]
fn el_extracto_imprime_las_lineas_de_un_banco() {
    let out = run_cobol(include_str!("../../examples/3-presentacion/extracto.cob"));
    let esperado = [
        "BANCO BMO - EXTRACTO DE CUENTA",
        "-----------------------------",
        "saldo disponible:",
        "$12,345.67",
        "talon a cobrar:",
        "*****0.45",
        "balance final:",
        "  120.00CR",
        "cuenta en descubierto",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(out, esperado);
}

/// Los ficheros escritos desde el anfitrión traen `\r\n`. Ese `\r` dentro
/// del número lo convertiría en otro.
#[test]
fn el_batch_aguanta_los_finales_de_windows() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/4-ficheros/batch.cob"),
        &[("datos/movim.txt", "1000.00\r\n234.56\r\n")],
    );
    assert!(salida.contains(" $1,234.56"), "{salida}");
}

/// ★ EL CIERRE POR CONCEPTO. `OCCURS` y File I/O juntos: dos ficheros en
/// paralelo, cada importe a la casilla de su concepto, y el informe con
/// máscara.
///
/// Es para esto que existe `OCCURS`: sin él harían falta `TOTAL-1`…
/// `TOTAL-4` y el mismo `IF` cuatro veces. Y el subíndice viene **de un
/// fichero**, o sea que la comprobación de rango no es teórica: la decide
/// el dato, no el programador.
#[test]
fn el_cierre_por_concepto_totaliza_en_su_casilla() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/5-tablas/conceptos.cob"),
        &[
            ("datos/concs.txt", "1\n3\n2\n3\n1\n"),
            ("datos/imps.txt", "100.00\n50.00\n25.50\n10.00\n5.00\n"),
        ],
    );
    let esperado = [
        "CIERRE POR CONCEPTO - BANCO BMO",
        "totales por concepto:",
        // 100.00 + 5.00 · 25.50 · 50.00 + 10.00 · nada
        "   $105.00",
        "    $25.50",
        "    $60.00",
        "     $0.00",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(salida, esperado);
}

/// Y si el concepto que trae el fichero se sale de la tabla, el programa
/// **para diciendo cuál** en vez de sumar en la casilla del vecino.
#[test]
fn un_concepto_fuera_de_la_tabla_para_el_cierre() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/5-tablas/conceptos.cob"),
        &[("datos/concs.txt", "1\n7\n"), ("datos/imps.txt", "100.00\n50.00\n")],
    );
    assert!(
        salida.contains("SUBINDICE FUERA DE RANGO EN TOTAL-CONCEPTO (1..4)"),
        "{salida}"
    );
    assert!(!salida.contains("totales por concepto"), "no debe seguir: {salida}");
}

/// ★ LA CARTERA. El mismo batch escrito con nombres en vez de números:
/// `PERFORM UNTIL SE-ACABO` y `IF NO-HUBO-NADA`.
///
/// Es el nivel 88 haciendo lo único que hace: que la condición se lea en
/// voz alta. Quien audite esto no tiene que acordarse de qué significaba
/// el 1.
#[test]
fn la_cartera_reparte_cobros_y_devoluciones() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/6-condiciones/cartera.cob"),
        &[("datos/movim.txt", "1000.00\n234.56\n-100.00\n0.44\n-50.00\n")],
    );
    let esperado = [
        "CARTERA DEL DIA - BANCO BMO",
        "cobros:",
        " $1,235.00",
        "devoluciones:",
        // `CR` y no un menos: una máscara sin signo se come el negativo —
        // correcto según el estándar, y mentira en un informe. Escribirlo
        // así fue el error de quien montó este test, no del compilador.
        "   $150.00CR",
    ]
    .map(|l| format!("{l}\n"))
    .concat();
    assert_eq!(salida, esperado);
}

/// Y sin movimientos lo DICE, en vez de imprimir ceros callando. En un
/// cierre nocturno, un fichero vacío y uno que no se pudo leer se parecen
/// demasiado si los dos dan cero.
#[test]
fn la_cartera_sin_movimientos_lo_dice_con_su_nombre() {
    let (salida, _) =
        run_cobol_con_disco(include_str!("../../examples/6-condiciones/cartera.cob"), &[]);
    assert!(salida.contains("sin movimientos hoy"), "{salida}");
    assert!(!salida.contains("cobros:"), "no debe imprimir el informe: {salida}");
}

/// El ejemplo del repositorio, ejecutado. Si alguien vuelve a romper el
/// flujo de control, este test lo dice antes de que haga falta flashear
/// nada.
#[test]
fn banco_example_produces_its_documented_output() {
    let out = run_cobol(include_str!("../../examples/2-decimal/banco.cob"));
    assert_eq!(
        out,
        // ★ `59.97` y `19.99` NO son literales del programa: son el
        // contenido de SALDO formateado en ejecución por el código que
        // emite `emit_display_var`. Antes el ejemplo imprimía una cadena
        // escrita a mano que decía el resultado — la aritmética era real
        // pero lo que se veía no lo demostraba. Ahora sí: si el decimal se
        // perdiera, este test lo cazaría solo.
        "BMO-X: caja COBOL\n\
         cobrada una cuota\ncobrada una cuota\ncobrada una cuota\n\
         saldo tras 3 cuotas:\n59.97\ncuadra\n\
         recibo emitido\nrecibo emitido\n\
         saldo tras 2 devoluciones:\n19.99\n\
         dos devoluciones aplicadas\n"
    );
}

/// El último registro cuenta aunque el fichero no acabe en salto de línea.
/// Es el clásico que se come el movimiento de más valor: el último.
#[test]
fn el_ultimo_registro_cuenta_sin_salto_final() {
    let (salida, _) = run_cobol_con_disco(
        include_str!("../../examples/4-ficheros/batch.cob"),
        &[("datos/movim.txt", "10.00\n5.50")],
    );
    assert!(salida.contains("    $15.50"), "{salida}");
}


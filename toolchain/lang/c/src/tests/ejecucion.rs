//! EJECUTAR y comprobar el numero: aritmetica y comparaciones
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

/// Los operadores NO conmutativos estaban invertidos: se emitían sobre
/// `b - a` en vez de `a - b`. Con `+` y `*` no se notaba; con `-`, `/`,
/// `%` y los desplazamientos, sí. Nadie lo vio en 1.600 líneas de
/// codegen porque ningún test los ejecutaba.
#[test]
fn non_commutative_operators_respect_operand_order() {
    for (expr, expected) in [
        ("10 - 3", "7"),
        ("3 - 10", "-7"),
        ("10 / 3", "3"),
        ("10 % 3", "1"),
        ("1 << 3", "8"),
        ("16 >> 2", "4"),
        ("10 + 3", "13"),
        ("10 * 3", "30"),
    ] {
        let out = run_c(&format!("int main() {{ printf(\"%d\\n\", {expr}); return 0; }}"));
        assert_eq!(out.trim(), expected, "expresion: {expr}");
    }
}

/// La división entera es CON SIGNO. Antes dividía sin signo, así que un
/// negativo daba un número astronómico.
#[test]
fn integer_division_is_signed() {
    let out = run_c("int main() { printf(\"%d %d\\n\", 0 - 10, (0 - 10) / 3); return 0; }");
    assert_eq!(out, "-10 -3\n");
}

/// Todas las comparaciones, en ambos sentidos. `<`, `>` y `>=` daban el
/// resultado contrario.
#[test]
fn comparisons_answer_in_the_right_direction() {
    for (expr, expected) in [
        ("1 < 2", "1"), ("2 < 1", "0"),
        ("2 > 1", "1"), ("1 > 2", "0"),
        ("1 <= 1", "1"), ("2 <= 1", "0"),
        ("1 >= 1", "1"), ("1 >= 2", "0"),
        ("1 == 1", "1"), ("1 == 2", "0"),
        ("1 != 2", "1"), ("1 != 1", "0"),
    ] {
        let out = run_c(&format!("int main() {{ printf(\"%d\\n\", {expr}); return 0; }}"));
        assert_eq!(out.trim(), expected, "comparacion: {expr}");
    }
}

/// `setcc` solo escribe `al`. Sin extender a cero el resto de `rax`, el
/// resultado de una comparación arrastraba los bits altos del operando
/// derecho: parecía correcto con valores chicos y fallaba con grandes.
#[test]
fn comparison_result_is_clean_with_large_operands() {
    let out = run_c(
        "int main() { long a = 4294967296; long b = 4294967296; printf(\"%d\\n\", a == b); return 0; }",
    );
    assert_eq!(out, "1\n");
}

/// Un `int` con signo debe releerse con signo. Antes `mov eax,[..]`
/// rellenaba de ceros y `-7` volvía como 4294967289.
#[test]
fn negative_int_survives_a_round_trip_through_memory() {
    let out = run_c("int main() { int y = 0 - 7; printf(\"%d\\n\", y); return 0; }");
    assert_eq!(out, "-7\n");
}

#[test]
fn errors_report_real_line() {
    // Antes TODO error decía "línea 1".
    let src = "int main() {\n    int x;\n    x = ;\n    return 0;\n}";
    let err = parse(src).unwrap_err();
    assert_eq!(err.line, 3, "el error de 'x = ;' está en la línea 3, no la 1");
}


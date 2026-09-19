//! EJECUTAR y comprobar el numero: aritmetica y comparaciones
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

/// Los operadores NO conmutativos estaban invertidos: se emitian sobre
/// `b - a` en vez de `a - b`. Con `+` y `*` no se notaba; con `-`, `/`,
/// `%` y los desplazamientos, si. Nadie lo vio en 1.600 lineas de
/// codegen porque ningun test los ejecutaba.
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

/// ** EL INMEDIATO (2026-09-18): `x <op> constante` con una VARIABLE a la
/// izquierda, para que nada se pliegue y el operador lleve el numero dentro
/// (`83 /op ib`, `81 /op id`, `6B`/`69`, `C1`). Cada fila tiene su pareja
/// corta (cabe en 8 bits) y larga (cabe en 32), y las dos que NO caben --
/// `0xFFFFFFFF` y `1 << 40`-- siguen por la pila y tienen que dar lo mismo.
#[test]
fn el_operador_con_inmediato_da_lo_mismo_que_por_la_pila() {
    for (cuerpo, expected) in [
        ("int x = 41; printf(\"%d\", x + 1);", "42"),
        ("int x = 41; printf(\"%d\", x + 1000);", "1041"),
        ("int x = 41; printf(\"%d\", x - 5);", "36"),
        ("int x = 41; printf(\"%d\", x - 300);", "-259"),
        ("int x = 6; printf(\"%d\", x * 7);", "42"),
        ("int x = 6; printf(\"%d\", x * 1000);", "6000"),
        ("int x = 6; printf(\"%d\", x * -7);", "-42"),
        ("int x = 0x1234; printf(\"%d\", x & 0xFF);", "52"),
        ("int x = 0x1234; printf(\"%d\", x & 0xF);", "4"),
        ("int x = 1; printf(\"%d\", x | 0x100);", "257"),
        ("int x = 1; printf(\"%d\", x | 2);", "3"),
        ("int x = 3; printf(\"%d\", x ^ 1);", "2"),
        ("int x = 3; printf(\"%d\", x ^ 0x100);", "259"),
        ("int x = 5; printf(\"%d\", x << 3);", "40"),
        ("int x = -16; printf(\"%d\", x >> 2);", "-4"),
        ("unsigned x = 0xFFFFFFF0u; printf(\"%u\", x >> 4);", "268435455"),
        ("int x = 5; printf(\"%d %d %d\", x < 1900, x < 5, x < 3);", "1 0 0"),
        ("int x = 5; printf(\"%d %d\", x == 5, x != 5);", "1 0"),
        ("int x = -5; printf(\"%d %d\", x < 0, x > -100);", "1 1"),
        ("long x = 5; printf(\"%ld\", x + 2147483647L);", "2147483652"),
        // los que NO caben en 32 bits con signo: camino largo
        ("long x = 0x123456789L; printf(\"%ld\", x & 0xFFFFFFFF);", "591751049"),
        ("long x = 1; printf(\"%ld\", x + (1L << 40));", "1099511627777"),
        // el recorte a 32 sigue despues del inmediato
        ("int x = 2147483647; x = x + 1; printf(\"%d\", x);", "-2147483648"),
        // aritmetica de punteros: `p + 1` llega como `p + (1*4)` plegado
        ("int t[3] = {7, 8, 9}; int *p = t; printf(\"%d %d\", *(p + 1), *(p + 2 - 1));", "8 8"),
        ("int t[3] = {7, 8, 9}; int *p = t + 2; printf(\"%d\", *(p - 1));", "8"),
    ] {
        let out = run_c(&format!("int main() {{ {cuerpo} return 0; }}"));
        assert_eq!(out.trim(), expected, "cuerpo: {cuerpo}");
    }
}

/// ** LA COMPARACION FUNDIDA EN EL SALTO (2026-09-18): en un `if`, `while`,
/// `for` o `do` la comparacion ya no fabrica un 0/1, salta. Las doce
/// condiciones (seis con signo, seis sin el) en las dos direcciones, con el
/// valor justo en la frontera; y lo que NO se funde --un puntero, un `&&`,
/// una comparacion de coma flotante-- tiene que seguir contestando igual.
#[test]
fn la_comparacion_fundida_en_el_salto_decide_igual_que_el_0_o_1() {
    let mut cuerpo = String::new();
    // con signo: -5 contra 3, y el empate
    for (op, a, b, esperado) in [
        ("<", -5, 3, 1), ("<", 3, -5, 0), ("<", 3, 3, 0),
        (">", -5, 3, 0), (">", 3, -5, 1), (">", 3, 3, 0),
        ("<=", 3, 3, 1), ("<=", 4, 3, 0),
        (">=", 3, 3, 1), (">=", 2, 3, 0),
        ("==", 3, 3, 1), ("==", -3, 3, 0),
        ("!=", 3, 3, 0), ("!=", -3, 3, 1),
    ] {
        cuerpo.push_str(&format!(
            "{{ int a = {a}; int b = {b}; int r = 0; if (a {op} b) r = 1; printf(\"%d \", r == {esperado}); }}\n"
        ));
    }
    // sin signo: 0xFFFFFFF0 es GRANDE, no negativo. Con OTROS nombres, porque
    // hoy una variable redeclarada en un bloque hermano conserva el TIPO de
    // la primera (`a` seguiria siendo `int` aqui): es un fallo aparte del
    // emisor, visto el 18-09 al escribir esta fila, y esta fila no es suya.
    for (op, a, b, esperado) in [
        ("<", "0xFFFFFFF0u", "3u", 0), (">", "0xFFFFFFF0u", "3u", 1),
        ("<=", "3u", "0xFFFFFFF0u", 1), (">=", "3u", "0xFFFFFFF0u", 0),
    ] {
        cuerpo.push_str(&format!(
            "{{ unsigned ua = {a}; unsigned ub = {b}; int r = 0; if (ua {op} ub) r = 1; printf(\"%d \", r == {esperado}); }}\n"
        ));
    }
    // while, for y do-while cuentan lo mismo; y el do-while entra al menos una vez
    cuerpo.push_str("{ int i = 0; int n = 0; while (i < 10) { i = i + 1; n = n + 1; } printf(\"%d \", n); }\n");
    cuerpo.push_str("{ int i; int n = 0; for (i = 10; i >= 1; i = i - 1) n = n + 1; printf(\"%d \", n); }\n");
    cuerpo.push_str("{ int i = 0; int n = 0; do { n = n + 1; i = i + 1; } while (i != 7); printf(\"%d \", n); }\n");
    cuerpo.push_str("{ int i = 100; int n = 0; do { n = n + 1; } while (i < 10); printf(\"%d \", n); }\n");
    // lo que NO se funde
    cuerpo.push_str("{ int t[1]; int *p = t; int *q = 0; int r = 0; if (p) r = r + 1; if (q) r = r + 10; printf(\"%d \", r); }\n");
    cuerpo.push_str("{ int a = 1; int b = 0; int r = 0; if (a && b) r = 1; if (a || b) r = r + 2; if (!(a < b)) r = r + 4; printf(\"%d \", r); }\n");
    cuerpo.push_str("{ double d = 2.5; int r = 0; if (d < 3.0) r = 1; if (d > 3.0) r = r + 2; printf(\"%d \", r); }\n");
    // y la sonda: el 0/1 de siempre, para saber de quien es la culpa si falla
    cuerpo.push_str("{ unsigned ua = 0xFFFFFFF0u; unsigned ub = 3u; printf(\"%d%d%d%d\", ua < ub, ua > ub, ub <= ua, ub >= ua); }\n");
    let out = run_c(&format!("int main() {{\n{cuerpo} return 0; }}"));
    assert_eq!(out, "1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 10 10 7 1 1 6 1 0110");
}

/// La division entera es CON SIGNO. Antes dividia sin signo, asi que un
/// negativo daba un numero astronomico.
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
/// resultado de una comparacion arrastraba los bits altos del operando
/// derecho: parecia correcto con valores chicos y fallaba con grandes.
#[test]
fn comparison_result_is_clean_with_large_operands() {
    let out = run_c(
        "int main() { long a = 4294967296; long b = 4294967296; printf(\"%d\\n\", a == b); return 0; }",
    );
    assert_eq!(out, "1\n");
}

/// Un `int` con signo debe releerse con signo. Antes `mov eax,[..]`
/// rellenaba de ceros y `-7` volvia como 4294967289.
#[test]
fn negative_int_survives_a_round_trip_through_memory() {
    let out = run_c("int main() { int y = 0 - 7; printf(\"%d\\n\", y); return 0; }");
    assert_eq!(out, "-7\n");
}

#[test]
fn errors_report_real_line() {
    // Antes TODO error decia "linea 1".
    let src = "int main() {\n    int x;\n    x = ;\n    return 0;\n}";
    let err = parse(src).unwrap_err();
    assert_eq!(err.line, 3, "el error de 'x = ;' está en la línea 3, no la 1");
}


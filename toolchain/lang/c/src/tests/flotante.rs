//! Coma flotante: SSE, conversiones y lo que se rechaza
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn parses_float_double() {
    let src = "int main() { float f; double d; return 0; }";
    let p = parse(src).unwrap();
    assert!(p.functions.len() > 0);
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn float_in_int_context_truncates() {
    // Evolucion del test de Fase 0: 1.5 ya NO se rechaza. En contexto ENTERO
    // (int x = 1.5) se trunca via cvttsd2si -- semantica C correcta.
    let src = "int main() { int x; x = 1.5; return x; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2C, 0xC0]),
        "1.5 en contexto entero debe truncar con cvttsd2si");
}

// ---- Floats SSE (Fase 2) ----

#[test]
fn float_literal_is_number_now() {
    // 1.5 ya NO es error: se acepta y se compila por la ruta SSE.
    let src = "int main() { double d; d = 1.5; return 0; }";
    let p = parse(src).unwrap();
    // d = FloatLit(1.5)
    let ok = p.functions[0].body.iter().any(|s| matches!(s,
        Stmt::Expr(Expr::Assign(n, v)) if n == "d" && matches!(v.as_ref(), Expr::FloatLit(_))));
    assert!(ok, "1.5 debe ser FloatLit, ya no un error");
    let bef = compile_source_to_bef(src).unwrap();
    // movq xmm0, rax (66 48 0F 6E C0) del literal + movsd store (F2 0F 11)
    assert!(bef.windows(5).any(|w| w == [0x66, 0x48, 0x0F, 0x6E, 0xC0]), "falta movq xmm0,rax del literal");
    assert!(bef.windows(3).any(|w| w == [0xF2, 0x0F, 0x11]), "falta movsd store del double");
}

#[test]
fn double_arithmetic_uses_sse() {
    // d = a + b * c -> addsd/mulsd, no aritmetica entera.
    let src = r#"
int main() {
double a; double b; double c; double d;
a = 2.0; b = 3.0; c = 4.0;
d = a + b * c;
return 0;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x59, 0xC1]), "falta mulsd xmm0,xmm1");
    assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x58, 0xC1]), "falta addsd xmm0,xmm1");
}

#[test]
fn double_from_int_converts() {
    // double d = 5; -> cvtsi2sd (entero a double).
    let src = "int main() { double d; d = 5; return 0; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2A, 0xC0]), "falta cvtsi2sd de 5");
}

#[test]
fn float_to_int_truncates() {
    // int x = (int)2.7; -> cvttsd2si (double a entero, trunca).
    let src = "int main() { int x; x = (int)2.7; return x; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2C, 0xC0]), "falta cvttsd2si");
}

#[test]
fn double_comparison_uses_comisd() {
    // if (d > 0.5) -> comisd + seta, NO comparacion entera de bits.
    let src = r#"
int main() {
double d; d = 1.0;
if (d > 0.5) { return 1; }
return 0;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(4).any(|w| w == [0x66, 0x0F, 0x2F, 0xC1]), "falta comisd xmm0,xmm1");
    assert!(bef.windows(3).any(|w| w == [0x0F, 0x97, 0xC0]), "falta seta (a > b unsigned)");
}

#[test]
fn float_f32_narrows_on_store() {
    // float f = 1.5; -> cvtsd2ss (double a float) + movss store.
    let src = "int main() { float f; f = 1.5; return 0; }";
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x5A, 0xC0]), "falta cvtsd2ss");
    assert!(bef.windows(3).any(|w| w == [0xF3, 0x0F, 0x11]), "falta movss store");
}

/// * **Un `double` como PARAMETRO se rechaza con motivo.**
///
/// Antes compilaba en silencio y devolvia basura: BMO C evalua floats por la
/// ruta paralela de xmm, pero **los argumentos van por la pila como enteros**,
/// asi que `g(1.5)` empujaba los bits del double en una ranura y el prologo
/// los leia como si fueran un `long`.
///
/// Los floats GLOBALES ya se rechazaban desde el principio; esta puerta se
/// quedo abierta porque nadie habia escrito una funcion que tomara un
/// `double`. Lo destapo **C++** al probar una sobrecarga `f(int)`/`f(double)`,
/// que es lo que pasa cuando un lenguaje nuevo se apoya en el mismo backend.
#[test]
fn un_parametro_double_se_rechaza_con_motivo() {
    let e = compile_source_to_bef("int g(double a) { return 1; } int main() { return 0; }")
        .expect_err("un parametro de coma flotante no se puede pasar todavia");
    assert!(
        e.message.contains("coma flotante"),
        "el error tiene que decir que es de coma flotante: {}", e.message,
    );
    assert!(
        e.message.contains("xmm"),
        "y decir QUE falta (la ABI de xmm), no solo que no se puede: {}", e.message,
    );
}

/// La asimetria, fijada a proposito: **devolver** un double si se puede.
/// Si algun dia se rechazara, este test lo dice antes de que alguien lo
/// descubra con un programa.
#[test]
fn double_return_value_in_xmm0() {
    // double f() { return d; } -- el valor de retorno queda en xmm0.
    let src = r#"
double half(void) { double d; d = 0.5; return d; }
int main() { return 0; }
"#;
    let bef = compile_source_to_bef(src).unwrap();
    // el return de half carga d con movsd xmm0,[rbp+off] (F2 0F 10 45 ..)
    assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x10, 0x45]), "falta movsd load del return");
}


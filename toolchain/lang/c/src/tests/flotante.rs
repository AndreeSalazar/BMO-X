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

/// ** **UN `double` COMO PARAMETRO, DE PUNTA A PUNTA.**
///
/// Este test exigia lo contrario: que se RECHAZARA, y el motivo escrito era
/// *"la ABI de argumentos xmm esta pendiente"*. Resulto que **no hacia falta
/// ninguna ABI de xmm**: en BMO los argumentos van por la pila en ranuras de
/// ocho bytes, y un `double` cabe entero en una. Lo que fallaba era el sitio de
/// llamada, que evaluaba el argumento con `emit_expr` -- y esa ruta TRUNCA a
/// entero. La ranura llevaba `-2` donde iba `-2.5`.
///
/// Se comprueba por el VALOR y no por los bytes emitidos: se multiplica por
/// diez y se baja a entero, asi que si lo que llegara fuera el truncado, o la
/// mitad de la mantisa, el numero no saldria.
#[test]
fn un_double_como_parametro_llega_entero() {
    let src = r#"
double fabs(double v) { if (v < 0.0) { return -v; } return v; }
int main() {
    printf("%d %d\n", (int)(fabs(-2.5) * 10.0), (int)(fabs(2.5) * 10.0));
    return 0;
}
"#;
    let bef = compile_source_to_bef(src).expect("un double como parametro ya se compila");
    assert_eq!(ejecutar_bef(&bef), "25 25\n");
}

/// ** Un argumento ENTERO a un parametro `double` se convierte, que es lo que
/// C manda. `fabs(3)` tiene que valer 3.0 y no los bits del entero 3 leidos
/// como coma flotante -- que serian 1.5e-323, o sea cero a efectos practicos.
#[test]
fn un_entero_a_un_parametro_double_se_convierte() {
    let src = r#"
double doble(double v) { return v + v; }
int main() {
    printf("%d\n", (int)doble(3));
    return 0;
}
"#;
    let bef = compile_source_to_bef(src).expect("debe compilar");
    assert_eq!(ejecutar_bef(&bef), "6\n");
}

/// ** Un parametro `float` son CUATRO bytes, no ocho.
///
/// El callee lo lee con `movss`, asi que si el sitio de llamada empujara los
/// ocho bytes de un double, leeria la mitad baja de la mantisa como si fuera el
/// numero. La conversion la decide el tipo del PARAMETRO, no la expresion.
#[test]
fn un_parametro_float_se_estrecha_a_cuatro_bytes() {
    let src = r#"
float mitad(float v) { return v * 0.5; }
int main() {
    printf("%d\n", (int)(mitad(2.5) * 100.0));
    return 0;
}
"#;
    let bef = compile_source_to_bef(src).expect("debe compilar");
    assert_eq!(ejecutar_bef(&bef), "125\n");
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



/// *** LOS TRES INTRINSECOS SSE, **EJECUTADOS** -- no solo emitidos.
///
/// La matriz de conformidad de `semantic.rs` dice que la fila es EMITIBLE.
/// Esto dice que hace lo que promete, y es la diferencia entre `maxsd` y
/// `minsd`: los dos emiten cuatro bytes y solo uno da el numero correcto.
///
/// ** Y los numeros estan elegidos para que la ruta EQUIVOCADA se vea. Si los
/// argumentos pasaran por la pila --por `rax`, que es lo que hacia este camino
/// hasta el 2026-08-23-- se convertirian a entero ANTES de la instruccion y la
/// salida seria `200 100 100`. Un `2` en vez de un `2.5` no se nota a simple
/// vista; un `250` contra un `200`, si.
///
/// [!] Todos los valores son exactos en binario (1.25, 2.5, 2.25, 1.5) a
/// proposito: un test de coma flotante que depende del redondeo mide el
/// redondeo, no lo que dice medir.
#[test]
fn los_intrinsecos_sse_calculan_y_no_solo_compilan() {
    let out = run_c_con_pp(
        r#"
#include <semantic/semantic.h>
int main() {
    printf("%d %d %d\n",
        (int)(__maxsd(1.25, 2.5) * 100),
        (int)(__minsd(1.25, 2.5) * 100),
        (int)(__sqrtsd(2.25) * 100));
    return 0;
}
"#,
    );
    assert_eq!(out.trim(), "250 125 150");
}

/// Y el valor VUELVE por donde debe: un intrinseco que declara
/// `returns = "xmm0"` es una expresion FLOTANTE, no un entero.
///
/// Sin eso, `double m = __maxsd(a, b)` iria a buscar el resultado a `rax` y
/// guardaria basura -- el mismo fallo que ya se cazo una vez con las llamadas
/// normales, y que el comentario de `expr_is_float` describe palabra por
/// palabra: *un numero cualquiera, no un error*.
#[test]
fn un_intrinseco_que_devuelve_xmm0_se_puede_guardar_en_un_double() {
    let out = run_c_con_pp(
        r#"
#include <semantic/semantic.h>
int main() {
    double m;
    m = __maxsd(1.25, 2.5);
    printf("%d\n", (int)(m * 100));
    return 0;
}
"#,
    );
    assert_eq!(out.trim(), "250");
}

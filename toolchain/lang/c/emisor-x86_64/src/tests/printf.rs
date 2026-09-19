//! `printf`: el unico trozo de libc que baja hasta la consola
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn printf_prints_signed_integers() {
    let out = run_c(
        "int main() { int x = 42; int y = 0 - 7; printf(\"x=%d y=%d\\n\", x, y); return 0; }",
    );
    assert_eq!(out, "x=42 y=-7\n");
}

/// El caso que motivo todo: antes `printf(\"%d\", x)` descartaba `x` en
/// el parser e imprimia el literal `%d`.
#[test]
fn printf_no_longer_prints_the_format_specifier() {
    let out = run_c("int main() { printf(\"%d\\n\", 5); return 0; }");
    assert_eq!(out, "5\n");
    assert!(!out.contains('%'), "no debe salir el especificador crudo");
}

#[test]
fn printf_supports_the_common_conversions() {
    let out = run_c(
        "int main() { printf(\"[%d][%u][%x][%c][%s][%%]\\n\", 0 - 3, 3, 255, 65, \"hola\"); return 0; }",
    );
    assert_eq!(out, "[-3][3][ff][A][hola][%]\n");
}

/// Los modificadores de longitud se aceptan y no cambian nada: en BMO
/// todo entero viaja en 64 bits.
#[test]
fn printf_accepts_length_modifiers() {
    let out = run_c("int main() { printf(\"%ld\\n\", 123456789); return 0; }");
    assert_eq!(out, "123456789\n");
}

#[test]
fn printf_computes_its_arguments() {
    let out = run_c("int main() { int a = 6; int b = 7; printf(\"%d\\n\", a * b); return 0; }");
    assert_eq!(out, "42\n");
}

/// Un formato que aun no se compila debe FALLAR, no imprimir basura.
#[test]
fn printf_rejects_unsupported_conversions() {
    let err = compile_source_to_bef("int main() { printf(\"%f\\n\", 1); return 0; }").unwrap_err();
    assert!(err.message.contains("%f"), "mensaje: {}", err.message);
}

#[test]
fn printf_rejects_missing_arguments() {
    let err = compile_source_to_bef("int main() { printf(\"%d %d\\n\", 1); return 0; }").unwrap_err();
    assert!(err.message.contains("argumento"), "mensaje: {}", err.message);
}

/// El puente L2->L1: `printf("literal")` debe bajar a la puerta de
/// consola del ABI, byte por byte igual que lo que emite `bmo-lower`.
///
/// Antes de esto, C emitia `syscall 0x1F0` con un puntero -- numero que
/// el kernel no despacha y forma que la superficie congelada rechaza.
/// Compilaba, validaba, y en hardware no imprimia nada.
#[test]
fn printf_literal_lowers_to_the_console_door() {
    let bef = compile_source_to_bef("int main() { printf(\"hola\\n\"); return 0; }").unwrap();
    let mut door = Vec::new();
    bmo_lower::console::write_const(&mut door, b"hola\n");
    assert!(
        contains_bytes(&bef, &door),
        "el BEF debe contener la secuencia INVOKE/CONSOLE_WRITE de la puerta"
    );
}

/// `printf` con argumentos NO puede tomar el atajo del literal: hacerlo
/// descartaba los argumentos en silencio e imprimia "%d" tal cual.
#[test]
fn printf_with_arguments_keeps_them() {
    let program = parse("int main() { int x = 7; printf(\"%d\\n\", x); return 0; }").unwrap();
    let body = &program.functions[0].body;
    let has_literal_shortcut = body.iter().any(|s| {
        matches!(s, Stmt::Printf(_) | Stmt::PrintfLn(_))
    });
    assert!(
        !has_literal_shortcut,
        "printf variádico no debe degradarse a impresión de literal"
    );
}

/// * **Los argumentos de `printf` se evaluan ANTES de escribir un byte.**
///
/// Antes no: el emisor recorria la plantilla y evaluaba cada argumento al
/// llegar a su `%`, intercalado con la salida de los literales. Con argumentos
/// sin efectos daba igual -- por eso ninguna fila de la matriz lo cazo-- pero
/// `printf("[%d]", f())` con `f` imprimiendo sacaba `[` **antes** que lo de
/// `f`, y en C estandar todos los argumentos se evaluan antes de la llamada.
///
/// Lo destapo la matriz de **C++** al probar RAII: un destructor que imprime
/// es justo un argumento con efectos.
#[test]
fn los_argumentos_de_printf_se_evaluan_antes_de_imprimir() {
    let src = r#"
int ruido(int n) { printf("(%d)", n); return n; }
int main() { printf("[%d]", ruido(7)); return 0; }
"#;
    assert_eq!(run_c(src).trim(), "(7)[7]",
        "el argumento tiene que ejecutarse ENTERO antes de que salga el `[`");
}

/// Y con varios argumentos, el orden entre ellos tambien es el de evaluacion.
#[test]
fn printf_evalua_todos_sus_argumentos_en_orden() {
    let src = r#"
int ruido(int n) { printf("<%d>", n); return n; }
int main() { printf("a%db%dc", ruido(1), ruido(2)); return 0; }
"#;
    assert_eq!(run_c(src).trim(), "<1><2>a1b2c");
}


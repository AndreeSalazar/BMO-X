//! `enum`: constantes que no ocupan memoria
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

/// Las constantes de `enum` valen lo que dicen. Antes el parser
/// calculaba el valor y lo descartaba.
#[test]
fn enum_constants_carry_their_value() {
    let out = run_c(
        "enum Color { ROJO, VERDE, AZUL }; \
         int main() { printf(\"%d %d %d\\n\", ROJO, VERDE, AZUL); return 0; }",
    );
    assert_eq!(out, "0 1 2\n");
}

#[test]
fn enum_explicit_values_continue_from_there() {
    let out = run_c(
        "enum E { A = 10, B, C = 100, D }; \
         int main() { printf(\"%d %d %d %d\\n\", A, B, C, D); return 0; }",
    );
    assert_eq!(out, "10 11 100 101\n");
}

#[test]
fn enum_constants_work_in_expressions_and_conditions() {
    let out = run_c(
        "enum E { UNO = 1, DOS = 2 }; \
         int main() { if (DOS > UNO) { printf(\"mayor %d\\n\", DOS + UNO); } return 0; }",
    );
    assert_eq!(out, "mayor 3\n");
}


// =============== What DOOM needs from an enum ===============
//
// These four rows came from compiling DOOM's 81 translation units one at a
// time. Between them they were the first error in 30 of them, which is why
// they are grouped: the shapes are different, the missing piece was one.

/// `typedef enum { ... } name;` -- how C code declares an enum in practice.
///
/// It failed with "expected type, got Enum": the specifier was only understood
/// at file scope, not where a TYPE is parsed.
#[test]
fn typedef_of_an_anonymous_enum_is_a_type() {
    let out = run_c(
        "typedef enum { LOW, MID, HIGH } level_t; \
         int main() { level_t l; l = HIGH; printf(\"%d\n\", (int)l); return 0; }",
    );
    assert_eq!(out, "2\n");
}

/// An enum with no tag at all. Legal C, and it used to say "expected enum
/// name".
#[test]
fn an_enum_without_a_tag_still_declares_its_constants() {
    let out = run_c(
        "enum { PU_STATIC = 1, PU_LEVEL = 2 }; \
         int main() { printf(\"%d %d\n\", PU_STATIC, PU_LEVEL); return 0; }",
    );
    assert_eq!(out, "1 2\n");
}

/// The value of a constant is a CONSTANT EXPRESSION, not an integer literal.
///
/// Both of these are DOOM's, in `d_mode.h` and `doomdef.h`: `sk_noitems = -1`
/// and `INVULNTICS = (30*TICRATE)`. `TICRATE` is a macro, so what reaches the
/// parser is the literal written here.
#[test]
fn an_enum_value_can_be_a_constant_expression() {
    let out = run_c(
        "enum { NEG = -1, ZERO, TICS = (30*35), MASK = 1 << 4 }; \
         int main() { printf(\"%d %d %d %d\\n\", NEG, ZERO, TICS, MASK); return 0; }",
    );
    assert_eq!(out, "-1 0 1050 16\n");
}

/// And what is NOT a constant expression is an ERROR, not a zero.
///
/// The same rule the loader applies to a global it cannot evaluate: a value
/// the compiler cannot compute is reported, never invented. A silent zero here
/// would make every constant in the enum after it wrong too.
#[test]
fn an_enum_value_that_is_not_constant_is_rejected() {
    let err = compile_source_to_bef(
        "int f(void); enum { BAD = f() }; int main() { return BAD; }",
    )
    .expect_err("a call is not a constant expression");
    assert!(
        err.message.contains("constant expression"),
        "message: {}",
        err.message
    );
}

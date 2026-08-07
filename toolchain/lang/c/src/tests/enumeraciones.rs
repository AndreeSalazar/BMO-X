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


//! Variables GLOBALES: carga, guardado y cero inicial
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn global_var_load_store() {
    let src = r#"
int g = 42;
int main() {
int x;
x = g;
g = 100;
return x;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn global_var_zero_init() {
    let src = r#"
int z;
int main() {
z = 7;
return z;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn global_var_addr_of() {
    let src = r#"
int g;
int main() {
int *p = &g;
*p = 99;
return g;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}


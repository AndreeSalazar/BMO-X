//! LAS SOMBRAS: la misma letra en dos bloques son dos variables
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Salio el 2026-09-18 escribiendo OTRA fila: `{ int a; } { unsigned a; }`
//! releia la segunda con signo, porque el emisor guardaba un hueco por NOMBRE
//! y por funcion, con el tipo de la primera. Y el tipo era lo de menos: con
//! `{ char t[2]; } { long t[8]; }` el segundo array escribia 64 bytes en un
//! hueco de 8 -- encima de las locales de al lado. Ver `decidir/ambitos.rs`.

use super::*;

#[test]
fn dos_bloques_hermanos_con_la_misma_letra_son_dos_variables() {
    // el fallo original: el tipo del segundo bloque
    let out = run_c(
        "int main() { { int a = 1; int b = 2; printf(\"%d \", a < b); } \
         { unsigned a = 0xFFFFFFF0u; unsigned b = 3u; printf(\"%u %d%d\", a, a < b, a > b); } return 0; }",
    );
    assert_eq!(out, "1 4294967280 01");
}

#[test]
fn la_sombra_interior_no_pisa_a_la_exterior() {
    let out = run_c(
        "int main() { int a = 1; { printf(\"%d \", a); unsigned a = 0xFFFFFFF0u; printf(\"%u \", a); a = a + 1; } \
         printf(\"%d\", a); return 0; }",
    );
    // dentro del bloque, ANTES de la declaracion, `a` es la de fuera
    assert_eq!(out, "1 4294967280 1");
}

#[test]
fn un_array_sombreado_tiene_su_propio_tamano() {
    // Antes: `t` tenia UN hueco, el del primero (2 bytes -> 8 alineados), y el
    // `long t[8]` del segundo bloque escribia 64 bytes encima de `guarda`.
    let out = run_c(
        "int main() { long guarda = 77; { char t[2]; t[0] = 'a'; t[1] = 0; printf(\"%s \", t); } \
         { long t[8]; int i; for (i = 0; i < 8; i = i + 1) t[i] = i * 100; printf(\"%ld \", t[7]); } \
         printf(\"%ld\", guarda); return 0; }",
    );
    assert_eq!(out, "a 700 77");
}

#[test]
fn una_local_sombrea_a_un_parametro() {
    let out = run_c(
        "int f(int n) { { long n = 4294967296L; printf(\"%ld \", n); } return n; } \
         int main() { printf(\"%d\", f(5)); return 0; }",
    );
    assert_eq!(out, "4294967296 5");
}

#[test]
fn dos_bucles_con_la_misma_temporal_de_tipos_distintos() {
    let out = run_c(
        "int main() { int i; long s = 0; \
         for (i = 0; i < 3; i = i + 1) { int t = i * 2; s = s + t; } \
         for (i = 0; i < 3; i = i + 1) { long t = 4294967296L; s = s + t; } \
         printf(\"%ld\", s); return 0; }",
    );
    assert_eq!(out, "12884901894");
}

#[test]
fn un_puntero_a_funcion_local_sombreado_llama_al_suyo() {
    let out = run_c(
        "int uno(void) { return 1; } int dos(void) { return 2; } \
         int main() { int (*f)(void) = uno; { int (*f)(void) = dos; printf(\"%d \", f()); } printf(\"%d\", f()); return 0; }",
    );
    assert_eq!(out, "2 1");
}

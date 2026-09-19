//! SIN PILA: el operando derecho y la direccion de `t[i]` sin push/pop
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Filas del 2026-09-18 (noche) para `operando.rs`, `sin_pila` y la base de
//! `t[i]` en rdx. Lo que se comprueba es la ANCHURA y el SIGNO de cada carga a
//! rcx/rdx: tiene que dar lo mismo que cargar en rax, para cada tipo, en
//! local, en global, en el troquel y como constante de enum.

use super::*;

/// `x op y` con `y` de cada tipo, local y global, y con signo o sin el.
#[test]
fn el_operando_derecho_se_carga_con_su_anchura_y_su_signo() {
    let out = run_c(
        "char gc = -3; unsigned char guc = 250; short gs = -300; unsigned short gus = 65000; \
         int gi = -7; unsigned gu = 4000000000u; long gl = 4294967296L; \
         enum { CINCO = 5 }; \
         int main() { \
           char c = -3; unsigned char uc = 250; short s = -300; unsigned short us = 65000; \
           int i = -7; unsigned u = 4000000000u; long l = 4294967296L; long x = 1000; \
           printf(\"%ld %ld %ld %ld %ld %lu %ld ; \", x + c, x + uc, x + s, x + us, x + i, x + u, x + l); \
           printf(\"%ld %ld %ld %ld %ld %lu %ld ; \", x + gc, x + guc, x + gs, x + gus, x + gi, x + gu, x + gl); \
           printf(\"%ld %ld %ld %ld %ld %d %d\", x - c, x * i, x & us, x | uc, x ^ s, x + CINCO, x < l); \
           return 0; }",
    );
    assert_eq!(
        out,
        "997 1250 700 66000 993 4000001000 4294968296 ; \
         997 1250 700 66000 993 4000001000 4294968296 ; \
         1003 -7000 488 1018 -708 1005 1"
    );
}

/// Las variables del troquel operan directamente (`add rax, r12`), y la
/// resta no es conmutativa: `x - y` con `y` en la matriz.
#[test]
fn con_la_matriz_se_opera_directo_y_en_el_orden_correcto() {
    let out = run_c(
        "int main() { int i; long a = 0; long b = 0; long c = 0; long d = 1000; \
         for (i = 0; i < 10; i = i + 1) { a = a + i; b = b - i; c = c * 2 + i; d = d - a; } \
         printf(\"%ld %ld %ld %ld %d\", a, b, c, d, a < d); return 0; }",
    );
    assert_eq!(out, "45 -45 1013 835 1");
}

/// `t[i] = v` para cada anchura, con la base en local, en global, en un
/// puntero local, en un puntero global y en un puntero del troquel; y `v`
/// constante, variable, expresion sin pila, y una LLAMADA (que va por la
/// pila de siempre).
#[test]
fn escribir_en_una_tabla_por_cualquier_base_y_cualquier_valor() {
    let out = run_c(
        "char gt[4]; long gl[4]; int *gp; int dos(void) { return 2; } \
         int main() { \
           char t1[4]; short t2[4]; int t4[4]; long t8[4]; int *p; int k; int v = 300; \
           p = t4; gp = t4; \
           for (k = 0; k < 4; k = k + 1) { \
             t1[k] = (unsigned char)(k & 0xFF); t2[k] = v; t4[k] = k + v; t8[k] = 4294967296L + k; \
             gt[k] = 'a' + k; gl[k] = k * 3; p[k] = p[k] + dos(); gp[k] = gp[k] * 2; \
           } \
           printf(\"%d %d %d %d %ld %c %ld %d\", t1[3], t2[1], t4[2], p[2], t8[3], gt[2], gl[3], gp[1]); \
           return 0; }",
    );
    // t4[k] = k + 300; p[k] += 2; gp[k] *= 2  ->  t4[2] = (302 + 2) * 2 = 608; gp[1] = (301+2)*2 = 606
    assert_eq!(out, "3 300 608 608 4294967299 c 9 606");
}

/// El puntero del troquel como base (`p[k]` con `p` en r12) y una asignacion
/// cuyo valor es la propia asignacion (`t[i] = t[j] = v`).
#[test]
fn base_en_la_matriz_y_asignacion_encadenada() {
    let out = run_c(
        "int main() { int t[8]; int *p = t; int k; int s = 0; \
         for (k = 0; k < 8; k = k + 1) { p[k] = k * k; s = s + p[k]; } \
         t[0] = t[1] = 99; \
         printf(\"%d %d %d %d\", s, t[0], t[1], p[7]); return 0; }",
    );
    assert_eq!(out, "140 99 99 49");
}

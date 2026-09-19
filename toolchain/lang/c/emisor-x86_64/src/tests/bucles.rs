//! LOS BUCLES ROTADOS: la condicion abajo, y `break`/`continue` a su sitio
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Filas del 2026-09-19. La primera existe por un fallo que salio al rotar:
//! `break` dentro de un `do ... while` NO rompia -- su etiqueta estaba ANTES
//! de la condicion, la volvia a evaluar y, si se cumplia, seguia. `i == 2`
//! con `while (i < 10)` daba 10 vueltas.

use super::*;

#[test]
fn break_en_un_do_while_rompe_de_verdad() {
    let out = run_c(
        "int main() { int i = 0; int n = 0; \
         do { i = i + 1; n = n + 1; if (i == 2) break; } while (i < 10); \
         printf(\"%d %d\", i, n); return 0; }",
    );
    assert_eq!(out, "2 2");
}

#[test]
fn continue_y_break_en_los_tres_bucles() {
    let out = run_c(
        "int main() { int i; int s = 0; int k; \
         for (i = 0; i < 10; i = i + 1) { if (i % 2) continue; if (i > 6) break; s = s + i; } \
         printf(\"%d \", s); \
         i = 0; s = 0; while (i < 10) { i = i + 1; if (i % 3 == 0) continue; if (i == 8) break; s = s + i; } \
         printf(\"%d \", s); \
         i = 0; s = 0; do { i = i + 1; if (i == 4) continue; if (i == 7) break; s = s + i; } while (i < 10); \
         printf(\"%d \", s); \
         k = 0; for (;;) { k = k + 1; if (k == 5) break; } printf(\"%d \", k); \
         i = 5; while (i > 100) { i = 999; } printf(\"%d \", i); \
         for (i = 3; i < 0; i = i + 1) { s = -1; } printf(\"%d\", i); \
         return 0; }",
    );
    // for: 0+2+4+6 = 12; while: 1+2+4+5+7 = 19; do: 1+2+3+5+6 = 17; for(;;): 5; while falso: 5; for falso: 3
    assert_eq!(out, "12 19 17 5 5 3");
}

#[test]
fn bucles_anidados_con_continue_del_interior_y_break_del_exterior() {
    let out = run_c(
        "int main() { int i; int j; int n = 0; \
         for (i = 0; i < 5; i = i + 1) { for (j = 0; j < 5; j = j + 1) { if (j == 2) continue; if (i == 3) break; n = n + 1; } if (i == 3) break; } \
         printf(\"%d %d %d\", n, i, j); return 0; }",
    );
    // i=0,1,2: 4 vueltas utiles cada uno = 12; i=3: j=0 -> break (n no sube), y el exterior rompe
    assert_eq!(out, "12 3 0");
}

/// `x = constante` sin pasar por rax (19-09): la constante se recorta al
/// compilar como la recortaria la pila, en el marco y en la matriz, y una
/// local sin inicializar sigue valiendo cero.
#[test]
fn la_constante_se_guarda_directa_y_recortada() {
    let out = run_c(
        "int main() { char c; unsigned char uc; int i; unsigned u; long l; long g; int *p; int sin_valor; int k; int v; \
         c = 200; uc = 300; i = 2147483648; u = 4294967295u; l = 4294967296L; g = -5; p = 0; \
         printf(\"%d %d %d %u %ld %ld %d %d \", c, uc, i, u, l, g, p == 0, sin_valor); \
         for (k = 0; k < 3; k = k + 1) { c = -1; uc = 255; i = -2147483647 - 1; u = 3000000000u; l = -4294967296L; v = (i = 7) + 1; } \
         printf(\"%d %d %d %u %ld %d\", c, uc, i, u, l, v); return 0; }",
    );
    assert_eq!(out, "-56 44 -2147483648 4294967295 4294967296 -5 1 0 -1 255 7 3000000000 -4294967296 8");
}

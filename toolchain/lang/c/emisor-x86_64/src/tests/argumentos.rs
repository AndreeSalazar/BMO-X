//! LOS ARGUMENTOS DIRECTOS: intrinsecos sin pila, `t[i] = v` desde la
//! matriz, y `p->arr[i]` sin push/pop
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Filas del 2026-09-19 (segunda tanda).

use super::*;

/// La puerta con argumentos simples (constantes, locales, de la matriz,
/// globales) cruza DIRECTA a los registros; con uno complejo (una llamada)
/// va por la pila. Lo que se mira es lo que el emulador VIO cruzar: las dos
/// rutas tienen que dejar los mismos numeros en rax, rdi, rsi y rdx.
///
/// `CURRENT_TASK` / `YIELD` (0x03) porque es la operacion que no hace nada.
#[test]
fn los_argumentos_de_la_puerta_llegan_iguales_por_las_dos_rutas() {
    let simple = run_c_maquina(
        "unsigned long g = 3; unsigned long dos(void) { return 2; }          int main() { unsigned long cap = 0xFFFFFFFFFFFFFFFEul; unsigned long a0 = 5; int i;            for (i = 0; i < 3; i = i + 1) { __syscall(0, cap, g, a0 + i, 0, 0); }            __syscall(0, cap, g, dos() + 3, 0, 0); return 0; }",
    );
    let vistas: Vec<(u64, u64, u64, u64)> = simple.syscalls.iter().map(|s| (s.nr, s.capability, s.operation, s.arg0)).collect();
    let cap = 0xFFFF_FFFF_FFFF_FFFEu64;
    // las tres del bucle (directas: `a0 + i` es sin pila pero NO es simple,
    // asi que estas tres van por la pila), la de la llamada (pila), y el EXIT
    assert_eq!(&vistas[..4], &[(0, cap, 3, 5), (0, cap, 3, 6), (0, cap, 3, 7), (0, cap, 3, 5)]);
    let directa = run_c_maquina(
        "unsigned long g = 3;          int main() { unsigned long cap = 0xFFFFFFFFFFFFFFFEul; unsigned long a0 = 5; int i; enum { OCHO = 8 };            for (i = 0; i < 3; i = i + 1) { __syscall(0, cap, g, i, a0, OCHO); } return 0; }",
    );
    let vistas: Vec<(u64, u64, u64, u64)> = directa.syscalls.iter().map(|s| (s.nr, s.capability, s.operation, s.arg0)).collect();
    assert_eq!(&vistas[..3], &[(0, cap, 3, 0), (0, cap, 3, 1), (0, cap, 3, 2)]);
}

/// `t[i] = v` con `v` en la matriz, para cada anchura, y `p[i] = v` por un
/// puntero; y en contexto de expresion sigue dejando el valor.
#[test]
fn escribir_en_una_tabla_desde_la_matriz() {
    let out = run_c(
        "int main() { char t1[8]; int t4[8]; long t8[8]; int *p = t4; int i; long v = 0; int w; int r; \
         for (i = 0; i < 8; i = i + 1) { v = i * 3 - 4; t1[i] = v; t4[i] = v; t8[i] = v; p[i] = v; w = v; } \
         r = (t4[0] = w); \
         printf(\"%d %d %ld %d %d %d\", t1[7], t4[7], t8[7], t1[0], r, t4[0]); return 0; }",
    );
    assert_eq!(out, "17 17 17 -4 17 17");
}

/// `p->arr[i]` y `(p + 1)[i]` (IndexPtr) con el indice en la matriz, en el
/// marco y como expresion, para escalas 1, 4 y 8.
#[test]
fn indexar_por_puntero_sin_pila() {
    let out = run_c(
        "struct s { char c[4]; int n[4]; long l[4]; }; \
         int main() { struct s v; struct s *p = &v; int i; int k = 1; long acc = 0; long base[6] = {10, 20, 30, 40, 50, 60}; long *q = base; \
         for (i = 0; i < 4; i = i + 1) { p->c[i] = i + 1; p->n[i] = i * 100; p->l[i] = i * 1000000000L; } \
         for (i = 0; i < 4; i = i + 1) { acc = acc + p->c[i] + p->n[i] + p->l[i]; } \
         printf(\"%ld %d %d %ld %ld %ld\", acc, p->n[k], p->c[k + 1], p->l[k * 2], (q + 1)[i], (q + 2)[k]); return 0; }",
    );
    // acc = (1+2+3+4) + 100*6 + 1e9*6 = 10 + 600 + 6000000000 = 6000000610; (q+1)[4] = base[5] = 60; (q+2)[1] = base[3] = 40
    assert_eq!(out, "6000000610 100 3 2000000000 60 40");
}

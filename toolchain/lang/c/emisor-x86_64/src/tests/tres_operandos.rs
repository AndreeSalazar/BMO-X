//! TRES OPERANDOS: `lea`, `imul rax, rN, imm`, `cmp rN, ...`, el divisor en
//! rcx y la escala dentro del `lea`
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Filas del 2026-09-18 (noche, segunda pasada) para el izquierdo en la
//! matriz (`operando.rs`), la division con el derecho en rcx, `t[i]` con la
//! escala en el `lea`, la direccion aparcada en la pila cuando el valor pisa
//! rdx, y el cast que sobra (`cast_redundante`).

use super::*;

/// `i + 5`, `i - 5`, `i + j`, `i + y`, `i * 7` con `i` (y `j`) en la matriz:
/// un `lea` o un `imul` de tres operandos. Con el recorte a 32 detras, el
/// desborde de `int` sigue dando la vuelta, y `i - INT_MIN` (que no se puede
/// negar) va por el camino largo y da lo mismo.
#[test]
fn el_izquierdo_en_la_matriz_opera_con_tres_operandos() {
    let out = run_c(
        "int main() { int i; int j = 3; long y = 4294967296L; long s = 0; int k = 2147483647; int t[8] = {10, 20, 30, 40, 50, 60, 70, 80}; int *p = t; \
         for (i = 0; i < 5; i = i + 1) { \
           s = s + (i + 5) + (i - 5) + (i + j) + (i + y) + (i * 7) + (i * -3) + (i * 1000) + (j + i); \
           j = j + 1; p = p + 1; \
         } \
         printf(\"%ld %d %d %d %d\", s, k + 1, i - (-2147483647 - 1), *(p - 1), j); return 0; }",
    );
    // por vuelta (i, j=3+i): (i+5)+(i-5)+(i+j)+(i+2^32)+7i-3i+1000i+(j+i) = 1009i + 2j + 2^32
    // i=0..4, j=3..7 -> 1009*10 + 2*25 + 5*2^32 = 21474846620; p acaba en t+5
    assert_eq!(out, "21474846620 -2147483648 -2147483643 50 8");
}

/// `cmp rN, imm`, `cmp rN, rM`, `cmp rN, rcx`: con signo y sin el, en la
/// frontera, y con negativos.
#[test]
fn comparar_desde_la_matriz_no_pasa_por_rax() {
    let out = run_c(
        "int main() { int i; int j; long n = 10; unsigned u; int r = 0; \
         for (i = -3; i < 3; i = i + 1) { for (j = -3; j <= 3; j = j + 1) { \
           if (i < j) r = r + 1; if (i >= j) r = r + 100; if (i < 0) r = r + 10000; if (j < n) r = r + 1000000; \
         } } \
         u = 0xFFFFFFF0u; for (i = 0; i < 4; i = i + 1) { if (u > 3u) r = r + 100000000; } \
         printf(\"%d\", r); return 0; }",
    );
    // 42 parejas: i<j 21 veces? i in -3..2, j in -3..3: count i<j = 6*7 - count(i>=j) ; i>=j: i=-3:1, -2:2, -1:3, 0:4, 1:5, 2:6 = 21 -> i<j = 21
    // i<0: 3 valores de i * 7 = 21 -> 210000 ; j<n siempre: 42 -> 42000000 ; u>3u 4 veces -> 400000000
    assert_eq!(out, "442212121");
}

/// La division y el resto con el divisor en rcx: constante, variable local,
/// de la matriz, global, y una llamada; con signo y sin el; y el
/// desplazamiento por variable.
#[test]
fn dividir_y_desplazar_con_el_derecho_en_rcx() {
    let out = run_c(
        "long gd = 6; int tres(void) { return 3; } \
         int main() { int i; long a = -100; long d = 7; unsigned long u = 0xFFFFFFFFFFFFFFF0ul; long s = 0; int sh = 3; \
         for (i = 1; i < 4; i = i + 1) { s = s + a / i + a % i + (a / 7) + (a % -7) + a / d + a % gd + a / tres() + (u / 16) % 1000; } \
         printf(\"%ld %ld %ld %ld %ld %lu\", s, a / d, a % d, (long)(-16) >> sh, 5L << sh, u >> sh); return 0; }",
    );
    // i=1: -100 + 0 + -14 + -2 + -14 + -4 + -33 + (0xFFFFFFFFFFFFFFF0/16 = 1152921504606846975 % 1000 = 975) = 808
    // i=2: -50 + 0 + -14 + -2 + -14 + -4 + -33 + 975 = 858
    // i=3: -33 + -1 + -14 + -2 + -14 + -4 + -33 + 975 = 874   -> 2540
    assert_eq!(out, "2540 -14 -2 -2 40 2305843009213693950");
}

/// `t[i]` con cada anchura y la escala dentro del `lea`; un struct de 12
/// bytes (escala que no cabe) por el camino de siempre; el indice en la
/// matriz, en el marco y como expresion.
#[test]
fn la_escala_va_dentro_del_lea() {
    let out = run_c(
        "struct tres { int a; int b; int c; }; \
         int main() { char t1[6]; short t2[6]; int t4[6]; long t8[6]; struct tres t12[6]; int i; int k = 2; long s = 0; \
         for (i = 0; i < 6; i = i + 1) { t1[i] = i; t2[i] = i * 100; t4[i] = i * 10000; t8[i] = i * 1000000000L; t12[i].b = i * 3; } \
         for (i = 0; i < 6; i = i + 1) { s = s + t1[i] + t2[i] + t4[i] + t8[i] + t12[i].b; } \
         printf(\"%ld %d %d %ld %d\", s, t4[k], t2[k + 1], t8[k * 2], t12[k].b); return 0; }",
    );
    // sum i=0..5 of (i + 100i + 10000i + 1e9 i + 3i) = 15 * 1000010104 = 15000151560
    assert_eq!(out, "15000151560 20000 300 4000000000 6");
}

/// `t[i] = v` cuando `v` pisa rdx (una division) o todo (una llamada): la
/// DIRECCION se aparca en la pila y el valor queda en rax como resultado.
#[test]
fn escribir_con_un_valor_que_pisa_rdx_aparca_la_direccion() {
    let out = run_c(
        "int f(int x) { return x * 2; } \
         int main() { int t[4] = {0, 0, 0, 0}; int i; int r; long d = 3; \
         for (i = 0; i < 4; i = i + 1) { t[i] = (i * 7) % 5; } \
         r = (t[1] = f(21)); t[2] = t[3] = 100 / d; t[0] = t[1] % 4; \
         printf(\"%d %d %d %d %d\", r, t[0], t[1], t[2], t[3]); return 0; }",
    );
    assert_eq!(out, "42 2 42 33 33");
}

/// El cast que sobra y el que no: `(unsigned char)(x & 0xFF)` no cambia
/// nada; `(char)(x & 0xFF)` SI (200 es -56 con signo), `(unsigned char)(x &
/// 0x1FF)` SI, y una mascara negativa no acota.
#[test]
fn el_cast_sobra_solo_cuando_la_mascara_ya_acota() {
    let out = run_c(
        "int main() { long x = -56; long y = 0x1FFFF; \
         printf(\"%d %d %d %d %u %d %d\", (unsigned char)(x & 0xFF), (char)(x & 0xFF), (unsigned char)(y & 0x1FF), \
                (unsigned short)(y & 0xFFFF), (unsigned)(x & 0xFFFFFFFF), (unsigned char)(x & -1), (unsigned char)(0xFF & y)); \
         return 0; }",
    );
    assert_eq!(out, "200 -56 255 65535 4294967240 200 255");
}

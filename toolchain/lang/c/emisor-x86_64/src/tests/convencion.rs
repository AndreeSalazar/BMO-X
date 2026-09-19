//! LA CONVENCION DE LLAMADA HIBRIDA (19-09): registros, pila, variadicas,
//! reenvio y residencia
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_maquina`, `ejecutar_bef`) viven en `tests/mod.rs`.
//!
//! Lo que estas filas afirman esta en `decidir/llamada.rs`: escalares en
//! rdi, rsi, rdx, rcx, r8, r9; el septimo, los structs y los flotantes por
//! la pila; las variadicas TODO por la pila. Y las tres piezas que la hacen
//! ganar: los argumentos que se cargan directos, el reenvio
//! (`decidir/reenvio.rs`) y la residencia en rdi/rsi (`registros.rs`).

use super::*;

/// Nueve parametros: seis en registro, tres en la pila, en el orden justo;
/// y una funcion con un struct en medio y un `double` detras, que no
/// gastan registro.
#[test]
fn seis_en_registro_y_el_resto_por_la_pila_llegan_en_orden() {
    let out = run_c(
        "struct par { int a; int b; }; \
         int nueve(int a, int b, int c, int d, int e, int g, int h, int i, int j) { \
           return a + b*10 + c*100 + d*1000 + e*10000 + g*100000 + h*1000000 + i*10000000 + j*100000000; } \
         int mezcla(int a, struct par p, int b, double d, int c) { return a + p.a * 10 + p.b * 100 + b * 1000 + (int)d * 10000 + c * 100000; } \
         int main() { struct par p; p.a = 2; p.b = 3; \
           printf(\"%d %d\", nueve(1,2,3,4,5,6,7,8,9), mezcla(1, p, 4, 5.5, 6)); return 0; }",
    );
    assert_eq!(out, "987654321 654321");
}

/// Los argumentos por las tres rutas: constantes y variables (directos), una
/// expresion sin pila (`mov`), y llamadas anidadas (push/pop); y el orden de
/// los registros no se pierde cuando el de rcx es una expresion.
#[test]
fn los_argumentos_complejos_llegan_a_su_registro() {
    let out = run_c(
        "int f(int a, int b, int c, int d, int e, int g) { return a + b*10 + c*100 + d*1000 + e*10000 + g*100000; } \
         int dos(void) { return 2; } \
         int main() { int x = 1; int y = 3; \
           printf(\"%d %d %d\", f(dos(), x + 1, dos() + 1, y + 1, 5, x * 6), \
                  f(f(1,2,3,4,5,6) % 7, dos(), 3, x + y, dos() * 2 + 1, 6), f(x, y, x + y, y - x, x * y, y * y)); \
           return 0; }",
    );
    // f(2,2,3,4,5,6) = 654322 ; f(654321 % 7 = 3, 2, 3, 4, 5, 6) = 654323 ; f(1,3,4,2,3,9) = 932431
    assert_eq!(out, "654322 654323 932431");
}

/// Una variadica recibe TODO por la pila y su `va_arg` sigue leyendo bien,
/// llamada por su nombre desde la misma unidad; y con mas de seis argumentos.
#[test]
fn una_variadica_recibe_todo_por_la_pila() {
    let out = run_c_con_pp(
        "#include <stdarg.h>\n\
         int suma(int n, ...) { va_list ap; int i; int s = 0; va_start(ap, n); \
           for (i = 0; i < n; i = i + 1) { s = s + va_arg(ap, int); } va_end(ap); return s; } \
         int main() { int x = 5; printf(\"%d %d %d\", suma(2, 10, 20), suma(7, 1, 2, 3, 4, 5, 6, x), suma(0)); return 0; }",
    );
    assert_eq!(out, "30 26 0");
}

/// Tomar la direccion de una variadica NO compila: a traves de un puntero
/// el llamante no sabria que todo va por la pila.
#[test]
fn la_direccion_de_una_variadica_no_compila() {
    let e = compile_source_to_bef(
        "int v(int n, ...) { return n; } int main() { int (*p)(int, ...) = v; return p(1); }",
    );
    let msg = match e {
        Ok(_) => panic!("tenia que negarse"),
        Err(e) => e.message,
    };
    assert!(msg.contains("variadica") && msg.contains("direccion"), "{msg}");
}

/// Llamadas por puntero: una variable, un elemento de tabla (destino
/// complejo, aparcado en la pila) y un parametro puntero; con argumentos
/// simples y con llamadas dentro.
#[test]
fn las_llamadas_por_puntero_pasan_los_argumentos_en_registro() {
    let out = run_c(
        "int suma(int a, int b) { return a + b; } int resta(int a, int b) { return a - b; } \
         int aplica(int (*f)(int, int), int x, int y) { return f(x, y); } \
         int main() { int (*t[2])(int, int); int (*g)(int, int) = resta; int i = 1; \
           t[0] = suma; t[1] = resta; \
           printf(\"%d %d %d %d\", g(10, 3), t[i](10, 3), t[0](suma(1, 2), resta(9, 4)), aplica(suma, 20, 22)); return 0; }",
    );
    assert_eq!(out, "7 7 8 42");
}

/// El REENVIO: una funcion que solo reexpide sus parametros no tiene marco.
/// Con reordenacion (un ciclo rdi <-> rsi), con una constante en medio, hacia
/// un intrinseco (la puerta) y hacia otra funcion (`jmp`); y lo que NO es
/// reenvio (una sentencia mas) sigue por el camino normal.
#[test]
fn el_reenvio_mueve_los_registros_y_salta() {
    let out = run_c(
        "int base(int a, int b, int c) { return a * 100 + b * 10 + c; } \
         int cruzada(int a, int b) { return base(b, a, 7); } \
         int con_constante(int x) { return base(1, x, 2); } \
         int en_cadena(int a, int b) { return cruzada(a, b); } \
         int no_reenvio(int a, int b) { int s = a; return base(s, b, 0); } \
         int main() { printf(\"%d %d %d %d\", cruzada(1, 2), con_constante(5), en_cadena(3, 4), no_reenvio(8, 9)); return 0; }",
    );
    assert_eq!(out, "217 152 437 890");
    // y la puerta: lo que el emulador VIO cruzar tras un reenvio a __syscall
    let m = run_c_maquina(
        "unsigned long puerta(unsigned long cap, unsigned long op, unsigned long a0) { return __syscall(0, cap, op, a0, 0, 0); } \
         int main() { puerta(0xFFFFFFFFFFFFFFFEul, 3, 77); return 0; }",
    );
    let v: Vec<(u64, u64, u64, u64)> = m.syscalls.iter().map(|s| (s.nr, s.capability, s.operation, s.arg0)).collect();
    assert_eq!(v[0], (0, 0xFFFF_FFFF_FFFF_FFFE, 3, 77));
}

/// La RESIDENCIA: en una funcion hoja los dos primeros parametros viven en
/// rdi y rsi. Se modifican, se comparan, se guardan en globales (que ya no
/// usan rdi), llegan recortados a su tipo, y con `&a` no hay residencia.
#[test]
fn los_parametros_residentes_en_rdi_y_rsi() {
    let out = run_c(
        "int g1 = 0; int g2 = 0; \
         int hoja(int a, int b) { a = a + 1; b = b * 2; g1 = a; g2 = b; if (a < b) return a + b; return a - b; } \
         int estrecho(int x, unsigned char c) { if (x < 0) return c - x; return x + c; } \
         int con_direccion(int a, int b) { int *p = &a; *p = *p + b; return a; } \
         long largo(long a, long b) { return a * b; } \
         int main() { unsigned int u1 = 100; unsigned int u2 = 200; \
           printf(\"%d %d %d %d %d %ld\", hoja(3, 4), g1, g2, estrecho(u1 - u2, 200), con_direccion(5, 6), largo(4294967296L, 3)); \
           return 0; }",
    );
    // hoja(3,4): a=4, b=8 -> 12; g1=4, g2=8; estrecho(-100, 200) = 300 (sin recorte, x seria 4294967196 y saldria 100); con_direccion = 11; largo = 12884901888
    assert_eq!(out, "12 4 8 300 11 12884901888");
}

/// Guardar en un campo, por puntero y por valor, con valor sin pila y con
/// una llamada; `*p = f()`; y la asignacion encadenada por campo.
#[test]
fn guardar_en_campos_y_por_puntero_sin_pila() {
    let out = run_c(
        "struct s { char c; int n; long l; }; int dos(void) { return 2; } \
         int main() { struct s v; struct s *p = &v; int x = 5; int r; long *q = &v.l; \
           p->c = x + 1; p->n = dos() * 10; p->l = x; v.c = v.c + 1; v.n = dos(); *q = dos() + 40; \
           r = (p->n = 9); \
           printf(\"%d %d %ld %d\", v.c, v.n, v.l, r); return 0; }",
    );
    assert_eq!(out, "7 9 42 9");
}

/// La residencia de los SEIS (19-09, tarde): en una hoja rdi, rsi, r8 y r9
/// se quedan y rdx/rcx se TRASLADAN a r10/r11 -- y por eso una division (que
/// pisa rdx) y un desplazamiento (que pisa rcx) sobre el tercero y el cuarto
/// dan lo correcto. Con tipos estrechos, que el traslado lleva el recorte
/// dentro; y modificando los seis.
#[test]
fn los_seis_parametros_residen_y_los_de_rdx_rcx_se_trasladan() {
    let out = run_c(
        "int seis(int a, unsigned int b, char c, unsigned char d, long e, short f) {            a = a / c; b = b >> d; c = c + 1; d = d * 2; e = e % 7; f = f - a;            if (a < 0 && c < 0) return a + b + c + d + e + f; return 0; }          int division(int x, int y, int p3, int p4, int p5, int p6) { return p3 / p4 + (p5 << p6) + (p6 % p3) + x - y; }          int main() { unsigned int u = 100;            printf(\"%d %d %d\", seis(-90, 1024, u - 103, 3, 30, 7), division(1, 2, 45, 9, 3, 4), division(1, 2, u - 103, 3, 3, 4)); return 0; }",
    );
    // seis: c = u - 103 = -3 (char, recortado al entrar); a = -90 / -3 = 30, no es negativo -> 0
    // division: 45/9=5, 3<<4=48, 4%45=4, +1-2 = 56; y con p3 = u - 103 = -3 SIN convertir
    // por el llamante (unsigned -> int, la misma anchura): el traslado a r10 tiene que
    // extender el signo, o -3/3 seria 4294967293/3. -1 + 48 + (4 % -3 = 1) + 1 - 2 = 47
    assert_eq!(out, "0 56 47");
    let out = run_c(
        "int seis(int a, unsigned int b, char c, unsigned char d, long e, short f) {            a = a / c; b = b >> d; c = c + 1; d = d * 2; e = e % 7; f = f - a;            return a + (int)b + c + d + (int)e + f; }          int main() { unsigned int u = 100; printf(\"%d\", seis(90, 1024, u - 103, 3, 30, 7)); return 0; }",
    );
    // a = 90 / -3 = -30; b = 1024 >> 3 = 128; c = -2; d = 6; e = 2; f = 7 - (-30) = 37 -> 141
    assert_eq!(out, "141");
}

//! **`abs` DE UN `unsigned`: LAS BANDAS DE DOOM, encontradas.** 2026-09-11.
//!
//! # El camino hasta aqui, porque es lo que vale
//!
//! ```text
//!    09-09  R_GetColumn y dc_iscale             exonerados en el banco
//!    10-09  la lista de recorte                 exonerada
//!    10-09  el metal: columnas 320/320, iscale0 0   -> las columnas, limpias
//!    11-09  tablas de luz, structs grandes, R_DrawSpan, visplanes  exonerados
//!    11-09  el metal: spans 36, filas 3 de 200  -> EL FONDO NO SE PINTA
//! ```
//!
//! Con el dibujado limpio y el recorrido de planos limpio, quedaba lo que ENTRA:
//! `rw_scale`. Y `R_ScaleFromGlobalAngle` tiene un tope --si `rw_distance` sale
//! cero, `scale = 64*FRACUNIT`-- que explica las DOS mitades de la foto a la
//! vez: la pared se proyecta a toda la altura (no queda sitio para suelo ni
//! techo: 36 spans) y `dc_iscale` es tan pequeno que la columna muestrea un
//! texel (la banda plana).
//!
//! # Y `rw_distance` salia cero por UNA instruccion que faltaba
//!
//! `r_segs.c:421`:
//!
//! ```c
//!    offsetangle = abs(rw_normalangle - rw_angle1);   // dos angle_t: UNSIGNED
//!    if (offsetangle > ANG90) offsetangle = ANG90;
//!    distangle = ANG90 - offsetangle;                  // 0
//!    sineval = finesine[distangle >> 19];              // finesine[0] = 0
//!    rw_distance = FixedMul(hyp, sineval);             // 0
//! ```
//!
//! ** DOOM cuenta con que la resta de dos `unsigned` envuelva, pase a `int`
//! --negativa si es grande-- y `abs` la enderece. Es C de manual: el argumento
//! se convierte al tipo del parametro, y `abs` es `int abs(int)`.
//!
//! BMO C le daba a `abs` el valor de `rax` tal cual: `0x00000000_FFFFFF9C`,
//! POSITIVO. `abs` lo devolvia sin tocar, `offsetangle` se clavaba en ANG90, y
//! toda pared con `rw_normalangle < rw_angle1` --la mitad, segun orientacion--
//! salia a toda altura y plana.
//!
//! El arreglo es `movsxd rax, eax` antes del valor absoluto: los 32 bits bajos,
//! con su signo. La conversion a `int`, ni mas ni menos.
//!
//!   > Cuatro zonas exoneradas y dos instrumentos en el metal para llegar a una
//!   > instruccion. La foto tenia razon desde agosto; lo que faltaba era saber
//!   > donde NO estaba.
//!
//! [!] Y se queda como sonda con las palabras de siempre: lo que sobrevive al
//! descarte es el culpable, pero para que el descarte valga, tiene que quedar
//! escrito que se descarto. Este fichero es el ultimo eslabon de esa lista.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// *** LA FILA QUE ESTABA ROJA. `abs` de una resta sin signo que envuelve.
#[test]
fn abs_de_una_resta_sin_signo_que_envuelve() {
    cuadra("a < b", "100",
        "int main(){ unsigned int a; unsigned int b; a = 100; b = 200; \
         printf(\"%d\", abs(a - b)); return 0; }");
    cuadra("a > b (control)", "100",
        "int main(){ unsigned int a; unsigned int b; a = 200; b = 100; \
         printf(\"%d\", abs(a - b)); return 0; }");
    cuadra("int negativo (control)", "100",
        "int main(){ int d; d = -100; printf(\"%d\", abs(d)); return 0; }");
}

/// *** EL BLOQUE DE `r_segs.c`, tal cual: `offsetangle` no se clava en ANG90 y
/// `distangle` NO es cero. Son los dos numeros que producian las bandas.
#[test]
fn el_offsetangle_de_doom_no_se_clava_en_ang90() {
    cuadra("offsetangle y distangle", "65536,0",
        "unsigned int rw_normalangle; unsigned int rw_angle1; \
         int main(){ unsigned int offsetangle; unsigned int distangle; \
         rw_normalangle = 0x10000000; rw_angle1 = 0x10010000; \
         offsetangle = abs(rw_normalangle - rw_angle1); \
         if (offsetangle > 0x40000000) offsetangle = 0x40000000; \
         distangle = 0x40000000 - offsetangle; \
         printf(\"%u,%d\", offsetangle, (int)(distangle == 0)); return 0; }");
}

/// Y la misma conversion en una funcion DE USUARIO con parametro `int`: el
/// argumento `unsigned` tiene que llegar convertido. Si esta fila se pone roja,
/// el fallo de `abs` era la punta de algo mas ancho.
#[test]
fn un_unsigned_pasado_a_un_parametro_int_llega_convertido() {
    cuadra("f(a - b) con int f(int)", "-100",
        "int f(int x){ return x; } \
         int main(){ unsigned int a; unsigned int b; a = 100; b = 200; \
         printf(\"%d\", f(a - b)); return 0; }");
    cuadra("y abs sobre ese parametro", "100",
        "int g(int x){ return abs(x); } \
         int main(){ unsigned int a; unsigned int b; a = 100; b = 200; \
         printf(\"%d\", g(a - b)); return 0; }");
}

/// `abs` de un `long` que no cabe en `int` RECORTA, como en C. No es un fallo:
/// es lo que `abs` significa, y los compiladores de verdad avisan.
#[test]
fn abs_de_un_long_recorta_a_int_como_dice_c() {
    cuadra("abs(long grande)", "1",
        "int main(){ long v; v = 0x100000001L; printf(\"%d\", abs(v)); return 0; }");
}

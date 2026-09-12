//! **LA ALTURA DEL MURO: las formas de `R_StoreWallRange` que ponen `yl`/`yh`.**
//! 2026-09-12.
//!
//! # El instrumento C4 contesto
//!
//! ```text
//!    [bmo/c4] subsec 12 suelo 12 techo 8 | muros 20 marca-suelo 20 marca-techo 15
//!             | pisa-techo 320 pisa-suelo 960 | planos 10 cielo 0 cols 1280
//!             | mapplane 36 fuera 0 | viewz 41
//! ```
//!
//! Todas las etapas vivas: los planos se marcan (1.280 columnas con
//! `top <= bottom`) y se recorren. Pero salen **36 spans en 3 filas**. Un plano
//! solo da tantos spans como filas tiene; 1.280 columnas y 3 filas es que cada
//! columna marca **un hueco de UNA fila**: el muro ocupa la vista casi entera.
//! O sea que `yl`/`yh` salen de una ESCALA demasiado grande -- y eso no lo
//! arreglo `abs`.
//!
//! Aqui van, tal cual, las formas que hay entre `rw_scale` y `yl`.
//!
//! *** Y LAS CINCO ESTAN BIEN. La primera version de este fichero tenia TRES
//! filas rojas, y las tres eran MIAS: esperados tecleados a mano (el `>> 16` de
//! un negativo redondea hacia abajo, -2745057 y no -2744320). Python dio lo
//! mismo que el compilador en las tres. Es la misma leccion que ya estaba
//! escrita en `sonda_planos_de_doom.rs`, y la volvi a pagar.
//!
//! [!] Esperados calculados con Python, no a mano -- esta vez de verdad.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// `worldbottom >>= 4;` con `worldbottom` GLOBAL y negativo (suelo bajo el ojo).
#[test]
fn desplazar_asignando_un_global_negativo_conserva_el_signo() {
    cuadra(">>= global", "-257,-257,16",
        "int worldbottom; int worldtop; \
         int main(){ int l; worldbottom = -4112; worldtop = 256; l = -4112; \
           worldbottom >>= 4; l >>= 4; worldtop >>= 4; \
           printf(\"%d,%d,%d\", worldbottom, l, worldtop); return 0; }");
}

/// `FixedMul` de m_fixed.c: `((int64_t) a * (int64_t) b) >> FRACBITS`.
#[test]
fn fixedmul_con_int64() {
    cuadra("FixedMul", "-2745057,1572864,-65536",
        "typedef long long int64_t; typedef int fixed_t; \
         fixed_t FixedMul(fixed_t a, fixed_t b){ return ((int64_t) a * (int64_t) b) >> 16; } \
         int main(){ printf(\"%d,%d,%d\", FixedMul(-257, 700000000), FixedMul(3<<16, 8<<16), \
           FixedMul(-1<<16, 1<<16)); return 0; }");
}

/// `FixedDiv`: `(abs(a) >> 14) >= abs(b)` y `((int64_t) a << 16) / b`.
#[test]
fn fixeddiv_con_int64() {
    cuadra("FixedDiv", "196608,-196608,2147483647,-2147483648",
        "typedef long long int64_t; typedef int fixed_t; \
         fixed_t FixedDiv(fixed_t a, fixed_t b){ \
           if ((abs(a) >> 14) >= abs(b)) return (a^b) < 0 ? (-2147483647-1) : 2147483647; \
           else { int64_t result; result = ((int64_t) a << 16) / b; return (fixed_t) result; } } \
         int main(){ printf(\"%d,%d,%d,%d\", FixedDiv(6<<16, 2<<16), FixedDiv(-(6<<16), 2<<16), \
           FixedDiv(1<<30, 1), FixedDiv(-(1<<30), 1)); return 0; }");
}

/// `topstep = -FixedMul(...)` y `topfrac = (centeryfrac>>4) - FixedMul(worldtop, rw_scale)`,
/// con los numeros de una vista de 168 y un muro a media distancia.
#[test]
fn la_cadena_de_topfrac_hasta_yl() {
    cuadra("topfrac -> yl", "-1500,311296,76,-1500,376832,92",
        "typedef long long int64_t; typedef int fixed_t; \
         fixed_t centeryfrac; fixed_t rw_scale; fixed_t rw_scalestep; int worldtop; \
         fixed_t topfrac; fixed_t topstep; \
         fixed_t FixedMul(fixed_t a, fixed_t b){ return ((int64_t) a * (int64_t) b) >> 16; } \
         int main(){ int yl; centeryfrac = 84<<16; rw_scale = 1<<16; rw_scalestep = 3000; \
           worldtop = 8<<16; worldtop >>= 4; \
           topstep = -FixedMul(rw_scalestep, worldtop); \
           topfrac = (centeryfrac>>4) - FixedMul(worldtop, rw_scale); \
           yl = (topfrac + (1<<12) - 1) >> 12; \
           printf(\"%d,%d,%d,\", topstep, topfrac, yl); \
           worldtop = -(8<<16); worldtop >>= 4; \
           topstep = -FixedMul(rw_scalestep, worldtop); \
           topfrac = (centeryfrac>>4) - FixedMul(worldtop, rw_scale); \
           yl = (topfrac + (1<<12) - 1) >> 12; \
           printf(\"%d,%d,%d\", -topstep, topfrac, yl); return 0; }");
}

/// `R_ScaleFromGlobalAngle`, entero: `angle_t` sin signo, tablas indexadas con
/// `>>19`, `<<detailshift`, y la comparacion `den > num>>16`.
#[test]
fn la_escala_desde_el_angulo() {
    cuadra("R_ScaleFromGlobalAngle", "41943,4194304",
        "typedef long long int64_t; typedef int fixed_t; typedef unsigned int angle_t; \
         fixed_t finesine[8192]; fixed_t projection; fixed_t rw_distance; \
         angle_t viewangle; angle_t rw_normalangle; int detailshift; \
         fixed_t FixedMul(fixed_t a, fixed_t b){ return ((int64_t) a * (int64_t) b) >> 16; } \
         fixed_t FixedDiv(fixed_t a, fixed_t b){ \
           if ((abs(a) >> 14) >= abs(b)) return (a^b) < 0 ? (-2147483647-1) : 2147483647; \
           else { int64_t r; r = ((int64_t) a << 16) / b; return (fixed_t) r; } } \
         fixed_t R_ScaleFromGlobalAngle(angle_t visangle){ \
           fixed_t scale; angle_t anglea; angle_t angleb; int sinea; int sineb; fixed_t num; int den; \
           anglea = 0x40000000 + (visangle-viewangle); \
           angleb = 0x40000000 + (visangle-rw_normalangle); \
           sinea = finesine[anglea>>19]; sineb = finesine[angleb>>19]; \
           num = FixedMul(projection,sineb)<<detailshift; den = FixedMul(rw_distance,sinea); \
           if (den > num>>16) { scale = FixedDiv(num, den); \
             if (scale > 64*65536) scale = 64*65536; else if (scale < 256) scale = 256; } \
           else scale = 64*65536; return scale; } \
         int main(){ int i; for (i = 0; i < 8192; i++) finesine[i] = 65536; \
           projection = 160<<16; detailshift = 0; viewangle = 0xC0000000; rw_normalangle = 0x80000000; \
           rw_distance = 250<<16; printf(\"%d,\", R_ScaleFromGlobalAngle(0xC0000000)); \
           rw_distance = 0; printf(\"%d\", R_ScaleFromGlobalAngle(0xC0000000)); return 0; }");
}

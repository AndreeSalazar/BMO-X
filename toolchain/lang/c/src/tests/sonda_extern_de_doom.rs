//! **`extern short floorclip[320];` ANTES de `short floorclip[320];`**: el orden
//! del unity build de DOOM. 2026-09-12.
//!
//! # Por que
//!
//! `sonda_marcar_planos.rs` pone el bloque de `R_RenderSegLoop` tal cual y da
//! `0/48 130/167`. El Ryzen da `0/2` en todos los planos. Con `yl = 49` (C5) la
//! unica forma de un techo `0/2` es `floorclip[x] == 3` al marcar. O sea que el
//! bloque esta bien y lo que le LLEGA no.
//!
//! En DOOM `floorclip` y `ceilingclip` se declaran `extern` en `r_plane.h` y se
//! DEFINEN en `r_plane.c`, y el unity build los ve en ese orden. Si el `extern`
//! reserva otro sitio, o con otro tamano, los dos arrays se solapan: escribir
//! `ceilingclip[i] = -1` pisaria `floorclip`.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// *** El orden de DOOM: extern de los dos, luego las definiciones, y el bucle
/// de `R_ClearPlanes`.
#[test]
fn extern_antes_de_la_definicion_no_solapa_los_arrays() {
    cuadra("extern + definicion", "168,168,-1,-1,640,168",
        "extern short floorclip[320]; extern short ceilingclip[320]; \
         short floorclip[320]; short ceilingclip[320]; \
         int viewheight; int viewwidth; \
         int main(){ int i; viewheight = 168; viewwidth = 320; \
           for (i = 0; i < viewwidth; i++) { floorclip[i] = viewheight; ceilingclip[i] = -1; } \
           printf(\"%d,%d,%d,%d,%d,%d\", floorclip[5], floorclip[319], ceilingclip[0], ceilingclip[319], \
             (int)sizeof(floorclip), floorclip[0]); \
           return 0; }");
}

/// La distancia entre los dos arrays, en bytes: no pueden estar a menos de 640.
#[test]
fn los_dos_arrays_extern_no_se_pisan_en_memoria() {
    cuadra("distancia", "1",
        "extern short floorclip[320]; extern short ceilingclip[320]; \
         short floorclip[320]; short ceilingclip[320]; \
         int main(){ long d; d = (long)((char*)ceilingclip - (char*)floorclip); \
           if (d < 0) d = -d; printf(\"%d\", d >= 640); return 0; }");
}

/// Y la forma de `r_segs.c`: el extern en un "fichero" y el uso en una funcion
/// definida ANTES de la definicion del array.
#[test]
fn usar_el_extern_antes_de_que_exista_la_definicion() {
    cuadra("uso antes de la definicion", "168,-1",
        "extern short floorclip[320]; extern short ceilingclip[320]; \
         void limpiar(void){ int i; for (i = 0; i < 320; i++) { floorclip[i] = 168; ceilingclip[i] = -1; } } \
         short floorclip[320]; short ceilingclip[320]; \
         int main(){ limpiar(); printf(\"%d,%d\", floorclip[200], ceilingclip[200]); return 0; }");
}

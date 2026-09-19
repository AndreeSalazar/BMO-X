//! **LOS PLANOS DE DOOM, CON EL SUELO DONDE DE VERDAD ESTA: filas >= 128.**
//! 2026-09-12.
//!
//! # Por que este fichero
//!
//! El metal de hoy --DOOM ya en ventana, con el arreglo de `abs` dentro-- repite
//! el numero de antes de ese arreglo:
//!
//! ```text
//!    [bmo/c3] spans 36, filas 3 de 200, fuera 0, nulos 0
//! ```
//!
//! El censo se limpia al entrar en `R_RenderPlayerView` y habla solo cuando
//! cambia: dicho UNA vez significa que **ningun fotograma del nivel pinto el
//! fondo**. Las rayas de la foto son el fotograma anterior sin borrar.
//!
//! `sonda_visplanes_de_doom.rs` exonero el recorrido de los planos, pero con
//! `top` 10/30 y `bottom` 20/40: **todo por debajo de 128**. El suelo de DOOM
//! vive en las filas 84..167 de una vista de 168, y `top`/`bottom` son `byte`.
//! De 128 para arriba, un byte leido CON SIGNO vale negativo, y el centinela
//! `0xff` pasa a ser -1 -- que es un `top <= bottom` VERDADERO. Eso tiraria
//! justo los spans del fondo sin un solo error.
//!
//! [!] Esperados generados con Python (`planos_altos.py`, el mismo algoritmo),
//! NO a mano. Ver `sonda_planos_de_doom.rs` para lo que costo aprender eso.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// `R_MakeSpans(x, pl->top[x-1], ...)`: un byte de un struct por puntero,
/// pasado como argumento `int`. 200 tiene que llegar 200 y `0xff` 255.
#[test]
fn un_byte_alto_pasado_como_argumento_llega_sin_signo() {
    cuadra("argumento", "200,255,200,255",
        "typedef unsigned char byte; \
         typedef struct { int minx; byte pad1; byte top[320]; } vp; \
         vp v; vp *pl; \
         void f(int a, int b){ printf(\"%d,%d\", a, b); } \
         int main(){ int x; pl = &v; pl->top[4] = 200; pl->top[5] = 0xff; \
           f(pl->top[4], pl->top[5]); printf(\",\"); \
           x = 5; f(pl->top[x-1], pl->top[x]); return 0; }");
}

/// `ceilingplane->top[rw_x] = top;` -- puntero GLOBAL, indice GLOBAL, valor int.
#[test]
fn guardar_en_el_plano_con_puntero_e_indice_globales() {
    cuadra("store", "150,0,167",
        "typedef unsigned char byte; \
         typedef struct { int minx; byte pad1; byte top[320]; byte pad2; byte bottom[320]; } vp; \
         vp v; vp *ceilingplane; int rw_x; \
         int main(){ int top; int bottom; ceilingplane = &v; rw_x = 7; top = 150; bottom = 167; \
           ceilingplane->top[rw_x] = top; ceilingplane->bottom[rw_x] = bottom; \
           printf(\"%d,%d,%d\", (int)v.top[7], (int)v.pad1, (int)v.bottom[7]); return 0; }");
}

/// `top = ceilingclip[rw_x]+1` con `ceilingclip` un `short` global a -1.
#[test]
fn el_short_global_a_menos_uno_suma_cero() {
    cuadra("short", "0,1",
        "short ceilingclip[320]; int rw_x; \
         int main(){ int top; rw_x = 3; ceilingclip[rw_x] = -1; top = ceilingclip[rw_x] + 1; \
           printf(\"%d,%d\", top, ceilingclip[rw_x] < 0); return 0; }");
}

/// `yl = (topfrac+HEIGHTUNIT-1)>>HEIGHTBITS` con `topfrac` negativo: sar, no shr.
#[test]
fn el_desplazamiento_de_un_fixed_negativo_conserva_el_signo() {
    cuadra("sar", "-1",
        "typedef int fixed_t; fixed_t topfrac; \
         int main(){ int yl; topfrac = -5000; yl = (topfrac + (1<<12) - 1) >> 12; \
           printf(\"%d\", yl); return 0; }");
}

/// ** QUIEN MARCA: los cuatro `boolean` globales de `r_segs.c`, seguidos, y la
/// asignacion encadenada `markfloor = markceiling = true`. Con el `enum` de
/// `doomtype.h` --que lleva `undef = 0xFFFFFFFF`-- y con la otra rama, la de
/// `typedef unsigned int boolean`. Si el tipo se guardara con 8 bytes, el
/// segundo almacen pisaria a su vecino.
#[test]
fn los_boolean_globales_de_r_segs_no_se_pisan() {
    let cuerpo = "boolean segtextured; boolean markfloor; boolean markceiling; boolean maskedtexture; \
         int main(){ segtextured = 7; maskedtexture = 9; \
           markfloor = markceiling = true; \
           printf(\"%d,%d,%d,%d,%d,\", (int)sizeof(boolean), (int)segtextured, (int)markfloor, \
             (int)markceiling, (int)maskedtexture); \
           markceiling = false; \
           printf(\"%d,%d,%d\", (int)markfloor, (int)markceiling, (int)maskedtexture); \
           if (markfloor) printf(\",si\"); if (markceiling) printf(\",NO\"); return 0; }";
    cuadra("enum de doomtype.h", "4,7,1,1,9,1,0,9,si",
        &format!("typedef enum {{ false = 0, true = 1, undef = 0xFFFFFFFF }} boolean; {cuerpo}"));
    cuadra("unsigned int", "4,7,1,1,9,1,0,9,si",
        // El banco no tiene preprocesador: el `#define true 1` de `stdbool.h`
        // se escribe como el enum que da los MISMOS literales enteros.
        &format!("enum {{ false = 0, true = 1 }}; typedef unsigned int boolean; {cuerpo}"));
}

/// *** EL PORT DE `sonda_visplanes_de_doom.rs`, igual, con el SUELO: filas
/// 100..167 y 130..160. Python dice 99 spans.
#[test]
fn el_recorrido_de_los_planos_con_filas_altas_da_la_referencia() {
    let salida = run_c(
        "typedef unsigned char byte; typedef int fixed_t;
typedef struct { fixed_t height; int picnum; int lightlevel; int minx; int maxx;
  byte pad1; byte top[320]; byte pad2; byte pad3; byte bottom[320]; byte pad4; } visplane_t;
visplane_t visplanes[128];
visplane_t *lastvisplane;
short spanstart[200];
int llamadas; unsigned int suma;

void R_MapPlane(int y, int x1, int x2)
{
    llamadas = llamadas + 1;
    suma = suma * 31 + y * 1000003 + x1 * 1009 + x2;
}

void R_MakeSpans(int x, int t1, int b1, int t2, int b2)
{
    while (t1 < t2 && t1 <= b1) { R_MapPlane(t1, spanstart[t1], x - 1); t1++; }
    while (b1 > b2 && b1 >= t1) { R_MapPlane(b1, spanstart[b1], x - 1); b1--; }
    while (t2 < t1 && t2 <= b2) { spanstart[t2] = x; t2++; }
    while (b2 > b1 && b2 >= t2) { spanstart[b2] = x; b2--; }
}

visplane_t *R_FindPlane(fixed_t height, int picnum, int lightlevel)
{
    visplane_t *check;
    for (check = visplanes; check < lastvisplane; check++) {
        if (height == check->height && picnum == check->picnum && lightlevel == check->lightlevel)
            break;
    }
    if (check < lastvisplane) return check;
    lastvisplane++;
    check->height = height; check->picnum = picnum; check->lightlevel = lightlevel;
    check->minx = 320; check->maxx = -1;
    memset(check->top, 0xff, sizeof(check->top));
    return check;
}

visplane_t *R_CheckPlane(visplane_t *pl, int start, int stop)
{
    int intrl, intrh, unionl, unionh, x;
    if (start < pl->minx) { intrl = pl->minx; unionl = start; }
    else { unionl = pl->minx; intrl = start; }
    if (stop > pl->maxx) { intrh = pl->maxx; unionh = stop; }
    else { unionh = pl->maxx; intrh = stop; }
    for (x = intrl; x <= intrh; x++)
        if (pl->top[x] != 0xff) break;
    if (x > intrh) { pl->minx = unionl; pl->maxx = unionh; return pl; }
    lastvisplane->height = pl->height;
    lastvisplane->picnum = pl->picnum;
    lastvisplane->lightlevel = pl->lightlevel;
    pl = lastvisplane++;
    pl->minx = start; pl->maxx = stop;
    memset(pl->top, 0xff, sizeof(pl->top));
    return pl;
}

void R_DrawPlanes(void)
{
    visplane_t *pl; int x; int stop;
    for (pl = visplanes; pl < lastvisplane; pl++) {
        if (pl->minx > pl->maxx) continue;
        pl->top[pl->maxx + 1] = 0xff;
        pl->top[pl->minx - 1] = 0xff;
        stop = pl->maxx + 1;
        for (x = pl->minx; x <= stop; x++)
            R_MakeSpans(x, pl->top[x - 1], pl->bottom[x - 1], pl->top[x], pl->bottom[x]);
    }
}

int main(void)
{
    visplane_t *A; visplane_t *B; int x;
    lastvisplane = visplanes; llamadas = 0; suma = 0;
    A = R_FindPlane(100, 1, 5);
    A = R_CheckPlane(A, 50, 100);
    for (x = 50; x <= 100; x++) { A->top[x] = 100; A->bottom[x] = 167; }
    B = R_CheckPlane(A, 60, 70);
    for (x = 60; x <= 70; x++) { B->top[x] = 130; B->bottom[x] = 160; }
    R_DrawPlanes();
    printf(\"planos %d, A es B %d, spans %d, suma %u\\n\",
           (int)(lastvisplane - visplanes), (int)(A == B), llamadas, suma);
    return 0;
}
",
    );
    assert_eq!(
        salida.trim_end(),
        "planos 2, A es B 0, spans 99, suma 3346860335",
        "con filas >= 128 el recorrido de los visplanes no da lo que da la referencia"
    );
}

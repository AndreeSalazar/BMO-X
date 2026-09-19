//! **LOS VISPLANES DE DOOM: donde el metal dijo que se pierden los spans.**
//!
//! # El instrumento senalo (2026-09-11)
//!
//! ```text
//!    [bmo/c3] vistas 320 de 320, tramos 1, iscale0 0      paredes: todo
//!    [bmo/c3] spans 36, filas 3 de 200, fuera 0, nulos 0  fondo: CASI NADA
//! ```
//!
//! Un fotograma normal dibuja cientos de spans sobre cien filas. **Treinta y
//! seis en tres filas es que el fondo no se pinta**, y `fuera 0, nulos 0` dice
//! que los pocos que llegan son sanos. O sea que el fallo esta ANTES de
//! `R_DrawSpan`: los planos llegan vacios, o sus spans no se generan.
//!
//! # La estructura, que es sospechosa por si misma
//!
//! ```text
//!    height, picnum, lightlevel, minx, maxx   int    @0..20
//!    pad1                                     byte   @20
//!    top[320]                                 byte   @21    <- DESALINEADO
//!    pad2, pad3                               byte   @341, 342
//!    bottom[320]                              byte   @343
//!    pad4                                     byte   @663
//!    sizeof                                          664    <- NO potencia de 2
//! ```
//!
//! Y el codigo que la recorre usa **`pl->top[pl->minx-1]`** --indice MENOS UNO
//! en un campo, a proposito: los pads son los bordes--, `pl = lastvisplane++`
//! sobre un puntero a 664 bytes, y `lastvisplane - visplanes` como cuenta.
//!
//! # Lo que se prueba
//!
//! El port TAL CUAL de `R_FindPlane`, `R_CheckPlane`, `R_MakeSpans` y el bucle
//! de `R_DrawPlanes`, con `R_MapPlane` sustituido por un contador que suma las
//! ternas `(y, x1, x2)`. La referencia la calculo Python con el mismo
//! algoritmo: dos planos, veintidos spans, y una suma de control.
//!
//! [!] Esperados generados con Python, NO a mano. Ver `sonda_planos_de_doom.rs`
//! para lo que costo aprender eso.

use super::*;

/// `sizeof(visplane_t)` y los offsets de `top` y `bottom`, que es lo primero
/// que puede estar mal y lo que arrastra a todo lo demas.
#[test]
fn la_visplane_mide_664_y_top_empieza_en_21() {
    let bef = compile_source_to_bef(
        "typedef unsigned char byte; typedef int fixed_t; \
         typedef struct { fixed_t height; int picnum; int lightlevel; int minx; int maxx; \
           byte pad1; byte top[320]; byte pad2; byte pad3; byte bottom[320]; byte pad4; } visplane_t; \
         visplane_t v; \
         int main(){ printf(\"%d,%d,%d\", (int)sizeof(visplane_t), \
           (int)((char*)&v.top[0] - (char*)&v), (int)((char*)&v.bottom[0] - (char*)&v)); return 0; }",
    )
    .expect("compila");
    assert_eq!(ejecutar_bef(&bef).trim_end(), "664,21,343");
}

/// `pl->top[pl->minx-1] = 0xff` con `minx = 0`: escribe en `pad1`, no antes
/// del struct. Y `pl->top[x] != 0xff` compara un byte con 255, no con -1.
#[test]
fn el_indice_menos_uno_y_la_comparacion_con_0xff() {
    let bef = compile_source_to_bef(
        "typedef unsigned char byte; \
         typedef struct { int a; int b; int c; int minx; int maxx; byte pad1; byte top[320]; byte pad2; } vp; \
         vp planes[2]; \
         int main(){ vp *pl; int x; int cuantos; pl = &planes[1]; pl->minx = 0; \
           memset(pl->top, 0xff, sizeof(pl->top)); \
           pl->top[pl->minx-1] = 0x77; \
           cuantos = 0; for (x = 0; x < 320; x = x + 1) if (pl->top[x] != 0xff) cuantos = cuantos + 1; \
           printf(\"%d,%d,%d,%d\", (int)pl->pad1, (int)planes[0].pad2, cuantos, (int)(pl->top[5] != 0xff)); \
           return 0; }",
    )
    .expect("compila");
    // pad1 recibio el 0x77, el struct de al lado no se toco, ningun top != 0xff,
    // y la comparacion con 0xff sobre un byte a 255 es FALSA.
    assert_eq!(ejecutar_bef(&bef).trim_end(), "119,0,0,0");
}

/// `pl = lastvisplane++` y `lastvisplane - visplanes` sobre un struct de 664.
#[test]
fn el_puntero_a_visplane_avanza_664_y_se_resta() {
    let bef = compile_source_to_bef(
        "typedef unsigned char byte; \
         typedef struct { int a; int b; int c; int minx; int maxx; byte pad1; byte top[320]; \
           byte pad2; byte pad3; byte bottom[320]; byte pad4; } vp; \
         vp visplanes[128]; vp *lastvisplane; \
         int main(){ vp *pl; lastvisplane = visplanes; \
           pl = lastvisplane++; pl->minx = 7; \
           pl = lastvisplane++; pl->minx = 9; \
           printf(\"%d,%d,%d,%d\", (int)(lastvisplane - visplanes), visplanes[0].minx, visplanes[1].minx, \
             (int)((char*)lastvisplane - (char*)visplanes)); return 0; }",
    )
    .expect("compila");
    assert_eq!(ejecutar_bef(&bef).trim_end(), "2,7,9,1328");
}

/// *** EL PORT ENTERO: dos planos, veintidos spans, y la suma de Python.
#[test]
fn el_recorrido_de_los_planos_da_los_spans_de_la_referencia() {
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
    for (x = 50; x <= 100; x++) { A->top[x] = 10; A->bottom[x] = 20; }
    B = R_CheckPlane(A, 60, 70);
    for (x = 60; x <= 70; x++) { B->top[x] = 30; B->bottom[x] = 40; }
    R_DrawPlanes();
    printf(\"planos %d, A es B %d, spans %d, suma %u\\n\",
           (int)(lastvisplane - visplanes), (int)(A == B), llamadas, suma);
    return 0;
}
",
    );
    assert_eq!(
        salida.trim_end(),
        "planos 2, A es B 0, spans 22, suma 2421757864",
        "el recorrido de los visplanes no da lo que da la referencia"
    );
}

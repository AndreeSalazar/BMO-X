//! **LOS PLANOS DE DOOM: la cuarta zona, la que el METAL senalo.**
//!
//! # De donde sale: el instrumento contesto
//!
//! El **2026-09-10** el censo de columnas corrio en el Ryzen y dijo:
//!
//! ```text
//!    [bmo/c3] vistas 320 de 320, tramos 1, de 0 a 319, iscale0 0
//! ```
//!
//! Todas las columnas visitadas, en un tramo, ninguna con paso cero. **Eso
//! exonera el dibujado de columnas entero** -- geometria y muestreo. Y el dueno
//! dijo con sus palabras donde sigue el fallo: *"el FONDO del juego es
//! corrompido"*. El fondo son los planos: suelos y techos, que no pasan por
//! `R_DrawColumn` sino por `R_DrawSpan`.
//!
//! # Lo que se prueba aqui, y en que orden se descubrio
//!
//! ```text
//!    1. las tablas de luz     comparten paredes y planos     verde
//!    2. campos a offset >=128 `visplane_t.bottom` esta a 324   ver abajo
//!    3. R_DrawSpan            la aritmetica empaquetada       verde
//! ```
//!
//! ## *** Y LA 2 DESTAPO UN OJO CIEGO DEL BANCO, no del compilador
//!
//! Un campo de struct a offset 128 o mas **paraba el emulador**: *"opcode 0x05
//! no emitido por BMO"*. Pero `emit_add_offset` SI lo emite --`48 05 imm32`,
//! `add rax, imm32`, x86-64 legitimo-- para todo campo que no cabe en un
//! `imm8` con signo. El emulador no lo conocia.
//!
//! ** O sea que **el banco era ciego a todo campo mas alla de 127 bytes**:
//! `player_t`, `mobj_t`, `visplane_t.bottom`... los structs grandes de DOOM,
//! donde un fallo del emisor mas cuesta. Cualquier fila que los tocara moria en
//! el emulador y no decia nada del compilador. Se le enseno la instruccion, y
//! **el compilador estaba bien ahi**: siete de siete.
//!
//!   > Un emulador que dice "esto no se emite" tiene que tener razon, o su
//!   > banco entero prueba menos de lo que parece.
//!
//! ## [!] Y la 3 casi acusa al compilador por un error MIO
//!
//! Las primeras filas de `R_DrawSpan` salieron rojas, y al mirar despacio **los
//! esperados estaban mal**: `0xD1590000` convertido a decimal a mano, y mal.
//! El compilador tenia razon en las cinco. Desde entonces **cada esperado de
//! este fichero lo genera Python**, y se anota aqui para que nadie vuelva a
//! teclear uno.
//!
//!   > Un esperado tecleado a mano prueba al que lo tecleo, no al compilador.
//!
//! # Lo que queda, y no se puede desde el anfitrion
//!
//! Con columnas, luz, planos y recorte exonerados, lo que falta es VER el fallo
//! actual --la foto de evidencia es anterior al arreglo de `signed`-- y un censo
//! de spans en el metal, gemelo del de columnas. Ver `METAL_2026-09-10.md`.

use super::*;

struct Casilla {
    nombre: &'static str,
    fuente: &'static str,
    espera: &'static str,
}

fn barrer(casillas: &[Casilla]) {
    let mut malas = Vec::new();
    for c in casillas {
        let sale = run_c(c.fuente);
        let sale = sale.trim_end();
        if sale != c.espera {
            malas.push(format!("  {}\n    espera {:?}, sale {:?}", c.nombre, c.espera, sale));
        }
    }
    assert!(
        malas.is_empty(),
        "UNA ZONA EXONERADA DE LOS PLANOS SE PUSO EN ROJO. Antes de acusar al \
         compilador: los esperados de este fichero los genero Python -- si se \
         cambio uno a mano, mirar ahi primero.\n{}",
        malas.join("\n")
    );
}

/// **ZONA 4a: las tablas de luz**, que comparten paredes y planos.
/// `r_main.c:100-102`, `R_InitLightTables`, y `dc_colormap[dc_source[i]]`.
#[test]
fn las_tablas_de_luz_estan_exoneradas() {
    barrer(&[
        Casilla {
            nombre: "tabla 2D de punteros: escribir y leer",
            fuente: "unsigned char cmap[8192]; unsigned char *t[16][48]; \
                     int main(){ int i; for (i = 0; i < 8192; i = i + 1) cmap[i] = (unsigned char)(i / 256); \
                     t[3][5] = cmap + 7*256; printf(\"%d\", (int)t[3][5][0]); return 0; }",
            espera: "7",
        },
        Casilla {
            // `walllights = scalelight[lightnum]; dc_colormap = walllights[index]`
            nombre: "la fila de la tabla, con indice variable",
            fuente: "unsigned char cmap[8192]; unsigned char *t[16][48]; unsigned char **pp; \
                     int main(){ int i; int n; int k; for (i = 0; i < 8192; i = i + 1) cmap[i] = (unsigned char)(i / 256); \
                     n = 3; k = 5; t[n][k] = cmap + 7*256; pp = t[n]; printf(\"%d\", (int)pp[k][0]); return 0; }",
            espera: "7",
        },
        Casilla {
            // Un indice de `byte` >= 128 NO puede extenderse con signo.
            nombre: "dc_colormap[dc_source[x]] con byte >= 128",
            fuente: "unsigned char cmap[256]; unsigned char src[128]; unsigned char *dc_colormap; unsigned char *dc_source; \
                     int main(){ int i; for (i = 0; i < 256; i = i + 1) cmap[i] = (unsigned char)(255 - i); \
                     src[0] = 250; dc_colormap = cmap; dc_source = src; \
                     printf(\"%d\", (int)dc_colormap[dc_source[0]]); return 0; }",
            espera: "5",
        },
        Casilla {
            nombre: "el bucle entero de R_InitLightTables",
            fuente: "unsigned char cmap[8192]; unsigned char *zl[16][128]; \
                     int main(){ int i; int j; int level; int startmap; int scale; \
                     for (i = 0; i < 8192; i = i + 1) cmap[i] = (unsigned char)(i / 256); \
                     for (i = 0; i < 16; i = i + 1) { startmap = ((16-1-i)*2)*32/16; \
                       for (j = 0; j < 128; j = j + 1) { scale = (320/2*65536) / ((j+1)<<20); scale >>= 12; \
                         level = startmap - scale/2; if (level < 0) level = 0; if (level >= 32) level = 31; \
                         zl[i][j] = cmap + level*256; } } \
                     printf(\"%d,%d,%d\", (int)zl[0][0][0], (int)zl[15][127][0], (int)zl[0][127][0]); return 0; }",
            espera: "31,0,31",
        },
    ]);
}

/// **ZONA 4b: un campo de struct mas alla del byte 127.** `visplane_t.bottom`
/// esta a 324, `player_t` y `mobj_t` pasan de sobra.
///
/// *** Estas filas MORIAN en el emulador hasta el 2026-09-11 -- ver la cabecera.
#[test]
fn un_campo_mas_alla_del_byte_127_esta_exonerado() {
    barrer(&[
        Casilla {
            nombre: "leer pl->b a offset 128 (la frontera del imm8)",
            fuente: "struct vp { int h; unsigned char top[124]; int b; }; struct vp planes[4]; \
                     int main(){ struct vp *pl; pl = &planes[1]; pl->b = 41; printf(\"%d\", pl->b); return 0; }",
            espera: "41",
        },
        Casilla {
            nombre: "leer y escribir pl->bottom[x] a offset 324 (byte, como DOOM)",
            fuente: "struct vp { int h; unsigned char top[320]; unsigned char bottom[320]; }; struct vp planes[4]; \
                     int main(){ struct vp *pl; int x; pl = &planes[1]; x = 200; \
                     pl->bottom[x] = 77; printf(\"%d,%d\", (int)pl->bottom[200], (int)planes[1].bottom[200]); return 0; }",
            espera: "77,77",
        },
        Casilla {
            nombre: "memset(pl->top, 0xff, sizeof(pl->top)) y bottom intacto",
            fuente: "struct vp { int h; unsigned char top[320]; unsigned char bottom[320]; }; struct vp planes[4]; \
                     int main(){ struct vp *pl; pl = &planes[1]; memset(pl->top, 0xff, sizeof(pl->top)); \
                     printf(\"%d,%d,%d,%d\", (int)sizeof(pl->top), (int)pl->top[0], (int)pl->top[319], (int)pl->bottom[0]); return 0; }",
            espera: "320,255,255,0",
        },
        Casilla {
            nombre: "v.b por valor, offset 644",
            fuente: "struct vp { int h; unsigned short top[320]; int b; }; struct vp v; \
                     int main(){ v.b = 9; printf(\"%d\", v.b); return 0; }",
            espera: "9",
        },
    ]);
}

/// **ZONA 4c: `R_DrawSpan`**, la aritmetica empaquetada de `r_draw.c`.
///
/// `position` lleva x en los 16 altos e y en los 16 bajos, en un `unsigned int`
/// que tiene que ENVOLVER a 32 bits en cada `+=`. Es justo donde el recorte a
/// 32 y el signo de los literales se cruzan.
///
/// [!] Esperados generados con Python (`u32(...)`), NO a mano. Ver la cabecera.
#[test]
fn la_aritmetica_de_r_drawspan_esta_exonerada() {
    barrer(&[
        Casilla {
            nombre: "position y step iniciales",
            fuente: "int ds_xfrac; int ds_yfrac; int ds_xstep; int ds_ystep; \
                     int main(){ unsigned int position; unsigned int step; \
                     ds_xfrac = 0x12345678; ds_yfrac = 0x0ABCDEF0; ds_xstep = 0x00010000; ds_ystep = 0x00008000; \
                     position = ((ds_xfrac << 10) & 0xffff0000) | ((ds_yfrac >> 6) & 0x0000ffff); \
                     step = ((ds_xstep << 10) & 0xffff0000) | ((ds_ystep >> 6) & 0x0000ffff); \
                     printf(\"%u,%u\", position, step); return 0; }",
            espera: "3512333179,67109376",
        },
        Casilla {
            nombre: "los cinco spots del bucle",
            fuente: "int ds_xfrac; int ds_yfrac; int ds_xstep; int ds_ystep; \
                     int main(){ unsigned int position; unsigned int step; unsigned int xtemp; unsigned int ytemp; int spot; int i; \
                     ds_xfrac = 0x12345678; ds_yfrac = 0x0ABCDEF0; ds_xstep = 0x00010000; ds_ystep = 0x00008000; \
                     position = ((ds_xfrac << 10) & 0xffff0000) | ((ds_yfrac >> 6) & 0x0000ffff); \
                     step = ((ds_xstep << 10) & 0xffff0000) | ((ds_ystep >> 6) & 0x0000ffff); \
                     for (i = 0; i < 5; i = i + 1) { \
                       ytemp = (position >> 4) & 0x0fc0; xtemp = (position >> 26); spot = xtemp | ytemp; \
                       if (i) printf(\",\"); printf(\"%d\", spot); position += step; } \
                     return 0; }",
            espera: "3892,3957,3958,4023,4024",
        },
        Casilla {
            nombre: "position += step ENVUELVE a 32 bits",
            fuente: "int main(){ unsigned int position; unsigned int step; \
                     position = 0xFFFFFF00; step = 0x00000100 + 1000; position += step; \
                     printf(\"%u\", position); return 0; }",
            espera: "1000",
        },
        Casilla {
            nombre: "(x << 10) & 0xffff0000",
            fuente: "int main(){ int x; unsigned int r; x = 0x12345678; r = (x << 10) & 0xffff0000; \
                     printf(\"%u\", r); return 0; }",
            espera: "3512270848",
        },
        Casilla {
            nombre: "x << 10 a unsigned, y a long",
            fuente: "int main(){ int x; unsigned int r; long l; x = 0x12345678; r = x << 10; l = (long)x << 10; \
                     printf(\"%u,%ld\", r, l); return 0; }",
            espera: "3512328192,312749973504",
        },
    ]);
}

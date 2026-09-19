//! **LAS COLUMNAS DE DOOM: dos zonas EXONERADAS, y eso es el resultado.**
//!
//! # De donde sale, y es una foto
//!
//! El **2026-09-09**, con `signed` ya arreglado (ver `probe_signedness`), la
//! pantalla de titulo salio PERFECTA en el Ryzen y el demo empezo a
//! reproducirse bien --`IS TURBO!` desaparecio y salieron mensajes de verdad--.
//! Pero el 3D seguia con **bandas verticales de color plano**, de arriba abajo,
//! con parte de la habitacion bien dibujada al lado.
//!
//! ```text
//!    la parte de la izquierda   texturada, con detalle       BIEN
//!    las bandas                 un color plano por columna   MAL
//! ```
//!
//! *** Una columna de color plano tiene una causa mecanica: **el paso de
//! muestreo vale 0**, o el puntero de textura apunta a lo mismo siempre. Asi
//! que las dos hipotesis se escribieron y **las dos se probaron falsas**.
//!
//! # HIPOTESIS 1: la bandera `-1` de `R_GetColumn` (r_data.c:391)
//!
//! ```c
//!    short**  texturecolumnlump;      // r_data.c:150
//!    collump[x] = -1;                 // "esta columna usa el compuesto"
//!    ...
//!    lump = texturecolumnlump[tex][col];   // int <- short, DOS niveles
//!    if (lump > 0) return W_CacheLumpNum(lump, PU_CACHE) + ofs;
//! ```
//!
//! Encajaba con la foto entera: si leer ese `short` negativo no extiende el
//! signo, `-1` se lee `65535`, `lump > 0` sale cierto y DOOM cachea el lump
//! 65535. Las columnas de UN patch (`lump > 0`) saldrian bien y las de VARIOS
//! (`-1`) en bandas -- que es exactamente lo que se ve.
//!
//! **Falsa.** Las doce casillas en verde.
//!
//! # HIPOTESIS 2: la division sin signo de `dc_iscale` (r_segs.c:267)
//!
//! ```c
//!    dc_iscale = 0xffffffffu / (unsigned)rw_scale;
//! ```
//!
//! Si esa division sale CON signo, `0xffffffffu` es `-1`, y `-1 / positivo` es
//! **0**. Con `dc_iscale = 0` la columna muestrea un solo texel: banda plana.
//! Y esta casa ya pago una division sin signo mal (`unsigned long / con el bit
//! 63`), asi que no era una sospecha barata.
//!
//! **Falsa tambien.** Y de paso queda dicho que el sufijo `u` de un literal
//! SI se lee, que era la mitad de la hipotesis.
//!
//! # ** POR QUE UN FICHERO DE VEINTIDOS VERDES SE QUEDA
//!
//! > Lo que sobrevive al descarte es el culpable. Pero para que el descarte
//! > valga algo, **tiene que quedar escrito QUE se descarto**.
//!
//! Sin este fichero, la proxima vez que aparezcan bandas verticales alguien
//! --yo-- volvera a sospechar de `R_GetColumn`, porque es la sospecha que
//! parece buena. Aqui queda que ya se miro, con las formas del fichero y no
//! con formas genericas, y que dijo que no.
//!
//! [!] Y las sondas SABEN decir que no: son las mismas formas que en su dia
//! cazaron cinco fallos del codegen. Veintidos que solo pudieran salir bien no
//! probarian nada -- es la regla de las hojas de metal.
//!
//! ** Lo que queda por descartar, y no se puede desde el anfitrion: los VALORES
//! de `rw_scale`, `dc_texturemid` y `walllights` en tiempo de ejecucion. Eso
//! pide una sonda DENTRO de DOOM, no en el banco.

use super::*;

/// Una casilla: el programa, lo que debe salir, y su nombre para el informe.
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
            malas.push(format!(
                "  {}\n    espera {:?}, sale {:?}",
                c.nombre, c.espera, sale
            ));
        }
    }
    assert!(
        malas.is_empty(),
        "UNA ZONA QUE ESTABA EXONERADA SE PUSO EN ROJO.\n\
         Estas casillas son las formas EXACTAS de r_data.c y r_segs.c: si una\n\
         falla, las bandas verticales de DOOM vuelven a tener un culpable en el\n\
         compilador. Ver la cabecera de este fichero.\n{}",
        malas.join("\n")
    );
}

/// **ZONA 1: `R_GetColumn` y las tablas de columnas.** `r_data.c:380-401`.
#[test]
fn la_busqueda_de_columna_de_textura_esta_exonerada() {
    barrer(&[
        Casilla {
            // `lump = texturecolumnlump[tex][col]`, un nivel.
            nombre: "short negativo por array, a int",
            fuente: "short a[4]; int main() { int l; a[0] = -1; l = a[0]; \
                     printf(\"%d\\n\", l); return 0; }",
            espera: "-1",
        },
        Casilla {
            nombre: "y la bandera `lump > 0`",
            fuente: "short a[4]; int main() { int l; a[0] = -1; l = a[0]; \
                     printf(\"%d\\n\", (int)(l > 0)); return 0; }",
            espera: "0",
        },
        Casilla {
            // ** La forma DE VERDAD: `short**`, dos indirecciones.
            nombre: "short negativo por puntero a puntero",
            fuente: "short f0[4]; short *f[2]; int main() { int l; \
                     f[0] = f0; f[0][0] = -1; l = f[0][0]; \
                     printf(\"%d\\n\", l); return 0; }",
            espera: "-1",
        },
        Casilla {
            nombre: "la bandera, dos niveles (R_GetColumn)",
            fuente: "short f0[4]; short *f[2]; int main() { int l; \
                     f[0] = f0; f[0][0] = -1; l = f[0][0]; \
                     printf(\"%d\\n\", (int)(l > 0)); return 0; }",
            espera: "0",
        },
        Casilla {
            // `collump[x] = patch->patch`, un numero de lump en un short.
            nombre: "short guardado desde un int (el lump)",
            fuente: "short a[4]; int main() { int n; n = 1234; a[0] = n; \
                     printf(\"%d\\n\", (int)a[0]); return 0; }",
            espera: "1234",
        },
        Casilla {
            // `colofs[x] = LONG(realpatch->columnofs[x-x1])+3`, y `colofs` es
            // `unsigned short*`: DOOM cuenta con que trunque a 16 bits.
            nombre: "unsigned short trunca un int a 16 bits",
            fuente: "unsigned short a[4]; int main() { int n; n = 70000; a[0] = n; \
                     printf(\"%d\\n\", (int)a[0]); return 0; }",
            espera: "4464",
        },
        Casilla {
            nombre: "unsigned short se lee SIN signo",
            fuente: "unsigned short a[4]; int main() { int o; a[0] = 65531; o = a[0]; \
                     printf(\"%d\\n\", o); return 0; }",
            espera: "65531",
        },
        Casilla {
            nombre: "LONG(x)+3 metido en unsigned short",
            fuente: "unsigned short a[4]; int main() { int n; n = 65534; \
                     a[0] = ((signed int)n) + 3; \
                     printf(\"%d\\n\", (int)a[0]); return 0; }",
            espera: "1",
        },
        Casilla {
            // `col &= texturewidthmask[tex]` con una columna negativa, que las
            // hay: el recorte del BSP las produce.
            nombre: "and de un negativo con una mascara",
            fuente: "int w[2]; int main() { int c; w[0] = 63; c = -3; c = c & w[0]; \
                     printf(\"%d\\n\", c); return 0; }",
            espera: "61",
        },
        Casilla {
            nombre: "frac >> FRACBITS con frac negativa",
            fuente: "int main() { int frac; frac = -65536; \
                     printf(\"%d\\n\", frac >> 16); return 0; }",
            espera: "-1",
        },
        Casilla {
            // El bucle interior de `R_DrawColumn`, con la mascara de altura.
            nombre: "indice byte con la mascara de altura",
            fuente: "unsigned char t[128]; int main() { int frac; int h; \
                     t[61] = 200; h = 127; frac = 61 << 16; \
                     printf(\"%d\\n\", (int)t[(frac >> 16) & h]); return 0; }",
            espera: "200",
        },
        Casilla {
            nombre: "byte* + int, y leer detras",
            fuente: "unsigned char t[64]; int main() { unsigned char *p; \
                     t[10] = 77; p = t; p = p + 10; \
                     printf(\"%d\\n\", (int)*p); return 0; }",
            espera: "77",
        },
    ]);
}

/// **ZONA 2: el paso de muestreo y los recortes de la escala.**
/// `r_segs.c:171` y `:267`, `r_main.c:449`.
#[test]
fn el_paso_de_muestreo_de_una_columna_esta_exonerado() {
    barrer(&[
        Casilla {
            // *** LA LINEA. Si sale 0, toda columna es un color plano.
            nombre: "0xffffffffu / (unsigned)scale [r_segs:267]",
            fuente: "int main() { int s; unsigned int r; s = 4096; \
                     r = 0xffffffffu / (unsigned)s; printf(\"%u\\n\", r); return 0; }",
            espera: "1048575",
        },
        Casilla {
            // La mitad de la hipotesis: si el sufijo se ignora, el literal es
            // un `int` y vale -1.
            nombre: "el SUFIJO u de un literal de 32 bits",
            fuente: "int main() { printf(\"%u\\n\", 0xffffffffu); return 0; }",
            espera: "4294967295",
        },
        Casilla {
            nombre: "el mismo literal SIN sufijo",
            fuente: "int main() { unsigned int a; a = 0xffffffff; \
                     printf(\"%u\\n\", a); return 0; }",
            espera: "4294967295",
        },
        Casilla {
            nombre: "cast (unsigned) de un int con signo",
            fuente: "int main() { int s; s = 4096; \
                     printf(\"%u\\n\", (unsigned)s); return 0; }",
            espera: "4096",
        },
        Casilla {
            nombre: "la division con el numerador en variable",
            fuente: "int main() { unsigned int n; unsigned int d; \
                     n = 0xffffffff; d = 4096; \
                     printf(\"%u\\n\", n / d); return 0; }",
            espera: "1048575",
        },
        Casilla {
            // `r_segs.c:171`, la version de los sprites y las medias texturas.
            nombre: "la misma, con spryscale pequeno",
            fuente: "int main() { int s; unsigned int r; s = 256; \
                     r = 0xffffffffu / (unsigned)s; printf(\"%u\\n\", r); return 0; }",
            espera: "16777215",
        },
        Casilla {
            // ** Los dos recortes de `R_ScaleFromGlobalAngle`. Si el de arriba
            // no muerde, `rw_scale` se va y `dc_iscale` se queda en nada.
            nombre: "recorte scale > 64*FRACUNIT",
            fuente: "int main() { int sc; sc = 9000000; \
                     if (sc > 64*(1<<16)) { sc = 64*(1<<16); } \
                     else if (sc < 256) { sc = 256; } \
                     printf(\"%d\\n\", sc); return 0; }",
            espera: "4194304",
        },
        Casilla {
            nombre: "recorte scale < 256",
            fuente: "int main() { int sc; sc = 10; \
                     if (sc > 64*(1<<16)) { sc = 64*(1<<16); } \
                     else if (sc < 256) { sc = 256; } \
                     printf(\"%d\\n\", sc); return 0; }",
            espera: "256",
        },
        Casilla {
            nombre: "64*FRACUNIT plegado",
            fuente: "int main() { printf(\"%d\\n\", 64*(1<<16)); return 0; }",
            espera: "4194304",
        },
        Casilla {
            nombre: "den > num>>16 con num grande",
            fuente: "int main() { int num; int den; num = 268435456; den = 3000; \
                     printf(\"%d\\n\", (int)(den > (num>>16))); return 0; }",
            espera: "0",
        },
    ]);
}

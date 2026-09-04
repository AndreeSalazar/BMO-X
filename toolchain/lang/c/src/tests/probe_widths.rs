//! # THE WIDTH PROBE -- what happens to a number stored in something narrower
//!
//! ## The axis, and where it comes from
//!
//! DOOM's `SHORT(x)` is literally `((signed short)(x))` (`i_swap.h:26`), and
//! **every field that comes out of the WAD goes through it**. What usually
//! follows is a promotion to `int` or a `<<FRACBITS`. So between the disk and
//! the screen there is a narrow -> widen chain for every number in the game.
//!
//! And the values being negative is not hypothetical: a sprite's
//! `patch->leftoffset` and `topoffset` almost always are --that is how DOOM
//! centres a sprite on its thing-- and so is `sector->floorheight`. A `short`
//! that fails to sign-extend on load does not produce an error: it produces a
//! floor at an absurd height, or a sprite off the edge of the screen.
//!
//! ## ** And the first sweep found NOTHING
//!
//! All 16 cells came back green first time. That is a result too, and it is
//! worth writing down: **the width axis is clean**, so when something comes out
//! crooked in `R_Init`, this is not where to start.
//!
//! [!] A census that finds nothing is not a census that was wasted. Its job
//! starts now: if somebody touches the load of a `short` tomorrow and forgets
//! the `movsx`, these 16 rows say so in 0.2 seconds instead of three reflashes.

use super::census::{sweep, Cell};
use super::run_c;

fn census() -> [Cell; 16] {
    [
        Cell {
            name: "short stores and returns negative",
            source: "int main() { short s; s = -5; printf(\"%d\\n\", (int)s); return 0; }",
            expects: "-5",
        },
        Cell {
            name: "negative global short",
            source: "short g;\n\
                     int main() { g = -300; printf(\"%d\\n\", (int)g); return 0; }",
            expects: "-300",
        },
        Cell {
            name: "negative short field",
            source: "typedef struct { short a; short b; } p_t;\n\
                     int main() { p_t s; s.a = -7; s.b = 3; \
                       printf(\"%d %d\\n\", (int)s.a, (int)s.b); return 0; }",
            expects: "-7 3",
        },
        Cell {
            name: "(short) narrows keeping the sign",
            source: "int main() { int n; n = 65535; \
                       printf(\"%d\\n\", (int)(short)n); return 0; }",
            expects: "-1",
        },
        Cell {
            name: "(short) of 0x8000",
            source: "int main() { int n; n = 32768; \
                       printf(\"%d\\n\", (int)(short)n); return 0; }",
            expects: "-32768",
        },
        Cell {
            name: "unsigned short carries NO sign",
            source: "int main() { unsigned short u; u = 65535; \
                       printf(\"%d\\n\", (int)u); return 0; }",
            expects: "65535",
        },
        Cell {
            name: "negative short << 16",
            source: "int main() { short s; s = -3; \
                       printf(\"%d\\n\", (int)(((int)s) << 16)); return 0; }",
            expects: "-196608",
        },
        Cell {
            name: "short << 16 uncast (DOOM's form)",
            source: "int main() { short s; int r; s = -3; r = s << 16; \
                       printf(\"%d\\n\", r); return 0; }",
            expects: "-196608",
        },
        Cell {
            name: "char is signed",
            source: "int main() { char c; c = -1; printf(\"%d\\n\", (int)c); return 0; }",
            expects: "-1",
        },
        Cell {
            name: "unsigned char is unsigned",
            source: "int main() { unsigned char c; c = 200; \
                       printf(\"%d\\n\", (int)c); return 0; }",
            expects: "200",
        },
        Cell {
            name: "negative short in an array",
            source: "short t[4];\n\
                     int main() { t[2] = -1000; printf(\"%d\\n\", (int)t[2]); return 0; }",
            expects: "-1000",
        },
        Cell {
            name: "short from raw bytes",
            source: "unsigned char b[4];\n\
                     int main() { short *p; b[0] = 0xFE; b[1] = 0xFF; \
                       p = (short *)b; printf(\"%d\\n\", (int)*p); return 0; }",
            expects: "-2",
        },
        Cell {
            name: "short overflows on store",
            source: "int main() { short s; s = 40000; printf(\"%d\\n\", (int)s); return 0; }",
            expects: "-25536",
        },
        Cell {
            name: "negative division truncates to 0",
            source: "int main() { int a; a = -7; printf(\"%d %d\\n\", a / 2, a % 2); return 0; }",
            expects: "-3 -1",
        },
        Cell {
            name: "right shift keeps the sign",
            source: "int main() { int a; a = -256; printf(\"%d\\n\", a >> 4); return 0; }",
            expects: "-16",
        },
        Cell {
            name: "short as parameter and return",
            source: "short dob(short x) { return (short)(x * 2); }\n\
                     int main() { printf(\"%d\\n\", (int)dob(-1000)); return 0; }",
            expects: "-2000",
        },
    ]
}

#[test]
fn the_width_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DE LOS ANCHOS CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION,\n\
         no un defecto viejo que sale a la luz. Mirar la carga del tipo\n\
         estrecho (`movsx` contra `movzx`) antes que nada.",
    );
}

/// **THE WIDTH CENSUS, as of 2026-08-13.** Green throughout from the very
/// first sweep: nothing needed fixing.
const CENSUS: &str = "\
short stores and returns negative GOOD
negative global short          GOOD
negative short field           GOOD
(short) narrows keeping the sign GOOD
(short) of 0x8000              GOOD
unsigned short carries NO sign GOOD
negative short << 16           GOOD
short << 16 uncast (DOOM's form) GOOD
char is signed                 GOOD
unsigned char is unsigned      GOOD
negative short in an array     GOOD
short from raw bytes           GOOD
short overflows on store       GOOD
negative division truncates to 0 GOOD
right shift keeps the sign     GOOD
short as parameter and return  GOOD
";

/// **UN `unsigned int` TIENE QUE ENVOLVER A 32 BITS**, y el Ryzen dijo que no.
///
/// # *** De donde sale este numero
///
/// El 04-09, DOOM imprimio esto justo antes de morir:
///
/// ```text
///    [bmo] R_AddLine AL REVES: x1=7 x2=0 | fino1=2957 fino2=9215
/// ```
///
/// `fino2` es el indice con el que `R_AddLine` entra en `viewangletox[]`, y sale
/// de una sola linea de C:
///
/// ```c
///    angle2 = (angle2 + ANG90) >> ANGLETOFINESHIFT;   /* >> 19 */
/// ```
///
/// `angle2` es `angle_t`, o sea `unsigned int`. **Un `unsigned int` desplazado 19
/// bits no puede pasar de 8191**, porque 0xFFFFFFFF >> 19 = 8191. Y salio 9215.
///
/// 9215 << 19 = 0x1_1FE0_0000: **treinta y tres bits**. O sea que la suma
/// `angle2 + ANG90` se hizo en 64 bits y NO se recorto a 32 antes de desplazar.
///
/// # Por que esto es peor que un fallo
///
/// La aritmetica de angulos de DOOM ES envolvente: sumar 270 grados a 180 tiene
/// que dar 90, y eso es exactamente el acarreo que se pierde. Con ella rota,
/// `viewangletox[9215]` lee **fuera de un array de 4096**, y devuelve un numero
/// que no es una columna de pantalla. De ahi `x1=7 x2=0` --al reves-- y de ahi
/// el `Bad R_RenderWallRange` que lleva una semana matando la partida.
///
/// Un desbordamiento que no envuelve no da un error: da otro numero. Es la
/// sexta vez este mes que un ANCHO mal elegido contesta en silencio.
#[test]
fn un_unsigned_int_envuelve_a_32_bits() {
    let out = run_c(
        "int main() {\n\
        \x20 unsigned int a; unsigned int b; unsigned int c;\n\
        \x20 a = 4160749568;\n\
        \x20 b = 1073741824;\n\
        \x20 c = a + b;\n\
        \x20 printf(\"%u %u\n\", c, c >> 19);\n\
        \x20 return 0;\n\
        }",
    );
    // 0xF8000000 + 0x40000000 = 0x138000000, que en 32 bits es 0x38000000
    // (939524096), y desplazado 19 son 1792. Si NO envuelve salen 5234491392
    // y 9984 -- los dos con mas de 32 bits, como el 9215 del Ryzen.
    assert_eq!(
        out.trim(),
        "939524096 1792",
        "la suma de dos `unsigned int` no envolvio a 32 bits"
    );
}
/// **Y LA MISMA SUMA SIN GUARDARLA NO ENVUELVE.** Este es el bug.
///
/// La de arriba pasa porque el resultado se GUARDA en un `unsigned int`, y
/// guardar en 32 bits trunca. DOOM no guarda -- lo hace en una sola expresion:
///
/// ```c
///    angle2 = (angle2 + ANG90) >> ANGLETOFINESHIFT;
/// ```
///
/// ** El `>>` ve el resultado de la suma **todavia en 64 bits**, sin recortar.
/// Con `a = 0xDFE00000` la suma da `0x1_1FE0_0000` --treinta y tres bits-- y el
/// desplazamiento devuelve 9212 en vez de 1020.
///
/// El Ryzen imprimio **9215** el 04-09 con el angulo de verdad. Mismo sitio,
/// mismo tamano, un pixel de diferencia.
///
/// # Por que esto mataba la partida
///
/// Ese numero es el indice de `viewangletox[]`, que tiene **4096 entradas**.
/// 9212 lee fuera del array y devuelve algo que no es una columna. De ahi salio
/// `x1=7 x2=0` --al reves-- y de ahi el `Bad R_RenderWallRange` que lleva una
/// semana matando a DOOM.
///
/// [!] Y fijate en lo que NO fallaba: la suma. La suma esta bien. Lo que falta
/// es el RECORTE A 32 BITS entre ella y el siguiente operador, y solo se nota
/// cuando no hay una asignacion en medio que lo haga por accidente.
#[test]
fn la_misma_suma_sin_guardarla_tambien_envuelve() {
    let out = run_c(
        "int main() {
          unsigned int a;
          a = 3755999232;
          printf(\"%u\n\", (a + 1073741824) >> 19);
          return 0;
        }",
    );
    assert_eq!(
        out.trim(),
        "1020",
        "la suma de 32 bits no se recorto antes del `>>`: es el 9215 de DOOM"
    );
}
/// **EL MENOS UNARIO TAMBIEN ENVUELVE.** El agujero que dejo el barrido de ayer.
///
/// La misma cuenta escrita de dos maneras daba dos resultados distintos:
///
/// ```text
///    (0 - a) >> 19    1028                    correcto
///    (-a)    >> 19    18446744073709544452
/// ```
///
/// `Sub` se recortaba desde el 04-09 y `Neg` no, porque el juez de tipos no
/// tenia brazo para el menos unario y contestaba `None`. **Un arreglo que cubre
/// una forma de escribir la operacion y no la otra no es un arreglo: es una
/// coincidencia.**
///
/// [!] Y no es teorico en DOOM: `R_AddLine` hace `angle2 = -clipangle;`. Ahi se
/// salvaba de milagro --guardar en un `unsigned int` recorta-- pero el mismo
/// `-x` dentro de una expresion mas larga no se salva.
#[test]
fn el_menos_unario_envuelve_igual_que_la_resta() {
    let out = run_c(
        "int main() {
          unsigned int a;
          a = 3755999232;
          printf(\"%u %u\n\", (0 - a) >> 19, (-a) >> 19);
          return 0;
        }",
    );
    assert_eq!(
        out.trim(),
        "1028 1028",
        "`-a` y `0 - a` tienen que dar lo MISMO sobre un `unsigned int`"
    );
}

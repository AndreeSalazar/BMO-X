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

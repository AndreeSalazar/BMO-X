//! # THE ASSIGNMENT PROBE -- the shorthand forms
//!
//! ## The axis
//!
//! `x += 1` is not innocent sugar: **it is an lvalue read and written inside
//! one expression**, which forces a decision about how many times it is
//! evaluated. C is blunt about it -- `E1 op= E2` evaluates `E1` **exactly once**
//! (C11 6.5.16.2p3).
//!
//! The rest of the rows cover what travels with it: `++`/`--` on lvalues that
//! are not a plain variable, chained assignment, `static` locals, `sizeof` and
//! the comma operator. All of this shows up on every line of DOOM's playsim
//! (`sector->floorheight += speed`, `players[i].health -= damage`).
//!
//! ## ** ONE BROKEN, and it is left open on purpose
//!
//! ```text
//!   int g[4]; int i = 1;
//!   g[i++] += 7;      ->  i ends at 3 and the 7 lands in g[2]
//!                         (should be: i at 2 and the 7 in g[1])
//! ```
//!
//! **The cause, with file and line**: `parser/mod.rs::sub_assign_op` builds the
//! tree by cloning the index --one copy for the read and one for the write--
//! so `g[i++] += 7` ends up being `g[i++] = g[i++] + 7`. The same goes for
//! `field_assign_op`, `arrow_assign_op` and `idxptr_assign_op`.
//!
//! ## Why it is left BROKEN instead of being fixed now
//!
//! **Because it is MEASURED not to block DOOM**: the tree has 10 indices with
//! `++` inside and **none** combined with a compound assignment
//! (`grep -E "\[[a-z_]+\+\+\]\s*[-+*/|&^]="` returns not one line).
//!
//! And fixing it is not one line: the lvalue's ADDRESS has to be evaluated once
//! and operated on, which means a new AST node, six parser arms and their
//! codegen. That is the size of the whole codegen split, with regression risk,
//! for a defect that cannot fire today.
//!
//! [!] A `BROKEN` with its exact symptom beside it is more useful than a line
//! in a `TODO`, and **the suite stays green because the census tells the
//! truth**. The day somebody writes `a[i++] += x` in BMO C, this row explains
//! what happens without debugging anything.

use super::census::{sweep, Cell};

fn census() -> Vec<Cell> {
    vec![
        Cell {
            name: "+= -= *= on a local",
            source: "int main() { int a; a = 10; a += 5; a -= 3; a *= 2; \
                       printf(\"%d\\n\", a); return 0; }",
            expects: "24",
        },
        Cell {
            name: "/= and %=",
            source: "int main() { int a; int b; a = 100; a /= 7; b = 100; b %= 7; \
                       printf(\"%d %d\\n\", a, b); return 0; }",
            expects: "14 2",
        },
        Cell {
            // [!] THIRD time this session the census caught MY arithmetic:
            // 252 was written and it answered 254. `0xF0 |0x0C` = 0xFC,
            // `&0xFE` = 0xFC, `^0x02` = **0xFE** = 254. Three for three --
            // eyeballing bit arithmetic is not trustworthy, and comparing a
            // whole report is what makes an error by the person WRITING the
            // test visible instead of letting it slip through.
            name: "&= |= ^= (DOOM's masks)",
            source: "int main() { int f; f = 0xF0; f |= 0x0C; f &= 0xFE; f ^= 0x02; \
                       printf(\"%d\\n\", f); return 0; }",
            expects: "254",
        },
        Cell {
            name: "<<= and >>= shift in place",
            source: "int main() { int a; a = 3; a <<= 16; a >>= 8; \
                       printf(\"%d\\n\", a); return 0; }",
            expects: "768",
        },
        Cell {
            // The shape of `sector->floorheight += speed` in `p_floor.c`.
            name: "+= on p->field",
            source: "typedef struct { int h; int v; } sec_t;\n\
                     int main() { sec_t s; sec_t *p; s.h = 100; s.v = 0; p = &s; \
                       p->h += 8; p->v -= 3; \
                       printf(\"%d %d\\n\", p->h, p->v); return 0; }",
            expects: "108 -3",
        },
        Cell {
            // `players[i].health -= damage`
            name: "+= on a[i].field",
            source: "typedef struct { int hp; } jug_t;\n\
                     jug_t t[4];\n\
                     int main() { t[2].hp = 100; t[2].hp -= 35; \
                       printf(\"%d\\n\", t[2].hp); return 0; }",
            expects: "65",
        },
        Cell {
            name: "+= on a global array",
            source: "int g[4] = {1,2,3,4};\n\
                     int main() { g[1] += 10; g[3] *= 5; \
                       printf(\"%d %d\\n\", g[1], g[3]); return 0; }",
            expects: "12 20",
        },
        Cell {
            // ** The index with a SIDE EFFECT. An `a[i++] += 1` expanded into
            // `a[i++] = a[i++] + 1` steps `i` TWICE and writes into the wrong
            // slot.
            name: "a[i++] += 1 steps i ONCE",
            source: "int main() { int g[4]; int i; \
                       g[0]=0; g[1]=0; g[2]=0; g[3]=0; i = 1; \
                       g[i++] += 7; \
                       printf(\"%d %d %d %d\\n\", i, g[0], g[1], g[2]); return 0; }",
            expects: "2 0 7 0",
        },
        Cell {
            name: "++ and -- on p->field",
            source: "typedef struct { int n; } c_t;\n\
                     int main() { c_t s; c_t *p; s.n = 5; p = &s; \
                       p->n++; p->n++; p->n--; \
                       printf(\"%d\\n\", p->n); return 0; }",
            expects: "6",
        },
        Cell {
            name: "++ on a[i]",
            source: "int g[3] = {10,20,30};\n\
                     int main() { g[1]++; ++g[2]; printf(\"%d %d\\n\", g[1], g[2]); return 0; }",
            expects: "21 31",
        },
        Cell {
            // Post against pre: `x++` returns the OLD value.
            name: "post and pre differ",
            source: "int main() { int i; int a; int b; i = 5; a = i++; b = ++i; \
                       printf(\"%d %d %d\\n\", a, b, i); return 0; }",
            expects: "5 7 7",
        },
        Cell {
            name: "chained assign a = b = c",
            source: "int main() { int a; int b; int c; a = b = c = 9; \
                       printf(\"%d %d %d\\n\", a, b, c); return 0; }",
            expects: "9 9 9",
        },
        Cell {
            // `static` inside a function: survives between calls and is
            // initialised ONCE. DOOM uses it in the renderer and the menu.
            name: "static local survives",
            source: "int cuenta(void) { static int n = 0; n = n + 1; return n; }\n\
                     int main() { cuenta(); cuenta(); printf(\"%d\\n\", cuenta()); return 0; }",
            expects: "3",
        },
        Cell {
            name: "sizeof of type and of expr",
            source: "typedef struct { int a; char b; } p_t;\n\
                     int main() { p_t s; int v[10]; \
                       printf(\"%d %d %d %d\\n\", (int)sizeof(int), (int)sizeof(p_t), \
                         (int)sizeof(s), (int)sizeof(v)); return 0; }",
            expects: "4 8 8 40",
        },
        Cell {
            // `Z_Malloc(sizeof(texture_t) + sizeof(texpatch_t)*(n-1), ...)`
            name: "sizeof inside arithmetic",
            source: "typedef struct { short a; short b; int c; } t_t;\n\
                     int main() { int n; n = 4; \
                       printf(\"%d\\n\", (int)(sizeof(t_t) * (n - 1))); return 0; }",
            expects: "24",
        },
        Cell {
            // The comma operator, which shows up in DOOM's `for` loops.
            name: "the comma operator in a for",
            source: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0, j = 10; i < j; i++, j--) { n = n + 1; } \
                       printf(\"%d %d %d\\n\", i, j, n); return 0; }",
            expects: "5 5 5",
        },
    ]
}

#[test]
fn the_assignment_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DE LA ASIGNACION CAMBIO.\n\
         [!] Este censo tiene UNA fila en ROTO a proposito y documentada arriba\n\
         (`a[i++] += 1`). Si se ha puesto en GOOD, alguien arreglo la doble\n\
         evaluacion del lvalue: **actualiza el censo y borra el parrafo de la\n\
         cabecera**, que si no queda un documento que miente.",
    );
}

/// **EL CENSUS DE LA ASIGNACION, al 2026-08-13.**
///
/// Quince de dieciseis. La que falta es la doble evaluacion del lvalue en una
/// asignacion compuesta, explicada en la cabecera: esta abierta a proposito
/// porque **esta medido que DOOM no la usa** y arreglarla es un nodo de AST
/// nuevo mas seis brazos del parser.
const CENSUS: &str = "\
+= -= *= on a local            GOOD
/= and %=                      GOOD
&= |= ^= (DOOM's masks)        GOOD
<<= and >>= shift in place     GOOD
+= on p->field                 GOOD
+= on a[i].field               GOOD
+= on a global array           GOOD
a[i++] += 1 steps i ONCE       BROKEN gives \"3 0 0 7\", wants \"2 0 7 0\"
++ and -- on p->field          GOOD
++ on a[i]                     GOOD
post and pre differ            GOOD
chained assign a = b = c       GOOD
static local survives          GOOD
sizeof of type and of expr     GOOD
sizeof inside arithmetic       GOOD
the comma operator in a for    GOOD
";

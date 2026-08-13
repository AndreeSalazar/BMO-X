//! # THE TABLE PROBE -- the data a program carries built in
//!
//! ## The axis
//!
//! Not *"can an array be read"* --`probe_language` covers that-- but **whether
//! the bytes the compiler puts in the `.bex` are the ones the source said**. A
//! global with initialisers is not computed at startup: it is emitted, and if
//! the emitter truncates, shifts, or skips a relocation, the program reads
//! perfectly valid numbers that are not its own.
//!
//! It is the hardest class of failure to see, and this house has paid for it:
//! the raycaster's map **never existed** --`char *mapa = "1111..."` evaluated to
//! ZERO-- and the walls being drawn were the machine code of the program
//! itself. Nobody noticed, because a raycaster drawing walls out of arbitrary
//! bytes still draws walls.
//!
//! ## Where the rows come from
//!
//! From `tables.c` and `info.c`, which are 6,889 lines of hand-written data:
//!
//! | DOOM's | the row |
//! |---|---|
//! | `const int finesine[10240]` | a genuinely long table (512 and 4096) |
//! | `const fixed_t *finecosine = &finesine[FINEANGLES/4]` | global pointer to `&table[k]` |
//! | `const byte gammatable[5][256]` | two-dimensional global |
//! | `char *sprnames[]` | global table of strings |
//! | `mobjinfo_t mobjinfo[NUMMOBJTYPES]` | global array of structs |
//!
//! ** The one most worth hunting is `finecosine`: a **global** pointer
//! initialised to the address of an element **in the middle** of another
//! global. That is a relocation with an addend, and a different shape from the
//! ones already solved (`char *p = "x"` and pointer tables). If it were zero,
//! or pointed at the start, the game's cosine would be its sine.
//!
//! ## ** First sweep: nothing broken
//!
//! All 11 green first time, `finecosine` included. The axis is clean, and that
//! is what you want to know when `R_Init` fails: **do not start here**.

use super::census::{sweep, Cell};

/// The long table is generated: pasting 512 numbers into the source would be
/// unreadable, and the size could not be changed to bisect with.
fn long_table(n: usize) -> &'static str {
    let mut nums = String::new();
    for i in 0..n {
        if i > 0 {
            nums.push(',');
        }
        nums.push_str(&i.to_string());
    }
    let src = format!(
        "const int t[{n}] = {{{nums}}};\n\
         int main() {{ printf(\"%d %d %d\\n\", t[0], t[{}], t[{}]); return 0; }}",
        n - 1,
        n / 2
    );
    Box::leak(src.into_boxed_str())
}

fn census() -> Vec<Cell> {
    vec![
        Cell {
            name: "global const int, read 3 spots",
            source: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     int main() { printf(\"%d %d %d\\n\", t[0], t[4], t[7]); return 0; }",
            expects: "10 50 80",
        },
        Cell {
            // The shape of `finesine[10240]`: a genuinely long table. If the
            // global emitter truncates or shifts, the last entry shows it.
            name: "512 table: first, last, middle",
            source: long_table(512),
            expects: "0 511 256",
        },
        Cell {
            name: "4096 table (finetangent's size)",
            source: long_table(4096),
            expects: "0 4095 2048",
        },
        Cell {
            // ** `const fixed_t *finecosine = &finesine[FINEANGLES/4];`
            // A GLOBAL pointer initialised to the address of an element in the
            // MIDDLE of another global. That is a relocation with an addend.
            name: "global pointer to &table[k]",
            source: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     const int *medio = &t[4];\n\
                     int main() { printf(\"%d %d\\n\", medio[0], medio[3]); return 0; }",
            expects: "50 80",
        },
        Cell {
            name: "global pointer to &table[0]",
            source: "const int t[4] = {7,8,9,10};\n\
                     const int *p = &t[0];\n\
                     int main() { printf(\"%d\\n\", p[2]); return 0; }",
            expects: "9",
        },
        Cell {
            name: "global pointer to the bare array",
            source: "const int t[4] = {7,8,9,10};\n\
                     const int *p = t;\n\
                     int main() { printf(\"%d\\n\", p[2]); return 0; }",
            expects: "9",
        },
        Cell {
            // `gammatable[5][256]`: a two-dimensional global with data.
            name: "2D global with initialisers",
            source: "const int g[3][4] = { {1,2,3,4}, {5,6,7,8}, {9,10,11,12} };\n\
                     int main() { printf(\"%d %d %d\\n\", g[0][0], g[1][2], g[2][3]); return 0; }",
            expects: "1 7 12",
        },
        Cell {
            // `char *sprnames[]`: a table of string pointers.
            name: "global table of strings",
            source: "char *n[4] = {\"TROO\",\"SHTG\",\"PUNG\",\"PISG\"};\n\
                     int main() { printf(\"%s %s\\n\", n[0], n[3]); return 0; }",
            expects: "TROO PISG",
        },
        Cell {
            // `mobjinfo[NUMMOBJTYPES]`: global array of structs with initialisers.
            name: "global array of structs",
            source: "typedef struct { int id; char *nom; int hp; } info_t;\n\
                     info_t tabla[3] = { {1,\"posesso\",20}, {2,\"imp\",60}, {3,\"baron\",1000} };\n\
                     int main() { printf(\"%d %s %d\\n\", tabla[1].id, tabla[1].nom, tabla[2].hp); return 0; }",
            expects: "2 imp 1000",
        },
        Cell {
            // With a short inside, which is how DOOM's are built.
            name: "global array of structs, short",
            source: "typedef struct { short w; short h; int ofs; } pt_t;\n\
                     pt_t t[3] = { {10,20,100}, {-30,40,200}, {50,-60,300} };\n\
                     int main() { printf(\"%d %d %d\\n\", (int)t[1].w, (int)t[2].h, t[2].ofs); return 0; }",
            expects: "-30 -60 300",
        },
        Cell {
            // `finecosine` arithmetic: a negative index on the shifted
            // pointer, which is what the renderer does.
            name: "negative index on a pointer",
            source: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     const int *medio = &t[4];\n\
                     int main() { printf(\"%d %d\\n\", medio[-1], medio[-4]); return 0; }",
            expects: "40 10",
        },
    ]
}

#[test]
fn the_table_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DE LAS TABLAS CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION.\n\
         Si la que cae es una de las de puntero global, el sospechoso es el\n\
         emisor de relocations (`SeccionAbs64`) y no el de datos.",
    );
}

/// **EL CENSUS DE LAS TABLAS, al 2026-08-13.** Verde entero desde el primer
/// barrido.
const CENSUS: &str = "\
global const int, read 3 spots GOOD
512 table: first, last, middle GOOD
4096 table (finetangent's size) GOOD
global pointer to &table[k]    GOOD
global pointer to &table[0]    GOOD
global pointer to the bare array GOOD
2D global with initialisers    GOOD
global table of strings        GOOD
global array of structs        GOOD
global array of structs, short GOOD
negative index on a pointer    GOOD
";

//! # THE LAYOUT PROBE -- where each field falls, and how big the whole is
//!
//! ## Why a second census instead of one more row in the first
//!
//! `probe_language` crosses two axes: CONTAINER x OPERATION. It answers *"can
//! this shape be read / written / addressed?"*, and its 28 cells came back
//! green.
//!
//! This is **a different axis**, and the first one's green cells say nothing
//! about it: a field can be read perfectly and still be at the wrong byte. No
//! number of `p->field` cells would ever have found what is below, because the
//! program reads **the field the compiler placed**, not the one the file has.
//!
//! ## And why this axis shows up NOW
//!
//! Because it is the one `R_Init` and `P_Init` are about to step on. DOOM does
//! not parse its data: it **casts a struct straight onto the raw bytes of the
//! WAD** and reads the fields. `r_data.c:569` is literally
//!
//! ```c
//! mtexture = (maptexture_t *) ( (byte *)maptex + offset );
//! texture->width = SHORT(mtexture->width);
//! ```
//!
//! So the compiler **has no freedom here**: there is one correct layout and it
//! is the file's, fixed since 1993. It is the only place in the language where
//! "compiles and does what it says" can be checked against an authority from
//! outside.
//!
//! ## ** What the first sweep found
//!
//! **Alignment was being derived from the member's SIZE** (`alineado_de(tam)`
//! in `bmo_abi::types::disposicion`), and that is false for anything that is
//! not a scalar: an array aligns like its ELEMENT. `char name[8]` is eight
//! bytes wide, same as a `long`, and aligns to **one**, not to eight.
//!
//! The consequences, with the WAD format's numbers beside them:
//!
//! | struct | what came out | what the disk says |
//! |---|---|---|
//! | `maptexture_t.patches` | offset 24 | **22** |
//! | `maplinedef_t` whole | 16 bytes | **14** |
//! | `mapsidedef_t.toptexture` | offset 8 | **4** |
//! | `mapnode_t` whole | 32 bytes | **28** |
//!
//! The two that are the SIZE are the worst: `p_setup.c` walks the lump as an
//! array, so one byte too many shifts **every record after the first**. The
//! level's first linedef would have come out fine.
//!
//! ## How to read a failure from here
//!
//! Every cell seeds known bytes with `memcpy` and reads the fields. A `BROKEN`
//! does not say "the field is wrong": it says **which number came out**, and
//! with the format table beside it that gives you the wrong offset without
//! debugging anything.

use super::census::{sweep, Cell};

/// What goes in front of the cells that seed bytes: a seeder that fills a
/// buffer with `0,1,2,3...` so **every byte announces its own offset**.
///
/// That is the whole idea of this file. If a `short` that should be at byte 10
/// answers `0x0D0C` instead of `0x0B0A`, the number **is** the offset it was
/// read from. Nothing needs disassembling.
const SEEDER: &str = "void sembrar(unsigned char *b, int n) { int i; \
                         for (i = 0; i < n; i++) { b[i] = (unsigned char)i; } }\n";

fn census() -> [Cell; 17] {
    [
        // == The minimum: that the alignment hole exists =================
        Cell {
            name: "char then int: there is a hole",
            source: "typedef struct { char c; int n; } h_t;\n\
                     int main() { h_t s; char *b; b = (char *)&s; \
                       printf(\"%d %d\\n\", (int)((char *)&s.n - b), (int)sizeof(h_t)); return 0; }",
            expects: "4 8",
        },
        // == ** AN ARRAY ALIGNS LIKE ITS ELEMENT =========================
        Cell {
            // The parent cell. `char t[8]` is 8 wide and aligns to 1: FLUSH.
            name: "char[8] sits flush, not at 8",
            source: "typedef struct { short a; char t[8]; short b; } s_t;\n\
                     int main() { s_t s; char *b; b = (char *)&s; \
                       printf(\"%d %d %d\\n\", (int)(s.t - b), \
                         (int)((char *)&s.b - b), (int)sizeof(s_t)); return 0; }",
            expects: "2 10 12",
        },
        Cell {
            name: "short[2] aligns to 2",
            source: "typedef struct { char c; short v[2]; } s_t;\n\
                     int main() { s_t s; \
                       printf(\"%d %d\\n\", (int)((char *)s.v - (char *)&s), \
                         (int)sizeof(s_t)); return 0; }",
            expects: "2 6",
        },
        Cell {
            // A struct of shorts aligns to 2 even though it is 10 wide,
            // which is not a power of two. Size cannot be the alignment.
            name: "a 10-byte struct aligns to 2",
            source: "typedef struct { short a; short b; short c; short d; short e; } diez_t;\n\
                     typedef struct { char c; diez_t d; } s_t;\n\
                     int main() { \
                       printf(\"%d %d %d\\n\", (int)sizeof(diez_t), \
                         (int)((char *)&((s_t *)0)->d - (char *)0), (int)sizeof(s_t)); return 0; }",
            expects: "10 2 12",
        },
        // == DOOM'S ON-DISK STRUCTS ======================================
        Cell {
            // `maptexture_t` from `r_data.c`. The field that matters is
            // `patches`: with alignment derived from size it landed at 24.
            name: "maptexture_t: patches at 22",
            source: "typedef struct { short ox; short oy; short p; short sd; short cm; } mappatch_t;\n\
                     typedef struct { char name[8]; int masked; short w; short h; \
                       int obsolete; short pc; mappatch_t patches[1]; } maptexture_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d %d %d\\n\", \
                         (int)((char *)&((maptexture_t *)z)->w - z), \
                         (int)((char *)&((maptexture_t *)z)->pc - z), \
                         (int)((char *)((maptexture_t *)z)->patches - z), \
                         (int)sizeof(mappatch_t)); return 0; }",
            expects: "12 20 22 10",
        },
        Cell {
            // ** THE ONE THAT HURTS MOST: the SIZE. `p_setup.c` walks the
            // lump as an array, so 16 instead of 14 shifts every linedef from
            // the second onwards -- and the first comes out fine, which is what
            // makes the symptom look like anything but this.
            name: "maplinedef_t is 14 bytes",
            source: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)((maplinedef_t *)z)->sidenum - z), \
                         (int)sizeof(maplinedef_t)); return 0; }",
            expects: "10 14",
        },
        Cell {
            name: "mapsidedef_t is 30 bytes",
            source: "typedef struct { short txo; short rwo; char top[8]; char bot[8]; \
                       char mid[8]; short sector; } mapsidedef_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d %d\\n\", \
                         (int)((char *)((mapsidedef_t *)z)->top - z), \
                         (int)((char *)&((mapsidedef_t *)z)->sector - z), \
                         (int)sizeof(mapsidedef_t)); return 0; }",
            expects: "4 28 30",
        },
        Cell {
            // The BSP node. Here the offsets came out right by luck and the
            // SIZE did not -- i.e. the whole tree shifted from the second node.
            name: "mapnode_t is 28 bytes",
            source: "typedef struct { short x; short y; short dx; short dy; \
                       short bbox[2][4]; unsigned short children[2]; } mapnode_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)((mapnode_t *)z)->children - z), \
                         (int)sizeof(mapnode_t)); return 0; }",
            expects: "24 28",
        },
        // == LOS CINCO QUE FALTABAN, y no faltaban por poco ==============
        //
        // `p_setup.c` divide la longitud del lump entre `sizeof(map*_t)` OCHO
        // veces, una por cada clase de dato de un nivel. Este eje solo media
        // TRES de las ocho. Los cinco de abajo entraron el 2026-08-14, el dia
        // que DOOM murio con `Z_CheckHeap` justo despues de `P_SetupLevel` --
        // o sea que las cinco filas que faltaban eran las del sitio donde
        // estaba muriendo.
        //
        // [!] Y el tamano importa el doble aqui: no solo desplaza los
        // registros a partir del segundo, es que **`numX` sale mal**, y ese
        // numero es el que se le pasa a `Z_Malloc` y el que gobierna el bucle
        // que lo llena.
        Cell {
            // El mas peligroso de los cinco: DOS `char[8]`, que es exactamente
            // la forma que se alineaba mal (un array se alinea como su
            // ELEMENTO). Con la regla vieja serian 32 en vez de 26.
            name: "mapsector_t is 26 bytes",
            source: "typedef struct { short fh; short ch; char fp[8]; char cp[8]; \
                       short light; short special; short tag; } mapsector_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d %d %d\\n\", \
                         (int)((char *)((mapsector_t *)z)->fp - z), \
                         (int)((char *)((mapsector_t *)z)->cp - z), \
                         (int)((char *)&((mapsector_t *)z)->tag - z), \
                         (int)sizeof(mapsector_t)); return 0; }",
            expects: "4 12 24 26",
        },
        Cell {
            name: "mapseg_t is 12 bytes",
            source: "typedef struct { short v1; short v2; short angle; short linedef; \
                       short side; short offset; } mapseg_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)&((mapseg_t *)z)->offset - z), \
                         (int)sizeof(mapseg_t)); return 0; }",
            expects: "10 12",
        },
        Cell {
            name: "mapthing_t is 10 bytes",
            source: "typedef struct { short x; short y; short angle; short type; \
                       short options; } mapthing_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)&((mapthing_t *)z)->options - z), \
                         (int)sizeof(mapthing_t)); return 0; }",
            expects: "8 10",
        },
        Cell {
            // Cuatro bytes, dos shorts. Parece imposible de fallar, y por eso
            // esta: si ESTE sale mal, lo que esta roto no es el alineado de
            // agregados sino algo mucho mas basico.
            name: "mapsubsector_t is 4 bytes",
            source: "typedef struct { short numsegs; short firstseg; } mapsubsector_t;\n\
                     int main() { printf(\"%d\\n\", (int)sizeof(mapsubsector_t)); return 0; }",
            expects: "4",
        },
        Cell {
            name: "mapvertex_t is 4 bytes",
            source: "typedef struct { short x; short y; } mapvertex_t;\n\
                     int main() { printf(\"%d\\n\", (int)sizeof(mapvertex_t)); return 0; }",
            expects: "4",
        },
        // == And that the disk bytes are actually read ===================
        Cell {
            // The offset being the right number is not enough: the field has
            // to be read. Bytes that announce their own offset are seeded, so
            // the value that comes out GIVES AWAY where it was read from.
            name: "read a short from byte 10",
            source: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     unsigned char crudo[32];\n\
                     int main() { maplinedef_t *l; int i; \
                       for (i = 0; i < 32; i++) { crudo[i] = (unsigned char)i; } \
                       l = (maplinedef_t *)crudo; \
                       printf(\"%d %d\\n\", (int)l->tag, (int)l->sidenum[0]); return 0; }",
            expects: "2312 2826",
        },
        Cell {
            // The array of on-disk records, which is how `p_setup.c` walks
            // them. With the size wrong, the SECOND one is already shifted.
            //
            // ** And this row earned its keep before it even existed: it was
            // written expecting `3598 7196` and the compiler answered
            // `3854 7452`, **which is correct** -- `l[1]` starts at byte 14, so
            // its `v1` is bytes 14 and 15: 14 + 15*256 = 3854. The census's
            // first BROKEN was the census. A seeder that makes every byte
            // announce its offset shows that at a glance; a row reading
            // `assert_eq!(x, 3598)` would have been "fixed" into agreement.
            name: "the 2nd record of the array",
            source: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     unsigned char crudo[64];\n\
                     int main() { maplinedef_t *l; int i; \
                       for (i = 0; i < 64; i++) { crudo[i] = (unsigned char)i; } \
                       l = (maplinedef_t *)crudo; \
                       printf(\"%d %d\\n\", (int)l[1].v1, (int)l[2].v1); return 0; }",
            expects: "3854 7452",
        },
        Cell {
            // `char name[8]` read as a string from raw bytes: how DOOM pulls
            // out a texture name and a flat name.
            name: "char name[8] from raw bytes",
            source: "typedef struct { char name[8]; int masked; short w; } cab_t;\n\
                     unsigned char crudo[32];\n\
                     int main() { cab_t *c; int i; \
                       for (i = 0; i < 32; i++) { crudo[i] = 0; } \
                       crudo[0] = 'S'; crudo[1] = 'T'; crudo[2] = 'A'; crudo[3] = 'R'; \
                       crudo[8] = 1; \
                       c = (cab_t *)crudo; \
                       printf(\"%s %d\\n\", c->name, c->masked); return 0; }",
            expects: "STAR 1",
        },
        Cell {
            // And the seeder, the helper above put to real use: it checks
            // that an `unsigned char` does not pick up a sign on its way
            // through `%d`, which is how WAD bytes get read.
            name: "unsigned char carries no sign",
            source: "void sembrar(unsigned char *b, int n) { int i; \
                       for (i = 0; i < n; i++) { b[i] = (unsigned char)(200 + i); } }\n\
                     unsigned char crudo[4];\n\
                     int main() { sembrar(crudo, 4); \
                       printf(\"%d %d\\n\", (int)crudo[0], (int)crudo[3]); return 0; }",
            expects: "200 203",
        },
    ]
}

#[test]
fn the_layout_census_has_not_changed() {
    let _ = SEEDER; // documents the idea; each cell carries its own inline
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DE LA DISPOSICION CAMBIO.\n\
         Los numeros de la derecha NO son una convencion de BMO: son el formato\n\
         del fichero WAD, que lleva fijo desde 1993. Si una casilla se puso en\n\
         ROJO, el compilador dejo de coincidir con el disco y DOOM leera el\n\
         campo de al lado -- **arregla la disposicion, no el censo**.\n\
         Actualiza `CENSUS` solo cuando se arregle un ROTO.",
    );
}

/// **EL CENSUS DE LA DISPOSICION, al 2026-08-13.**
///
/// Verde entero desde que `Disposicion::coloca` recibe el alineado en vez de
/// deducirlo del tamano.
///
/// ** Y el "antes" esta MEDIDO, no supuesto: se volvio a poner la regla vieja
/// a proposito y el barrido dio **NUEVE ROTAS de doce**. Escribir el censo
/// despues del arreglo deja una prueba que nunca ha visto fallar, que es una
/// prueba a medias. Lo que contesto la regla vieja, para que quede el numero:
///
/// ```text
///   char[8] va pegado, no al 8     da "8 16 24"     , wants "2 10 12"
///   short[2] se alinea a 2         da "4 8"         , wants "2 6"
///   struct de 10 B se alinea a 2   da "10 8 24"     , wants "10 2 12"
///   maptexture_t: patches en 22    da "12 20 24 10" , wants "12 20 22 10"
///   maplinedef_t mide 14           da "12 16"       , wants "10 14"
///   mapsidedef_t mide 30           da "8 32 40"     , wants "4 28 30"
///   mapnode_t mide 28              da "24 32"       , wants "24 28"
///   leer un short del byte 10      da "2312 3340"   , wants "2312 2826"
///   el 2o registro del array       da "4368 8480"   , wants "3854 7452"
/// ```
///
/// Las tres que sobrevivian son las que no llevan array dentro. Todo lo demas
/// --o sea todo lo que DOOM lee del WAD-- estaba corrido.
const CENSUS: &str = "\
char then int: there is a hole GOOD
char[8] sits flush, not at 8   GOOD
short[2] aligns to 2           GOOD
a 10-byte struct aligns to 2   GOOD
maptexture_t: patches at 22    GOOD
maplinedef_t is 14 bytes       GOOD
mapsidedef_t is 30 bytes       GOOD
mapnode_t is 28 bytes          GOOD
mapsector_t is 26 bytes        GOOD
mapseg_t is 12 bytes           GOOD
mapthing_t is 10 bytes         GOOD
mapsubsector_t is 4 bytes      GOOD
mapvertex_t is 4 bytes         GOOD
read a short from byte 10      GOOD
the 2nd record of the array    GOOD
char name[8] from raw bytes    GOOD
unsigned char carries no sign  GOOD
";

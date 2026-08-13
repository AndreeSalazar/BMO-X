//! # THE LANGUAGE PROBE -- the census of what BMO C can actually do
//!
//! ## Why it exists, and it is a decision about METHOD
//!
//! On 2026-08-13 three codegen defects fell in one day, and all three were
//! found the same way: flash, watch DOOM die, hunt the bug, flash again. Four
//! round trips. And all three were the same shape --a container, an operation,
//! and a codegen arm that did not cover that cell--:
//!
//! | | |
//! |---|---|
//! | `&c->defaults[i]` | evaluated to ZERO |
//! | `p + 1` on a `struct T *` | advanced ONE byte |
//! | `p++` on any pointer | advanced ONE byte |
//!
//! Chasing them one at a time with reboots does not scale. **They can be
//! enumerated.**
//!
//! ## And why it is a MATRIX rather than a list of cases
//!
//! [!] A probe written freehand tests **whatever occurred to whoever wrote
//! it** -- that is house pattern 15, already paid for once with `c/sonda.bex`:
//! *"the probe was written by the same side that wrote the defences"*.
//!
//! So this is not a list of ideas: it is the product of two axes that enumerate
//! themselves.
//!
//! ```text
//!   CONTAINER   variable . array . field via `.` . field via `->`
//!               pointer subscript . array of structs . double pointer
//!
//!   OPERATION   read . write . take the address . ++ . -- . + n
//!               call (when the contents are a function pointer)
//! ```
//!
//! The cells come out of the cross product, not out of inspiration. If a new
//! container turns up tomorrow, you add the row and its cells fall out.
//!
//! ## The harness does not live here any more
//!
//! The forty lines that sweep and compare moved to `census.rs` when the second
//! axis appeared (`probe_layout`). This file keeps the only thing that is its
//! own: **the cells and the census**.

//! ## El arnes ya no vive aqui
//!
//! Las cuarenta lineas que barren y comparan estan en `censo.rs` desde que
//! aparecio el segundo eje (`sonda_de_disposicion`). Este fichero se queda con
//! lo unico que es suyo: **las casillas y el censo**.

use super::census::{sweep, Cell};

fn census() -> [Cell; 28] {
    [
        // -- The simplest container: a variable --------------------------
        Cell {
            name: "read and write a local",
            source: "int main() { int a; a = 7; a = a + 1; printf(\"%d\\n\", a); return 0; }",
            expects: "8",
        },
        Cell {
            name: "&local, and write through it",
            source: "void pon(int *d, int v) { *d = v; }\n\
                     int main() { int a; a = 0; pon(&a, 9); printf(\"%d\\n\", a); return 0; }",
            expects: "9",
        },
        Cell {
            name: "an int ++ steps by ONE",
            source: "int main() { int i; i = 5; i++; ++i; printf(\"%d\\n\", i); return 0; }",
            expects: "7",
        },
        // -- Array ------------------------------------------------------
        Cell {
            name: "read and write a global array",
            source: "int g[4] = {1,2,3,4};\n\
                     int main() { g[2] = 30; printf(\"%d %d\\n\", g[0], g[2]); return 0; }",
            expects: "1 30",
        },
        Cell {
            name: "&global[i]",
            source: "int g[4] = {1,2,3,4};\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { pon(&g[1], 20); printf(\"%d\\n\", g[1]); return 0; }",
            expects: "20",
        },
        // -- Field via dot -----------------------------------------------
        Cell {
            name: "read and write s.field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     int main() { par_t s; s.a = 1; s.b = 2; s.b = s.b + 10; \
                       printf(\"%d %d\\n\", s.a, s.b); return 0; }",
            expects: "1 12",
        },
        Cell {
            name: "&s.field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { par_t s; s.a = 0; s.b = 0; pon(&s.b, 5); \
                       printf(\"%d %d\\n\", s.a, s.b); return 0; }",
            expects: "0 5",
        },
        // -- Field via arrow ---------------------------------------------
        Cell {
            name: "read and write p->field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     int main() { par_t s; par_t *p; s.a = 1; s.b = 2; p = &s; \
                       p->b = p->b + 10; printf(\"%d %d\\n\", p->a, p->b); return 0; }",
            expects: "1 12",
        },
        Cell {
            name: "&p->field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { par_t s; par_t *p; s.b = 0; p = &s; pon(&p->b, 7); \
                       printf(\"%d\\n\", s.b); return 0; }",
            expects: "7",
        },
        // -- Pointer subscript ---------------------------------------------
        Cell {
            name: "read and write p[i]",
            source: "int g[4] = {1,2,3,4};\n\
                     int main() { int *p; p = g; p[2] = 30; printf(\"%d %d\\n\", p[0], p[2]); return 0; }",
            expects: "1 30",
        },
        Cell {
            name: "&p[i]",
            source: "int g[4] = {1,2,3,4};\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { int *p; p = g; pon(&p[3], 40); printf(\"%d\\n\", g[3]); return 0; }",
            expects: "40",
        },
        Cell {
            name: "&c->field[i]  (DOOM's one)",
            source: "typedef struct { char *name; int v; } def_t;\n\
                     typedef struct { def_t *tabla; int n; } col_t;\n\
                     def_t lista[3] = { {\"a\",10}, {\"b\",20}, {\"c\",30} };\n\
                     col_t col = { lista, 3 };\n\
                     def_t *dame(col_t *c, int i) { return &c->tabla[i]; }\n\
                     int main() { def_t *r; r = dame(&col, 1); \
                       if (r == 0) { printf(\"NULO\\n\"); } \
                       else { printf(\"%s %d\\n\", r->name, r->v); } return 0; }",
            expects: "b 20",
        },
        // -- Array of structs ----------------------------------------------
        Cell {
            name: "read and write a[i].field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     int main() { t[1].a = 4; t[1].b = 5; \
                       printf(\"%d %d\\n\", t[1].a, t[1].b); return 0; }",
            expects: "4 5",
        },
        Cell {
            name: "&a[i].field",
            source: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     void pon(int *d, int v) { *d = v; }\n\
                     int main() { t[2].b = 0; pon(&t[2].b, 6); printf(\"%d\\n\", t[2].b); return 0; }",
            expects: "6",
        },
        // -- Pointer arithmetic ----------------------------------------------
        Cell {
            name: "p + n on an int*",
            source: "int g[4] = {1,2,3,4};\n\
                     int main() { int *p; p = g; printf(\"%d\\n\", *(p + 2)); return 0; }",
            expects: "3",
        },
        Cell {
            name: "p + n on a struct*",
            source: "typedef struct { int a; int b; } par_t;\n\
                     par_t t[3];\n\
                     int main() { par_t *p; t[2].a = 9; p = t; p = p + 2; \
                       printf(\"%d %d\\n\", p->a, (int)(p == &t[2])); return 0; }",
            expects: "9 1",
        },
        Cell {
            name: "p++ and p-- on an int*",
            source: "int g[4] = {10,20,30,40};\n\
                     int main() { int *p; p = g; p++; p++; printf(\"%d \", *p); \
                       p--; printf(\"%d\\n\", *p); return 0; }",
            expects: "30 20",
        },
        Cell {
            name: "*p++  (the va_arg macro)",
            source: "int g[3] = {11,22,33};\n\
                     int main() { int *p; p = g; \
                       printf(\"%d %d %d\\n\", *p++, *p++, *p++); return 0; }",
            expects: "11 22 33",
        },
        Cell {
            name: "subtract two pointers",
            source: "int g[8];\n\
                     int main() { int *a; int *b; a = g; b = g + 5; \
                       printf(\"%d\\n\", (int)(b - a)); return 0; }",
            expects: "5",
        },
        Cell {
            name: "walk to the sentinel",
            source: "char *s = \"hola\";\n\
                     int main() { char *p; int n; p = s; n = 0; \
                       while (*p) { n = n + 1; p++; } printf(\"%d\\n\", n); return 0; }",
            expects: "4",
        },
        // -- Function pointers, all four containers -------------------------
        Cell {
            name: "call through a variable",
            source: "int doble(int x) { return x * 2; }\n\
                     int main() { int (*f)(int); f = doble; printf(\"%d\\n\", f(21)); return 0; }",
            expects: "42",
        },
        Cell {
            name: "call through a global table",
            source: "int doble(int x) { return x * 2; }\n\
                     int triple(int x) { return x * 3; }\n\
                     int (*tabla[2])(int) = { doble, triple };\n\
                     int main() { printf(\"%d %d\\n\", tabla[0](5), tabla[1](5)); return 0; }",
            expects: "10 15",
        },
        Cell {
            name: "call through s.field",
            source: "typedef struct { int (*fn)(int); } caja_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { caja_t c; c.fn = doble; printf(\"%d\\n\", c.fn(8)); return 0; }",
            expects: "16",
        },
        Cell {
            // ** LA DE DOOM EN `W_AddFile`: `wad_file->file_class->Read(...)`.
            name: "call through p->field",
            source: "typedef struct { int (*fn)(int); } caja_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { caja_t c; caja_t *p; c.fn = doble; p = &c; \
                       printf(\"%d\\n\", p->fn(8)); return 0; }",
            expects: "16",
        },
        Cell {
            // Y la de DOS saltos, que es literalmente la forma de DOOM.
            name: "call through p->other->field",
            source: "typedef struct { int (*fn)(int); } clase_t;\n\
                     typedef struct { clase_t *clase; int n; } obj_t;\n\
                     int doble(int x) { return x * 2; }\n\
                     int main() { clase_t k; obj_t o; obj_t *p; \
                       k.fn = doble; o.clase = &k; p = &o; \
                       printf(\"%d\\n\", p->clase->fn(8)); return 0; }",
            expects: "16",
        },
        Cell {
            // ** LA FORMA DE `w_file.c`: un struct GLOBAL cuyos campos son
            // punteros a funcion, inicializados estaticamente.
            //
            // No es lo mismo que un ARRAY global de punteros a funcion --la
            // casilla de arriba-- y por eso son dos: un array tiene un solo
            // tipo de elemento y una sola relocation por hueco; un struct
            // mezcla campos de clases distintas y sus punteros van a la seccion
            // de CODIGO, no a la de datos.
            name: "global struct of fn ptrs",
            source: "typedef struct { int (*abre)(int); int (*lee)(int); } clase_t;
                     int doble(int x) { return x * 2; }
                     int triple(int x) { return x * 3; }
                     clase_t global = { doble, triple };
                     int main() { clase_t *p; p = &global;                        printf(\"%d %d\n\", global.abre(4), p->lee(4)); return 0; }",
            expects: "8 12",
        },
        Cell {
            // Y la de DOOM entera: el objeto lleva un puntero a su CLASE, que
            // es un struct global de punteros a funcion. `wad->clase->lee(..)`.
            name: "obj->class->fn() with global class",
            source: "typedef struct { int (*lee)(int); } clase_t;
                     int doble(int x) { return x * 2; }
                     clase_t la_clase = { doble };
                     typedef struct { clase_t *clase; int n; } obj_t;
                     int main() { obj_t o; obj_t *p; o.clase = &la_clase; o.n = 1; p = &o;                        printf(\"%d\n\", p->clase->lee(21)); return 0; }",
            expects: "42",
        },
        // -- Double pointer --------------------------------------------------
        Cell {
            name: "double pointer: **pp",
            source: "int main() { int a; int *p; int **pp; a = 3; p = &a; pp = &p; \
                       **pp = 4; printf(\"%d\\n\", a); return 0; }",
            expects: "4",
        },
    ]
}

/// El barrido entero, en una ejecucion. El arnes esta en `censo.rs`.
#[test]
fn the_language_census_has_not_changed() {
    sweep(
        &census(),
        CENSUS,
        "EL CENSUS DEL LENGUAJE CAMBIO.\n\
         Si se arreglo una casilla o se rompio otra, **actualiza la constante\n\
         `CENSUS` de este fichero**: es el sitio donde esta escrito que soporta\n\
         BMO C, y un censo que no se actualiza es justo el documento que miente.",
    );
}

/// **EL CENSUS, al 2026-08-13.**
///
/// Esto no es la lista de deseos: es lo que la maquina contesto la ultima vez
/// que alguien corrio el barrido. Cada `ROTO` es un defecto abierto **con su
/// sintoma exacto al lado**, que es mas util que una fila en un `TODO`.
const CENSUS: &str = "\
read and write a local         GOOD
&local, and write through it   GOOD
an int ++ steps by ONE         GOOD
read and write a global array  GOOD
&global[i]                     GOOD
read and write s.field         GOOD
&s.field                       GOOD
read and write p->field        GOOD
&p->field                      GOOD
read and write p[i]            GOOD
&p[i]                          GOOD
&c->field[i]  (DOOM's one)     GOOD
read and write a[i].field      GOOD
&a[i].field                    GOOD
p + n on an int*               GOOD
p + n on a struct*             GOOD
p++ and p-- on an int*         GOOD
*p++  (the va_arg macro)       GOOD
subtract two pointers          GOOD
walk to the sentinel           GOOD
call through a variable        GOOD
call through a global table    GOOD
call through s.field           GOOD
call through p->field          GOOD
call through p->other->field   GOOD
global struct of fn ptrs       GOOD
obj->class->fn() with global class GOOD
double pointer: **pp           GOOD
";

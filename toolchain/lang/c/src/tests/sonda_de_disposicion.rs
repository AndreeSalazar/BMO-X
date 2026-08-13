//! # LA SONDA DE LA DISPOSICION -- donde cae cada campo, y cuanto mide el todo
//!
//! ## Por que hay un segundo censo y no una fila mas en el primero
//!
//! `sonda_del_lenguaje` cruza dos ejes: CONTENEDOR x OPERACION. Contesta
//! *"se puede leer/escribir/direccionar esta forma?"*, y las 28 casillas
//! salieron verdes.
//!
//! Esto es **otro eje**, y las casillas verdes del primero no dicen nada de
//! el: un campo se puede leer perfectamente y estar en el byte equivocado.
//! Ninguna cantidad de casillas de `p->campo` habria encontrado lo de abajo,
//! porque el programa lee **el campo que el compilador coloco**, no el que el
//! fichero tiene.
//!
//! ## Y por que este eje aparece AHORA
//!
//! Porque es el que `R_Init` y `P_Init` van a pisar. DOOM no parsea sus datos:
//! **castea un struct encima de los bytes crudos del WAD** y lee los campos.
//! `r_data.c:569` es literalmente
//!
//! ```c
//! mtexture = (maptexture_t *) ( (byte *)maptex + offset );
//! texture->width = SHORT(mtexture->width);
//! ```
//!
//! O sea que aqui el compilador **no tiene libertad**: hay una disposicion
//! correcta y es la del fichero, que lleva fija desde 1993. Es el unico sitio
//! del lenguaje donde "compila y hace lo que dice" se puede comprobar contra
//! una autoridad de fuera.
//!
//! ## ** Lo que encontro el primer barrido
//!
//! **El alineado se deducia del TAMANO del miembro** (`alineado_de(tam)` en
//! `bmo_abi::types::disposicion`), y eso es falso para todo lo que no sea un
//! escalar: un array se alinea como su ELEMENTO. `char name[8]` mide ocho
//! bytes igual que un `long` y se alinea a **uno**, no a ocho.
//!
//! Consecuencia, con los numeros del formato WAD al lado:
//!
//! | struct | que salia | que dice el disco |
//! |---|---|---|
//! | `maptexture_t.patches` | offset 24 | **22** |
//! | `maplinedef_t` entero | 16 bytes | **14** |
//! | `mapsidedef_t.toptexture` | offset 8 | **4** |
//! | `mapnode_t` entero | 32 bytes | **28** |
//!
//! Los dos que son el TAMANO son los peores: `p_setup.c` recorre el lump como
//! un array, asi que un byte de mas corre **todos los registros a partir del
//! segundo**. El primer linedef del nivel habria salido bien.
//!
//! ## Como se lee un fallo de aqui
//!
//! Cada casilla siembra bytes conocidos con `memcpy` y lee los campos. Un
//! `ROTO` no dice "el campo esta mal": dice **que numero salio**, y con la
//! tabla del formato al lado eso da el offset equivocado sin depurar nada.

use super::censo::{barrer, Casilla};

/// Lo que se pone delante de las casillas que siembran bytes: un sembrador que
/// llena un buffer con `0,1,2,3...` para que **cada byte diga su propio
/// offset**.
///
/// Esa es toda la idea del fichero. Si un `short` que deberia estar en el byte
/// 10 contesta `0x0D0C` en vez de `0x0B0A`, el numero **es** el offset donde
/// se leyo. No hace falta desensamblar nada.
const SEMBRADOR: &str = "void sembrar(unsigned char *b, int n) { int i; \
                         for (i = 0; i < n; i++) { b[i] = (unsigned char)i; } }\n";

fn censo() -> [Casilla; 12] {
    [
        // == Lo minimo: que el hueco de alineado exista ==================
        Casilla {
            nombre: "char, luego int: hay hueco",
            fuente: "typedef struct { char c; int n; } h_t;\n\
                     int main() { h_t s; char *b; b = (char *)&s; \
                       printf(\"%d %d\\n\", (int)((char *)&s.n - b), (int)sizeof(h_t)); return 0; }",
            espera: "4 8",
        },
        // == ** EL ARRAY SE ALINEA COMO SU ELEMENTO =======================
        Casilla {
            // La casilla madre. `char t[8]` mide 8 y se alinea a 1: va PEGADO.
            nombre: "char[8] va pegado, no al 8",
            fuente: "typedef struct { short a; char t[8]; short b; } s_t;\n\
                     int main() { s_t s; char *b; b = (char *)&s; \
                       printf(\"%d %d %d\\n\", (int)(s.t - b), \
                         (int)((char *)&s.b - b), (int)sizeof(s_t)); return 0; }",
            espera: "2 10 12",
        },
        Casilla {
            nombre: "short[2] se alinea a 2",
            fuente: "typedef struct { char c; short v[2]; } s_t;\n\
                     int main() { s_t s; \
                       printf(\"%d %d\\n\", (int)((char *)s.v - (char *)&s), \
                         (int)sizeof(s_t)); return 0; }",
            espera: "2 6",
        },
        Casilla {
            // Un struct de shorts se alinea a 2 aunque mida 10, que no es
            // potencia de dos. El tamano no puede ser el alineado ni de lejos.
            nombre: "struct de 10 B se alinea a 2",
            fuente: "typedef struct { short a; short b; short c; short d; short e; } diez_t;\n\
                     typedef struct { char c; diez_t d; } s_t;\n\
                     int main() { \
                       printf(\"%d %d %d\\n\", (int)sizeof(diez_t), \
                         (int)((char *)&((s_t *)0)->d - (char *)0), (int)sizeof(s_t)); return 0; }",
            espera: "10 2 12",
        },
        // == LAS ESTRUCTURAS DE DISCO DE DOOM ============================
        Casilla {
            // `maptexture_t` de `r_data.c`. El campo que importa es `patches`:
            // con el alineado deducido del tamano caia en el 24.
            nombre: "maptexture_t: patches en 22",
            fuente: "typedef struct { short ox; short oy; short p; short sd; short cm; } mappatch_t;\n\
                     typedef struct { char name[8]; int masked; short w; short h; \
                       int obsolete; short pc; mappatch_t patches[1]; } maptexture_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d %d %d\\n\", \
                         (int)((char *)&((maptexture_t *)z)->w - z), \
                         (int)((char *)&((maptexture_t *)z)->pc - z), \
                         (int)((char *)((maptexture_t *)z)->patches - z), \
                         (int)sizeof(mappatch_t)); return 0; }",
            espera: "12 20 22 10",
        },
        Casilla {
            // ** LA QUE MAS DUELE: el TAMANO. `p_setup.c` recorre el lump como
            // un array, asi que 16 en vez de 14 corre todos los linedefs a
            // partir del segundo -- y el primero sale bien, que es lo que hace
            // que el sintoma parezca cualquier otra cosa.
            nombre: "maplinedef_t mide 14",
            fuente: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)((maplinedef_t *)z)->sidenum - z), \
                         (int)sizeof(maplinedef_t)); return 0; }",
            espera: "10 14",
        },
        Casilla {
            nombre: "mapsidedef_t mide 30",
            fuente: "typedef struct { short txo; short rwo; char top[8]; char bot[8]; \
                       char mid[8]; short sector; } mapsidedef_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d %d\\n\", \
                         (int)((char *)((mapsidedef_t *)z)->top - z), \
                         (int)((char *)&((mapsidedef_t *)z)->sector - z), \
                         (int)sizeof(mapsidedef_t)); return 0; }",
            espera: "4 28 30",
        },
        Casilla {
            // El nodo del BSP. Aqui los offsets salian bien por casualidad y el
            // TAMANO no -- o sea el arbol entero corrido desde el segundo nodo.
            nombre: "mapnode_t mide 28",
            fuente: "typedef struct { short x; short y; short dx; short dy; \
                       short bbox[2][4]; unsigned short children[2]; } mapnode_t;\n\
                     int main() { char *z; z = (char *)0; \
                       printf(\"%d %d\\n\", \
                         (int)((char *)((mapnode_t *)z)->children - z), \
                         (int)sizeof(mapnode_t)); return 0; }",
            espera: "24 28",
        },
        // == Y que los bytes del disco se lean de verdad =================
        Casilla {
            // No basta con que el offset sea el numero correcto: hay que leer
            // el campo. Se siembran bytes que dicen su propio offset, asi que
            // el valor que sale DELATA de donde se leyo.
            nombre: "leer un short del byte 10",
            fuente: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     unsigned char crudo[32];\n\
                     int main() { maplinedef_t *l; int i; \
                       for (i = 0; i < 32; i++) { crudo[i] = (unsigned char)i; } \
                       l = (maplinedef_t *)crudo; \
                       printf(\"%d %d\\n\", (int)l->tag, (int)l->sidenum[0]); return 0; }",
            espera: "2312 2826",
        },
        Casilla {
            // El array de registros de disco, que es como `p_setup.c` los
            // recorre. Con el tamano mal, el SEGUNDO ya viene corrido.
            //
            // ** Y esta fila ya se gano el sueldo antes de existir del todo: la
            // escribi esperando `3598 7196` y el compilador contesto
            // `3854 7452`, **que es lo correcto** -- `l[1]` empieza en el byte
            // 14, asi que su `v1` son los bytes 14 y 15: 14 + 15*256 = 3854.
            // El primer ROTO del censo era el censo. Un sembrador que hace que
            // cada byte diga su offset deja ver eso de un vistazo; una fila con
            // `assert_eq!(x, 3598)` lo habria dado por bueno al "arreglarlo".
            nombre: "el 2o registro del array",
            fuente: "typedef struct { short v1; short v2; short flags; short special; \
                       short tag; short sidenum[2]; } maplinedef_t;\n\
                     unsigned char crudo[64];\n\
                     int main() { maplinedef_t *l; int i; \
                       for (i = 0; i < 64; i++) { crudo[i] = (unsigned char)i; } \
                       l = (maplinedef_t *)crudo; \
                       printf(\"%d %d\\n\", (int)l[1].v1, (int)l[2].v1); return 0; }",
            espera: "3854 7452",
        },
        Casilla {
            // `char name[8]` leido como cadena desde bytes crudos: es como DOOM
            // saca el nombre de una textura y el de un flat.
            nombre: "char name[8] desde bytes",
            fuente: "typedef struct { char name[8]; int masked; short w; } cab_t;\n\
                     unsigned char crudo[32];\n\
                     int main() { cab_t *c; int i; \
                       for (i = 0; i < 32; i++) { crudo[i] = 0; } \
                       crudo[0] = 'S'; crudo[1] = 'T'; crudo[2] = 'A'; crudo[3] = 'R'; \
                       crudo[8] = 1; \
                       c = (cab_t *)crudo; \
                       printf(\"%s %d\\n\", c->name, c->masked); return 0; }",
            espera: "STAR 1",
        },
        Casilla {
            // Y el sembrador, que es el ayudante de arriba usado de verdad:
            // comprueba que un `unsigned char` no se lleva el signo puesto al
            // pasar por `%d`, que es como se leen los bytes de un WAD.
            nombre: "unsigned char no lleva signo",
            fuente: "void sembrar(unsigned char *b, int n) { int i; \
                       for (i = 0; i < n; i++) { b[i] = (unsigned char)(200 + i); } }\n\
                     unsigned char crudo[4];\n\
                     int main() { sembrar(crudo, 4); \
                       printf(\"%d %d\\n\", (int)crudo[0], (int)crudo[3]); return 0; }",
            espera: "200 203",
        },
    ]
}

#[test]
fn el_censo_de_la_disposicion_no_ha_cambiado() {
    let _ = SEMBRADOR; // documenta la idea; cada casilla lleva la suya inline
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DE LA DISPOSICION CAMBIO.\n\
         Los numeros de la derecha NO son una convencion de BMO: son el formato\n\
         del fichero WAD, que lleva fijo desde 1993. Si una casilla se puso en\n\
         ROJO, el compilador dejo de coincidir con el disco y DOOM leera el\n\
         campo de al lado -- **arregla la disposicion, no el censo**.\n\
         Actualiza `CENSO` solo cuando se arregle un ROTO.",
    );
}

/// **EL CENSO DE LA DISPOSICION, al 2026-08-13.**
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
///   char[8] va pegado, no al 8     da "8 16 24"      y toca "2 10 12"
///   short[2] se alinea a 2         da "4 8"          y toca "2 6"
///   struct de 10 B se alinea a 2   da "10 8 24"      y toca "10 2 12"
///   maptexture_t: patches en 22    da "12 20 24 10"  y toca "12 20 22 10"
///   maplinedef_t mide 14           da "12 16"        y toca "10 14"
///   mapsidedef_t mide 30           da "8 32 40"      y toca "4 28 30"
///   mapnode_t mide 28              da "24 32"        y toca "24 28"
///   leer un short del byte 10      da "2312 3340"    y toca "2312 2826"
///   el 2o registro del array       da "4368 8480"    y toca "3854 7452"
/// ```
///
/// Las tres que sobrevivian son las que no llevan array dentro. Todo lo demas
/// --o sea todo lo que DOOM lee del WAD-- estaba corrido.
const CENSO: &str = "\
char, luego int: hay hueco     BIEN
char[8] va pegado, no al 8     BIEN
short[2] se alinea a 2         BIEN
struct de 10 B se alinea a 2   BIEN
maptexture_t: patches en 22    BIEN
maplinedef_t mide 14           BIEN
mapsidedef_t mide 30           BIEN
mapnode_t mide 28              BIEN
leer un short del byte 10      BIEN
el 2o registro del array       BIEN
char name[8] desde bytes       BIEN
unsigned char no lleva signo   BIEN
";

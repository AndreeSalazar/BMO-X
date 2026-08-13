//! # LA SONDA DE LAS TABLAS -- los datos que el programa trae puestos
//!
//! ## El eje
//!
//! No es *"se puede leer un array"* --eso lo cubre `sonda_del_lenguaje`-- sino
//! **si los bytes que el compilador mete en el `.bex` son los que decia el
//! fuente**. Un global con iniciales no se calcula al arrancar: se emite, y si
//! el emisor trunca, se descoloca o se salta una relocation, el programa lee
//! numeros perfectamente validos que no son los suyos.
//!
//! Es la clase de fallo mas dificil de ver, y esta casa ya la pago: el mapa
//! del raycaster **nunca existio** --`char *mapa = "1111..."` valia CERO-- y
//! las paredes que salian eran el codigo maquina del propio programa. No se
//! noto porque un raycaster que dibuja paredes desde bytes cualesquiera sigue
//! dibujando paredes.
//!
//! ## De donde salen las filas
//!
//! De `tables.c` y `info.c`, que son 6.889 lineas de datos puestos a mano:
//!
//! | lo de DOOM | la fila |
//! |---|---|
//! | `const int finesine[10240]` | una tabla larga de verdad (512 y 4096) |
//! | `const fixed_t *finecosine = &finesine[FINEANGLES/4]` | puntero global a `&tabla[k]` |
//! | `const byte gammatable[5][256]` | global de dos dimensiones |
//! | `char *sprnames[]` | tabla global de cadenas |
//! | `mobjinfo_t mobjinfo[NUMMOBJTYPES]` | array global de structs |
//!
//! ** La que mas se buscaba es `finecosine`: un puntero **global** inicializado
//! a la direccion de un elemento **en medio** de otro global. Eso es una
//! relocation con addend, y es una forma distinta de las que ya estaban
//! resueltas (`char *p = "x"` y las tablas de punteros). Si valiera cero o
//! apuntara al principio, el coseno del juego seria el seno.
//!
//! ## ** Resultado del primer barrido: nada roto
//!
//! Las 11 verdes a la primera, `finecosine` incluida. El eje esta limpio, y eso
//! es lo que hay que saber cuando `R_Init` falle: **no empezar por aqui**.

use super::censo::{barrer, Casilla};

/// La tabla larga se genera: pegar 512 numeros a mano en el fuente seria
/// ilegible y ademas no se podria cambiar el tamano para bisecar.
fn tabla_larga(n: usize) -> &'static str {
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

fn censo() -> Vec<Casilla> {
    vec![
        Casilla {
            nombre: "global const int, leer 3 sitios",
            fuente: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     int main() { printf(\"%d %d %d\\n\", t[0], t[4], t[7]); return 0; }",
            espera: "10 50 80",
        },
        Casilla {
            // La forma de `finesine[10240]`: una tabla larga de verdad. Si el
            // emisor de globales trunca o se descoloca, se ve en el ultimo.
            nombre: "tabla de 512: 0, ultimo, medio",
            fuente: tabla_larga(512),
            espera: "0 511 256",
        },
        Casilla {
            nombre: "tabla de 4096 (la de finetangent)",
            fuente: tabla_larga(4096),
            espera: "0 4095 2048",
        },
        Casilla {
            // ** `const fixed_t *finecosine = &finesine[FINEANGLES/4];`
            // Un puntero GLOBAL inicializado a la direccion de un elemento en
            // MEDIO de otro global. Es una relocation con addend.
            nombre: "puntero global a &tabla[k]",
            fuente: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     const int *medio = &t[4];\n\
                     int main() { printf(\"%d %d\\n\", medio[0], medio[3]); return 0; }",
            espera: "50 80",
        },
        Casilla {
            nombre: "puntero global a &tabla[0]",
            fuente: "const int t[4] = {7,8,9,10};\n\
                     const int *p = &t[0];\n\
                     int main() { printf(\"%d\\n\", p[2]); return 0; }",
            espera: "9",
        },
        Casilla {
            nombre: "puntero global al array a secas",
            fuente: "const int t[4] = {7,8,9,10};\n\
                     const int *p = t;\n\
                     int main() { printf(\"%d\\n\", p[2]); return 0; }",
            espera: "9",
        },
        Casilla {
            // `gammatable[5][256]`: un global de dos dimensiones con datos.
            nombre: "global 2D con iniciales",
            fuente: "const int g[3][4] = { {1,2,3,4}, {5,6,7,8}, {9,10,11,12} };\n\
                     int main() { printf(\"%d %d %d\\n\", g[0][0], g[1][2], g[2][3]); return 0; }",
            espera: "1 7 12",
        },
        Casilla {
            // `char *sprnames[]`: tabla de punteros a cadena.
            nombre: "tabla global de cadenas",
            fuente: "char *n[4] = {\"TROO\",\"SHTG\",\"PUNG\",\"PISG\"};\n\
                     int main() { printf(\"%s %s\\n\", n[0], n[3]); return 0; }",
            espera: "TROO PISG",
        },
        Casilla {
            // `mobjinfo[NUMMOBJTYPES]`: array global de structs con iniciales.
            nombre: "array global de structs",
            fuente: "typedef struct { int id; char *nom; int hp; } info_t;\n\
                     info_t tabla[3] = { {1,\"posesso\",20}, {2,\"imp\",60}, {3,\"baron\",1000} };\n\
                     int main() { printf(\"%d %s %d\\n\", tabla[1].id, tabla[1].nom, tabla[2].hp); return 0; }",
            espera: "2 imp 1000",
        },
        Casilla {
            // Con un short dentro, que es como son los de DOOM.
            nombre: "array global de structs, short",
            fuente: "typedef struct { short w; short h; int ofs; } pt_t;\n\
                     pt_t t[3] = { {10,20,100}, {-30,40,200}, {50,-60,300} };\n\
                     int main() { printf(\"%d %d %d\\n\", (int)t[1].w, (int)t[2].h, t[2].ofs); return 0; }",
            espera: "-30 -60 300",
        },
        Casilla {
            // La aritmetica de `finecosine`: indice negativo sobre el puntero
            // desplazado, que es lo que hace el renderizador.
            nombre: "indice negativo sobre puntero",
            fuente: "const int t[8] = {10,20,30,40,50,60,70,80};\n\
                     const int *medio = &t[4];\n\
                     int main() { printf(\"%d %d\\n\", medio[-1], medio[-4]); return 0; }",
            espera: "40 10",
        },
    ]
}

#[test]
fn el_censo_de_las_tablas_no_ha_cambiado() {
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DE LAS TABLAS CAMBIO.\n\
         Este eje estaba limpio entero, asi que un ROTO aqui es una REGRESION.\n\
         Si la que cae es una de las de puntero global, el sospechoso es el\n\
         emisor de relocations (`SeccionAbs64`) y no el de datos.",
    );
}

/// **EL CENSO DE LAS TABLAS, al 2026-08-13.** Verde entero desde el primer
/// barrido.
const CENSO: &str = "\
global const int, leer 3 sitios BIEN
tabla de 512: 0, ultimo, medio BIEN
tabla de 4096 (la de finetangent) BIEN
puntero global a &tabla[k]     BIEN
puntero global a &tabla[0]     BIEN
puntero global al array a secas BIEN
global 2D con iniciales        BIEN
tabla global de cadenas        BIEN
array global de structs        BIEN
array global de structs, short BIEN
indice negativo sobre puntero  BIEN
";

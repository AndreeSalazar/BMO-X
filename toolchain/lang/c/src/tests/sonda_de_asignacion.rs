//! # LA SONDA DE LA ASIGNACION -- las formas abreviadas
//!
//! ## El eje
//!
//! `x += 1` no es azucar inocente: **es un lvalue que se lee y se escribe en la
//! misma expresion**, y eso obliga a decidir cuantas veces se evalua. C es
//! taxativo -- `E1 op= E2` evalua `E1` **una sola vez** (C11 6.5.16.2p3).
//!
//! El resto de las filas cubren lo que va con ello: `++`/`--` sobre lvalues que
//! no son una variable suelta, la asignacion en cadena, los `static` locales,
//! `sizeof` y el operador coma. Todo esto sale en cada linea del playsim de
//! DOOM (`sector->floorheight += speed`, `players[i].health -= damage`).
//!
//! ## ** UNA ROTA, y esta ABIERTA a proposito
//!
//! ```text
//!   int g[4]; int i = 1;
//!   g[i++] += 7;      ->  i queda en 3 y el 7 cae en g[2]
//!                         (toca: i en 2 y el 7 en g[1])
//! ```
//!
//! **La causa, con fichero y linea**: `parser/mod.rs::sub_assign_op` construye
//! el arbol clonando el indice --uno para la lectura y otro para la
//! escritura-- asi que `g[i++] += 7` acaba siendo `g[i++] = g[i++] + 7`. Lo
//! mismo hacen `field_assign_op`, `arrow_assign_op` e `idxptr_assign_op`.
//!
//! ## Por que se deja ROTA en vez de arreglarla ya
//!
//! **Porque esta MEDIDO que no bloquea a DOOM**: el arbol tiene 10 indices con
//! `++` dentro y **ninguno** combinado con una asignacion compuesta
//! (`grep -E "\\[[a-z_]+\\+\\+\\]\\s*[-+*/|&^]="` no da una sola linea).
//!
//! Y arreglarlo no es una linea: hay que evaluar la DIRECCION del lvalue una
//! vez y operar sobre ella, o sea un nodo de AST nuevo y seis brazos del parser
//! mas su codegen. Eso es del tamano del reparto entero del codegen, con riesgo
//! de regresion, para un defecto que hoy no puede dispararse.
//!
//! [!] Un `ROTO` con su sintoma exacto al lado es mas util que una fila en un
//! `TODO`, y **la suite sigue en verde porque el censo dice la verdad**. El dia
//! que alguien escriba `a[i++] += x` en BMO C, esta fila explica lo que pasa
//! sin depurar nada.

use super::censo::{barrer, Casilla};

fn censo() -> Vec<Casilla> {
    vec![
        Casilla {
            nombre: "+= -= *= sobre una local",
            fuente: "int main() { int a; a = 10; a += 5; a -= 3; a *= 2; \
                       printf(\"%d\\n\", a); return 0; }",
            espera: "24",
        },
        Casilla {
            nombre: "/= %= <<= >>=",
            fuente: "int main() { int a; int b; a = 100; a /= 7; b = 100; b %= 7; \
                       printf(\"%d %d\\n\", a, b); return 0; }",
            espera: "14 2",
        },
        Casilla {
            // [!] TERCERA vez en la sesion que el censo caza una cuenta MIA:
            // escribi 252 y contesto 254. `0xF0 |0x0C` = 0xFC, `&0xFE` = 0xFC,
            // `^0x02` = **0xFE** = 254. Tres de tres -- la aritmetica de bits a
            // ojo no es de fiar, y comparar un informe entero es lo que hace
            // que un error del que escribe la prueba se vea en vez de colarse.
            nombre: "&= |= ^= (mascaras de DOOM)",
            fuente: "int main() { int f; f = 0xF0; f |= 0x0C; f &= 0xFE; f ^= 0x02; \
                       printf(\"%d\\n\", f); return 0; }",
            espera: "254",
        },
        Casilla {
            nombre: "<<= y >>= con desplazamiento",
            fuente: "int main() { int a; a = 3; a <<= 16; a >>= 8; \
                       printf(\"%d\\n\", a); return 0; }",
            espera: "768",
        },
        Casilla {
            // La forma de `sector->floorheight += speed` en `p_floor.c`.
            nombre: "+= sobre p->campo",
            fuente: "typedef struct { int h; int v; } sec_t;\n\
                     int main() { sec_t s; sec_t *p; s.h = 100; s.v = 0; p = &s; \
                       p->h += 8; p->v -= 3; \
                       printf(\"%d %d\\n\", p->h, p->v); return 0; }",
            espera: "108 -3",
        },
        Casilla {
            // `players[i].health -= damage`
            nombre: "+= sobre a[i].campo",
            fuente: "typedef struct { int hp; } jug_t;\n\
                     jug_t t[4];\n\
                     int main() { t[2].hp = 100; t[2].hp -= 35; \
                       printf(\"%d\\n\", t[2].hp); return 0; }",
            espera: "65",
        },
        Casilla {
            nombre: "+= sobre un array global",
            fuente: "int g[4] = {1,2,3,4};\n\
                     int main() { g[1] += 10; g[3] *= 5; \
                       printf(\"%d %d\\n\", g[1], g[3]); return 0; }",
            espera: "12 20",
        },
        Casilla {
            // ** El indice con EFECTO SECUNDARIO. Un `a[i++] += 1` que expanda
            // a `a[i++] = a[i++] + 1` incrementa `i` DOS veces y escribe en la
            // casilla equivocada.
            nombre: "a[i++] += 1 avanza i UNA vez",
            fuente: "int main() { int g[4]; int i; \
                       g[0]=0; g[1]=0; g[2]=0; g[3]=0; i = 1; \
                       g[i++] += 7; \
                       printf(\"%d %d %d %d\\n\", i, g[0], g[1], g[2]); return 0; }",
            espera: "2 0 7 0",
        },
        Casilla {
            nombre: "++ y -- sobre p->campo",
            fuente: "typedef struct { int n; } c_t;\n\
                     int main() { c_t s; c_t *p; s.n = 5; p = &s; \
                       p->n++; p->n++; p->n--; \
                       printf(\"%d\\n\", p->n); return 0; }",
            espera: "6",
        },
        Casilla {
            nombre: "++ sobre a[i]",
            fuente: "int g[3] = {10,20,30};\n\
                     int main() { g[1]++; ++g[2]; printf(\"%d %d\\n\", g[1], g[2]); return 0; }",
            espera: "21 31",
        },
        Casilla {
            // Post contra pre: `x++` devuelve el valor VIEJO.
            nombre: "post y pre devuelven distinto",
            fuente: "int main() { int i; int a; int b; i = 5; a = i++; b = ++i; \
                       printf(\"%d %d %d\\n\", a, b, i); return 0; }",
            espera: "5 7 7",
        },
        Casilla {
            nombre: "asignacion en cadena a = b = c",
            fuente: "int main() { int a; int b; int c; a = b = c = 9; \
                       printf(\"%d %d %d\\n\", a, b, c); return 0; }",
            espera: "9 9 9",
        },
        Casilla {
            // `static` dentro de una funcion: sobrevive entre llamadas y se
            // inicializa UNA vez. DOOM lo usa en el renderizador y en el menu.
            nombre: "static local sobrevive",
            fuente: "int cuenta(void) { static int n = 0; n = n + 1; return n; }\n\
                     int main() { cuenta(); cuenta(); printf(\"%d\\n\", cuenta()); return 0; }",
            espera: "3",
        },
        Casilla {
            nombre: "sizeof de tipo y de expresion",
            fuente: "typedef struct { int a; char b; } p_t;\n\
                     int main() { p_t s; int v[10]; \
                       printf(\"%d %d %d %d\\n\", (int)sizeof(int), (int)sizeof(p_t), \
                         (int)sizeof(s), (int)sizeof(v)); return 0; }",
            espera: "4 8 8 40",
        },
        Casilla {
            // `Z_Malloc(sizeof(texture_t) + sizeof(texpatch_t)*(n-1), ...)`
            nombre: "sizeof en una cuenta",
            fuente: "typedef struct { short a; short b; int c; } t_t;\n\
                     int main() { int n; n = 4; \
                       printf(\"%d\\n\", (int)(sizeof(t_t) * (n - 1))); return 0; }",
            espera: "24",
        },
        Casilla {
            // El operador coma, que sale en los `for` de DOOM.
            nombre: "el operador coma en un for",
            fuente: "int main() { int i; int j; int n; n = 0; \
                       for (i = 0, j = 10; i < j; i++, j--) { n = n + 1; } \
                       printf(\"%d %d %d\\n\", i, j, n); return 0; }",
            espera: "5 5 5",
        },
    ]
}

#[test]
fn el_censo_de_la_asignacion_no_ha_cambiado() {
    barrer(
        &censo(),
        CENSO,
        "EL CENSO DE LA ASIGNACION CAMBIO.\n\
         [!] Este censo tiene UNA fila en ROTO a proposito y documentada arriba\n\
         (`a[i++] += 1`). Si se ha puesto en BIEN, alguien arreglo la doble\n\
         evaluacion del lvalue: **actualiza el censo y borra el parrafo de la\n\
         cabecera**, que si no queda un documento que miente.",
    );
}

/// **EL CENSO DE LA ASIGNACION, al 2026-08-13.**
///
/// Quince de dieciseis. La que falta es la doble evaluacion del lvalue en una
/// asignacion compuesta, explicada en la cabecera: esta abierta a proposito
/// porque **esta medido que DOOM no la usa** y arreglarla es un nodo de AST
/// nuevo mas seis brazos del parser.
const CENSO: &str = "\
+= -= *= sobre una local       BIEN
/= %= <<= >>=                  BIEN
&= |= ^= (mascaras de DOOM)    BIEN
<<= y >>= con desplazamiento   BIEN
+= sobre p->campo              BIEN
+= sobre a[i].campo            BIEN
+= sobre un array global       BIEN
a[i++] += 1 avanza i UNA vez   ROTO da \"3 0 0 7\" y toca \"2 0 7 0\"
++ y -- sobre p->campo         BIEN
++ sobre a[i]                  BIEN
post y pre devuelven distinto  BIEN
asignacion en cadena a = b = c BIEN
static local sobrevive         BIEN
sizeof de tipo y de expresion  BIEN
sizeof en una cuenta           BIEN
el operador coma en un for     BIEN
";

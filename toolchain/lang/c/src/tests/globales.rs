//! Variables GLOBALES: carga, guardado y cero inicial
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn global_var_load_store() {
    let src = r#"
int g = 42;
int main() {
int x;
x = g;
g = 100;
return x;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn global_var_zero_init() {
    let src = r#"
int z;
int main() {
z = 7;
return z;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn global_var_addr_of() {
    let src = r#"
int g;
int main() {
int *p = &g;
*p = 99;
return g;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}


// -- TABLAS GLOBALES: `int t[4] = {...}` a nivel de fichero --------------
//
// Hasta el 2026-08-07 esto no se PARSEABA: `unexpected token: OpenBrace`.
// Dentro de una funcion funcionaba desde siempre, asi que la diferencia era el
// AMBITO y no el inicializador. Importa porque es la forma de las tablas
// estaticas, y un programa grande de C es en buena parte tablas -- el `info.c`
// de DOOM son cuatro mil lineas de `{ ... }` a nivel global.
//
// Estos tests EJECUTAN. Los tres que ya habia en este fichero solo comprobaban
// que el programa compilara, y por eso nadie vio nunca que un global con
// inicializador que el codegen no entendia valia cero.

/// * Una tabla de enteros, leida por indice.
#[test]
fn una_tabla_global_de_enteros_conserva_sus_valores() {
    let fuente = "int numeros[4] = {10, 20, 30, 40}; \
                  int main() { printf(\"%d,%d,%d,%d\", numeros[0], numeros[1], \
                  numeros[2], numeros[3]); return 0; }";
    assert_eq!(run_c(fuente), "10,20,30,40");
}

/// Los designadores valen igual que en un local, porque el parser reusa el
/// MISMO aplanador (`parse_inicializador`). Y lo que la lista no menciona
/// queda a cero, que es lo que dice C -- aqui el indice 1.
#[test]
fn una_tabla_global_con_designadores_deja_los_huecos_a_cero() {
    let fuente = "int t[5] = {[2] = 30, 40}; \
                  int main() { printf(\"%d,%d,%d,%d\", t[0], t[1], t[2], t[3]); \
                  return 0; }";
    assert_eq!(run_c(fuente), "0,0,30,40");
}

/// Las constantes se PLIEGAN: `{2*3, 100-1, 0-7}`. Un `.bex` lleva los bytes ya
/// puestos, asi que si esto no se evaluara en compilacion no habria donde
/// calcularlo.
#[test]
fn una_tabla_global_pliega_constantes_incluida_la_negativa() {
    let fuente = "int t[3] = {2 * 3, 100 - 1, 0 - 7}; \
                  int main() { printf(\"%d,%d,%d\", t[0], t[1], t[2]); return 0; }";
    assert_eq!(run_c(fuente), "6,99,-7");
}

/// Cada elemento toma el ancho de SU tipo, no ocho bytes. Si un `char` de la
/// tabla escribiera ocho, se llevaria por delante a los tres siguientes -- y el
/// terminador de esta cadena es justamente el cuarto.
#[test]
fn en_una_tabla_de_char_cada_elemento_ocupa_un_byte() {
    let fuente = "char letras[4] = {65, 66, 67, 0}; \
                  int main() { printf(\"%s\", letras); return 0; }";
    assert_eq!(run_c(fuente), "ABC");
}

/// ** UNA TABLA DE PUNTEROS A CADENA, QUE YA FUNCIONA.
///
/// Esto se rechazaba con un error hasta que existieron las relocations
/// `SeccionAbs64`: el valor de cada elemento es la DIRECCION de una cadena, y
/// esa depende de donde cargue el programa. El compilador deja el hueco a cero
/// y anota quien lo rellena; lo escribe el cargador.
///
/// Es `char *sprnames[]` de DOOM, y es la mitad de sus tablas.
#[test]
fn una_tabla_global_de_punteros_a_cadena_funciona() {
    let fuente = "char *nombres[3] = {\"imp\", \"shotgun\", \"cyberdemon\"};                   int main() { int i; i = 0;                   while (i < 3) { printf(\"%s|\", nombres[i]); i = i + 1; } return 0; }";
    assert_eq!(run_c(fuente), "imp|shotgun|cyberdemon|");
}

/// ** Y EL CASO QUE LO PIDIO: `char *mapa = "..."` como global suelto.
///
/// Es exactamente lo que tenia `raycaster_C.c` y valia CERO, con las paredes
/// del laberinto siendo el codigo maquina del propio programa.
#[test]
fn un_puntero_global_a_cadena_apunta_a_la_cadena() {
    let fuente = "char *mapa = \"1111000011110000\";                   int main() { printf(\"%s,%d,%d\", mapa, mapa[0], mapa[4]); return 0; }";
    assert_eq!(run_c(fuente), "1111000011110000,49,48");
}

/// El mismo puntero, leido por dos sitios: si la reloc escribiera una direccion
/// plausible pero equivocada, un `%s` podria dar algo y el indice otra cosa.
/// Aqui los dos tienen que cuadrar con la misma cadena.
#[test]
fn dos_punteros_globales_a_la_misma_cadena_coinciden() {
    let fuente = "char *a = \"hola\"; char *b = \"hola\"; char *c = \"otra\";                   int main() { printf(\"%d,%d,%s\", a == b, a == c, c); return 0; }";
    // La tabla de cadenas deduplica por valor, asi que `a` y `b` apuntan al
    // MISMO sitio. Que no sea asi seria un desperdicio silencioso.
    assert_eq!(run_c(fuente), "1,0,otra");
}

/// [!] Un struct GLOBAL y el global de al lado. Sonda: si el struct no reserva
/// su tamano, lo que venga despues cae encima.
#[test]
fn un_struct_global_no_pisa_al_global_siguiente() {
    let fuente = "struct P { int x; int y; }; \
                  struct P g; \
                  int centinela = 12345; \
                  int main() { g.x = 7; g.y = 9; \
                  printf(\"%d,%d,%d\", g.x, g.y, centinela); return 0; }";
    assert_eq!(run_c(fuente), "7,9,12345");
}

/// ** LA FORMA DE DOOM: una tabla global de STRUCTS.
///
/// `state_t states[]` y `mobjinfo_t mobjinfo[]` son esto, y el `info.c` de DOOM
/// son cuatro mil lineas asi. Antes esta rama del parser ni lo intentaba:
/// ignoraba el `[N]` --o sea que una tabla de dos structs se declaraba como
/// UNO-- y luego reventaba con "expected type, got OpenBracket".
#[test]
fn una_tabla_global_de_structs_es_indexable() {
    let fuente = "struct estado { int tics; int siguiente; }; \
                  struct estado estados[3] = { {4, 1}, {8, 2}, {15, 0} }; \
                  int main() { int i; i = 0; \
                  while (i < 3) { printf(\"%d>%d,\", estados[i].tics, \
                  estados[i].siguiente); i = i + 1; } return 0; }";
    assert_eq!(run_c(fuente), "4>1,8>2,15>0,");
}

/// Y con designadores anidados, que es lo que hace legible una tabla grande:
/// `{[1].tics = 8}` deja el resto a cero sin escribirlo.
#[test]
fn una_tabla_global_de_structs_admite_designadores_anidados() {
    let fuente = "struct estado { int tics; int siguiente; }; \
                  struct estado t[3] = { [1].tics = 8, [2].siguiente = 5 }; \
                  int main() { printf(\"%d,%d,%d,%d\", t[0].tics, t[1].tics, \
                  t[2].tics, t[2].siguiente); return 0; }";
    assert_eq!(run_c(fuente), "0,8,0,5");
}

/// El tamano de la tabla tiene que ser el de N structs, no el de uno: el global
/// que venga despues no puede caer dentro.
#[test]
fn una_tabla_de_structs_reserva_el_tamano_de_todos() {
    let fuente = "struct P { int x; int y; }; \
                  struct P tabla[4] = { {1,2}, {3,4}, {5,6}, {7,8} }; \
                  int centinela = 999; \
                  int main() { tabla[3].y = 77; \
                  printf(\"%d,%d,%d\", tabla[3].y, tabla[0].x, centinela); return 0; }";
    assert_eq!(run_c(fuente), "77,1,999");
}

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


// ── TABLAS GLOBALES: `int t[4] = {…}` a nivel de fichero ──────────────
//
// Hasta el 2026-08-07 esto no se PARSEABA: `unexpected token: OpenBrace`.
// Dentro de una función funcionaba desde siempre, así que la diferencia era el
// ÁMBITO y no el inicializador. Importa porque es la forma de las tablas
// estáticas, y un programa grande de C es en buena parte tablas — el `info.c`
// de DOOM son cuatro mil líneas de `{ … }` a nivel global.
//
// Estos tests EJECUTAN. Los tres que ya había en este fichero sólo comprobaban
// que el programa compilara, y por eso nadie vio nunca que un global con
// inicializador que el codegen no entendía valía cero.

/// ★ Una tabla de enteros, leída por índice.
#[test]
fn una_tabla_global_de_enteros_conserva_sus_valores() {
    let fuente = "int numeros[4] = {10, 20, 30, 40}; \
                  int main() { printf(\"%d,%d,%d,%d\", numeros[0], numeros[1], \
                  numeros[2], numeros[3]); return 0; }";
    assert_eq!(run_c(fuente), "10,20,30,40");
}

/// Los designadores valen igual que en un local, porque el parser reusa el
/// MISMO aplanador (`parse_inicializador`). Y lo que la lista no menciona
/// queda a cero, que es lo que dice C — aquí el índice 1.
#[test]
fn una_tabla_global_con_designadores_deja_los_huecos_a_cero() {
    let fuente = "int t[5] = {[2] = 30, 40}; \
                  int main() { printf(\"%d,%d,%d,%d\", t[0], t[1], t[2], t[3]); \
                  return 0; }";
    assert_eq!(run_c(fuente), "0,0,30,40");
}

/// Las constantes se PLIEGAN: `{2*3, 100-1, 0-7}`. Un `.bex` lleva los bytes ya
/// puestos, así que si esto no se evaluara en compilación no habría dónde
/// calcularlo.
#[test]
fn una_tabla_global_pliega_constantes_incluida_la_negativa() {
    let fuente = "int t[3] = {2 * 3, 100 - 1, 0 - 7}; \
                  int main() { printf(\"%d,%d,%d\", t[0], t[1], t[2]); return 0; }";
    assert_eq!(run_c(fuente), "6,99,-7");
}

/// Cada elemento toma el ancho de SU tipo, no ocho bytes. Si un `char` de la
/// tabla escribiera ocho, se llevaría por delante a los tres siguientes — y el
/// terminador de esta cadena es justamente el cuarto.
#[test]
fn en_una_tabla_de_char_cada_elemento_ocupa_un_byte() {
    let fuente = "char letras[4] = {65, 66, 67, 0}; \
                  int main() { printf(\"%s\", letras); return 0; }";
    assert_eq!(run_c(fuente), "ABC");
}

/// ★ Y el límite, dicho: una tabla de PUNTEROS necesita direcciones, que no se
/// conocen hasta cargar. Es la mitad de las tablas de DOOM (`char *sprnames[]`,
/// las de punteros a función) y lo que hace falta es una relocation `Abs64`.
/// Mientras no exista, se rechaza con el offset y el motivo.
#[test]
fn una_tabla_global_de_punteros_a_cadena_se_rechaza_diciendolo() {
    let err = compile_source_to_bef(
        "char *nombres[2] = {\"imp\", \"cyberdemon\"}; int main() { return 0; }",
    )
    .expect_err("una direccion no se puede poner en compilacion");
    let msg = format!("{err:?}");
    assert!(msg.contains("nombres"), "tiene que decir QUE tabla: {msg}");
    assert!(msg.contains("Abs64"), "y que hace falta: {msg}");
}

/// ⚠️ Un struct GLOBAL y el global de al lado. Sonda: si el struct no reserva
/// su tamaño, lo que venga después cae encima.
#[test]
fn un_struct_global_no_pisa_al_global_siguiente() {
    let fuente = "struct P { int x; int y; }; \
                  struct P g; \
                  int centinela = 12345; \
                  int main() { g.x = 7; g.y = 9; \
                  printf(\"%d,%d,%d\", g.x, g.y, centinela); return 0; }";
    assert_eq!(run_c(fuente), "7,9,12345");
}

/// ★★ LA FORMA DE DOOM: una tabla global de STRUCTS.
///
/// `state_t states[]` y `mobjinfo_t mobjinfo[]` son esto, y el `info.c` de DOOM
/// son cuatro mil líneas así. Antes esta rama del parser ni lo intentaba:
/// ignoraba el `[N]` —o sea que una tabla de dos structs se declaraba como
/// UNO— y luego reventaba con "expected type, got OpenBracket".
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

/// El tamaño de la tabla tiene que ser el de N structs, no el de uno: el global
/// que venga después no puede caer dentro.
#[test]
fn una_tabla_de_structs_reserva_el_tamano_de_todos() {
    let fuente = "struct P { int x; int y; }; \
                  struct P tabla[4] = { {1,2}, {3,4}, {5,6}, {7,8} }; \
                  int centinela = 999; \
                  int main() { tabla[3].y = 77; \
                  printf(\"%d,%d,%d\", tabla[3].y, tabla[0].x, centinela); return 0; }";
    assert_eq!(run_c(fuente), "77,1,999");
}

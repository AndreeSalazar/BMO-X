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

// -- LOS CEROS NO VIAJAN: la seccion `Bss` -------------------------------
//
// Hasta el 2026-08-09 TODOS los globales iban a `.data`, con o sin
// inicializador. En DOOM eso eran **582.291 bytes de ceros** viajando en el
// fichero -- el 90,3% de su seccion `data`-- que se leian del disco y se
// copiaban dos veces para acabar valiendo lo que ya se sabia al compilar.
//
// `Codegen::separar_bss` los aparta. Estas filas EJECUTAN, porque el fallo que
// hay que cazar aqui no es que el fichero encoja: es que un global se lea desde
// la direccion equivocada, y eso da un numero, no un error.

/// El caso base: un global sin inicializador vale cero y se puede escribir.
///
/// Parece trivial y es la fila que mas cubre -- ahora ese global vive en OTRA
/// seccion, con otra base de direcciones, y quien la calcula mal lee la pagina
/// de al lado.
#[test]
fn un_global_a_cero_se_lee_cero_y_luego_conserva_lo_escrito() {
    let fuente = "int z; \
                  int main() { printf(\"%d,\", z); z = 7; printf(\"%d\", z); return 0; }";
    assert_eq!(run_c(fuente), "0,7");
}

/// ** Un global CON valor y otro a cero conviven, y no se pisan.
///
/// Es la sonda del reparto: uno se queda en `.data` y el otro se va a `.bss`,
/// asi que sus direcciones salen de dos bases distintas. Si las dos cuentas no
/// cuadran, lo normal es que uno lea al otro.
#[test]
fn un_global_con_valor_y_otro_a_cero_no_se_pisan() {
    let fuente = "int lleno = 12345; int vacio; int otro_lleno = 999; int otro_vacio; \
                  int main() { vacio = 7; otro_vacio = 8; \
                  printf(\"%d,%d,%d,%d\", lleno, vacio, otro_lleno, otro_vacio); return 0; }";
    assert_eq!(run_c(fuente), "12345,7,999,8");
}

/// La direccion de un global de `.bss`, tomada DESDE EL CODIGO.
///
/// `&g` en una funcion es un `lea [rip+disp]` que resuelve `patch_all_fixups`,
/// y es ahi donde vive la cuenta de las dos bases.
#[test]
fn la_direccion_de_un_global_a_cero_apunta_a_ese_global() {
    let fuente = "int g; \
                  int main() { int *p; p = &g; *p = 99; printf(\"%d\", g); return 0; }";
    assert_eq!(run_c(fuente), "99");
}

/// ** **El motivo 3 del anclaje**: un global a cero cuya DIRECCION se guarda en
/// otro global tiene que quedarse en `.data`.
///
/// El codigo de seccion de una relocation solo sabe decir code/data/rodata --
/// no hay valor para `bss`. Si `contador` se fuera a `.bss`, su reloc no
/// tendria como nombrarlo. Es `doom_defaults[]` entero, que es una tabla de
/// punteros a variables de configuracion que empiezan todas a cero.
#[test]
fn un_global_a_cero_apuntado_desde_otro_global_sigue_siendo_alcanzable() {
    let fuente = "int contador; int *puntero = &contador; \
                  int main() { *puntero = 42; printf(\"%d,%d\", contador, *puntero); return 0; }";
    assert_eq!(run_c(fuente), "42,42");
}

/// ** **El motivo 2**: `char *p = "x"` guarda CEROS en el fichero y aun asi no
/// puede irse a `.bss` -- su valor lo escribe el cargador con una relocation.
///
/// Es el que mas facil se cuela: por bytes es indistinguible de un global
/// vacio. Aqui se mezcla con uno de verdad vacio para que se vea que el
/// compilador los separa bien.
#[test]
fn un_puntero_a_cadena_no_se_confunde_con_un_global_vacio() {
    let fuente = "char *texto = \"hola\"; int vacio; \
                  int main() { vacio = 3; printf(\"%s,%d\", texto, vacio); return 0; }";
    assert_eq!(run_c(fuente), "hola,3");
}

/// Una TABLA grande a cero: el caso por el que se hizo todo esto. Se escribe en
/// los dos extremos para que un reparto corto se note.
#[test]
fn una_tabla_global_grande_a_cero_funciona_entera() {
    let fuente = "int enorme[4096]; int centinela = 7; \
                  int main() { enorme[0] = 1; enorme[4095] = 2; \
                  printf(\"%d,%d,%d,%d\", enorme[0], enorme[2048], enorme[4095], centinela); \
                  return 0; }";
    assert_eq!(run_c(fuente), "1,0,2,7");
}

/// Y una tabla PARCIALMENTE escrita se queda entera en `.data`: el reparto es
/// por global, no por byte. Aqui lo que se comprueba es que sus ceros siguen
/// leyendose a cero -- que es lo unico que el programa nota.
#[test]
fn una_tabla_con_un_solo_valor_conserva_sus_ceros() {
    let fuente = "int t[8] = {[3] = 5}; \
                  int main() { printf(\"%d,%d,%d\", t[0], t[3], t[7]); return 0; }";
    assert_eq!(run_c(fuente), "0,5,0");
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

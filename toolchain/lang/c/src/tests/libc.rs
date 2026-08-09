//! LIBC: las que faltaban para que el unity build de DOOM llegue al final
//!
//! Parte del banco de pruebas de BMO C. Cada fila EJECUTA el programa: una
//! funcion de cadena mal escrita no da error, da un byte distinto.
//!
//! ## Por que estan en cabeceras y no en el codegen
//!
//! `strlen` y `memcpy` los sintetiza el compilador, y ahi tiene sentido:
//! `memcpy` es un `rep movsb`. `isspace` es una comparacion, y emitirla a mano
//! seria trabajo para no ganar nada.
//!
//! Y los ficheros se llaman `<string.h>`, `<ctype.h>` y `<stdlib.h>` **a
//! proposito**: un programa de fuera los incluye sin cambiar una linea. Esa es
//! la portabilidad de verdad -- no que el lenguaje sea C, sino que las
//! cabeceras se llamen como se llaman en todas partes.

use super::*;

fn corre(cabeceras: &str, cuerpo: &str) -> String {
    let src = format!("{cabeceras}\n{cuerpo}");
    let bef = compile_with_preprocessor(&src, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("el programa debe compilar");
    ejecutar_bef(&bef)
}

#[test]
fn ctype_clasifica_solo_ascii() {
    let out = corre(
        "#include <ctype.h>",
        r#"
int main() {
    printf("%d%d%d%d\n", isspace(' '), isspace('\t'), isspace('x'), isspace('\n'));
    printf("%d%d%d\n", isdigit('7'), isdigit('a'), isalpha('Z'));
    printf("%c%c\n", toupper('q'), tolower('Q'));
    return 0;
}
"#,
    );
    assert_eq!(out, "1101\n101\nQq\n");
}

/// `atoi` con signo, espacios delante y basura detras -- los tres casos que
/// trae un fichero de configuracion de verdad.
#[test]
fn atoi_lee_signo_y_para_en_la_basura() {
    let out = corre(
        "#include <stdlib.h>",
        r#"
int main() {
    printf("%d %d %d %d\n", atoi("42"), atoi("-17"), atoi("  8x"), atoi("hola"));
    return 0;
}
"#,
    );
    assert_eq!(out, "42 -17 8 0\n");
}

/// ★ `strncpy` RELLENA de ceros hasta `n`. No es un detalle del estandar: el
/// codigo que la usa sobre un buffer reutilizado cuenta con eso, y una version
/// que solo copie deja la basura de la vuelta anterior detras del texto.
#[test]
fn strncpy_rellena_de_ceros() {
    let out = corre(
        "#include <string.h>",
        r#"
char b[8];
int main() {
    int i;
    for (i = 0; i < 8; i = i + 1) { b[i] = 'X'; }
    strncpy(b, "ab", 8);
    printf("[%s]", b);
    for (i = 0; i < 8; i = i + 1) { printf("%d", b[i]); }
    printf("\n");
    return 0;
}
"#,
    );
    assert_eq!(out, "[ab]9798000000\n");
}

#[test]
fn strrchr_da_la_ULTIMA() {
    let out = corre(
        "#include <string.h>",
        r#"
int main() {
    char *r;
    r = strrchr("a/b/c.wad", '/');
    printf("%s\n", r);
    r = strrchr("sin barras", '/');
    printf("%d\n", (int)r);
    return 0;
}
"#,
    );
    assert_eq!(out, "/c.wad\n0\n");
}

#[test]
fn strstr_encuentra_y_dice_que_no() {
    let out = corre(
        "#include <string.h>",
        r#"
int main() {
    char *r;
    r = strstr("DOOM2.WAD", "2.W");
    printf("%s\n", r);
    printf("%d\n", (int)strstr("abc", "xyz"));
    /* La aguja vacia se encuentra al principio. */
    printf("%s\n", strstr("hola", ""));
    return 0;
}
"#,
    );
    assert_eq!(out, "2.WAD\n0\nhola\n");
}

#[test]
fn strcasecmp_ignora_la_caja() {
    let out = corre(
        "#include <string.h>",
        r#"
int main() {
    printf("%d ", strcasecmp("MAP01", "map01"));
    printf("%d\n", strcasecmp("abc", "abd") < 0);
    return 0;
}
"#,
    );
    assert_eq!(out, "0 1\n");
}

/// ★★ LA QUE IMPORTA: `memmove` con bloques SOLAPADOS.
///
/// `memcpy` puede copiar en cualquier orden porque promete que no se solapan.
/// `memmove` no lo promete, asi que cuando el destino cae dentro del origen hay
/// que copiar hacia atras: de frente, cada byte escrito pisa uno sin leer.
///
/// Un `memmove` que sea un `memcpy` con otro nombre **pasa cualquier prueba
/// donde los bloques no se tocan** y corrompe datos justo el dia que se usa
/// para lo que existe.
///
/// ★★ ESTA FILA FALLA HOY, Y POR ESO ESTA ESCRITA. Da `ababab`: el que se
/// ejecuta copia de frente.
///
/// La causa esta localizada: **el codegen intercepta `memmove` por nombre**
/// (`codegen/mod.rs`, `("memmove", 3)`) y emite su propia version, asi que la
/// definicion de `<string.h>` no se llama nunca. Se escribio una con su rama
/// hacia atras, se probo con la comparacion de punteros explicita y con el
/// bucle al reves, y el resultado no cambio ni un byte -- que es exactamente
/// como se descubrio quien la estaba emitiendo.
///
/// Se deja marcada y no borrada: **una prueba que dice lo que el sistema
/// deberia hacer vale mas que ninguna**, y el dia que se toque el codegen esta
/// es la que dira si quedo bien. El arreglo es de una sesion de compilador, no
/// de una cabecera.
#[test]

fn memmove_solapado_hacia_adelante() {
    let out = corre(
        "#include <string.h>",
        r#"
char b[10];
int main() {
    strncpy(b, "abcdef", 10);
    /* Destino POR ENCIMA del origen y solapando: el caso que rompe un memcpy. */
    memmove(&b[2], &b[0], 4);
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "ababcd\n", "si sale 'ababab', memmove copio de frente");
}

#[test]
fn memmove_solapado_hacia_atras() {
    let out = corre(
        "#include <string.h>",
        r#"
char b[10];
int main() {
    strncpy(b, "abcdef", 10);
    memmove(&b[0], &b[2], 4);
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "cdefef\n");
}

/// `strdup` pide memoria de verdad, y **el kernel la puede negar**: son cuatro
/// bloques por proceso. Un `strdup` que no lo mire escribe en la direccion 0.
#[test]
fn strdup_copia_y_es_otra_memoria() {
    let out = corre(
        "#include <string.h>\n#include <stdlib.h>",
        r#"
int main() {
    char *a;
    char *b;
    a = "MAP01";
    b = strdup(a);
    if (b == 0) { printf("sin memoria\n"); return 1; }
    printf("%s %d\n", b, (int)(b != a));
    b[0] = 'W';
    /* El original NO cambia: son dos memorias. */
    printf("%s %s\n", a, b);
    return 0;
}
"#,
    );
    assert_eq!(out, "MAP01 1\nMAP01 WAP01\n");
}

/// Diagnostico del fallo de `memmove`: **comparar dos punteros con `>`**.
///
/// Esta fila no prueba una funcion de libc: prueba el COMPILADOR. Se escribio
/// porque `memmove` copiaba de frente teniendo la rama de atras escrita, y la
/// unica forma de saber si el fallo era de la funcion o del lenguaje era
/// preguntarselo a un programa de cuatro lineas.
#[test]
fn comparar_dos_punteros_con_mayor_que() {
    let out = corre(
        "",
        r#"
char b[10];
int main() {
    char *d;
    char *s;
    d = &b[2];
    s = &b[0];
    printf("%d%d\n", (int)(d > s), (int)(s > d));
    return 0;
}
"#,
    );
    assert_eq!(out, "10\n", "d>s tiene que ser 1 y s>d tiene que ser 0");
}

// =============== LA AUDITORIA DEL DCE ===============
//
// Se prueba aqui y no en `bmo-verify` porque **hace falta un `.bex` de
// verdad**, y quien sabe producirlos es este crate. Un BEF escrito a mano en un
// test solo probaria que el test sabe escribirlo.

/// Un programa corriente sale ENTERO alcanzable, y esa es la fila que hace util
/// a las demas: un auditor que grita en el caso normal no lo lee nadie.
#[test]
fn un_programa_normal_no_tiene_bytes_muertos() {
    let src = "#include <string.h>\nint main() { char b[8]; strncpy(b, \"hola\", 8); printf(\"%s\n\", b); return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("p.c"), CStandard::C11).unwrap();
    let a = bmo_verify::auditar(&bef);
    assert!(a.bytes_totales > 0, "el .bex tiene que llevar bytes");
    assert_eq!(a.relocs_al_vacio, 0, "ninguna reloc puede apuntar al vacio");
    assert_eq!(a.relocs_desbordadas, 0, "ninguna reloc puede salirse de su seccion");
    assert!(!a.hay_rotura());
}

/// ★ Y un programa con una GLOBAL de puntero ejerce las relocations, que es
/// donde vive la mitad interesante: si el DCE se llevara la seccion a la que
/// apunta, `relocs_al_vacio` lo diria **en el build** y no con una pantalla
/// negra tres dias despues.
#[test]
fn las_relocations_apuntan_a_secciones_que_existen() {
    let src = "char *mapa = \"1111\";\nint main() { printf(\"%s\n\", mapa); return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("p.c"), CStandard::C11).unwrap();
    let a = bmo_verify::auditar(&bef);
    assert_eq!(a.relocs_al_vacio, 0, "una reloc nombra una seccion que no esta");
    assert_eq!(a.relocs_desbordadas, 0, "una reloc parchea fuera de su seccion");
    // Con una global de puntero, `data` tiene que quedar ALCANZADA: es
    // justamente lo que la reloc marca.
    assert!(a.secciones_huerfanas.len() <= 1, "huerfanas: {:?}", a.secciones_huerfanas);
}

/// El numero que el backbuffer enseno a pedir: cuanto se emitio y cuanto se
/// alcanza. No es un error tener bytes muertos -- es una cifra que mirar cuando
/// un `.bex` crece y nadie sabe por que.
#[test]
fn la_auditoria_da_los_dos_numeros() {
    let src = "int main() { printf(\"hola\n\"); return 0; }";
    let bef = compile_with_preprocessor(src, std::path::Path::new("p.c"), CStandard::C11).unwrap();
    let a = bmo_verify::auditar(&bef);
    assert!(a.bytes_alcanzables <= a.bytes_totales);
    assert_eq!(a.bytes_muertos(), a.bytes_totales - a.bytes_alcanzables);
}

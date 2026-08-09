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

/// ** `strncpy` RELLENA de ceros hasta `n`. No es un detalle del estandar: el
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

/// **** LA QUE IMPORTA: `memmove` con bloques SOLAPADOS.
///
/// `memcpy` puede copiar en cualquier orden porque promete que no se solapan.
/// `memmove` no lo promete, asi que cuando el destino cae dentro del origen hay
/// que copiar hacia atras: de frente, cada byte escrito pisa uno sin leer.
///
/// Un `memmove` que sea un `memcpy` con otro nombre **pasa cualquier prueba
/// donde los bloques no se tocan** y corrompe datos justo el dia que se usa
/// para lo que existe.
///
/// **** ESTA FILA FALLA HOY, Y POR ESO ESTA ESCRITA. Da `ababab`: el que se
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

/// ** Y un programa con una GLOBAL de puntero ejerce las relocations, que es
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

// =============== ftell / feof / fwrite ===============
//
// Las tres ultimas de la lista de DOOM. Se prueban con el disco SEMBRADO: un
// `feof` contra un fichero que no existe contesta lo mismo que uno bien escrito
// contra un fichero vacio, y eso no prueba nada.

fn corre_con_disco(cuerpo: &str, ruta: &str, contenido: &str) -> String {
    let src = format!("#include <bmo/archivo.h>\n{cuerpo}");
    let bef = compile_with_preprocessor(&src, std::path::Path::new("p.c"), CStandard::C11)
        .expect("debe compilar");
    let (r, c) = (ruta.to_string(), contenido.to_string());
    ejecutar_bef_con(&bef, move |m| m.poner_archivo(&r, c.as_bytes()))
}

/// ** DESMARCADA el 2026-08-09: el emulador YA modela `ARCH_OP_LEER_EN`.
///
/// Estuvo marcada porque `fread` usa `ARCH_OP_LEER_EN` y el emulador caia en su
/// `_ => {}` y contesta 0, asi que el cursor no se mueve aqui dentro. Es la
/// tercera vez hoy que muerde el mismo patron -- ya paso con
/// `TASK_OP_MEMORIA_PEDIR` y con `KIND_AUDIO`, y esta contado en la cabecera de
/// `emu.rs`: una operacion sin modelar contesta exito con el valor a cero.
///
/// La fila se queda escrita porque dice lo que el sistema debe hacer, y en el
/// Ryzen --donde `LEER_EN` si existe-- es la que lo comprobara. Modelarlo en el
/// emulador es el arreglo, y es otra sesion.
///
/// ** Y AL DESMARCARLA SALTO QUE LA FILA ESTABA MAL ESCRITA: el destino era un
/// `char b[16]` **de la pila**, y `archivo.h` dice en su cabecera que eso no
/// vale -- el kernel solo acepta escribir dentro de un bloque que el CONCEDIO,
/// porque comprobar es una resta contra lo que entrego y no hay validador de
/// punteros que valga. Con el emulador contestando 0 a todo, la fila parecia
/// esperar lo correcto; **en el Ryzen habria fallado igual, y por otro motivo**.
/// Una fila marcada como pendiente puede estar ademas equivocada, y eso solo se
/// ve al desmarcarla.
///
/// `ftell` empieza en 0 y **avanza por lo que se leyo de verdad**, no por lo
/// que se pidio. Sumar lo pedido haria que mintiera justo al final del fichero,
/// que es donde se le pregunta.
#[test]
fn ftell_sigue_al_cursor() {
    let out = corre_con_disco(
        r#"
int main() {
    FILE *f;
    char *b;
    f = fopen("datos/x.txt", "r");
    if (f == 0) { printf("no abrio\n"); return 1; }
    b = (char *)malloc(16);
    printf("%d ", (int)ftell(f));
    fread(b, 1, 4, f);
    printf("%d ", (int)ftell(f));
    fseek(f, 2, 0);
    printf("%d\n", (int)ftell(f));
    fclose(f);
    return 0;
}
"#,
        "datos/x.txt",
        "abcdefgh",
    );
    assert_eq!(out, "0 4 2\n");
}

/// `feof` dice que no al principio y que si cuando el cursor llega al final.
///
/// ** Y aqui hay una DIFERENCIA con el C estandar, dicha a proposito: alli
/// `feof` solo se pone a 1 **despues** de que una lectura se quede corta, no
/// cuando el cursor llega al final. Un `while (!feof(f))` de manual lee una vez
/// de mas por eso. Aqui se compara cursor contra tamano, que es lo que ese
/// bucle espera de verdad -- y ademas no puede colgarse.
#[test]
fn feof_dice_que_si_al_llegar_al_final() {
    let out = corre_con_disco(
        r#"
int main() {
    FILE *f;
    char *b;
    f = fopen("datos/x.txt", "r");
    if (f == 0) { printf("no abrio\n"); return 1; }
    b = (char *)malloc(16);
    printf("%d", feof(f));
    fread(b, 1, 4, f);
    printf("%d", feof(f));
    fclose(f);
    return 0;
}
"#,
        "datos/x.txt",
        "abcd",
    );
    assert_eq!(out, "01\n".trim_end(), "al principio 0, tras leerlo entero 1");
}

/// **** `fwrite` devuelve CERO, y esta fila existe para que eso no se olvide.
///
/// El camino de escritura --`TASK_OP_ARCHIVO_CREAR` + `ARCH_OP_ESCRIBIR`--
/// existe en el kernel y **no esta cableado hasta `fopen`**, que ignora el
/// modo. Asi que `fwrite` no puede escribir, y lo que hace es DECIRLO.
///
/// Que exista igual vale: los 64 `fwrite` de DOOM compilan y enlazan, que es lo
/// que hoy bloquea el unity build. Lo que no se hace es fingir que escribio --
/// eso daria un programa que cree haber guardado la partida.
///
/// El dia que se cablee, esta fila falla, y eso es lo que se quiere: es la que
/// avisa de que hay que cambiarla.
#[test]
fn fwrite_existe_y_dice_que_no_escribio() {
    let out = corre_con_disco(
        r#"
int main() {
    FILE *f;
    char *b;
    b = (char *)malloc(16);
    b[0] = 'z';
    f = fopen("datos/x.txt", "r");
    if (f == 0) { printf("no abrio\n"); return 1; }
    printf("%d\n", (int)fwrite(b, 1, 1, f));
    fclose(f);
    return 0;
}
"#,
        "datos/x.txt",
        "abcd",
    );
    assert_eq!(out, "0\n", "si sale 1, alguien cableo la escritura: cambiar esta fila");
}

// == EL FORMATEADOR DE EJECUCION =======================================
//
// Lo que desbloquea el unity build de DOOM. Ver la cabecera de
// `toolchain/forge/sem-asm/tables/stdio.h` para el por que; aqui va lo que
// EJECUTA, que es lo unico que decide si funciona.

/// **** LA FILA QUE DESBLOQUEA DOOM: un `printf` cuyo formato es una VARIABLE.
///
/// `g_game.c:2184` hace exactamente esto (`printf(message, demoversion, ...)`)
/// y era donde se paraba el unity build de las 56.465 lineas. Si esta fila
/// falla con un error de compilacion, es que se volvio a la ruta de solo
/// literales; si falla con otro texto, es el formateador.
#[test]
fn printf_con_el_formato_en_una_variable() {
    let out = corre(
        "#include <stdio.h>",
        r#"
int main() {
    char *f;
    f = "%s tiene %d anos\n";
    printf(f, "andre", 26);
    return 0;
}
"#,
    );
    assert_eq!(out, "andre tiene 26 anos\n");
}

/// ** LA ANCHURA SE APLICA, y el `printf` en linea no puede aplicarla.
///
/// Su propio comentario lo dice: lee el `7` de `%7i` y lo tira, porque sus
/// conversores escriben directo a la consola y para rellenar hay que saber
/// cuanto ocupa el numero ANTES. Aqui el numero se arma en un array primero.
///
/// Las cuatro columnas son las cuatro combinaciones que existen: derecha,
/// izquierda, ceros y una cadena recortada por precision.
#[test]
fn la_anchura_y_las_banderas_se_aplican_de_verdad() {
    let out = corre(
        "#include <stdio.h>",
        r#"
int main() {
    char b[64];
    snprintf(b, 64, "[%5d][%-5d][%05d][%.3s]", 42, 42, 42, "abcdef");
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "[   42][42   ][00042][abc]\n");
}

/// **** El `va_list` VIAJA a otra funcion. Es el patron entero de la familia
/// `v*` y lo que `__va_arg(i)` no podia hacer: un indice describe una posicion
/// en el marco de quien pregunta, y en la funcion de destino ya no apunta a
/// nada.
///
/// Esta fila es la de `M_snprintf`/`M_vsnprintf` de `m_misc.c`, calcada.
#[test]
fn el_va_list_sobrevive_a_pasarlo_a_otra_funcion() {
    let out = corre(
        "#include <stdio.h>",
        r#"
int mi_snprintf(char *b, unsigned long long n, const char *s, ...) {
    va_list args;
    va_start(args, s);
    return vsnprintf(b, n, s, args);
}
int main() {
    char b[64];
    mi_snprintf(b, 64, "%s=%d,%s=%d", "alto", 200, "ancho", 320);
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "alto=200,ancho=320\n");
}

/// ** `snprintf` NO desborda y devuelve lo que HABRIA escrito.
///
/// Las dos mitades importan y son distintas: truncar sin decirlo da un buffer
/// correcto y un programa que cree que cabia. El `+1 < lim` de `bmo_fmt_byte`
/// es lo que deja sitio al cero final -- si alguien lo pone en `< lim`, esta
/// fila caza el byte de mas.
#[test]
fn snprintf_trunca_sin_desbordar_y_dice_cuanto_falto() {
    let out = corre(
        "#include <stdio.h>",
        r#"
char b[16];
int main() {
    int i;
    int n;
    for (i = 0; i < 16; i = i + 1) { b[i] = '#'; }
    n = snprintf(b, 8, "%s", "0123456789");
    printf("[%s] %d %d\n", b, n, (int)b[8]);
    return 0;
}
"#,
    );
    assert_eq!(
        out, "[0123456] 10 35\n",
        "siete bytes y el cero; devuelve 10; y el byte 8 sigue siendo '#' (35)"
    );
}

/// ** El hexadecimal va por DESPLAZAMIENTO y mascara, no por division.
///
/// La division de BMO C es `idiv` --con signo-- asi que un valor con el bit
/// alto puesto saldria negativo. El `& 15` tras el `>> 4` convierte el `sar`
/// que emite el compilador en el desplazamiento logico que hace falta. Si
/// alguien cambia esa linea por una division, esta fila lo dice.
#[test]
fn el_hexadecimal_aguanta_el_bit_alto() {
    let out = corre(
        "#include <stdio.h>",
        r#"
int main() {
    char b[32];
    snprintf(b, 32, "%x %X %x", 255, 255, -1);
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "ff FF ffffffffffffffff\n");
}

/// Una conversion que no se entiende se escribe TAL CUAL y **no consume
/// argumento**. Inventar un numero para un `%f` seria peor que no imprimirlo, y
/// consumir el argumento descolocaria todos los que vienen detras -- que es la
/// clase de fallo que aparece tres conversiones mas alla del sitio culpable.
#[test]
fn una_conversion_desconocida_no_se_come_el_argumento() {
    let out = corre(
        "#include <stdio.h>",
        r#"
int main() {
    char b[32];
    snprintf(b, 32, "%f=%d", 7);
    printf("%s\n", b);
    return 0;
}
"#,
    );
    assert_eq!(out, "%f=7\n");
}

// == EL PAQUETE, LEIDO POR EL PROGRAMA =================================

/// ***** LA FILA QUE CIERRA EL CIRCULO: **el programa lee los datos que viajan
/// dentro de su propia imagen**.
///
/// Se compila el programa, se EMPAQUETA su `.bex` con dos recursos, y el
/// paquete resultante se siembra en el disco del emulador con la ruta desde la
/// que el programa se abre a si mismo. O sea: la caja se lee **en el sitio**,
/// no se copia a ningun lado.
///
/// Los tres saltos que hace por dentro --cabecera BEF, tabla de secciones,
/// indice "BRES"-- son un `fseek` y un `fread` cada uno. Si el formato que
/// escribe `bmo-pack` y el que lee `<bmo/paquete.h>` dejaran de coincidir, es
/// esta fila la que lo dice: son dos implementaciones del MISMO formato, una en
/// Rust y otra en C, y esa es toda la razon de que esta prueba valga.
#[test]
fn un_programa_lee_los_recursos_de_su_propia_imagen() {
    let fuente = r#"
#include <bmo/paquete.h>
int main() {
    PAQUETE *p;
    char *b;
    unsigned long long n;
    p = paquete_abrir("apps/app.bex");
    if (p == 0) { printf("no es un paquete\n"); return 1; }
    printf("recursos=%d\n", (int)paquete_cuantos(p));
    b = (char *)malloc(64);
    n = paquete_leer(p, "saludo.txt", b, 64);
    b[n] = 0;
    printf("[%s] %d\n", b, (int)n);
    n = paquete_leer(p, "no-esta", b, 64);
    printf("falta=%d\n", (int)n);
    return 0;
}
"#;
    let bef = compile_with_preprocessor(fuente, std::path::Path::new("p.c"), CStandard::C11)
        .expect("debe compilar");
    let paquete = bmo_abi::bef::paquete::empaquetar(
        &bef,
        &[("saludo.txt", b"hola desde dentro"), ("otro.bin", &[7u8; 40])],
    )
    .expect("debe empaquetar");

    // El programa que corre es el MISMO que esta dentro del paquete: se ejecuta
    // la imagen y se le pone su paquete en el disco, en su ruta.
    let copia = paquete.clone();
    let out = ejecutar_bef_con(&bef, move |m| m.poner_archivo("apps/app.bex", &copia));
    assert_eq!(out, "recursos=2\n[hola desde dentro] 17\nfalta=0\n");
}

/// Un `.bex` SIN recursos --que es lo que son todos los de hoy-- contesta "no
/// es un paquete" y no revienta. Que la mayoria de los programas no lleven
/// datos dentro es el caso normal, no el error.
#[test]
fn un_bex_sin_recursos_lo_dice_y_no_revienta() {
    let fuente = r#"
#include <bmo/paquete.h>
int main() {
    PAQUETE *p;
    p = paquete_abrir("apps/app.bex");
    if (p == 0) { printf("no es un paquete\n"); return 0; }
    printf("recursos=%d\n", (int)paquete_cuantos(p));
    return 0;
}
"#;
    let bef = compile_with_preprocessor(fuente, std::path::Path::new("p.c"), CStandard::C11)
        .expect("debe compilar");
    let copia = bef.clone();
    let out = ejecutar_bef_con(&bef, move |m| m.poner_archivo("apps/app.bex", &copia));
    assert_eq!(out, "no es un paquete\n");
}

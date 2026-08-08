//! El preprocesador: macros con parametros
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// =============== Macros CON PARAMETROS ===============
//
// El preprocesador las guardaba y no las expandia nunca: el `if` de
// `expand_line` pedia `params.is_empty()`. `MAX(a,b)` se quedaba en el
// texto y el parser lo tomaba por una llamada a una funcion inexistente.

#[test]
fn una_macro_con_parametros_se_expande() {
    let out = run_c_con_pp(
        "#define DOBLE(x) ((x) + (x))\n\
         int main() { printf(\"%d\\n\", DOBLE(21)); return 0; }",
    );
    assert_eq!(out.trim(), "42");
}

/// Los parentesis del cuerpo no son adorno: sin ellos `DOBLE(1+1)` daria
/// `1+1+1+1`. Se comprueba que el argumento entra ENTERO.
#[test]
fn el_argumento_entra_entero_no_troceado() {
    let out = run_c_con_pp(
        "#define TRIPLE(x) ((x) * 3)\n\
         int main() { printf(\"%d\\n\", TRIPLE(2 + 5)); return 0; }",
    );
    assert_eq!(out.trim(), "21");
}

/// Una coma DENTRO de parentesis no separa argumentos. Sin esto,
/// `MAX(f(a,b), c)` se leeria como tres.
#[test]
fn las_comas_anidadas_no_separan_argumentos() {
    let out = run_c_con_pp(
        "#define SUMA(a, b) ((a) + (b))\n\
         int main() { printf(\"%d\\n\", SUMA(SUMA(1, 2), 4)); return 0; }",
    );
    assert_eq!(out.trim(), "7");
}

/// * El espacio manda, y es el unico sitio de C donde manda.
///
/// `#define X (760)` es un OBJETO cuyo cuerpo empieza por parentesis. El
/// lector viejo lo registraba como macro-funcion con un parametro llamado
/// `760` y cuerpo **vacio**: la constante desaparecia en silencio.
#[test]
fn un_parentesis_separado_del_nombre_no_hace_una_funcion() {
    let out = run_c_con_pp(
        "#define ANCHO (760)\n\
         int main() { printf(\"%d\\n\", ANCHO); return 0; }",
    );
    assert_eq!(out.trim(), "760");
}

/// Y pegado si: una funcion SIN parametros no es lo mismo que un objeto.
#[test]
fn una_macro_funcion_sin_parametros_se_invoca_con_parentesis() {
    let out = run_c_con_pp(
        "#define UNO() 1\n\
         int main() { printf(\"%d\\n\", UNO()); return 0; }",
    );
    assert_eq!(out.trim(), "1");
}

/// `#p` convierte el argumento en cadena. Es lo que hace posible un
/// `assert` que dice QUE fallo.
#[test]
fn el_sostenido_convierte_el_argumento_en_cadena() {
    let out = run_c_con_pp(
        "#define NOMBRE(x) #x\n\
         int main() { printf(\"%s\\n\", NOMBRE(hola)); return 0; }",
    );
    assert_eq!(out.trim(), "hola");
}

/// `##` pega dos piezas en UN simbolo, comiendose el espacio de los lados.
#[test]
fn el_doble_sostenido_pega_dos_piezas() {
    let out = run_c_con_pp(
        "#define UNE(a, b) a ## b\n\
         int main() { int xy; xy = 9; printf(\"%d\\n\", UNE(x, y)); return 0; }",
    );
    assert_eq!(out.trim(), "9");
}

/// Variadicas: lo que sobra entra por `__VA_ARGS__`.
#[test]
fn una_macro_variadica_pasa_el_resto() {
    let out = run_c_con_pp(
        "#define DI(fmt, ...) printf(fmt, __VA_ARGS__)\n\
         int main() { DI(\"%d-%d\\n\", 4, 7); return 0; }",
    );
    assert_eq!(out.trim(), "4-7");
}

/// Una macro que produce otra macro: hacen falta varias pasadas.
#[test]
fn una_macro_puede_producir_otra() {
    let out = run_c_con_pp(
        "#define A B\n#define B 5\n\
         int main() { printf(\"%d\\n\", A); return 0; }",
    );
    assert_eq!(out.trim(), "5");
}

/// * Ya NO se sustituye dentro de las cadenas. Antes `printf(\"ANCHO\")`
/// imprimia el valor: el texto de un literal es dato, no codigo.
#[test]
fn una_macro_no_se_expande_dentro_de_una_cadena() {
    let out = run_c_con_pp(
        "#define ANCHO 760\n\
         int main() { printf(\"ANCHO=%d\\n\", ANCHO); return 0; }",
    );
    assert_eq!(out.trim(), "ANCHO=760");
}

/// Invocarla con un numero de argumentos que no cuadra es un ERROR. Antes
/// no podia serlo: la macro no se expandia, asi que la llamada sobrevivia
/// hasta el codegen.
#[test]
fn invocar_una_macro_con_argumentos_de_mas_es_un_error() {
    let err = compile_with_preprocessor(
        "#define SUMA(a, b) ((a) + (b))\nint main() { return SUMA(1, 2, 3); }",
        std::path::Path::new("prueba.c"),
        CStandard::C11,
    )
    .expect_err("tres argumentos para dos parametros tiene que fallar");
    assert!(err.message.contains("SUMA"), "mensaje: {}", err.message);
}

/// Una macro que se nombra a si misma no puede colgar el compilador.
#[test]
fn una_macro_recursiva_no_cuelga() {
    let out = run_c_con_pp(
        "#define A A\n\
         int main() { printf(\"ok\\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "ok");
}

/// Matriz de conformidad de C: ejecuta TODO lo que el codegen dice
/// soportar y compara la salida real.
///
/// Cuando se escribio por primera vez, 18 de 36 casos fallaban -- entre
/// ellos que NINGUN bucle daba mas de una vuelta y que `switch` siempre
/// entraba por el primer caso. Todos compilaban y validaban.
///
/// Si anades una caracteristica al codegen, anadele aqui su fila. Es la
/// unica forma de que "soportado" signifique algo.

/// `#define` SUSTITUYE de verdad -- no se traga la linea y sigue.
///
/// La pregunta no es si COMPILA (tragarse una linea tambien compila) sino
/// si el valor LLEGA. Este test lo fija ejecutandolo: si algun dia el
/// preprocesador deja de expandir, aqui sale `0` en vez de `5`.
///
/// Y de paso documenta una asimetria real: el preprocesador SOLO corre en
/// `compile_with_preprocessor`, que es lo que usa la linea de ordenes. El
/// camino de biblioteca (`compile_source_to_bef`) no lo llama.
#[test]
fn el_define_sustituye_de_verdad() {
    let salida = run_c_con_pp("#define CINCO 5
int main(void){ printf(\"%d\", CINCO); return 0; }");
    assert_eq!(salida, "5", "el #define tiene que SUSTITUIR, no ignorarse");
}

/// * Y sin preprocesador, una directiva se RECHAZA en vez de ignorarse.
///
/// Esto es lo que estaba mal: el catch-all del lexer se tragaba el `#`, asi
/// que un `#define` dentro de una funcion **compilaba y no hacia nada** --
/// el programa corria con la constante sin sustituir y nadie decia una
/// palabra. Al principio del fichero daba un "expected type, got
/// Ident(define)", que manda a mirar donde no es.
#[test]
fn una_directiva_sin_preprocesador_se_rechaza() {
    // Dentro de una funcion: era el caso silencioso.
    let e = compile_source_to_bef("int main(void){
#define X 5
 return 0; }")
        .unwrap_err();
    assert!(format!("{e:?}").contains("no hay preprocesador"), "{e:?}");
    // Y al principio del fichero, con el mismo mensaje.
    let e = compile_source_to_bef("#define X 5
int main(void){ return 0; }").unwrap_err();
    assert!(format!("{e:?}").contains("no hay preprocesador"), "{e:?}");
}

// =============== Tres fallos que vivian en la FASE 3 ===============
//
// El estandar de C borra los comentarios ANTES de que el preprocesador mire
// una sola directiva (fase 3 de traduccion). BMO C no lo hacia, y de ahi
// salieron los tres de abajo. Ninguno tenia prueba: se cazaron los tres
// escribiendo un COMENTARIO en `raycaster_C.c` el 2026-08-08.

/// * El comentario de un `#define` no es parte del cuerpo de la macro.
///
/// `#define UNO 65536 /* 1.0 en 16.16 */` definia `UNO` como
/// `65536 /* 1.0 en 16.16 */`, comentario incluido. En codigo no se notaba
/// --un comentario de mas donde ya iba a haber uno--, pero la expansion se
/// aplica tambien DENTRO de los comentarios: nombrar esa macro en un
/// comentario metia ahi un `*/` que **lo cerraba antes de tiempo**, y el
/// resto del parrafo pasaba a ser codigo. El error salia varias lineas mas
/// abajo, en una linea que no tenia nada malo.
#[test]
fn el_comentario_de_un_define_no_entra_en_el_cuerpo() {
    let out = run_c_con_pp(
        "#define UNO 65536   /* 1.0 en 16.16 */
int main() {
    /* el tope del rayo es 20 * UNO, y decirlo aqui no puede romper nada. */
    printf(\"%d\", UNO);
    return 0;
}",
    );
    assert_eq!(out.trim(), "65536", "el cuerpo se llevo el comentario detras");
}

/// Y el comentario vale por UN espacio, no por nada: `1/**/+/**/1` son tres
/// piezas y suman 2. Borrarlo del todo pegaria `1+1` -- que aqui da igual,
/// pero `A/**/B` se convertiria en el identificador `AB`.
#[test]
fn el_comentario_de_un_define_vale_por_un_espacio() {
    let out = run_c_con_pp(
        "#define DOS 1/**/+/**/1
int main() { printf(\"%d\", DOS); return 0; }",
    );
    assert_eq!(out.trim(), "2");
}

/// ** Una `n` en un literal no puede multiplicar el binario por 65.536.
///
/// `expandir_una_pasada` y `copiar_literal` copiaban byte a byte con
/// `b[i] as char`, que lee cada byte como Latin-1: los DOS bytes UTF-8 de la
/// `n` salian como dos caracteres que al recodificarse ocupan CUATRO. Y como
/// el bucle repite mientras algo cambie --y esto cambiaba siempre-- hasta 16
/// veces, el factor es 2^16.
///
/// Medido antes del arreglo: un `hola mundo` con una sola `n` daba un `.bex`
/// de **492.032 bytes**, y donde iba la `n` habia 65.536 bytes de basura. Con
/// `MAX_BEX` en 1 MiB, dos palabras acentuadas dejaban un programa que ya no
/// carga -- o sea que un acento se manifestaba como un problema de TAMANO, que
/// es el ultimo sitio donde uno lo busca.
#[test]
fn una_enye_no_multiplica_el_binario() {
    let fuente = "int main() { printf(\"ma\u{f1}ana\"); return 0; }";
    let bef = compile_with_preprocessor(fuente, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("debe compilar");
    assert!(
        bef.len() < 4096,
        "el .bex mide {} bytes: la expansion volvio a inflarse",
        bef.len()
    );
    let esperado = "ma\u{f1}ana".as_bytes();
    assert!(
        bef.windows(esperado.len()).any(|v| v == esperado),
        "el literal no sobrevivio entero al preprocesador"
    );
}

/// Lo mismo pero en un COMENTARIO, que es el otro camino: ahi no hay literal
/// que copiar y quien mangla es `expandir_una_pasada`. Un parrafo con acentos
/// y rayas es lo normal en este repositorio, asi que esto se ejerce en cada
/// fichero que se compila.
#[test]
fn los_acentos_de_un_comentario_no_inflan_nada() {
    let fuente = "int main() {
    /* \u{2605} La l\u{ed}nea de arriba \u{2014}y \u{e9}sta\u{2014} llevan acento. */
    printf(\"ok\");
    return 0;
}";
    let bef = compile_with_preprocessor(fuente, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("debe compilar");
    assert!(bef.len() < 4096, "el .bex mide {} bytes", bef.len());
}

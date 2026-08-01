//! El preprocesador: macros con parametros
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ Macros CON PARÁMETROS ═══════════════
//
// El preprocesador las guardaba y no las expandía nunca: el `if` de
// `expand_line` pedía `params.is_empty()`. `MAX(a,b)` se quedaba en el
// texto y el parser lo tomaba por una llamada a una función inexistente.

#[test]
fn una_macro_con_parametros_se_expande() {
    let out = run_c_con_pp(
        "#define DOBLE(x) ((x) + (x))\n\
         int main() { printf(\"%d\\n\", DOBLE(21)); return 0; }",
    );
    assert_eq!(out.trim(), "42");
}

/// Los paréntesis del cuerpo no son adorno: sin ellos `DOBLE(1+1)` daría
/// `1+1+1+1`. Se comprueba que el argumento entra ENTERO.
#[test]
fn el_argumento_entra_entero_no_troceado() {
    let out = run_c_con_pp(
        "#define TRIPLE(x) ((x) * 3)\n\
         int main() { printf(\"%d\\n\", TRIPLE(2 + 5)); return 0; }",
    );
    assert_eq!(out.trim(), "21");
}

/// Una coma DENTRO de paréntesis no separa argumentos. Sin esto,
/// `MAX(f(a,b), c)` se leería como tres.
#[test]
fn las_comas_anidadas_no_separan_argumentos() {
    let out = run_c_con_pp(
        "#define SUMA(a, b) ((a) + (b))\n\
         int main() { printf(\"%d\\n\", SUMA(SUMA(1, 2), 4)); return 0; }",
    );
    assert_eq!(out.trim(), "7");
}

/// ★ El espacio manda, y es el único sitio de C donde manda.
///
/// `#define X (760)` es un OBJETO cuyo cuerpo empieza por paréntesis. El
/// lector viejo lo registraba como macro-función con un parámetro llamado
/// `760` y cuerpo **vacío**: la constante desaparecía en silencio.
#[test]
fn un_parentesis_separado_del_nombre_no_hace_una_funcion() {
    let out = run_c_con_pp(
        "#define ANCHO (760)\n\
         int main() { printf(\"%d\\n\", ANCHO); return 0; }",
    );
    assert_eq!(out.trim(), "760");
}

/// Y pegado sí: una función SIN parámetros no es lo mismo que un objeto.
#[test]
fn una_macro_funcion_sin_parametros_se_invoca_con_parentesis() {
    let out = run_c_con_pp(
        "#define UNO() 1\n\
         int main() { printf(\"%d\\n\", UNO()); return 0; }",
    );
    assert_eq!(out.trim(), "1");
}

/// `#p` convierte el argumento en cadena. Es lo que hace posible un
/// `assert` que dice QUÉ falló.
#[test]
fn el_sostenido_convierte_el_argumento_en_cadena() {
    let out = run_c_con_pp(
        "#define NOMBRE(x) #x\n\
         int main() { printf(\"%s\\n\", NOMBRE(hola)); return 0; }",
    );
    assert_eq!(out.trim(), "hola");
}

/// `##` pega dos piezas en UN símbolo, comiéndose el espacio de los lados.
#[test]
fn el_doble_sostenido_pega_dos_piezas() {
    let out = run_c_con_pp(
        "#define UNE(a, b) a ## b\n\
         int main() { int xy; xy = 9; printf(\"%d\\n\", UNE(x, y)); return 0; }",
    );
    assert_eq!(out.trim(), "9");
}

/// Variádicas: lo que sobra entra por `__VA_ARGS__`.
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

/// ★ Ya NO se sustituye dentro de las cadenas. Antes `printf(\"ANCHO\")`
/// imprimía el valor: el texto de un literal es dato, no código.
#[test]
fn una_macro_no_se_expande_dentro_de_una_cadena() {
    let out = run_c_con_pp(
        "#define ANCHO 760\n\
         int main() { printf(\"ANCHO=%d\\n\", ANCHO); return 0; }",
    );
    assert_eq!(out.trim(), "ANCHO=760");
}

/// Invocarla con un número de argumentos que no cuadra es un ERROR. Antes
/// no podía serlo: la macro no se expandía, así que la llamada sobrevivía
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

/// Una macro que se nombra a sí misma no puede colgar el compilador.
#[test]
fn una_macro_recursiva_no_cuelga() {
    let out = run_c_con_pp(
        "#define A A\n\
         int main() { printf(\"ok\\n\"); return 0; }",
    );
    assert_eq!(out.trim(), "ok");
}

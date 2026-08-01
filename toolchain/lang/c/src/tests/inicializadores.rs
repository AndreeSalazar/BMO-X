//! Listas de inicializacion, posicionales y designadas
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ Listas de inicialización ═══════════════
//
// No existían: ni siquiera `int a[3] = {1,2,3}`. Ver la cabecera de
// `parser/inicializador.rs` para el diseño y para qué hicieron GCC, Clang,
// chibicc, TCC y MSVC con esto mismo.

#[test]
fn una_lista_posicional_llena_un_array() {
    let out = run_c("int main() { int a[4] = {10, 20, 30, 40}; \
                     printf(\"%d %d %d\\n\", a[0], a[2], a[3]); return 0; }");
    assert_eq!(out.trim(), "10 30 40");
}

#[test]
fn una_lista_posicional_llena_un_struct() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int main() { struct P p = {1, 2, 3}; \
                     printf(\"%d %d %d\\n\", p.x, p.y, p.z); return 0; }");
    assert_eq!(out.trim(), "1 2 3");
}

/// ★ C99 §6.7.9/21: lo NO mencionado vale CERO.
///
/// Sin el borrado previo, `q.x` y `q.z` traerían lo que hubiera en la pila
/// — basura distinta en cada ejecución, y un bug que no se repite.
#[test]
fn lo_no_mencionado_vale_cero() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int main() { struct P q = {.y = 7}; \
                     printf(\"%d %d %d\\n\", q.x, q.y, q.z); return 0; }");
    assert_eq!(out.trim(), "0 7 0");
}

/// Los designadores pueden ir en cualquier orden: el offset lo pone el
/// nombre, no la posición.
#[test]
fn los_designadores_van_en_el_orden_que_quieran() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int main() { struct P r = {.z = 9, .x = 5}; \
                     printf(\"%d %d %d\\n\", r.x, r.y, r.z); return 0; }");
    assert_eq!(out.trim(), "5 0 9");
}

/// ★ La regla que más se olvida al implementar esto a mano: un designador
/// **reposiciona el cursor**, y lo siguiente sin designador sigue DESDE
/// AHÍ. La `d` va al índice 3, no al 0.
#[test]
fn tras_un_designador_se_sigue_desde_ahi() {
    let out = run_c("int main() { int b[5] = {[2] = 30, 40}; \
                     printf(\"%d %d %d %d\\n\", b[0], b[2], b[3], b[4]); return 0; }");
    assert_eq!(out.trim(), "0 30 40 0");
}

/// El último gana, y sale solo de emitir en orden.
#[test]
fn si_un_campo_se_inicializa_dos_veces_gana_el_ultimo() {
    let out = run_c("struct P { int x; int y; }; \
                     int main() { struct P p = {.x = 1, .y = 2, .x = 9}; \
                     printf(\"%d %d\\n\", p.x, p.y); return 0; }");
    assert_eq!(out.trim(), "9 2");
}

/// Anidado: `{ {..}, {..} }` sobre un array de structs.
#[test]
fn una_lista_anidada_recorre_los_subobjetos() {
    let out = run_c("struct P { int x; int y; }; \
                     int main() { struct P v[2] = { {1, 2}, {.y = 4} }; \
                     printf(\"%d %d %d %d\\n\", v[0].x, v[0].y, v[1].x, v[1].y); return 0; }");
    assert_eq!(out.trim(), "1 2 0 4");
}

/// Cadena de designadores: `[1].y = …` es legal C99.
#[test]
fn una_cadena_de_designadores_baja_dos_niveles() {
    let out = run_c("struct P { int x; int y; }; \
                     int main() { struct P v[3] = {[2].y = 8}; \
                     printf(\"%d %d\\n\", v[2].x, v[2].y); return 0; }");
    assert_eq!(out.trim(), "0 8");
}

/// Una cadena inicializa un `char[]` **byte a byte**. Es la única forma en
/// C de inicializar un agregado sin llaves.
#[test]
fn una_cadena_llena_un_array_de_char() {
    let out = run_c("int main() { char s[8] = \"hola\"; \
                     printf(\"%s|%d\\n\", s, s[4]); return 0; }");
    assert_eq!(out.trim(), "hola|0");
}

/// Y si no cabe, se dice. Escribir uno de más pisaría lo de al lado.
#[test]
fn una_cadena_que_no_cabe_es_un_error() {
    let err = compile_source_to_bef("int main() { char s[3] = \"hola\"; return 0; }")
        .expect_err("cinco bytes en un array de tres tiene que fallar");
    assert!(err.message.contains("array"), "mensaje: {}", err.message);
}

/// Un escalar entre llaves es legal.
#[test]
fn un_escalar_admite_llaves() {
    let out = run_c("int main() { int x = {5}; printf(\"%d\\n\", x); return 0; }");
    assert_eq!(out.trim(), "5");
}

/// Sobrarse del array es un error, no un desbordamiento silencioso.
#[test]
fn pasarse_del_final_del_array_es_un_error() {
    let err = compile_source_to_bef("int main() { int a[2] = {1,2,3}; return a[0]; }")
        .expect_err("tres valores en un array de dos tiene que fallar");
    assert!(err.message.contains("elementos"), "mensaje: {}", err.message);
}

/// Un campo que no existe se dice con el nombre delante.
#[test]
fn un_campo_inventado_es_un_error() {
    let err = compile_source_to_bef(
        "struct P { int x; }; int main() { struct P p = {.pepe = 1}; return 0; }",
    )
    .expect_err("un campo que no existe tiene que fallar");
    assert!(err.message.contains("pepe"), "mensaje: {}", err.message);
}

/// ★ La declaración se parsea en TRES sitios (cuerpo de función, bloque
/// anidado, `parse_stmt`) y estaba copiada en los tres. Al añadir las
/// listas sólo aprendió uno: dentro de un `if`, `int a[2] = {…}` no
/// compilaba. Ahora los tres llaman a `terminar_declaracion`.
#[test]
fn una_lista_tambien_compila_dentro_de_un_bloque() {
    let out = run_c("int main() { if (1) { int a[2] = {7, 8}; \
                     printf(\"%d %d\\n\", a[0], a[1]); } return 0; }");
    assert_eq!(out.trim(), "7 8");
}

/// ★ Y el emulador tiene que escribir el tamaño EXACTO.
///
/// `mov [mem], eax` toca CUATRO bytes; el emulador escribía ocho rellenando
/// de ceros. En un registro eso es correcto —escribir uno de 32 bits borra
/// la mitad alta— pero en memoria destruye lo de al lado. Este caso lo
/// destapó: con `{.x = 1, .y = 2, .x = 9}`, la última escritura de `x`
/// borraba la `y` de detrás y salía `9 0`.
///
/// Un emulador que hace fallar código correcto es peor que uno que no
/// existe: manda a buscar el bug al sitio equivocado.
#[test]
fn escribir_un_campo_no_toca_al_de_al_lado() {
    let out = run_c("struct P { int x; int y; }; \
                     int main() { struct P p; p.y = 77; p.x = 5; \
                     printf(\"%d %d\\n\", p.x, p.y); return 0; }");
    assert_eq!(out.trim(), "5 77");
}

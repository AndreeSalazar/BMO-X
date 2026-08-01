//! Structs POR VALOR: copiar, pasar y (no) devolver
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ Structs POR VALOR ═══════════════
//
// Ver `codegen/agregados.rs` para la ABI de agregados de BMO y para qué
// hacen SysV (clasificación por eightbytes) y Win64 (referencia oculta).

/// `q = p` copia TODOS los bytes. Antes emitía `mov rax,[p]; mov [q],rax`
/// — ocho— y un struct de 12 se copiaba a medias, en silencio.
#[test]
fn asignar_un_struct_copia_todos_sus_bytes() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int main() { struct P p = {1, 2, 3}; struct P q; q = p; \
                     printf(\"%d %d %d\\n\", q.x, q.y, q.z); return 0; }");
    assert_eq!(out.trim(), "1 2 3");
}

/// Y es una COPIA: tocar el destino no toca el origen.
#[test]
fn la_copia_de_un_struct_es_independiente() {
    let out = run_c("struct P { int x; int y; }; \
                     int main() { struct P p = {1, 2}; struct P q; q = p; q.y = 99; \
                     printf(\"%d %d\\n\", p.y, q.y); return 0; }");
    assert_eq!(out.trim(), "2 99");
}

/// Pasarlo a una función manda sus bytes, no su primera palabra.
#[test]
fn un_struct_viaja_entero_a_una_funcion() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int suma(struct P p) { return p.x + p.y + p.z; } \
                     int main() { struct P p = {1, 2, 3}; \
                     printf(\"%d\\n\", suma(p)); return 0; }");
    assert_eq!(out.trim(), "6");
}

/// ★ Y corre a los que vienen detrás. Un agregado de 12 bytes ocupa DOS
/// ranuras; con el `16 + i*8` de antes, el parámetro siguiente se leía
/// desde la mitad del anterior.
#[test]
fn un_struct_corre_los_parametros_que_van_detras() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int mezcla(int a, struct P p, int b) { return a * 100 + p.y + b; } \
                     int main() { struct P p = {1, 2, 3}; \
                     printf(\"%d\\n\", mezcla(7, p, 5)); return 0; }");
    assert_eq!(out.trim(), "707");
}

/// La función recibe una COPIA: modificarla no toca la del llamante.
#[test]
fn la_funcion_recibe_una_copia_no_el_original() {
    let out = run_c("struct P { int x; int y; }; \
                     int rompe(struct P p) { p.x = 99; return p.x; } \
                     int main() { struct P p = {1, 2}; int r; r = rompe(p); \
                     printf(\"%d %d\\n\", r, p.x); return 0; }");
    assert_eq!(out.trim(), "99 1");
}

/// Devolver un struct es un TERCER mecanismo (puntero oculto) y todavía no
/// está. Se dice con el nombre delante: devolver ocho bytes de un struct de
/// doce sería exactamente la mentira que este compilador no cuenta.
#[test]
fn devolver_un_struct_por_valor_se_rechaza_con_motivo() {
    let err = compile_source_to_bef(
        "struct P { int x; int y; }; \
         struct P haz() { struct P p = {1,2}; return p; } \
         int main() { struct P q; q = haz(); return q.x; }",
    )
    .expect_err("devolver un struct todavia no se compila");
    assert!(err.message.contains("haz"), "mensaje: {}", err.message);
}

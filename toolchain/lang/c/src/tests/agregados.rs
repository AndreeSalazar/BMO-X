//! Structs POR VALOR: copiar, pasar y (no) devolver
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// =============== Structs POR VALOR ===============
//
// Ver `codegen/agregados.rs` para la ABI de agregados de BMO y para que
// hacen SysV (clasificacion por eightbytes) y Win64 (referencia oculta).

/// `q = p` copia TODOS los bytes. Antes emitia `mov rax,[p]; mov [q],rax`
/// -- ocho-- y un struct de 12 se copiaba a medias, en silencio.
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

/// Pasarlo a una funcion manda sus bytes, no su primera palabra.
#[test]
fn un_struct_viaja_entero_a_una_funcion() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int suma(struct P p) { return p.x + p.y + p.z; } \
                     int main() { struct P p = {1, 2, 3}; \
                     printf(\"%d\\n\", suma(p)); return 0; }");
    assert_eq!(out.trim(), "6");
}

/// * Y corre a los que vienen detras. Un agregado de 12 bytes ocupa DOS
/// ranuras; con el `16 + i*8` de antes, el parametro siguiente se leia
/// desde la mitad del anterior.
#[test]
fn un_struct_corre_los_parametros_que_van_detras() {
    let out = run_c("struct P { int x; int y; int z; }; \
                     int mezcla(int a, struct P p, int b) { return a * 100 + p.y + b; } \
                     int main() { struct P p = {1, 2, 3}; \
                     printf(\"%d\\n\", mezcla(7, p, 5)); return 0; }");
    assert_eq!(out.trim(), "707");
}

/// La funcion recibe una COPIA: modificarla no toca la del llamante.
#[test]
fn la_funcion_recibe_una_copia_no_el_original() {
    let out = run_c("struct P { int x; int y; }; \
                     int rompe(struct P p) { p.x = 99; return p.x; } \
                     int main() { struct P p = {1, 2}; int r; r = rompe(p); \
                     printf(\"%d %d\\n\", r, p.x); return 0; }");
    assert_eq!(out.trim(), "99 1");
}

/// Devolver un struct es un TERCER mecanismo (puntero oculto) y todavia no
/// esta. Se dice con el nombre delante: devolver ocho bytes de un struct de
/// doce seria exactamente la mentira que este compilador no cuenta.
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

// -- Arrays DENTRO de un agregado --------------------------------------
//
// La sonda de c-gen los encontro en una `union`, pero fallaban **igual en un
// struct**: no era el agregado, era el declarador. Un `char nombre[8]` dentro
// de una estructura es lo primero que trae cualquier formato de fichero -- DOOM
// nombra asi cada lump de su WAD.

/// El array convive con los otros campos y **no los pisa**: es lo que prueba
/// que el reparto de offsets conto su tamano entero y no el de un elemento.
#[test]
fn un_struct_puede_llevar_un_array_dentro() {
    let fuente = "struct S { int i; char c[4]; int z; }; \
                  int main() { struct S s; s.i = 11; s.z = 22; \
                  s.c[0] = 7; s.c[3] = 9; \
                  printf(\"%d,%d,%d,%d\", s.i, s.c[0], s.c[3], s.z); return 0; }";
    assert_eq!(run_c(fuente), "11,7,9,22");
}

/// Y en una union, donde el array **comparte** el sitio con lo demas: escribir
/// el entero se tiene que ver por los bytes. Es la razon de existir de una
/// union, y lo que un test de compilacion no mira.
#[test]
fn una_union_reparte_el_mismo_sitio_entre_el_entero_y_los_bytes() {
    let fuente = "union U { int i; char c[4]; }; \
                  int main() { union U u; u.i = 0; u.c[0] = 65; \
                  printf(\"%d\", u.i); return 0; }";
    assert_eq!(run_c(fuente), "65");
}

/// Un campo de bits se ACEPTA y **guarda lo que le metas**. No esta
/// empaquetado --la estructura mide mas de lo que mediria en GCC-- y eso esta
/// dicho en BRECHA.md: lo que no vale es un layout binario ajeno, no el
/// programa.
#[test]
fn un_campo_de_bits_se_acepta_y_guarda_su_valor() {
    let fuente = "struct F { unsigned a:3; unsigned b:5; }; \
                  int main() { struct F f; f.a = 5; f.b = 17; \
                  printf(\"%d,%d\", f.a, f.b); return 0; }";
    assert_eq!(run_c(fuente), "5,17");
}

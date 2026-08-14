//! **Un parametro de tipo array decae a puntero.** C99 6.7.5.3/7.
//!
//! # El bug que mato a DOOM, y por que costo cuatro sondas encontrarlo
//!
//! `SHA1_Final(sha1_digest_t digest, sha1_context_t *hd)` con
//! `typedef byte sha1_digest_t[20]`. El decaimiento estaba escrito **dentro
//! del `if` que mira si el parametro lleva corchetes**, asi que un tipo que
//! YA era un array --porque lo traia un `typedef`-- se colaba entero.
//!
//! Medido en el codigo maquina, antes y despues:
//!
//! ```text
//!   ANTES   mov rax, [rbp+0x28]    arg2 a 24 bytes: el array ocupo 3 ranuras
//!   AHORA   mov rax, [rbp+0x18]    arg2 a 8: una ranura, como un puntero
//! ```
//!
//! No da un error de compilacion: da que **todo argumento detras del array
//! quede corrido 16 bytes**. `hd` se lee de la ranura equivocada, llega
//! basura, y el primer `hd->count` revienta con `#GP`.
//!
//! ** Y por eso las otras cuatro sondas salieron verdes: la disposicion del
//! struct, los typedefs encadenados, el reenvio de punteros a locales y el
//! typedef adelantado estaban TODOS bien. El fallo no estaba en el struct --
//! estaba en el argumento de al lado.

use super::*;

/// ** UN PARAMETRO DE TIPO ARRAY DECAE A PUNTERO. C99 6.7.5.3/7.
///
/// `void f(byte v[20], int *p)` es identico a `void f(byte *v, int *p)`: el
/// array ocupa UNA ranura, no veinte bytes. Si se pasara por valor, todo
/// argumento que venga detras queda corrido -- y el segundo se lee de donde no
/// esta.
#[test]
fn un_parametro_array_decae_a_puntero_y_no_corre_a_los_de_atras() {
    let salida = run_c(
        "
typedef unsigned char digest_t[20];
struct Ctx { int count; };
int fin(digest_t d, struct Ctx *hd) { d[0] = 7; return hd->count; }
int main() {
    struct Ctx c;
    unsigned char d[20];
    c.count = 42;
    printf(\"%d %d\", fin(d, &c), d[0]);
    return 0;
}",
    );
    assert_eq!(salida, "42 7", "el segundo argumento tiene que llegar entero");
}

/// Lo mismo sin typedef, por si el alias fuera lo que confunde.
#[test]
fn un_array_declarado_a_pelo_como_parametro_tambien_decae() {
    let salida = run_c(
        "
int fin(unsigned char d[20], int *n) { d[0] = 7; return *n; }
int main() {
    unsigned char d[20];
    int n = 42;
    printf(\"%d %d\", fin(d, &n), d[0]);
    return 0;
}",
    );
    assert_eq!(salida, "42 7");
}

/// Y `sizeof` de un parametro array es el de un PUNTERO, no el del array.
#[test]
fn sizeof_de_un_parametro_array_es_el_de_un_puntero() {
    let salida = run_c(
        "
int f(unsigned char d[20]) { return (int)sizeof(d); }
int main() { unsigned char d[20]; printf(\"%d\", f(d)); return 0; }",
    );
    assert_eq!(salida, "8", "dentro de la funcion es un puntero: 8 bytes");
}

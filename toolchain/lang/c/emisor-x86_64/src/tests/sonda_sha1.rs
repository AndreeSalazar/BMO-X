//! **La sonda de `W_Checksum`: un struct local, su direccion, y dos saltos.**
//!
//! # De donde sale
//!
//! El 2026-08-14 la autopsia de DOOM resolvio sus nombres por primera vez:
//!
//! ```text
//!   rip    +0x815f2  -> SHA1_Update+0x18     48 63 00 = movsxd rax, [rax]
//!   pila   +0x81988  -> SHA1_Final+0x65
//!          +0x89e30  -> W_Checksum+0xb8
//! ```
//!
//! Una cadena de llamadas NORMAL. Lo que tumba la hipotesis del 08-13 --*"nada
//! llama a SHA1 ahi, luego fue un salto perdido"*-- y deja una pregunta mucho
//! mas concreta: **por que el puntero que `SHA1_Final` le pasa a `SHA1_Update`
//! llega no canonico.**
//!
//! `W_Checksum` hace lo tipico de cualquier codigo que use SHA1:
//!
//! ```c
//!   sha1_context_t context;        // struct LOCAL, en la pila
//!   SHA1_Init(&context);
//!   SHA1_Update(&context, ...);
//!   SHA1_Final(digest, &context);  // que a su vez llama a SHA1_Update
//! ```
//!
//! Asi que los ingredientes son cuatro, y aqui se bisecan de uno en uno --el
//! metodo del patron 26, que ya localizo `&c->defaults[i]` reduciendo la
//! funcion sospechosa a ocho lineas--:
//!
//! 1. tomar la direccion de un struct LOCAL,
//! 2. pasarla como argumento,
//! 3. **reenviarla** desde el que la recibio a un tercero,
//! 4. y desreferenciar el primer campo alli.
//!
//! El reparto verde/rojo entre estas filas ES el diagnostico.

use super::*;

/// 1. Lo mas simple: `&local` y leer el campo en la misma funcion.
#[test]
fn la_direccion_de_un_struct_local_vale_en_su_propia_funcion() {
    let salida = run_c(
        "
struct Ctx { int count; int extra; };
int main() {
    struct Ctx c;
    c.count = 41;
    struct Ctx *p = &c;
    printf(\"%d\", p->count + 1);
    return 0;
}",
    );
    assert_eq!(salida, "42");
}

/// 2. Un salto: se pasa `&local` a otra funcion y esa lee el campo.
#[test]
fn la_direccion_de_un_struct_local_sobrevive_a_UNA_llamada() {
    let salida = run_c(
        "
struct Ctx { int count; int extra; };
int leer(struct Ctx *hd) { return hd->count; }
int main() {
    struct Ctx c;
    c.count = 42;
    printf(\"%d\", leer(&c));
    return 0;
}",
    );
    assert_eq!(salida, "42");
}

/// ** 3. DOS saltos: el que recibe el puntero lo REENVIA a un tercero.
///
/// Es la forma exacta de `W_Checksum -> SHA1_Final -> SHA1_Update`, y el
/// unico ingrediente que la fila 2 no tiene.
#[test]
fn un_puntero_recibido_se_puede_REENVIAR_a_otra_funcion() {
    let salida = run_c(
        "
struct Ctx { int count; int extra; };
int update(struct Ctx *hd) { return hd->count; }
int final(struct Ctx *hd) { return update(hd); }
int main() {
    struct Ctx c;
    c.count = 42;
    printf(\"%d\", final(&c));
    return 0;
}",
    );
    assert_eq!(salida, "42", "un puntero reenviado tiene que llegar entero");
}

/// 4. Y como en `SHA1_Final(digest, &context)`: el puntero NO es el primer
/// argumento, y el que lo reenvia le pone mas argumentos delante y detras.
#[test]
fn un_puntero_reenviado_como_segundo_argumento_y_con_companeros() {
    let salida = run_c(
        "
struct Ctx { int count; int extra; };
int update(struct Ctx *hd, char *buf, int len) { return hd->count + len; }
int final(char *digest, struct Ctx *hd) { return update(hd, digest, 0); }
int main() {
    struct Ctx c;
    char d[8];
    c.count = 42;
    printf(\"%d\", final(d, &c));
    return 0;
}",
    );
    assert_eq!(salida, "42");
}

/// 5. La forma COMPLETA de `W_Checksum`: inicializar por puntero, actualizar
/// en bucle, y cerrar -- con el struct siempre por direccion.
#[test]
fn la_forma_entera_de_W_Checksum() {
    let salida = run_c(
        "
struct Ctx { int count; int total; };
void init(struct Ctx *hd) { hd->count = 0; hd->total = 0; }
void update(struct Ctx *hd, int n) { hd->count = hd->count + 1; hd->total = hd->total + n; }
void fin(int *salida, struct Ctx *hd) { update(hd, 0); *salida = hd->total; }
int main() {
    struct Ctx c;
    int r;
    init(&c);
    update(&c, 20);
    update(&c, 22);
    fin(&r, &c);
    printf(\"%d %d\", r, c.count);
    return 0;
}",
    );
    assert_eq!(salida, "42 3");
}

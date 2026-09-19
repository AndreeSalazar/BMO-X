//! **La disposicion de un `sha1_context_t`: donde cae cada campo.**
//!
//! # Por que se puede probar sin tener el fichero de DOOM
//!
//! La forma del contexto de SHA-1 no es una eleccion de doomgeneric: **la dicta
//! el algoritmo**. Cinco palabras de estado (`h0..h4`), un contador de bloques,
//! un bufer de 64 bytes --el tamano de bloque de SHA-1-- y un contador de lo
//! que hay dentro. Todas las implementaciones derivadas de la de gnupg tienen
//! esa forma, con o sin el fichero delante.
//!
//! Asi que se puede escribir aqui y preguntarle a BMO C donde pone cada cosa.
//!
//! # Que se busca, exactamente
//!
//! La autopsia de DOOM dice:
//!
//! ```text
//!   rip  +0x815f2  -> SHA1_Update+0x18    48 63 00 = movsxd rax, [rax]
//!                                                                  ^^^^ CERO
//! ```
//!
//! Un desplazamiento CERO. Pero `count` va detras de `h0..h4` (20 B),
//! `nblocks` (4 B) y `buf[64]`, o sea en el **88**. Si BMO C lo pusiera en el 0
//! --por ejemplo si un array dentro de un struct no contara para el cursor--
//! ahi estaria el fallo, y explicaria el puntero podrido: no seria `hd` lo que
//! esta mal, seria **el campo que se lee de el**.
//!
//! Las filas comprueban la disposicion **por comportamiento**, no leyendo el
//! codegen: se escribe en un campo y se mira que no aparezca en otro. Un
//! desplazamiento mal calculado hace que dos campos se pisen, y eso se ve.

use super::*;

/// El contexto, escrito tal cual lo pide SHA-1.
const CTX: &str = "
struct sha1_ctx {
    unsigned int h0, h1, h2, h3, h4;
    unsigned int nblocks;
    unsigned char buf[64];
    int count;
};
";

/// ** LA FILA QUE VA DIRECTA AL FALLO: donde cae `count`.
///
/// `sizeof` no basta --puede salir bien con los campos mal repartidos-- asi que
/// se compara la DIRECCION del campo con la del struct. Esa resta es el
/// desplazamiento que el codegen emite, y es exactamente lo que la instruccion
/// que mato a DOOM lleva dentro.
#[test]
fn count_cae_detras_del_bufer_y_no_en_el_cero() {
    let salida = run_c(&format!(
        "{CTX}
int main() {{
    struct sha1_ctx c;
    char *base = (char *)&c;
    char *pc = (char *)&c.count;
    char *pb = (char *)&c.buf[0];
    printf(\"%d %d\", (int)(pc - base), (int)(pb - base));
    return 0;
}}"
    ));
    assert_eq!(
        salida, "88 24",
        "count tiene que caer en 88 y buf en 24; si count sale 0, el array no \
         cuenta para el cursor de disposicion"
    );
}

/// El tamano entero. Con `buf[64]` dentro, son 92 bytes.
#[test]
fn el_contexto_entero_mide_lo_que_suma() {
    let salida = run_c(&format!(
        "{CTX}
int main() {{ printf(\"%d\", (int)sizeof(struct sha1_ctx)); return 0; }}"
    ));
    assert_eq!(salida, "92", "5*4 + 4 + 64 + 4 = 92");
}

/// ** Y LA PRUEBA QUE NO SE PUEDE FINGIR: que los campos NO SE PISEN.
///
/// Un desplazamiento mal calculado puede dar un `sizeof` correcto y aun asi
/// solapar dos campos. Aqui se llena el bufer entero de `0xAA` y despues se
/// comprueba que `count` y las cabeceras siguen valiendo lo suyo -- si `count`
/// estuviera dentro del bufer, se habria llenado de `0xAA` tambien.
#[test]
fn llenar_el_bufer_no_pisa_ni_count_ni_las_cabeceras() {
    let salida = run_c(&format!(
        "{CTX}
int main() {{
    struct sha1_ctx c;
    int i;
    c.h0 = 11; c.h4 = 44; c.nblocks = 7; c.count = 42;
    for (i = 0; i < 64; i++) c.buf[i] = 0xAA;
    printf(\"%d %d %d %d\", c.h0, c.h4, c.nblocks, c.count);
    return 0;
}}"
    ));
    assert_eq!(salida, "11 44 7 42");
}

/// Y al reves: escribir en los campos no pisa el bufer.
#[test]
fn escribir_las_cabeceras_no_pisa_el_bufer() {
    let salida = run_c(&format!(
        "{CTX}
int main() {{
    struct sha1_ctx c;
    int i, malos = 0;
    for (i = 0; i < 64; i++) c.buf[i] = 7;
    c.h0 = 1; c.h1 = 2; c.h2 = 3; c.h3 = 4; c.h4 = 5;
    c.nblocks = 6; c.count = 8;
    for (i = 0; i < 64; i++) if (c.buf[i] != 7) malos = malos + 1;
    printf(\"%d\", malos);
    return 0;
}}"
    ));
    assert_eq!(salida, "0", "ningun byte del bufer puede haber cambiado");
}

/// La forma de `SHA1_Update+0x18`: leer `hd->count` a traves de un puntero
/// recibido. Es la instruccion exacta que fallo, con el campo en su sitio.
#[test]
fn leer_count_por_puntero_es_lo_que_hace_SHA1_Update() {
    let salida = run_c(&format!(
        "{CTX}
int update(struct sha1_ctx *hd) {{
    if (hd->count == 64) return 999;
    return hd->count;
}}
int main() {{
    struct sha1_ctx c;
    c.count = 42;
    printf(\"%d\", update(&c));
    return 0;
}}"
    ));
    assert_eq!(salida, "42");
}

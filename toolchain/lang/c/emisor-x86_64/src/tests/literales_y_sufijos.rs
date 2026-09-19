//! **EL SUFIJO DE UN LITERAL, Y EL LITERAL QUE NO CABE.** Dos fallos callados.
//!
//! # De donde salen, y no se buscaban
//!
//! El dueno pidio que BMO C dejara de adivinar con el UB. Al barrer las formas
//! clasicas aparecio que **el compilador ya adivinaba en C perfectamente
//! ordinario**, y en el sitio mas usado que hay: `1 << n`.
//!
//! ```text
//!    1 << 30     ->  1073741824             bien
//!    1UL << 31   ->  0xFFFFFFFF80000000     recortado y extendido con signo
//!    1UL << 32   ->  0
//!    1UL << 63   ->  0
//!    a << 63     ->  BIEN                   en ejecucion nadie recorta
//! ```
//!
//! ** TODA mascara de 64 bits escrita como literal salia CERO. Sin un error,
//! sin un aviso. Y `1 << n` no es un modismo cualquiera: es **el** modismo de
//! las banderas, los bits de pagina y las capabilities.
//!
//! ## La causa: el lexer leia el sufijo y LO TIRABA
//!
//! Una linea que avanzaba el cursor y no guardaba nada. Con el sufijo perdido,
//! `1UL` y `1` eran el mismo token y por tanto el mismo TIPO --`int`--, asi que
//! `recortar_a_32` metia su `mov eax,eax` detras de la cuenta y se llevaba la
//! mitad de arriba.
//!
//! *** El arreglo no anade un caso a ningun juez: el sufijo **se desazucara a
//! un `Cast`**, que es lo que el estandar dice que un sufijo ES. `tipo_de`,
//! `expr_is_float` y `expr_is_unsigned` ya leian `Expr::Cast`.
//!
//!   > Cuando la forma que hace falta ya existe en el arbol, el arreglo no es
//!   > escribir codigo: es dejar de tirar un dato.
//!
//! # Y el segundo: `unwrap_or(0)`, la misma linea que ya estaba arreglada
//!
//! ```text
//!    0xFFFFFFFFFFFFFFFF     ->  bien desde hace semanas
//!    18446744073709551615UL ->  0, en silencio
//! ```
//!
//! ** La rama HEXADECIMAL del lexer aprendio esta leccion y la dejo escrita en
//! un comentario de quince lineas. La rama DECIMAL, doce lineas mas abajo,
//! seguia con su `n.parse().unwrap_or(0)`.
//!
//! *** Y cero es la peor respuesta posible, porque **cero es un numero valido**:
//! el programa sigue, y la comprobacion que dependia de esa constante deja de
//! comprobar. Ahora se prueba `i64`, luego `u64`, y si no cabe **se dice** --que
//! es lo que el dueno pidio con estas palabras: *si adivina, no lo convierte en
//! BEX*.

use super::*;

fn cuadra(nombre: &str, espera: &str, fuente: &str) {
    let bef = compile_source_to_bef(fuente).expect("tiene que compilar");
    assert_eq!(ejecutar_bef(&bef).trim_end(), espera, "{nombre}");
}

/// *** LAS MASCARAS. La fila que estaba en cero.
#[test]
fn una_mascara_de_64_bits_escrita_como_literal() {
    cuadra("1UL << 32", "4294967296",
        "int main(){ unsigned long v; v = 1UL << 32; printf(\"%lu\", v); return 0; }");
    cuadra("1UL << 40", "1099511627776",
        "int main(){ unsigned long v; v = 1UL << 40; printf(\"%lu\", v); return 0; }");
    cuadra("1UL << 63", "9223372036854775808",
        "int main(){ unsigned long v; v = 1UL << 63; printf(\"%lu\", v); return 0; }");
    cuadra("1L << 40", "1099511627776",
        "int main(){ long v; v = 1L << 40; printf(\"%ld\", v); return 0; }");
}

/// ** Y LAS DE 32 BITS SIGUEN RECORTANDO, que es lo que las hace correctas.
///
/// El recorte no sobraba: lo puso el bug que costo una semana de DOOM --la
/// aritmetica de angulos ES envolvente--. Lo que faltaba era saber CUANDO no
/// aplicarlo. Si estas dos filas se ponen rojas, el arreglo del sufijo se llevo
/// por delante el recorte.
#[test]
fn un_literal_sin_sufijo_sigue_siendo_de_32_bits() {
    cuadra("1 << 31 envuelve", "-2147483648",
        "int main(){ int v; v = 1 << 31; printf(\"%d\", v); return 0; }");
    cuadra("1u << 31 no tiene signo", "2147483648",
        "int main(){ unsigned long v; v = 1u << 31; printf(\"%lu\", v); return 0; }");
    cuadra("la suma de angulos envuelve", "1020",
        "int main(){ unsigned a; a = 0xDFE00000; \
         printf(\"%u\", (a + 0x40000000) >> 19); return 0; }");
}

/// El literal decimal que no cabe en `i64` pero si en `u64`.
#[test]
fn un_literal_decimal_de_64_bits_sin_signo() {
    cuadra("18446744073709551615UL", "18446744073709551615",
        "int main(){ unsigned long u; u = 18446744073709551615UL; \
         printf(\"%lu\", u); return 0; }");
    cuadra("9223372036854775808UL", "9223372036854775808",
        "int main(){ unsigned long u; u = 9223372036854775808UL; \
         printf(\"%lu\", u); return 0; }");
    // El hexadecimal equivalente, que ya iba: si esta se rompe, el arreglo se
    // llevo por delante la rama que estaba bien.
    cuadra("0xFFFFFFFFFFFFFFFF", "18446744073709551615",
        "int main(){ unsigned long u; u = 0xFFFFFFFFFFFFFFFFUL; \
         printf(\"%lu\", u); return 0; }");
}

/// *** Y LO QUE NO CABE **SE DICE**, en vez de valer cero.
///
/// Es la mitad que hace util a la otra. Un compilador que contesta `0` a un
/// numero que no sabe representar no esta siendo permisivo: esta mintiendo.
#[test]
fn un_literal_que_no_cabe_en_64_bits_se_rechaza_diciendolo() {
    let e = compile_source_to_bef(
        "int main(){ unsigned long u; u = 99999999999999999999999; \
         printf(\"%lu\", u); return 0; }",
    )
    .expect_err("no cabe: tiene que decirlo");
    assert!(
        e.message.contains("fuera de 64 bits"),
        "el motivo tiene que nombrar el techo, y dice: {}",
        e.message
    );
}

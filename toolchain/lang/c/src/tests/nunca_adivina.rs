//! **NUNCA ADIVINA.** Donde el compilador tendria que suponer, NO compila.
//!
//! # La regla, con las palabras del dueno (2026-09-10)
//!
//! > *"si encuentra lo que es adivinar, no compila hasta que lo aclares. No me
//! > gustaria que la CPU tenga que perder tiempo en adivinar. NUNCA ADIVINA."*
//!
//! # Que se estaba adivinando, y por que mata
//!
//! Nueve sitios del emisor decian, de una forma o de otra, *"no se de que tipo
//! es esto, asumo ocho bytes"*. Y con ese tipo se elige **el ancho de la
//! instruccion que se emite**:
//!
//! ```text
//!    el tipo de verdad es `char`   ->  se escriben 8 bytes donde cabe 1
//!    y los 7 de al lado son        ->  la variable siguiente
//! ```
//!
//! ** Ese fallo ya se pago en este arbol, y esta escrito en `emitir/direccion.rs`:
//! *"`*(p+1)` con `int *p` leia 8 bytes, o sea dos enteros pegados: devolvia
//! 504403158366158848 en vez de 6"*.
//!
//! *** Un `unwrap_or` de un TIPO no es un valor por defecto. Un valor por
//! defecto es una eleccion sobre lo tuyo; esto es **una suposicion sobre la
//! memoria de otro**.
//!
//! # [!] Y NO ROMPIO NADA, porque se midio ANTES de escribirla
//!
//! Los nueve sitios se instrumentaron con un contador y se compilo el arbol
//! entero: **DOOM --56.976 lineas de C de verdad-- y los catorce ejemplos
//! dieron CERO**. Nunca hizo falta suponer.
//!
//!   > Una regla que nadie incumple hoy no es una regla inutil: es la unica que
//!   > se puede poner sin negociar.
//!
//! # ** Y EL CENSO POR SINTAXIS NO LOS ENCONTRO TODOS
//!
//! Ocho estaban escritos `unwrap_or(TypeSpec::Long)` y el noveno --el tipo de
//! los elementos de una tabla-- era un brazo `_ => TypeSpec::Long`. La misma
//! suposicion, otra forma de teclearla.
//!
//!   > Buscar una forma de escribir encuentra una forma de escribir. Adivinar
//!   > tiene mas de una.

use super::*;

fn se_niega(nombre: &str, fuente: &str) {
    let e = compile_source_to_bef(fuente)
        .expect_err(&format!("{nombre}: tenia que negarse"));
    assert!(
        e.message.contains("NO SE ADIVINA"),
        "{nombre}: el motivo tiene que decir que no se adivina, y dice: {}",
        e.message
    );
}

/// Desreferenciar algo que no es un puntero. No hay a que apuntar, asi que no
/// hay ancho que elegir -- y antes se elegian ocho bytes.
#[test]
fn desreferenciar_lo_que_no_es_un_puntero_no_compila() {
    se_niega("*a con a entero",
        "int main(){ int a; a = 5; printf(\"%d\", *a); return 0; }");
    se_niega("*p con p sin declarar",
        "int main(){ printf(\"%d\", *p); return 0; }");
}

/// *** `void*` ES NO SABERLO, y este es el que se cuela.
///
/// `pointee_type` contesta `Some(Void)`, o sea que el tipo SI se sabe. Lo que
/// no existe es su ANCHO. Y `void*` es **el** tipo con el que se pasa memoria
/// de un lado a otro en C: si esto se permite, la suposicion entra por la
/// puerta mas usada que hay.
#[test]
fn desreferenciar_un_void_no_compila() {
    se_niega("*p con void*",
        "int main(){ void *p; int a; a = 5; p = &a; printf(\"%d\", *p); return 0; }");
    se_niega("p[0] con void*",
        "int main(){ void *p; int a; a = 5; p = &a; printf(\"%d\", p[0]); return 0; }");
}

/// ** LA FILA DE CONTROL, y es la que hace util a las otras cuatro.
///
/// Una regla que dijera que no a todo tambien pasaria las cuatro de arriba. Si
/// esta se pone roja, el embudo se comio C valido.
#[test]
fn un_puntero_con_su_tipo_compila_y_da_el_numero() {
    let bef = compile_source_to_bef(
        "int main(){ int a; int *p; a = 5; p = &a; printf(\"%d\", *p); return 0; }",
    )
    .expect("esto es C perfectamente valido");
    assert_eq!(ejecutar_bef(&bef).trim_end(), "5");
}

/// Y un `void*` **con su cast** tambien: la regla pide que lo aclares, no que
/// no lo uses. Es la diferencia entre un embudo y un muro.
#[test]
fn un_void_con_cast_compila() {
    let bef = compile_source_to_bef(
        "int main(){ void *p; int a; a = 7; p = &a; \
         printf(\"%d\", *(int*)p); return 0; }",
    )
    .expect("el cast es exactamente la aclaracion que se pedia");
    assert_eq!(ejecutar_bef(&bef).trim_end(), "7");
}

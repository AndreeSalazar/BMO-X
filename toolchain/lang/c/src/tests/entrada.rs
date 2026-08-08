//! La entrada: `getchar` y `scanf`
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// =============== La ENTRADA: getchar y scanf ===============
//
// La mitad que le faltaba a `printf`. Ver `codegen/entrada.rs`.

/// Un byte cada vez, en orden.
#[test]
fn getchar_entrega_los_bytes_en_orden() {
    let fuente = "int main() { int c; for (;;) { c = getchar(); \
                  if (c == 10) break; printf(\"[%c]\", c); } return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("hola\n"));
    assert_eq!(out, "[h][o][l][a]");
}

/// * La puerta entrega **hasta 7 bytes de una vez y los CONSUME**. Sin el
/// buffer, un lector de un byte se comeria seis de cada siete pulsaciones y
/// pareceria un teclado que pierde letras. Trece bytes son dos paquetes.
#[test]
fn getchar_no_pierde_los_bytes_que_sobran_del_paquete() {
    let fuente = "int main() { int c; int n; n = 0; \
                  for (;;) { c = getchar(); if (c == 10) break; n = n + 1; } \
                  printf(\"%d\\n\", n); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("abcdefghijklm\n"));
    assert_eq!(out.trim(), "13");
}

/// El buffer es UNO: dos `getchar()` distintos comparten los bytes que
/// sobraron. Si cada sitio tuviera el suyo, el segundo empezaria a leer
/// desde cero y se perderian los del primero.
#[test]
fn dos_getchar_distintos_comparten_el_mismo_buffer() {
    let fuente = "int main() { int a; int b; a = getchar(); b = getchar(); \
                  printf(\"%c%c\\n\", a, b); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("xy\n"));
    assert_eq!(out.trim(), "xy");
}

#[test]
fn scanf_lee_un_entero() {
    let fuente = "int main() { int x; scanf(\"%d\", &x); \
                  printf(\"leido=%d\\n\", x * 2); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("21\n"));
    assert_eq!(out.trim(), "leido=42");
}

/// Un negativo tecleado es negativo. Sin el signo, `-5` daria 5 y la cuenta
/// saldria al reves sin una palabra.
#[test]
fn scanf_lee_un_entero_negativo() {
    let fuente = "int main() { int x; scanf(\"%d\", &x); \
                  printf(\"%d\\n\", x); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("-5\n"));
    assert_eq!(out.trim(), "-5");
}

/// `%s` lee la linea al buffer del llamante **con su cero final**: en C una
/// cadena sin terminador no es una cadena, y el `%s` de despues imprimiria
/// hasta el primer cero que hubiera por ahi.
#[test]
fn scanf_lee_una_cadena_y_la_termina() {
    let fuente = "int main() { char s[16]; scanf(\"%s\", s); \
                  printf(\"<%s>\\n\", s); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("mundo\n"));
    assert_eq!(out.trim(), "<mundo>");
}

#[test]
fn scanf_lee_un_caracter() {
    let fuente = "int main() { char c; scanf(\"%c\", &c); \
                  printf(\"%c%c\\n\", c, c); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("Z\n"));
    assert_eq!(out.trim(), "ZZ");
}

/// Mas de una conversion se RECHAZA. Un `scanf` que ignora la mitad de su
/// formato es un programa que lee mal en silencio -- y las reglas de espacio
/// en blanco de section 7.21.6.2 ocupan pagina y media que aqui no estan.
#[test]
fn scanf_con_dos_conversiones_se_rechaza_con_motivo() {
    let err = compile_source_to_bef(
        "int main() { int a; int b; scanf(\"%d %d\", &a, &b); return 0; }",
    )
    .expect_err("dos conversiones todavia no se compilan");
    assert!(err.message.contains("UNA conversion"), "mensaje: {}", err.message);
}

/// Y una conversion que no esta se dice con cual es.
#[test]
fn scanf_con_una_conversion_desconocida_se_rechaza() {
    let err = compile_source_to_bef("int main() { float f; scanf(\"%f\", &f); return 0; }")
        .expect_err("%f todavia no se compila");
    assert!(err.message.contains("%f"), "mensaje: {}", err.message);
}

/// Y las escrituras llevan el tamano EXACTO del campo: escribir 8 bytes
/// donde hay un `int` pisaria el campo siguiente.
#[test]
fn cada_escritura_usa_el_tamano_de_su_campo() {
    let out = run_c("struct M { char a; int b; char c; }; \
                     int main() { struct M m = {.a = 65, .b = 1000, .c = 66}; \
                     printf(\"%d %d %d\\n\", m.a, m.b, m.c); return 0; }");
    assert_eq!(out.trim(), "65 1000 66");
}

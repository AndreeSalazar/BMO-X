//! La PUERTA de syscalls y las cabeceras `<bmo/...>`
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ═══════════════ BMO C/Control: la puerta como instrucción ═══════════════

/// El literal que hacía falta antes que ningún otro: `CURRENT_TASK`.
///
/// `i64::from_str_radix` no puede con `0xFFFFFFFFFFFFFFFE` y el
/// `unwrap_or(0)` lo convertía en **cero, en silencio** — o sea, en la
/// capability 0. Escribir la constante correcta compilaba y llamaba a otro.
#[test]
fn hex_de_64_bits_no_se_convierte_en_cero() {
    let out = run_c("int main() { unsigned long long c; c = 0xFFFFFFFFFFFFFFFE; \
                     printf(\"%x\\n\", c); return 0; }");
    assert_eq!(out.trim(), "fffffffffffffffe");
}

/// Y si de verdad no cabe, se dice. Callarlo sería el mismo error con otro
/// valor.
#[test]
fn hex_mas_alla_de_64_bits_es_un_error_no_un_cero() {
    let err = compile_source_to_bef("int main() { return 0x1FFFFFFFFFFFFFFFF; }")
        .expect_err("no cabe en 64 bits: tiene que fallar, no valer 0");
    assert!(err.message.contains("64 bits"), "mensaje: {}", err.message);
}

/// `__syscall` es una fila de la tabla sem-asm, no una caja negra: sus
/// argumentos van a rdi/rsi/rdx/r10/r8, que es la convención de la puerta.
///
/// Se comprueba sobre `CONSOLE_WRITE` porque es la única operación cuyo
/// efecto se ve desde fuera: si un argumento cayera en otro registro, no
/// saldría este texto.
#[test]
fn syscall_intrinseco_coloca_los_argumentos_donde_dice_la_tabla() {
    // "hola" en little-endian dentro de un solo u64, que es como viaja la
    // consola: 8 bytes por llamada con el cero como final.
    let out = run_c(
        "int main() { __syscall(0, 0xFFFFFFFFFFFFFFFE, 6, 0x616C6F68, 0, 0); return 0; }",
    );
    assert_eq!(out, "hola");
}

/// La puerta contesta DOS cosas: el código en rax y el valor en rdx. Las
/// dos filas de la tabla existen para poder recoger cada una.
#[test]
fn syscall_valor_recoge_rdx_y_syscall_recoge_rax() {
    // CONSOLE_READ devuelve `(n << 56) | bytes` en rdx, y 0 (ok) en rax.
    let fuente = "int main() { \
         unsigned long long v; unsigned long long c; \
         v = __syscall_valor(0, 0xFFFFFFFFFFFFFFFE, 0x0F, 0, 0, 0); \
         c = __syscall(0, 0xFFFFFFFFFFFFFFFE, 0x0F, 0, 0, 0); \
         printf(\"valor=%x codigo=%d\\n\", v, (int)c); return 0; }";
    let out = run_c_sembrado(fuente, |m| m.poner_entrada("AB"));
    // n=2, bytes = 'A','B' → 0x0200000000004241. La segunda lectura ya no
    // tiene nada, así que el código sigue siendo 0 pero el valor sería 0.
    assert_eq!(out.trim(), "valor=200000000004241 codigo=0");
}

// ═══════════════ <bmo/bmo.h>: la superficie en C ═══════════════

#[test]
fn la_cabecera_baja_a_la_puerta_sin_runtime_que_enlazar() {
    let out = run_c_con_pp(
        "#include <bmo/bmo.h>\n\
         int main() { printf(\"pid=%d\\n\", (int)bmo_pid()); bmo_ceder(); \
         printf(\"cedi\\n\"); return 0; }",
    );
    assert_eq!(out, "pid=0\ncedi\n");
}

// ═══════════════ <bmo/entrada.h>: el ratón y el teclado ═══════════════

/// Sin ceder la entrada, reclamarla da 0. Es el caso NORMAL —el compositor
/// la tiene— y un programa que no lo comprueba lee ceros para siempre y
/// parece un ratón roto.
#[test]
fn reclamar_la_entrada_puede_fallar_y_se_nota() {
    let fuente = "#include <bmo/entrada.h>\n\
         int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
         if (e == 0) { printf(\"sin entrada\\n\"); } else { printf(\"handle\\n\"); } \
         return 0; }";
    assert_eq!(run_c_sembrado(fuente, |_| {}).trim(), "sin entrada");
    assert_eq!(run_c_sembrado(fuente, |m| m.ceder_entrada()).trim(), "handle");
}

/// Las teclas salen una por llamada, y `-1` significa "no hay", que es el
/// convenio de `getchar` y no un byte válido.
#[test]
fn las_teclas_salen_en_orden_y_el_final_es_menos_uno() {
    let fuente = "#include <bmo/entrada.h>\n\
         int main() { unsigned long long e; int t; \
         e = bmo_entrada_reclamar(); \
         for (;;) { t = bmo_entrada_tecla(e); if (t < 0) break; printf(\"%d \", t); } \
         printf(\"fin\\n\"); return 0; }";
    let out = run_c_sembrado(fuente, |m| {
        m.ceder_entrada();
        m.poner_teclas(&[b'a', b'b', 0x87]);
    });
    assert_eq!(out.trim(), "97 98 135 fin");
}

/// ★ La rueda **consume**: dos lecturas seguidas sin girar dan cero la
/// segunda. Es la propiedad que decide si un scroll se mueve solo, y sólo
/// se distingue de un acumulado EJECUTÁNDOLA.
#[test]
fn la_rueda_se_vacia_al_leerla() {
    let fuente = "#include <bmo/entrada.h>\n\
         int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
         printf(\"%d %d\\n\", bmo_entrada_rueda(e), bmo_entrada_rueda(e)); return 0; }";
    let out = run_c_sembrado(fuente, |m| {
        m.ceder_entrada();
        m.poner_rueda(4);
    });
    assert_eq!(out.trim(), "4 0");
}

/// Girar hacia atrás es NEGATIVO. Sin el `(int)` de la cabecera, el valor
/// viaja como i32 dentro de un u64 y una muesca hacia abajo daría cuatro
/// mil millones — un scroll que salta al principio del historial.
#[test]
fn la_rueda_hacia_atras_es_negativa() {
    let fuente = "#include <bmo/entrada.h>\n\
         int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
         printf(\"%d\\n\", bmo_entrada_rueda(e)); return 0; }";
    let out = run_c_sembrado(fuente, |m| {
        m.ceder_entrada();
        m.poner_rueda(-2);
    });
    assert_eq!(out.trim(), "-2");
}

/// Los tres datos del puntero viajan empaquetados en una sola llamada.
#[test]
fn el_puntero_se_desempaqueta_bien() {
    let fuente = "#include <bmo/entrada.h>\n\
         int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
         printf(\"%d,%d b=%d ev=%d\\n\", bmo_entrada_x(e), bmo_entrada_y(e), \
         bmo_entrada_botones(e), (int)bmo_entrada_eventos(e)); return 0; }";
    let out = run_c_sembrado(fuente, |m| {
        m.ceder_entrada();
        m.poner_puntero(1024, 600, 1);
    });
    assert_eq!(out.trim(), "1024,600 b=1 ev=1");
}

// ═══════════════ <bmo/scroll.h>: la ventana sobre el historial ═══════════

/// Los dos topes. Pasarse por arriba enseña filas en blanco —parece que se
/// ha perdido todo—; pasarse por abajo deja la vista en negativo.
#[test]
fn el_scroll_se_topa_solo_en_los_dos_extremos() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { \
         printf(\"%d %d %d\\n\", bmo_scroll_mover(0, -50, 200, 16), \
         bmo_scroll_mover(0, 9999, 200, 16), bmo_scroll_mover(0, 10, 200, 16)); \
         return 0; }",
    );
    assert_eq!(out.trim(), "0 184 10");
}

/// Un historial que todavía no llena la ventana sólo tiene un sitio válido.
#[test]
fn sin_historial_suficiente_la_unica_vista_es_el_fondo() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { printf(\"%d\\n\", bmo_scroll_mover(0, 5, 10, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "0");
}

/// Tres filas por muesca, y hacia atrás resta. Es el mismo paso que el
/// compositor: si divergieran, la rueda haría una cosa en Rust y otra en C.
#[test]
fn la_rueda_mueve_tres_filas_por_muesca_en_los_dos_sentidos() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { int v; v = bmo_scroll_rueda(0, 3, 200, 16); \
         printf(\"%d %d\\n\", v, bmo_scroll_rueda(v, -2, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "9 3");
}

/// Una página es `visibles - 1`: la fila que se solapa es lo que deja
/// seguir leyendo sin volver atrás.
#[test]
fn repag_y_avpag_dejan_una_fila_de_solape() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { int v; v = bmo_scroll_tecla(0, BMO_TECLA_REPAG, 200, 16); \
         printf(\"%d %d\\n\", v, bmo_scroll_tecla(v, BMO_TECLA_AVPAG, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "15 0");
}

/// Una tecla que no es de scroll no mueve la vista. Sin esto, escribir
/// movería el historial bajo los pies del que escribe.
#[test]
fn una_tecla_cualquiera_no_mueve_el_historial() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { printf(\"%d\\n\", bmo_scroll_tecla(7, 97, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "7");
}

/// Inicio y Fin van a los extremos de una vez.
#[test]
fn inicio_y_fin_saltan_a_los_extremos() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { int v; v = bmo_scroll_tecla(0, BMO_TECLA_INICIO, 200, 16); \
         printf(\"%d %d\\n\", v, bmo_scroll_tecla(v, BMO_TECLA_FIN, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "184 0");
}

/// La fila por la que empieza el dibujo. Es la cuenta que se reinventa mal
/// cuando se escribe a mano en el sitio de pintar.
#[test]
fn la_primera_fila_visible_sigue_a_la_vista() {
    let out = run_c_con_pp(
        "#include <bmo/scroll.h>\n\
         int main() { printf(\"%d %d\\n\", bmo_scroll_primera(0, 200, 16), \
         bmo_scroll_primera(10, 200, 16)); return 0; }",
    );
    assert_eq!(out.trim(), "184 174");
}


// ═══════════════ El ejemplo del repositorio, ejecutado ═══════════════

/// Con el compositor vivo la entrada es SUYA, y esto lo dice en vez de
/// quedarse leyendo ceros — que se ve igual que un ratón roto y manda a
/// depurar el USB sin motivo.
#[test]
fn scroll_sin_entrada_lo_dice_y_se_va() {
    let out = run_c_con_pp(include_str!("../../examples/scroll_C.c"));
    assert_eq!(out, "la entrada es de otro proceso: no hay scroll que hacer.
");
}

/// El programa entero: rueda hacia el pasado, RePag, Fin y ESC.
///
/// Es la mitad de la prueba que el Ryzen no puede dar todavía —el ratón
/// sigue sin verificar en metal—, y RePag/AvPag no dependen del ratón, así
/// que esa mitad se puede cerrar aquí.
#[test]
fn scroll_recorre_el_historial_con_la_rueda_y_con_las_teclas() {
    let out = run_c_sembrado(include_str!("../../examples/scroll_C.c"), |m| {
        m.ceder_entrada();
        m.poner_rueda(2);
        m.poner_teclas_por_fotograma(&[&[0x87], &[0x85], &[27]]);
    });
    let cabeceras: Vec<&str> = out.lines().filter(|l| l.starts_with("----")).collect();
    assert_eq!(
        cabeceras,
        vec![
            "---- filas 52..59 [al dia] ----",
            "---- filas 46..53 [historial] ----",
            "---- filas 39..46 [historial] ----",
            "---- filas 52..59 [al dia] ----",
        ],
        "salida completa:
{out}"
    );
    assert!(out.ends_with("hasta luego.
"), "salida completa:
{out}");
    // Y las filas son las que dicen ser: si el índice se calculara mal, la
    // cabecera seguiría cuadrando y el contenido no.
    assert!(out.contains("  fila 052
"), "salida completa:
{out}");
    assert!(out.contains("  fila 039
"), "salida completa:
{out}");
}

/// ★ La rueda se drena en la PRIMERA vuelta del bucle. Si el programa la
/// volviera a sumar en la siguiente, el historial seguiría subiendo solo
/// después de soltarla — el bug que la semántica de "consumir" evita, y que
/// sólo se ve dando varias vueltas al bucle.
#[test]
fn el_scroll_no_sigue_moviendose_solo_tras_soltar_la_rueda() {
    let out = run_c_sembrado(include_str!("../../examples/scroll_C.c"), |m| {
        m.ceder_entrada();
        m.poner_rueda(1);
        // Teclas que no mueven nada, una por fotograma: obligan al bucle a
        // dar vueltas sin que llegue ninguna muesca nueva.
        m.poner_teclas_por_fotograma(&[&[b'a'], &[b'b'], &[b'c'], &[b'd'], &[27]]);
    });
    let cabeceras: Vec<&str> = out.lines().filter(|l| l.starts_with("----")).collect();
    assert_eq!(
        cabeceras,
        vec![
            "---- filas 52..59 [al dia] ----",
            "---- filas 49..56 [historial] ----",
        ],
        "el historial se movió más de una vez con una sola muesca:
{out}"
    );
}

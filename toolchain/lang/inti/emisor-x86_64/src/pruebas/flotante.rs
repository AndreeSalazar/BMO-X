//! LA COMA FLOTANTE -- el numero que mide, no el que cuenta.
//!
//! Las cuatro operaciones, las seis comparaciones con el NaN correcto, y la
//! conversion. Y la Regla 11, que se comprueba en lo que NO se emite.

use super::*;
// ===================================================================
//  ** F5c -- LA COMA FLOTANTE. El cuarto tipo de numero que se puede tocar.
// ===================================================================
//
//  Hasta hoy INTI sabia contar y no sabia medir. Un `natural32` cabe un pixel,
//  pero no cabe una posicion, ni un angulo, ni una escala -- y por eso F5a
//  llegaba a rellenar un framebuffer de un color y no a mover nada dentro.
//
//  ** Y el modelo esta escrito en `flotante()`: los valores viven en registros
//  normales como PATRON DE BITS y solo cruzan para la operacion. Estas pruebas
//  no lo saben ni les importa; miran numeros. Ese es el punto de mirarlos.


const SUMA_FLOTANTE: &str = "\
perfil llano

funcion f devuelve flotante64
    devuelve 2.5 + 1.25
";

#[test]
fn una_suma_de_coma_flotante_corre_y_da_el_numero() {
    let r = como_numero(ejecuta(SUMA_FLOTANTE, 0, 0));
    assert_eq!(r, 3.75, "salio {}", r);
}

/// Las cuatro, y una de ellas es la que no se puede hacer con enteros: `/`.
///
/// ** `5 / 2` da `2.5` y no `2`. Es la sorpresa 10 de Python contestada al
/// reves: en INTI el simbolo divide de verdad y el cociente entero tiene su
/// propia palabra (`entre`). Aqui se ve que no es una promesa de la gramatica.
#[test]
fn las_cuatro_operaciones() {
    let de = |e: &str| {
        como_numero(ejecuta(
            &format!(
                "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve {}\n",
                e
            ),
            0,
            0,
        ))
    };
    assert_eq!(de("2.5 + 1.25"), 3.75);
    assert_eq!(de("2.5 - 1.25"), 1.25);
    assert_eq!(de("2.5 * 4.0"), 10.0);
    assert_eq!(de("5.0 / 2.0"), 2.5);
}

/// ** DIVIDIR ENTRE CERO NO ATRAPA, y es la prueba de que la Regla 3 esta bien
/// entendida.
///
/// La Regla 3 existe porque en los ENTEROS `1 / 0` no tiene respuesta: cualquier
/// bit que salga se lo invento el compilador. En IEEE-754 la tiene --infinito--
/// y esta escrita desde 1985. Atrapar aqui no anadiria seguridad: quitaria la
/// aritmetica.
#[test]
fn entre_cero_da_infinito_y_no_atrapa() {
    let f = "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve 1.0 / 0.0\n";
    assert_eq!(como_numero(ejecuta(f, 0, 0)), f64::INFINITY);

    // Y la comprobacion no esta ni en la IR.
    //
    // ** Se cuenta AQUI y no en los bytes emitidos, y la diferencia importa:
    // el emisor todavia no materializa la de division --lo dice el mismo, con
    // su motivo, en `Instr::Comprueba`--, asi que contando bytes esta prueba
    // saldria verde igual si la regla estuviera puesta. La regla vive en la
    // IR; es ahi donde hay que preguntar si esta.
    assert_eq!(reglas_de(f), 0, "un flotante no lleva comprobacion detras");
}

/// ** Y EL CONTRASTE, que es lo que hace valer la prueba de arriba: la misma
/// division con enteros SI trae su comprobacion.
#[test]
fn la_misma_division_con_enteros_si_trae_su_regla() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a / b\n";
    assert_eq!(reglas_de(f), 1, "la Regla 3 desaparecio de los enteros");
}


// -------------------------------------------------------------------
//  ** LAS COMPARACIONES, Y EL NaN
// -------------------------------------------------------------------

fn compara(e: &str) -> u64 {
    ejecuta(
        &format!(
            "perfil llano\n\nfuncion f devuelve logico\n    devuelve {}\n",
            e
        ),
        0,
        0,
    )
}

#[test]
fn las_seis_comparaciones() {
    assert_eq!(compara("1.5 < 2.5"), 1);
    assert_eq!(compara("2.5 < 1.5"), 0);
    assert_eq!(compara("2.5 > 1.5"), 1);
    assert_eq!(compara("1.5 > 2.5"), 0);
    assert_eq!(compara("1.5 <= 1.5"), 1);
    assert_eq!(compara("1.5 >= 2.5"), 0);
    assert_eq!(compara("1.5 = 1.5"), 1);
    assert_eq!(compara("1.5 no es 2.5"), 1);
}

/// ** ESTA ES LA PRUEBA QUE DECIDE SI LA COMA FLOTANTE ESTA BIEN HECHA.
///
/// Un NaN --lo que sale de `0.0 / 0.0`-- no es mayor, ni menor, ni igual a
/// nada. Y el silicio no lo regala: la comparacion enciende la bandera de
/// "iguales" A LA VEZ que la de "no comparables", asi que una igualdad escrita
/// de la forma obvia contesta **que si**.
///
/// Las cinco primeras tienen que salir falsas. Y la sexta, cierta -- porque
/// `x no es x` es exactamente como se pregunta si algo es NaN, y tiene que
/// poder contestarse.
#[test]
fn un_nan_pierde_las_cinco_comparaciones_y_gana_la_sexta() {
    assert_eq!(compara("0.0 / 0.0 < 1.0"), 0, "un NaN no es menor");
    assert_eq!(compara("0.0 / 0.0 > 1.0"), 0, "ni mayor");
    assert_eq!(compara("0.0 / 0.0 <= 1.0"), 0);
    assert_eq!(compara("0.0 / 0.0 >= 1.0"), 0);
    assert_eq!(compara("0.0 / 0.0 = 1.0"), 0, "ni igual");
    assert_eq!(
        compara("0.0 / 0.0 no es 1.0"),
        1,
        "y la desigualdad es la unica que un NaN hace CIERTA"
    );
}

/// El NaN contra si mismo, que es el caso que enganaria a la version ingenua.
#[test]
fn un_nan_no_es_igual_ni_a_si_mismo() {
    assert_eq!(compara("0.0 / 0.0 = 0.0 / 0.0"), 0);
    assert_eq!(compara("0.0 / 0.0 no es 0.0 / 0.0"), 1);
}

// -------------------------------------------------------------------
//  ** LA CONVERSION, que es la unica vez que los bits CAMBIAN
// -------------------------------------------------------------------

/// `flotante64(5)` da 5.0, no los bits de 5 mirados del reves.
///
/// ** Confundir las dos cosas da `2,47e-323` donde tiene que haber un `5.0`, y
/// no rompe nada: sigue siendo un flotante valido. Por eso hay una prueba.
#[test]
fn un_entero_se_convierte_de_verdad_y_no_se_reinterpreta() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve flotante64\n    devuelve flotante64(a)\n";
    assert_eq!(como_numero(ejecuta(f, 5, 0)), 5.0);
    assert_eq!(como_numero(ejecuta(f, 0, 0)), 0.0);
}

/// Con signo, que es la otra mitad: `-7` tiene que dar `-7.0` y no 1,8e19.
#[test]
fn la_conversion_es_con_signo() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve flotante64\n    devuelve flotante64(a)\n";
    assert_eq!(como_numero(ejecuta(f, (-7i64) as u64, 0)), -7.0);
}

/// Y de vuelta, TRUNCANDO. 2,9 da 2 y -2,9 da -2.
#[test]
fn de_flotante_a_entero_se_trunca_hacia_el_cero() {
    let f = "perfil llano\n\nfuncion f devuelve entero64\n    devuelve entero64(2.9)\n";
    assert_eq!(ejecuta(f, 0, 0), 2);
    let g = "perfil llano\n\nfuncion f devuelve entero64\n    devuelve entero64(0.0 - 2.9)\n";
    assert_eq!(ejecuta(g, 0, 0) as i64, -2, "hacia el cero, no hacia abajo");
}

/// Ida y vuelta por una variable declarada: el tipo escrito es lo que decide,
/// no el literal.
#[test]
fn el_tipo_declarado_manda_sobre_la_operacion() {
    let f = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve flotante64
    x es flotante64 = flotante64(a)
    devuelve x / 2.0
";
    assert_eq!(
        como_numero(ejecuta(f, 7, 0)),
        3.5,
        "si fuera entera, saldria 3"
    );
}

// -------------------------------------------------------------------
//  ** LA REGLA 11, que se comprueba en lo que NO se emite
// -------------------------------------------------------------------

/// **La Regla 11 no se puede probar mirando un resultado**: `a * b + c` da el
/// mismo numero con la operacion fundida y sin ella casi siempre. La diferencia
/// esta en el redondeo de en medio, y solo aparece en unos pocos valores de
/// cada millon.
///
/// Asi que se prueba mirando los BYTES: si no hay una instruccion de
/// multiplicar-y-sumar emitida, no hay forma de que el redondeo se salte.
///
/// ** Y esto es la portabilidad que C no da. Un compilador de C con las
/// banderas de siempre PUEDE fundir esas dos operaciones, y entonces el mismo
/// fuente da bits distintos en dos maquinas. INTI lo prohibe y paga el precio
/// en velocidad, porque el argumento de venta de este sistema es que se puede
/// verificar -- y no se verifica lo que no da el mismo resultado dos veces.
#[test]
fn la_regla_11_no_funde_la_multiplicacion_con_la_suma() {
    let f = "perfil llano\n\nfuncion f devuelve flotante64\n    devuelve 2.0 * 3.0 + 1.0\n";
    let e = emitido(f);
    // Las instrucciones de multiplicar-y-sumar viven todas detras de dos
    // prefijos concretos. Que no aparezca ninguno es la prueba.
    let fundida = e.codigo.iter().any(|b| *b == 0xC4 || *b == 0x62);
    assert!(!fundida, "se emitio una instruccion de multiplicar-y-sumar");
    // Y da el numero correcto, que sin esto seria una prueba que aprueba un
    // programa que no calcula nada.
    assert_eq!(como_numero(ejecuta(f, 0, 0)), 7.0);
}

/// El mismo fuente, los mismos bytes. Dos veces.
///
/// ** Parece tonto y no lo es: es la mitad comprobable de *"el mismo programa da
/// el mismo bit"*. Si el emisor tuviera cualquier cosa que dependiera del
/// entorno --el orden de un mapa, una direccion, la hora-- se veria aqui.
#[test]
fn el_mismo_fuente_emite_los_mismos_bytes() {
    let a = emitido(SUMA_FLOTANTE);
    let b = emitido(SUMA_FLOTANTE);
    assert_eq!(a.codigo, b.codigo);
}

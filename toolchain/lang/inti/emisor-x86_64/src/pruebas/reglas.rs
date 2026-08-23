//! LAS DOCE REGLAS, en bytes y corriendo.
//!
//! Que atrapen, que atrapen con SU codigo, y que no atrapen cuando no toca.
//! La ultima es la que mas se olvida y la que rompe programas correctos.

use super::*;
// ===================================================================
//  ** F5e -- LAS REGLAS QUE SE CALCULABAN Y NO LLEGABAN A UN BYTE
// ===================================================================
//
//  De las cuatro comprobaciones de la IR, **una sola llegaba al binario**. Las
//  otras tres estaban declaradas, contadas en la IR, documentadas... y el
//  emisor las descontaba y no emitia nada.
//
//  El motivo estaba escrito y era honesto -- *"piden mirar un operando ANTES de
//  la operacion"* -- pero era un diagnostico, no un arreglo. El arreglo era
//  mover la comprobacion al sitio donde sirve, y eso es de la IR, no del
//  emisor: por eso el fallo sobrevivio a que alguien lo entendiera.
//
//  ** Y la que sigue sin salir --la 2-- ahora esta sola y por OTRO motivo: no
//  hay contra que comprobar, porque un `bufer` no lleva su longitud. Esa espera
//  a `lista de T`. Un pendiente con su causa exacta vale mucho mas que tres
//  juntos con una causa que solo explicaba dos.

/// Los codigos con los que atrapa cada regla, tal como salen en el registro de
/// retorno.
const DESBORDE: u64 = 1001;
const ENTRE_CERO: u64 = 1003;
const CONVERSION: u64 = 1012;

// -------------------------------------------------------------------
//  REGLA 3 -- dividir entre cero
// -------------------------------------------------------------------

const DIVIDE: &str = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve a entre b
";

/// ** LA PRUEBA QUE NO SE PODIA ESCRIBIR HASTA HOY.
///
/// Antes esto no daba 1003: **se llevaba el emulador por delante**, igual que
/// se lleva un procesador de verdad. Dividir entre cero en x86 no da un numero
/// raro -- levanta una excepcion antes de dejar nada.
///
/// Y por eso la comprobacion tenia que ir ANTES: despues de la division no hay
/// programa que mire el resultado.
#[test]
fn dividir_entre_cero_atrapa_con_su_codigo() {
    assert_eq!(ejecuta(DIVIDE, 10, 0), ENTRE_CERO);
}

/// Y dividir de verdad sigue dividiendo. Sin esto, una comprobacion que
/// atrapara SIEMPRE pasaria la prueba de arriba.
#[test]
fn dividir_entre_algo_sigue_dando_el_cociente() {
    assert_eq!(ejecuta(DIVIDE, 10, 2), 5);
    assert_eq!(ejecuta(DIVIDE, 7, 7), 1);
}

/// El resto tambien: es la misma instruccion y el mismo cero.
#[test]
fn el_resto_entre_cero_tambien_atrapa() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a resto b\n";
    assert_eq!(ejecuta(f, 10, 0), ENTRE_CERO);
    assert_eq!(ejecuta(f, 10, 3), 1);
}

// -------------------------------------------------------------------
//  ** DOS REGLAS, DOS CODIGOS -- que es lo que el destino unico impedia
// -------------------------------------------------------------------

/// ** Con un solo sitio al que saltar, atrapar por dividir entre cero habria
/// devuelto **1001** -- el codigo de desbordar -- y el programa habria dicho
/// que le paso otra cosa.
///
/// No es un detalle de presentacion: un error como dato que miente sobre su
/// causa es peor que no tenerlo, porque quien lo lea va a buscar donde no es.
#[test]
fn cada_regla_atrapa_con_SU_codigo_y_no_con_el_de_otra() {
    let dos_reglas = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve (a * a) entre b
";
    // Multiplicar sin pasarse, dividir entre cero -> 1003.
    assert_eq!(ejecuta(dos_reglas, 3, 0), ENTRE_CERO);
    // Multiplicar pasandose -> 1001, y en el MISMO binario.
    assert_eq!(ejecuta(dos_reglas, 1 << 40, 1), DESBORDE);
    // Y sin pasarse ni dividir entre cero, el numero.
    assert_eq!(ejecuta(dos_reglas, 6, 4), 9);
}

// -------------------------------------------------------------------
//  REGLA 12 -- convertir un flotante que no cabe
// -------------------------------------------------------------------

fn convierte(tipo: &str, expr: &str) -> u64 {
    ejecuta(
        &format!(
            "perfil llano\n\nfuncion f(a es entero64, b es entero64) devuelve entero64\n    devuelve {}({})\n",
            tipo, expr
        ),
        0,
        0,
    )
}

/// El caso de la sonda `r12_conversion`: 1e30 no es ningun `entero32`.
#[test]
fn un_flotante_que_no_cabe_atrapa() {
    assert_eq!(convierte("entero32", "1e30"), CONVERSION);
    assert_eq!(convierte("entero64", "1e30"), CONVERSION);
}

/// ** Y EL ANCHO IMPORTA, que es por lo que la comprobacion lo lleva dentro.
///
/// El mismo numero cabe en uno y no en el otro. Una comprobacion que no supiera
/// contra que mide seria una que aprueba todo.
#[test]
fn el_mismo_numero_cabe_en_uno_y_no_en_el_otro() {
    assert_eq!(convierte("entero64", "1e10"), 10_000_000_000);
    assert_eq!(convierte("entero32", "1e10"), CONVERSION);
}

/// Los tres anchos estrechos, cada uno en su borde.
#[test]
fn cada_ancho_atrapa_en_su_borde() {
    assert_eq!(convierte("entero8", "127.0"), 127);
    assert_eq!(convierte("entero8", "128.0"), CONVERSION);
    assert_eq!(convierte("entero16", "32767.0"), 32767);
    assert_eq!(convierte("entero16", "32768.0"), CONVERSION);
}

/// ** TRUNCAR NO ES REDONDEAR, y aqui es donde se ve que la comprobacion mide
/// lo correcto.
///
/// `-128.5` no cabe *como numero* en un `entero8`, pero **truncado si**: da
/// -128. Una comprobacion escrita contra el valor original y no contra el
/// truncado rechazaria este programa, que es correcto.
#[test]
fn lo_que_truncado_cabe_no_atrapa_aunque_el_original_no_quepa() {
    assert_eq!(convierte("entero8", "0.0 - 128.5") as i8, -128);
    assert_eq!(convierte("entero8", "127.9"), 127);
}

/// ** EL NaN, que es el que se cuela sin la bandera de "no comparable".
///
/// Truncar un NaN devuelve el mismo centinela que un desbordamiento. Al
/// compararlo con el limite sale "no comparable", que enciende la bandera de
/// igualdad **a la vez** que la de paridad -- asi que sin mirar la segunda,
/// esto pasaria por un numero legitimo.
#[test]
fn un_nan_no_es_ningun_entero() {
    assert_eq!(convierte("entero64", "0.0 / 0.0"), CONVERSION);
    assert_eq!(convierte("entero32", "0.0 / 0.0"), CONVERSION);
}

/// Y el infinito tampoco.
#[test]
fn el_infinito_no_es_ningun_entero() {
    assert_eq!(convierte("entero64", "1.0 / 0.0"), CONVERSION);
    assert_eq!(convierte("entero64", "0.0 - 1.0 / 0.0"), CONVERSION);
}

/// ** EL CASO QUE HACE FALTA EL SEGUNDO PASO: `-2^63` SI cabe.
///
/// Truncarlo devuelve el mismo centinela que un desbordamiento, asi que una
/// comprobacion que solo mirara el centinela rechazaria un numero perfectamente
/// legitimo -- el mas negativo que existe.
///
/// Por eso, cuando sale el centinela, se compara el ORIGINAL con `-2^63` exacto.
#[test]
fn el_entero_mas_negativo_no_es_un_desbordamiento() {
    let r = convierte("entero64", "0.0 - 9223372036854775808.0");
    assert_eq!(r, i64::MIN as u64, "el mas negativo se rechazo, y es valido");
}

/// Lo justo por debajo si desborda.
#[test]
fn justo_por_debajo_del_mas_negativo_atrapa() {
    assert_eq!(
        convierte("entero64", "0.0 - 9300000000000000000.0"),
        CONVERSION
    );
}

/// Y lo normal sigue funcionando, que es lo que una comprobacion mal puesta se
/// lleva por delante sin que nadie lo note hasta que un programa real falla.
#[test]
fn las_conversiones_normales_no_atrapan() {
    assert_eq!(convierte("entero64", "2.9"), 2);
    assert_eq!(convierte("entero32", "1000.0"), 1000);
    assert_eq!(convierte("entero64", "0.0"), 0);
    assert_eq!(convierte("entero8", "0.0 - 1.0") as i8, -1);
}

// -------------------------------------------------------------------
//  Y la cuenta, que es lo que se puede seguir en el tiempo
// -------------------------------------------------------------------

/// ** Las que la IR pide y las que el binario lleva ya CUADRAN para tres de las
/// cuatro reglas.
///
/// El `Emitido` cuenta las que SALIERON, no las que se pidieron, y esa
/// diferencia es a proposito: el dia que haya eliminacion de comprobaciones,
/// restar los dos numeros dara exactamente lo que el optimizador quito.
///
/// Hoy la unica diferencia es la Regla 2, y tiene su motivo escrito.
#[test]
fn lo_que_la_ir_pide_y_lo_que_el_binario_lleva_ya_cuadran() {
    let f = "\
perfil llano

funcion f(a es entero64, b es entero64) devuelve entero64
    devuelve (a + b) entre (a - b)
";
    // Dos sumas/restas (Regla 1) y una division, que trae DOS: el divisor cero
    // (Regla 3) y el cociente que no cabe (Regla 1 otra vez, desde el 22-08).
    assert_eq!(reglas_de(f), 4);
    assert_eq!(
        emitido(f).comprobaciones,
        4,
        "la IR pide cuatro y el binario tiene que llevar cuatro"
    );
}

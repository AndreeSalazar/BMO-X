//! **LA CONVENCION DE LLAMADA** -- por donde viaja cada argumento.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- si esto manda un argumento por la pila y el otro lado lo
//!            espera en un registro, la funcion lee basura. El banco lo ve en
//!            la primera fila que pase seis argumentos o un struct
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # Hasta el 19-09: todo por la pila
//!
//! BMO C empujaba cada argumento (`push rax`, de derecha a izquierda) y la
//! funcion los leia en `[rbp+16]`, `[rbp+24]`... Simple y uniforme, y con dos
//! precios que el censo por patron puso en la mesa: el 4 % de lo que ejecutaba
//! el banco era `push` de argumentos, y **un parametro nunca podia vivir en un
//! registro**, porque su hueco estaba en la pila del llamante y el troquel lo
//! descartaba ("offset positivo = parametro").
//!
//! # Desde el 19-09: HIBRIDA, y el porque de cada mitad
//!
//! ```text
//!    escalares y punteros, los 6 primeros     rdi, rsi, rdx, rcx, r8, r9
//!    el septimo en adelante                    la pila, como siempre
//!    structs por valor y flotantes             la pila, como siempre
//!    funciones VARIADICAS                      TODO por la pila
//! ```
//!
//! ** La regla de las variadicas es la que decide el diseno. El `va_arg` de
//! BMO C es `*ap++` sobre la pila (ver `frame.rs`): si los seis primeros
//! llegaran en registros, el `va_list` tendria que saltar de un area de guardado
//! a la pila del llamante, que es exactamente la `struct va_list` de SysV y sus
//! tres campos. Mandar TODO por la pila cuando la funcion es variadica es una
//! linea aqui y cero lineas alli.
//!
//! ** Y tiene una consecuencia que hay que decir: **el llamante tiene que saber
//! si la funcion es variadica.** Por nombre lo sabe (la definicion o el
//! prototipo con `...`). A traves de un puntero NO puede saberlo, porque el tipo
//! de un puntero a funcion no lleva su lista de parametros. Por eso **tomar la
//! direccion de una funcion variadica es un error de compilacion**: es la unica
//! forma de que no compile algo que no hace lo que dice.
//!
//! # Lo que esto NO cambia
//!
//! El retorno sigue en `rax`. Los agregados siguen por valor en la pila, por
//! ranuras. `printf`, `scanf` y los intrinsecos tienen su camino propio. Y todas
//! las unidades (`.bo`) las compila este mismo emisor, asi que no hay dos
//! convenciones vivas en un mismo `.bex`.

/// Los seis registros de argumento, por su numero, en orden.
pub(in crate::codegen) const REGISTROS: [u8; 6] = [7, 6, 2, 1, 8, 9]; // rdi rsi rdx rcx r8 r9

/// Por donde viaja un argumento.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::codegen) enum Paso {
    /// En este registro (su numero).
    Registro(u8),
    /// Por la pila, empujado de derecha a izquierda con los demas de la pila.
    Pila,
}

/// El paso de cada argumento. `de_registro[i]` dice si el i-esimo es escalar
/// o puntero (los agregados y los flotantes no lo son); con `variadica`, todo
/// va por la pila.
pub(in crate::codegen) fn clasificar(de_registro: &[bool], variadica: bool) -> Vec<Paso> {
    let mut usados = 0;
    de_registro
        .iter()
        .map(|&escalar| {
            if !variadica && escalar && usados < REGISTROS.len() {
                usados += 1;
                Paso::Registro(REGISTROS[usados - 1])
            } else {
                Paso::Pila
            }
        })
        .collect()
}

/// **Donde se queda un parametro en una funcion HOJA** (19-09, tarde).
///
/// Una funcion que no llama a nadie, no copia structs y no pone a cero (ver
/// `registros::pisa_argumentos`) no tiene por que bajar sus parametros al
/// marco: pueden vivir en un registro toda la funcion, y sin push/pop, porque
/// ninguno de estos se le debe a quien llamo. Pero no todos pueden quedarse
/// DONDE llegaron:
///
/// ```text
///    rdi, rsi, r8, r9    se quedan: el emisor no los toca fuera de una llamada
///    rdx                 se TRASLADA a r10: el resto de la division cae en rdx
///                        y `t[i] = v` lo usa de direccion
///    rcx                 se TRASLADA a r11: es el scratch del operando derecho
///                        y la cuenta de todo desplazamiento
/// ```
///
/// El censo que lo pidio (19-09, `BMO_CENSO`): en los 25 programas del metro
/// habia 5 parametros en r8/r9 de funciones hoja y **22 en rdx/rcx**; en DOOM,
/// 7 y **54**. Lo que bajaba al marco sin motivo eran el tercero y el cuarto,
/// no el quinto y el sexto -- y por eso el traslado y no solo la residencia.
///
/// r10 y r11 son de los que una llamada PISA y el emisor solo los emite
/// dentro de `scanf`, `printf`, los intrinsecos y las sintetizadas: todo lo
/// que hace `pisa == true`. En una hoja estan enteros sin usar.
pub(in crate::codegen) fn residencia(reg: u8) -> u8 {
    match reg {
        2 => 10,
        1 => 11,
        r => r,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    #[test]
    fn en_una_hoja_los_de_argumento_se_quedan_y_los_scratch_se_trasladan() {
        assert_eq!(REGISTROS.map(residencia), [7, 6, 10, 11, 8, 9]);
        // y ningun destino es otro origen: el traslado no pisa a nadie
        for d in REGISTROS.map(residencia) {
            assert!(d == residencia(d));
        }
    }

    #[test]
    fn seis_escalares_van_en_orden_y_el_septimo_a_la_pila() {
        let p = clasificar(&[true; 7], false);
        assert_eq!(
            p,
            [
                Paso::Registro(7), Paso::Registro(6), Paso::Registro(2), Paso::Registro(1),
                Paso::Registro(8), Paso::Registro(9), Paso::Pila
            ]
        );
    }

    #[test]
    fn un_agregado_en_medio_no_gasta_registro() {
        // f(int, struct, int): el struct a la pila y el tercero en rsi
        assert_eq!(clasificar(&[true, false, true], false), [Paso::Registro(7), Paso::Pila, Paso::Registro(6)]);
    }

    #[test]
    fn una_variadica_manda_todo_por_la_pila() {
        assert_eq!(clasificar(&[true, true], true), [Paso::Pila, Paso::Pila]);
        assert!(clasificar(&[], true).is_empty());
    }
}

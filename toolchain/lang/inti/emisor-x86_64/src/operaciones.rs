//! `operaciones` -- COMO SE CALCULA, en bytes.
//!
//! ## Por que soy un fichero y no un trozo del de al lado (L6b)
//!
//! Porque contesto *"que instruccion hace esta cuenta"*, y ninguna otra cosa.
//! No se donde estan los valores --eso es `marco`--, ni si el programa tiene
//! derecho a hacerla --eso es `perfil`--, ni que pasa si sale mal --eso es
//! `reglas`.
//!
//! ** Y el corte tiene una consecuencia que se ve al leerlo: aqui dentro hay
//! DOS aritmeticas, la de enteros y la de coma flotante, escritas una debajo de
//! otra. Que quepan juntas y no se estorben es la prueba de que `Clase` esta
//! bien puesta en la IR -- si hubiera que mezclarlas, seria que el emisor esta
//! adivinando algo que le tendrian que haber dicho.

use bmo_inti_front::arbol::Op;
use bmo_lower::x86;

use crate::{DER, IZQ};

/// *** `sin_signo` cambia CUATRO instrucciones, y ninguna falla al emitirse:
/// las cuatro dan otro numero. Ver `medidas.toml`, seccion `sin_signo`.
pub(crate) fn binaria(out: &mut Vec<u8>, op: Op, sin_signo: bool) {
    match op {
        Op::Suma => x86::add_r64_r64(out, IZQ, DER),
        Op::Resta => x86::sub_r64_r64(out, IZQ, DER),
        Op::Por => x86::imul_r64_r64(out, IZQ, DER),
        Op::Entre | Op::Divide | Op::Resto => {
            // ** La guardia del cociente NO esta aqui: la pide la IR con
            // `Comprobacion::Cociente` y la emite `Instr::Comprueba`, como las
            // otras cuatro. Un emisor que anadiera reglas por su cuenta romperia
            // la cuenta que compara lo que la IR pide con lo que el binario
            // lleva -- y esa resta es la que medira al optimizador.
            // ** `div` limpia `rdx` con un `xor`; `idiv` lo rellena con el
            // signo de `rax` (`cqo`). Usar `cqo` + `div` o `xor` + `idiv` no
            // falla: divide otra cosa.
            if sin_signo {
                x86::zero_r32(out, 2); // xor edx, edx
                x86::div_r64(out, DER);
            } else {
                x86::cqo(out);
                x86::idiv_r64(out, DER);
            }
            if matches!(op, Op::Resto) {
                x86::mov_r64_r64(out, IZQ, 2); // el resto vive en rdx
            }
        }
        Op::BitsY => {
            out.extend_from_slice(&[0x48, 0x21, 0xC8]); // and rax, rcx
        }
        Op::BitsO => x86::or_r64_r64(out, IZQ, DER),
        Op::BitsXor => x86::xor_r64_r64(out, IZQ, DER),

        // ** LOS DESPLAZAMIENTOS, Y LA REGLA 7 DENTRO.
        //
        // Hasta el 21-08 estos dos caian en el `_ => {}` de abajo y **no se
        // emitia nada**: `x desplaza izquierda 8` devolvia `x` intacto.
        // Compilaba, corria, y daba otro numero. Lo destapo la sonda del Ryzen
        // al intentar imprimir un hexadecimal, que es el primer programa de
        // INTI que necesitaba desplazar de verdad.
        //
        // ** Y no basta con la instruccion, porque el silicio no hace lo que
        // INTI promete: se queda con los SEIS BITS BAJOS del contador, asi que
        // desplazar 64 posiciones desplaza cero y devuelve el numero entero. La
        // Regla 7 dice que da CERO, y eso hay que emitirlo.
        //
        // Tres instrucciones de mas, y el salto no salta salvo cuando el
        // programa pidio algo que no tiene sentido.
        Op::DesplazaIzquierda | Op::DesplazaDerecha => {
            if matches!(op, Op::DesplazaIzquierda) {
                x86::shl_r64_cl(out, IZQ);
            } else if sin_signo {
                x86::shr_r64_cl(out, IZQ);
            } else {
                // ** ARRASTRANDO EL SIGNO. Hasta el 2026-08-23 esto era `shr`
                // siempre, asi que `-8 desplaza derecha 1` daba
                // 9.223.372.036.854.775.804 en vez de -4. El fallo al reves del
                // de las comparaciones, y del mismo dia.
                x86::sar_r64_cl(out, IZQ);
            }
            // El `cmp` va DESPUES a proposito: el desplazamiento no toca el
            // contador, asi que sigue entero para poder mirarlo.
            x86::cmp_r64_imm32(out, DER, 64);
            let cabe = x86::salto_corto(out, 0x72); // jb
            x86::zero_r32(out, IZQ);
            x86::cierra_salto_corto(out, cabe);
        }

        // Las comparaciones dejan el resultado en 0/1.
        Op::Igual | Op::NoEs | Op::Menor | Op::Mayor | Op::MenorIgual | Op::MayorIgual => {
            x86::cmp_r64_r64(out, IZQ, DER);
            // ** El orden importa y costo un test: `setcc` PRIMERO y despues
            // extender. Poner el registro a cero antes con un `xor` --que es lo
            // que hace `zero_r32`-- **destruye las banderas que el `cmp` acaba
            // de dejar**, y entonces la comparacion contesta siempre lo mismo.
            // ** IGUAL y NO ES no dependen del signo: dos patrones de bits son
            // iguales o no lo son. Las otras cuatro SI, y esa es toda la
            // diferencia entre `setl` y `setb`.
            let cc = match op {
                Op::Igual => 0x94,
                Op::NoEs => 0x95,
                Op::Menor if sin_signo => 0x92,      // setb
                Op::Mayor if sin_signo => 0x97,      // seta
                Op::MenorIgual if sin_signo => 0x96, // setbe
                _ if sin_signo => 0x93,              // setae
                Op::Menor => 0x9C,                   // setl
                Op::Mayor => 0x9F,                   // setg
                Op::MenorIgual => 0x9E,              // setle
                _ => 0x9D,                           // setge
            };
            out.extend_from_slice(&[0x0F, cc, 0xC0]); // setcc al
            out.extend_from_slice(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
        }
        // Lo que pide runtime o no cabe en una instruccion.
        _ => {}
    }
}

/// Las operaciones de coma flotante.
///
/// ## ** EL MODELO, y por que este y no el bueno
///
/// Los valores viven en registros normales **como patron de bits** y solo cruzan
/// a los de coma flotante para la operacion. Cuesta dos cruces por operacion.
///
/// A cambio: **el asignador de registros, el marco y la convencion de llamada no
/// cambian ni una linea**. Ese es el trato entero. La version rapida --repartir
/// tambien los registros de coma flotante-- es un asignador nuevo, y no se
/// escribe hasta que haya algo que medir. El dia que se escriba, lo que cambia
/// es DONDE viven los valores, no que operacion se emite.
///
/// ## Lo que NO lleva detras: ninguna comprobacion
///
/// Y no es una excepcion a "INTI no tiene comportamiento indefinido". Es que
/// IEEE-754 **define** el desbordamiento y la division por cero: dan infinito y
/// NaN, que son valores. La Regla 1 y la Regla 3 existen porque en los enteros
/// esos dos casos no tienen respuesta; aqui la tienen, y desde 1985.
///
/// ## ** Y LA REGLA 11, que es la que se ve en lo que NO esta escrito aqui
///
/// No hay `fma`, y no hay reasociacion. `a * b + c` emite una multiplicacion y
/// una suma, con su redondeo en medio, **aunque la maquina sepa hacer las dos de
/// una vez y mas preciso**. Se deja rendimiento en la mesa a proposito: el mismo
/// programa tiene que dar el mismo bit en cualquier maquina, y esa es la unica
/// portabilidad que C no dio nunca.
pub(crate) fn flotante(out: &mut Vec<u8>, op: Op) {
    match op {
        Op::Suma | Op::Resta | Op::Por | Op::Divide => {
            x86::movq_xmm_de_r64(out, 0, IZQ);
            x86::movq_xmm_de_r64(out, 1, DER);
            match op {
                Op::Suma => x86::addsd(out),
                Op::Resta => x86::subsd(out),
                Op::Por => x86::mulsd(out),
                _ => x86::divsd(out),
            }
            x86::movq_r64_de_xmm(out, IZQ, 0);
        }

        // ** LAS COMPARACIONES, Y EL NaN, que es donde esto se gana o se pierde
        //
        // Comparar deja las banderas como una comparacion SIN SIGNO --el
        // silicio ya tradujo-- y por eso se reusan los mismos `setcc`. Lo que no
        // traduce es el NaN: sale "no comparable" y enciende las tres banderas a
        // la vez, incluida la de "menor".
        //
        // Consecuencia: preguntar `a < b` con la bandera de menor contestaria
        // **que si** cuando alguno es NaN. Asi que `<` y `<=` se hacen DANDO LA
        // VUELTA a los operandos y preguntando por `>` y `>=`, que miran la
        // bandera que el NaN deja en el otro sentido.
        //
        // Es un truco de una linea y evita dos saltos por comparacion.
        Op::Igual | Op::NoEs | Op::Menor | Op::Mayor | Op::MenorIgual | Op::MayorIgual => {
            let del_reves = matches!(op, Op::Menor | Op::MenorIgual);
            let (a, b) = if del_reves { (DER, IZQ) } else { (IZQ, DER) };
            x86::movq_xmm_de_r64(out, 0, a);
            x86::movq_xmm_de_r64(out, 1, b);
            x86::comisd(out);
            match op {
                // seta / setae: falsas ante un NaN, que es lo que manda IEEE.
                Op::Mayor | Op::Menor => x86::setcc_low(out, 0x97, IZQ),
                Op::MayorIgual | Op::MenorIgual => x86::setcc_low(out, 0x93, IZQ),
                // ** La igualdad NO se puede hacer con una sola bandera: el NaN
                // enciende la de igual. Hay que exigir ADEMAS que si fueran
                // comparables. Dos `setcc` y un `and`.
                Op::Igual => {
                    x86::setcc_low(out, 0x94, IZQ); // sete
                    x86::setcc_low(out, 0x9B, DER); // setnp -- y comparables
                    x86::and_low_low(out, IZQ, DER);
                }
                // ** Y la desigualdad es la unica comparacion que un NaN hace
                // CIERTA. No es una rareza: `x no es x` es como se pregunta si
                // algo es NaN, y tiene que contestar que si.
                _ => {
                    x86::setcc_low(out, 0x95, IZQ); // setne
                    x86::setcc_low(out, 0x9A, DER); // setp -- o no comparables
                    x86::or_low_low(out, IZQ, DER);
                }
            }
            x86::movzx_r64_low(out, IZQ, IZQ);
        }

        // Los bits, el resto y el cociente entero no existen aqui, y no se
        // emite nada **porque `disposicion` ya los denuncio con E0123**. Este
        // camino solo se recorre en un programa que no va a llegar a ejecutarse.
        _ => {}
    }
}

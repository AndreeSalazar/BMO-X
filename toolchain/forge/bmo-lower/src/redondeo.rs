//! Redondeo decimal -- **libreria, no puerta**.
//!
//! # Por que esto no es un detalle de formato
//!
//! En un banco el redondeo **es una decision legal**. Medio centimo repetido
//! cuatro millones de veces es dinero de verdad, y hay jurisdicciones que
//! obligan al *redondeo del banquero* (`NEAREST-EVEN`) precisamente porque el
//! clasico --siempre hacia arriba en el empate-- tiene sesgo: acumula a favor de
//! quien redondea.
//!
//! Por eso los modos van **todos**, con el nombre del estandar, y no "el
//! redondeo" a secas. Un compilador que ofrezca uno solo obliga a elegir el que
//! tiene, y esa eleccion la tiene que hacer quien responde del cuadre.
//!
//! # Que hace
//!
//! Una unica operacion: **dividir un entero escalado entre una potencia de
//! diez**, que es exactamente lo que pasa cuando un resultado tiene mas
//! decimales de los que su PICTURE guarda.
//!
//! ```text
//!   1999 x 3 = 5997 ... / 100 -> 59 o 60?   <- eso decide el modo
//! ```
//!
//! # Por que vive aqui y no en el frontend de COBOL
//!
//! Por la regla de la cabecera de [`crate::fmt`]: se comparten **contratos y
//! librerias, nunca cerebros**. Partir un entero y decidir el ultimo digito es
//! aritmetica, no la semantica de un lenguaje -- el Annex F de Ada define los
//! mismos modos con otros nombres, y PL/I tambien.
//!
//! Lo que **si** se queda en COBOL es *cual* se aplica y *cuando*: eso lo dice
//! la clausula `ROUNDED` y la escala de la PICTURE.

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI};

/// Los modos del estandar. Los nombres son los de COBOL-2002 traducidos, y cada
/// uno dice que hace **en el empate**, que es donde se diferencian.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modo {
    /// Tira los decimales sobrantes. **Es lo que hace COBOL sin `ROUNDED`**, y
    /// por eso es el que ya estaba emitido antes de que este modulo existiera.
    Truncar,
    /// `NEAREST-AWAY-FROM-ZERO` -- el `ROUNDED` clasico. El empate va **lejos
    /// del cero**: `0.5 -> 1`, `-0.5 -> -1`.
    ///
    /// Es el que espera cualquiera que escriba `ROUNDED` sin decir mas, y por
    /// eso es el que se aplica cuando no se nombra un modo.
    MasCercanoLejosDeCero,
    /// `NEAREST-EVEN` -- **el redondeo del banquero**. El empate va al digito
    /// PAR: `0.5 -> 0`, `1.5 -> 2`, `2.5 -> 2`.
    ///
    /// Existe porque el clasico tiene **sesgo**: en una muestra grande, los
    /// empates siempre suben, y eso son centimos que aparecen de la nada. Al
    /// mandar la mitad de los empates a cada lado, el sesgo desaparece.
    MasCercanoPar,
    /// `NEAREST-TOWARD-ZERO` -- el empate va **hacia el cero**.
    MasCercanoHaciaCero,
    /// `TOWARD-GREATER` -- hacia arriba siempre (techo).
    HaciaArriba,
    /// `TOWARD-LESSER` -- hacia abajo siempre (suelo).
    HaciaAbajo,
}

impl Modo {
    /// El nombre del estandar, para los mensajes de error.
    pub fn nombre(self) -> &'static str {
        match self {
            Modo::Truncar => "TRUNCATION",
            Modo::MasCercanoLejosDeCero => "NEAREST-AWAY-FROM-ZERO",
            Modo::MasCercanoPar => "NEAREST-EVEN",
            Modo::MasCercanoHaciaCero => "NEAREST-TOWARD-ZERO",
            Modo::HaciaArriba => "TOWARD-GREATER",
            Modo::HaciaAbajo => "TOWARD-LESSER",
        }
    }
}

/// Emite `rax = rax / rcx`, aplicando `modo` al resto.
///
/// - Entrada: `rax` = dividendo con signo, `rcx` = divisor **positivo**.
/// - Salida: `rax` = el cociente ya redondeado.
/// - Ensucia `rdx`, `rsi` y `rdi`. `rcx` sobrevive.
///
/// El divisor tiene que ser positivo porque aqui siempre es una potencia de
/// diez: la escala de una PICTURE. Con un divisor negativo el signo del resto
/// deja de coincidir con el del dividendo y las comparaciones de abajo dirian
/// otra cosa.
pub fn dividir(code: &mut Vec<u8>, modo: Modo) {
    // `idiv` deja cociente en rax y resto en rdx, **y el resto lleva el signo
    // del dividendo**. Eso es lo que permite decidir el ajuste mirando solo
    // rdx, sin acordarse de por donde entro el numero.
    x86::cqo(code);
    x86::idiv_r64(code, RCX);

    if modo == Modo::Truncar {
        return;
    }

    match modo {
        Modo::Truncar => unreachable!(),

        // Techo y suelo no miran cuanto sobra, solo hacia que lado: si hay
        // resto y el numero es positivo, el techo sube; si hay resto y es
        // negativo, el suelo baja.
        // * El techo NO usa el ajuste "en la direccion del resto": solo sube, y
        // solo cuando el resto es POSITIVO. Con un resto negativo el truncado
        // ya dio el techo, porque truncar va hacia el cero -- y hacia el cero,
        // desde un numero negativo, es hacia arriba. Escribirlo con el ajuste
        // simetrico daba `techo(-1.5) = -2`, que es el suelo.
        Modo::HaciaArriba => {
            x86::test_r64_r64(code, RDX, RDX);
            let sin_resto = x86::emit_jump(code, Jump::IfZero);
            let positivo = x86::emit_jump(code, Jump::IfNotSign);
            let negativo = x86::emit_jump(code, Jump::Always);
            x86::patch_jump(code, positivo);
            x86::inc_r64(code, RAX);
            x86::patch_jump(code, negativo);
            x86::patch_jump(code, sin_resto);
        }
        Modo::HaciaAbajo => {
            x86::test_r64_r64(code, RDX, RDX);
            let no_negativo = x86::emit_jump(code, Jump::IfNotSign);
            x86::dec_r64(code, RAX);
            x86::patch_jump(code, no_negativo);
        }

        // Los tres "mas cercano" comparan **el doble del resto** con el
        // divisor, que es la forma de preguntar "pasa de la mitad?" sin
        // dividir otra vez ni tocar fracciones.
        _ => {
            // rsi = |resto| x 2. Cabe siempre: el resto es menor que el
            // divisor, y el divisor es una potencia de diez de una PICTURE.
            x86::mov_r64_r64(code, RSI, RDX);
            x86::test_r64_r64(code, RSI, RSI);
            let ya_positivo = x86::emit_jump(code, Jump::IfNotSign);
            x86::neg_r64(code, RSI);
            x86::patch_jump(code, ya_positivo);
            x86::shl_r64_imm8(code, RSI, 1);

            // Se compara `divisor` contra `2|resto|` y no al reves para poder
            // salir con `ja`/`jae`, que es lo que hay: los dos son positivos,
            // asi que sin signo dice la verdad.
            x86::cmp_r64_r64(code, RCX, RSI);
            let fuera = match modo {
                // 2|r| >= d -> ajusta. Se sale si d > 2|r|.
                Modo::MasCercanoLejosDeCero => vec![x86::emit_jump(code, Jump::IfAbove)],
                // 2|r| > d -> ajusta. Se sale si d >= 2|r| (el empate NO ajusta).
                Modo::MasCercanoHaciaCero => vec![x86::emit_jump(code, Jump::IfAboveOrEqual)],
                // El del banquero: fuera si d > 2|r|; si son iguales, solo
                // ajusta cuando el cociente es IMPAR -- asi el empate acaba
                // siempre en par y el sesgo desaparece.
                Modo::MasCercanoPar => {
                    let mut salidas = vec![x86::emit_jump(code, Jump::IfAbove)];
                    let hay_que_ajustar = x86::emit_jump(code, Jump::IfNotZero);
                    // Empate: mirar el bit bajo del cociente.
                    x86::mov_r64_r64(code, RDI, RAX);
                    x86::and_r64_imm32(code, RDI, 1);
                    x86::test_r64_r64(code, RDI, RDI);
                    salidas.push(x86::emit_jump(code, Jump::IfZero)); // par -> se queda
                    x86::patch_jump(code, hay_que_ajustar);
                    salidas
                }
                _ => unreachable!(),
            };

            // El ajuste va en la direccion del RESTO, que es la del dividendo.
            x86::test_r64_r64(code, RDX, RDX);
            let negativo = negativo_salta(code);
            x86::inc_r64(code, RAX);
            x86::patch_jump(code, negativo);

            for s in fuera {
                x86::patch_jump(code, s);
            }
        }
    }
}

/// La MISMA regla, resuelta al compilar.
///
/// Hace falta porque un literal se escala en el compilador: `ADD 1.005 TO SALDO
/// ROUNDED` con `SALDO PIC V99` tiene que guardar `1.01`, y ese `1.005` nunca
/// llega a ejecutarse -- se convierte en un inmediato antes.
///
/// * Y de paso vale de **oraculo**: hay un test que compara esta funcion con lo
/// que hace el codigo emitido, valor a valor y modo a modo. Dos implementaciones
/// de la misma regla que tienen que coincidir es mucho mejor prueba que una sola
/// comparada contra una tabla escrita a mano -- porque la tabla la escribe quien
/// ya se equivoco.
pub fn dividir_en_rust(dividendo: i64, divisor: i64, modo: Modo) -> i64 {
    debug_assert!(divisor > 0, "el divisor es una potencia de diez de una PICTURE");
    let q = dividendo / divisor; // trunca hacia cero, como `idiv`
    let r = dividendo % divisor; // y el resto lleva el signo del dividendo
    if r == 0 || modo == Modo::Truncar {
        return q;
    }
    let paso = if r < 0 { -1 } else { 1 };
    let doble = (r as i128).unsigned_abs() as u128 * 2;
    let d = divisor as u128;
    match modo {
        Modo::Truncar => q,
        Modo::MasCercanoLejosDeCero => if doble >= d { q + paso } else { q },
        Modo::MasCercanoHaciaCero => if doble > d { q + paso } else { q },
        Modo::MasCercanoPar => {
            if doble > d || (doble == d && q % 2 != 0) {
                q + paso
            } else {
                q
            }
        }
        Modo::HaciaArriba => if r > 0 { q + 1 } else { q },
        Modo::HaciaAbajo => if r < 0 { q - 1 } else { q },
    }
}

/// `jns` + `dec rax` + `jmp`: resta uno si el resto era negativo, y devuelve el
/// salto que hay que apuntar al final para el camino positivo.
///
/// Existe porque el emisor de saltos tiene `jns` y no `js`, y escribir la
/// inversion a mano en los cuatro sitios que la necesitan es donde se cuela un
/// signo al reves.
fn negativo_salta(code: &mut Vec<u8>) -> usize {
    let positivo = x86::emit_jump(code, Jump::IfNotSign);
    x86::dec_r64(code, RAX);
    let fin = x86::emit_jump(code, Jump::Always);
    x86::patch_jump(code, positivo);
    fin
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    fn dividir_con(valor: i64, divisor: u64, modo: Modo) -> i64 {
        let mut code = Vec::new();
        dividir(&mut code, modo);
        let mut m = Machine::new(code);
        m.regs[RAX as usize] = valor as u64;
        m.regs[RCX as usize] = divisor;
        run(m, 200_000).regs[RAX as usize] as i64
    }

    /// * La tabla que decide todo: los mismos numeros por los seis modos.
    ///
    /// Se prueban **los dos signos** de cada caso, porque casi todos los
    /// errores de redondeo viven en el lado negativo: `-0.5` con el clasico es
    /// `-1` y con el truncado es `0`, y confundirlos convierte un cargo en un
    /// abono de un centimo que nadie ve hasta el cuadre anual.
    #[test]
    fn los_seis_modos_dicen_cosas_distintas_en_el_empate() {
        // divisor 10 -> una decima. 15 = 1.5, 25 = 2.5, 14 = 1.4, 16 = 1.6.
        //                        v=15  v=25  v=14  v=16
        let casos: &[(Modo, [i64; 4])] = &[
            (Modo::Truncar, [1, 2, 1, 1]),
            (Modo::MasCercanoLejosDeCero, [2, 3, 1, 2]),
            (Modo::MasCercanoPar, [2, 2, 1, 2]), // <- 2.5 baja a 2: par
            (Modo::MasCercanoHaciaCero, [1, 2, 1, 2]),
            (Modo::HaciaArriba, [2, 3, 2, 2]),
            (Modo::HaciaAbajo, [1, 2, 1, 1]),
        ];
        for (modo, esperados) in casos {
            for (i, v) in [15i64, 25, 14, 16].iter().enumerate() {
                assert_eq!(
                    dividir_con(*v, 10, *modo),
                    esperados[i],
                    "{} con {v}/10",
                    modo.nombre()
                );
            }
        }
    }

    /// El lado negativo, que es donde se cuelan los signos al reves.
    #[test]
    fn el_lado_negativo_es_el_espejo_menos_para_techo_y_suelo() {
        //                          v=-15 v=-25 v=-14 v=-16
        let casos: &[(Modo, [i64; 4])] = &[
            (Modo::Truncar, [-1, -2, -1, -1]),
            (Modo::MasCercanoLejosDeCero, [-2, -3, -1, -2]),
            (Modo::MasCercanoPar, [-2, -2, -1, -2]),
            (Modo::MasCercanoHaciaCero, [-1, -2, -1, -2]),
            // * Techo y suelo NO son simetricos, y ahi esta su gracia:
            // el techo de -1.5 es -1 (sube hacia el cero) y el suelo es -2.
            (Modo::HaciaArriba, [-1, -2, -1, -1]),
            (Modo::HaciaAbajo, [-2, -3, -2, -2]),
        ];
        for (modo, esperados) in casos {
            for (i, v) in [-15i64, -25, -14, -16].iter().enumerate() {
                assert_eq!(
                    dividir_con(*v, 10, *modo),
                    esperados[i],
                    "{} con {v}/10",
                    modo.nombre()
                );
            }
        }
    }

    /// Sin resto no hay nada que redondear, y ningun modo puede mover el
    /// numero. Un ajuste de mas aqui sumaria un centimo por operacion exacta.
    #[test]
    fn una_division_exacta_no_la_toca_nadie() {
        for modo in [
            Modo::Truncar,
            Modo::MasCercanoLejosDeCero,
            Modo::MasCercanoPar,
            Modo::MasCercanoHaciaCero,
            Modo::HaciaArriba,
            Modo::HaciaAbajo,
        ] {
            assert_eq!(dividir_con(500, 100, modo), 5, "{}", modo.nombre());
            assert_eq!(dividir_con(-500, 100, modo), -5, "{}", modo.nombre());
            assert_eq!(dividir_con(0, 100, modo), 0, "{}", modo.nombre());
        }
    }

    /// El caso del banquero contado con dinero: cuatro empates seguidos.
    ///
    /// Con el clasico los cuatro suben y aparecen dos centimos de la nada; con
    /// el del banquero, dos suben y dos bajan y la suma cuadra. **Ese es el
    /// sesgo por el que existe el modo.**
    #[test]
    fn el_sesgo_del_redondeo_clasico_se_ve_con_cuatro_empates() {
        let empates = [50i64, 150, 250, 350]; // 0.5, 1.5, 2.5, 3.5 en centesimas
        let clasico: i64 = empates
            .iter()
            .map(|v| dividir_con(*v, 100, Modo::MasCercanoLejosDeCero))
            .sum();
        let banquero: i64 = empates
            .iter()
            .map(|v| dividir_con(*v, 100, Modo::MasCercanoPar))
            .sum();
        // La suma exacta es 0.5+1.5+2.5+3.5 = 8.
        assert_eq!(clasico, 1 + 2 + 3 + 4, "el clasico sube los cuatro");
        assert_eq!(banquero, 0 + 2 + 2 + 4, "el del banquero reparte");
        assert_eq!(banquero, 8, "y por eso el del banquero cuadra con la suma exacta");
    }

    /// * LAS DOS IMPLEMENTACIONES TIENEN QUE COINCIDIR, valor a valor.
    ///
    /// Una la ejecuta el CPU y la otra corre en el compilador para los
    /// literales. Que digan lo mismo es mejor prueba que compararlas contra una
    /// tabla escrita a mano -- porque la tabla la escribe el mismo que se pudo
    /// equivocar en las dos.
    ///
    /// Se barre el rango entero alrededor de cada frontera: por debajo de la
    /// mitad, la mitad justa, por encima, y los dos signos.
    #[test]
    fn la_regla_emitida_y_la_del_compilador_dicen_lo_mismo() {
        let modos = [
            Modo::Truncar,
            Modo::MasCercanoLejosDeCero,
            Modo::MasCercanoPar,
            Modo::MasCercanoHaciaCero,
            Modo::HaciaArriba,
            Modo::HaciaAbajo,
        ];
        for modo in modos {
            for divisor in [10i64, 100] {
                // De -3 a +3 unidades enteras, paso a paso: cubre los empates,
                // los casi-empates y el cero por los dos lados.
                for v in (-3 * divisor)..=(3 * divisor) {
                    let en_rust = dividir_en_rust(v, divisor, modo);
                    let emitido = dividir_con(v, divisor as u64, modo);
                    assert_eq!(
                        emitido,
                        en_rust,
                        "{} con {v}/{divisor}: el codigo emitido dice {emitido} y el \
                         compilador {en_rust}",
                        modo.nombre()
                    );
                }
            }
        }
    }

    /// Con la escala del dinero, que es la que se usa de verdad.
    ///
    /// Estos son los numeros tal cual salen del emisor de `MULTIPLY`: los dos
    /// operandos llegan en centavos, el `imul` los deja en centavos al cuadrado
    /// y **esta division es la que los devuelve a centavos**.
    #[test]
    fn con_dos_decimales_y_dinero() {
        // 19.99 x 3 -> 1999 x 300 = 599 700, y /100 da 59.97 EXACTO.
        // Ningun modo puede tocarlo, y esa es la garantia que sostiene COBOL.
        for modo in [Modo::Truncar, Modo::MasCercanoLejosDeCero, Modo::MasCercanoPar] {
            assert_eq!(dividir_con(1999 * 300, 100, modo), 5997, "{}", modo.nombre());
        }
        // El 7,5 % de 133.33: 13333 x 750 = 9 999 750, /10 000 = 999.975
        // centavos. Truncado son 999 (9.99 EUR) y redondeado 1000 (10.00 EUR).
        // **Ese centimo es la razon por la que existe la clausula.**
        assert_eq!(dividir_con(9_999_750, 10_000, Modo::Truncar), 999);
        assert_eq!(dividir_con(9_999_750, 10_000, Modo::MasCercanoLejosDeCero), 1000);
    }
}

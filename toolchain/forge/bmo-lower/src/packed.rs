//! Decimal EMPAQUETADO (BCD) -- **libreria, no puerta**.
//!
//! # Que es
//!
//! Dos digitos por byte, en binario, y el ultimo nibble reservado para el
//! signo. `-1234` en 3 bytes es `01 23 4D`:
//!
//! ```text
//!   valor -1234, 3 bytes (5 huecos de digito + 1 de signo)
//!
//!    byte 0     byte 1     byte 2
//!   +----+----+----+----+----+----+
//!   | 0  | 1  | 2  | 3  | 4  | D  |   <- D = negativo
//!   +----+----+----+----+----+----+
//!     ^                        ^
//!     relleno              el SIGNO, no un digito
//! ```
//!
//! # Por que vive aqui y no en el frontend de COBOL
//!
//! Por la regla de la cabecera de [`crate::fmt`]: se comparten **contratos y
//! librerias, nunca cerebros**. Empaquetar dos digitos en un byte es una
//! REPRESENTACION, no la semantica de ningun lenguaje -- el `COMP-3` de COBOL,
//! el `Decimal` del Annex F de Ada y el `FIXED DECIMAL` de PL/I piden
//! exactamente los mismos nibbles en el mismo orden. Tenerlo tres veces seria
//! copiar el mismo bug de signo en tres sitios.
//!
//! Lo que **si** se queda en el lenguaje es decidir QUIEN es COMP-3, cuantos
//! digitos tiene y que escala lleva. Eso lo dice la PICTURE, y la PICTURE es de
//! COBOL.
//!
//! # Por que esto importa de verdad
//!
//! Porque es el formato en el que estan los datos que ya existen. Un banco no
//! guarda importes en `i64`: los guarda empaquetados, y lleva cuarenta anos
//! haciendolo. Sin esto se puede escribir COBOL nuevo, pero no LEER lo que hay.
//!
//! # Sin bucles
//!
//! El numero de bytes se conoce al compilar, asi que los dos emisores van
//! **desenrollados**: ni un salto hacia atras, ni un contador. Un campo de 18
//! digitos son diez bytes, o sea diez pasos -- desenrollarlo cuesta menos codigo
//! que el bucle que lo recorreria, y el emisor queda determinista.

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, R11};

/// El nibble de signo. Son los tres que produce un compilador; al LEER se
/// aceptan ademas los alternativos, ver [`desempaquetar`].
const POSITIVO: u32 = 0x0C;
const NEGATIVO: u32 = 0x0D;
/// Un campo sin `S` en su PIC. No es "positivo": es que no tiene signo, y el
/// estandar manda escribir `F` para que se distinga de uno que si lo tiene.
const SIN_SIGNO: u32 = 0x0F;

/// Cuantos bytes ocupa un campo empaquetado de `digitos` digitos.
///
/// Es `digitos + 1` nibbles (el de mas es el del signo) redondeado hacia
/// arriba. Coincide con lo que hace z/OS, que es el punto: los bytes tienen que
/// caer donde el fichero de origen los puso.
pub const fn bytes_para(digitos: u32) -> usize {
    (digitos as usize / 2) + 1
}

/// Emite codigo que ESCRIBE `rax` empaquetado en `[rcx]`.
///
/// - Entrada: `rax` = el entero escalado con signo, `rcx` = destino.
/// - Escribe exactamente `bytes` bytes a partir de `[rcx]`.
/// - Ensucia `rax`, `rdx`, `rdi` y `r11`. `rcx` queda intacto.
///
/// `con_signo` es el `S` de la PICTURE. Sin el, el campo guarda el VALOR
/// ABSOLUTO y marca `F` -- que es lo que dice el estandar y lo que evita que un
/// saldo en rojo se lea en verde en el sistema de al lado.
///
/// ## Truncamiento
///
/// Un campo empaquetado tiene un ancho EXACTO, asi que un valor que no cabe
/// pierde los digitos altos. Eso no es un descuido: es lo que COBOL manda hacer
/// al mover a una PIC mas corta, y es la diferencia entre este almacenamiento y
/// el de un `DISPLAY`, que hoy sigue siendo un registro de 64 bits entero.
pub fn empaquetar(code: &mut Vec<u8>, bytes: usize, con_signo: bool) {
    assert!(bytes >= 1 && bytes <= 127, "campo empaquetado de {bytes} bytes fuera de rango");

    // El nibble de signo se decide ANTES de tocar el valor, porque el paso
    // siguiente le quita el signo para poder dividir sin signo.
    x86::mov_r32_imm32(code, R11, if con_signo { POSITIVO } else { SIN_SIGNO });
    x86::test_r64_r64(code, RAX, RAX);
    let no_negativo = x86::emit_jump(code, Jump::IfNotSign);
    if con_signo {
        x86::mov_r32_imm32(code, R11, NEGATIVO);
    }
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, no_negativo);

    x86::mov_r32_imm32(code, RDI, 10);

    // -- El ultimo byte: un digito arriba, el SIGNO abajo --
    //
    // Se empieza por el final porque los digitos salen del `div` de menos a
    // mas peso, y en el byte de mas a la derecha es donde va el de menos peso.
    x86::zero_r32(code, RDX);
    x86::div_r64(code, RDI); // rax = resto del numero, rdx = digito
    x86::shl_r64_imm8(code, RDX, 4);
    x86::or_r64_r64(code, RDX, R11);
    x86::mov_byte_at_reg_disp_from_low(code, RCX, (bytes - 1) as u8, RDX);

    // -- El resto: dos digitos por byte --
    for i in (0..bytes - 1).rev() {
        // El de la derecha primero: es el de menos peso de los dos.
        x86::zero_r32(code, RDX);
        x86::div_r64(code, RDI);
        x86::mov_r64_r64(code, R11, RDX);
        // Y el de la izquierda, que se coloca en el nibble alto.
        x86::zero_r32(code, RDX);
        x86::div_r64(code, RDI);
        x86::shl_r64_imm8(code, RDX, 4);
        x86::or_r64_r64(code, RDX, R11);
        x86::mov_byte_at_reg_disp_from_low(code, RCX, i as u8, RDX);
    }
}

/// Emite codigo que LEE `bytes` bytes empaquetados de `[rcx]` a `rax`.
///
/// - Entrada: `rcx` = origen. Salida: `rax` = el entero escalado con signo.
/// - Ensucia `rax`, `rdx`, `rdi` y `r11`. `rcx` queda intacto.
///
/// ## Los cuatro nibbles negativos
///
/// Un compilador escribe `D` para negativo, pero los datos que hay por ahi
/// llevan tambien `B`, que es lo que produce el hardware de IBM en algunas
/// operaciones y lo que sale de una conversion desde EBCDIC con signo
/// sobrepunzado. Se aceptan los dos. Leer un `B` como positivo convertiria un
/// cargo en un abono sin que saltara nada, y ese es exactamente el fallo que no
/// se puede permitir: no rompe, descuadra.
pub fn desempaquetar(code: &mut Vec<u8>, bytes: usize) {
    assert!(bytes >= 1 && bytes <= 127, "campo empaquetado de {bytes} bytes fuera de rango");

    x86::zero_r32(code, RAX);
    x86::mov_r32_imm32(code, RDI, 10);

    // Los bytes completos: dos digitos cada uno, de mas peso a menos.
    for i in 0..bytes - 1 {
        x86::movzx_r32_byte_at_reg_disp(code, RDX, RCX, i as u8);
        x86::mov_r64_r64(code, R11, RDX);
        x86::shr_r64_imm8(code, R11, 4);
        x86::imul_r64_r64(code, RAX, RDI);
        x86::add_r64_r64(code, RAX, R11);
        x86::and_r64_imm32(code, RDX, 0x0F);
        x86::imul_r64_r64(code, RAX, RDI);
        x86::add_r64_r64(code, RAX, RDX);
    }

    // El ultimo: el nibble alto todavia es un digito, el bajo ya es el signo.
    x86::movzx_r32_byte_at_reg_disp(code, RDX, RCX, (bytes - 1) as u8);
    x86::mov_r64_r64(code, R11, RDX);
    x86::shr_r64_imm8(code, R11, 4);
    x86::imul_r64_r64(code, RAX, RDI);
    x86::add_r64_r64(code, RAX, R11);
    x86::and_r64_imm32(code, RDX, 0x0F);

    x86::cmp_r64_imm8(code, RDX, NEGATIVO as i8);
    let es_negativo = x86::emit_jump(code, Jump::IfEqual);
    x86::cmp_r64_imm8(code, RDX, 0x0B);
    let positivo = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump(code, es_negativo);
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, positivo);
}

/// La MISMA lectura, resuelta en el anfitrion.
///
/// Hace falta para las herramientas que miran un fichero **sin ejecutarlo**: un
/// visor de registros tiene que decodificar los mismos nibbles que el programa,
/// y si las dos reglas divergieran, el visor ensenaria un importe y el programa
/// leeria otro -- que es peor que no tener visor.
///
/// * Hay un test que compara esta con la emitida, byte a byte, sobre todos los
/// patrones. Es la misma pareja que `redondeo::dividir` / `dividir_en_rust`, y
/// por el mismo motivo: dos implementaciones que **tienen** que coincidir
/// prueban mas que una comparada contra una tabla escrita a mano.
pub fn desempaquetar_en_rust(bruto: &[u8]) -> i64 {
    let mut v: i64 = 0;
    let n = bruto.len();
    for (i, b) in bruto.iter().enumerate() {
        let alto = (b >> 4) as i64;
        let bajo = (b & 0x0F) as i64;
        v = v * 10 + alto;
        if i + 1 == n {
            // El nibble bajo del ultimo byte es el SIGNO, no un digito.
            return if bajo == 0x0D || bajo == 0x0B { -v } else { v };
        }
        v = v * 10 + bajo;
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    /// Deja `n` bytes de basura en memoria, empaqueta `valor` encima y
    /// devuelve los bytes escritos. La basura importa: un emisor que se dejara
    /// un byte sin tocar pasaria el test si el hueco empezara a cero.
    fn empaqueta(valor: i64, bytes: usize, con_signo: bool) -> Vec<u8> {
        let mut code = Vec::new();
        empaquetar(&mut code, bytes, con_signo);
        let mut m = Machine::new(code);
        let dir = m.load_data(&vec![0xA5u8; bytes]);
        m.regs[RAX as usize] = valor as u64;
        m.regs[RCX as usize] = dir;
        let m = run(m, 200_000);
        (0..bytes).map(|i| m.read_u8_pub(dir + i as u64)).collect()
    }

    fn desempaqueta(bruto: &[u8]) -> i64 {
        let mut code = Vec::new();
        desempaquetar(&mut code, bruto.len());
        let mut m = Machine::new(code);
        let dir = m.load_data(bruto);
        m.regs[RCX as usize] = dir;
        run(m, 200_000).regs[RAX as usize] as i64
    }

    /// Los bytes EXACTOS, no un ida y vuelta: si los dos emisores compartieran
    /// el mismo error de orden, un round-trip no lo veria y un fichero de un
    /// banco si.
    #[test]
    fn los_nibbles_caen_donde_manda_el_estandar() {
        // (valor, bytes, con signo, bruto esperado)
        let casos: &[(i64, usize, bool, &[u8])] = &[
            (1234, 3, true, &[0x01, 0x23, 0x4C]),
            (-1234, 3, true, &[0x01, 0x23, 0x4D]),
            (1234, 3, false, &[0x01, 0x23, 0x4F]),
            (0, 3, true, &[0x00, 0x00, 0x0C]),
            (-0, 3, true, &[0x00, 0x00, 0x0C]), // el cero no es negativo
            (5, 1, true, &[0x5C]),              // un solo digito
            (12345, 3, true, &[0x12, 0x34, 0x5C]), // llena los cinco huecos
            // 12 345 678,90 en centavos: el importe de un asiento de verdad.
            // Seis bytes son ONCE huecos de digito, asi que los diez digitos
            // van corridos a la derecha y sobra un cero por delante.
            (1234567890, 6, true, &[0x01, 0x23, 0x45, 0x67, 0x89, 0x0C]),
        ];
        for &(valor, bytes, signo, esperado) in casos {
            assert_eq!(
                empaqueta(valor, bytes, signo),
                esperado,
                "valor {valor} en {bytes} bytes (con_signo={signo})"
            );
        }
    }

    /// Un campo SIN `S` guarda el valor absoluto. Lo contrario --guardar el
    /// signo igualmente-- haria que el mismo byte se leyera distinto segun
    /// quien lo mirase.
    #[test]
    fn sin_signo_guarda_el_valor_absoluto() {
        assert_eq!(empaqueta(-42, 2, false), vec![0x04, 0x2F]);
        assert_eq!(desempaqueta(&[0x04, 0x2F]), 42);
    }

    #[test]
    fn ida_y_vuelta_con_signo() {
        for valor in [0i64, 1, -1, 7, -7, 99, -99, 12_345, -12_345, 999_999_999, -999_999_999] {
            let bruto = empaqueta(valor, 6, true);
            assert_eq!(desempaqueta(&bruto), valor, "valor {valor} bruto {bruto:02X?}");
        }
    }

    /// `B` es negativo aunque ningun emisor de aqui lo escriba: viene en los
    /// datos de fuera, y leerlo como positivo descuadra sin romper.
    #[test]
    fn el_nibble_b_tambien_es_negativo() {
        assert_eq!(desempaqueta(&[0x01, 0x2B]), -12);
        assert_eq!(desempaqueta(&[0x01, 0x2D]), -12);
        assert_eq!(desempaqueta(&[0x01, 0x2C]), 12);
        assert_eq!(desempaqueta(&[0x01, 0x2F]), 12);
    }

    /// Lo que no cabe se pierde por arriba, que es lo que manda el estandar al
    /// mover a una PIC mas corta. Se comprueba para que el dia que cambie se
    /// vea aqui y no en un cuadre.
    #[test]
    fn lo_que_no_cabe_se_trunca_por_arriba() {
        // 3 bytes = 5 digitos. 1234567 deja 34567.
        assert_eq!(empaqueta(1_234_567, 3, true), vec![0x34, 0x56, 0x7C]);
    }

    /// Ni un byte de mas ni de menos: el vecino de la derecha se queda como
    /// estaba. Un `off by one` aqui pisa el campo de al lado, y eso en un
    /// registro bancario es el descuadre que aparece semanas despues.
    #[test]
    fn no_se_sale_del_campo() {
        let mut code = Vec::new();
        empaquetar(&mut code, 3, true);
        let mut m = Machine::new(code);
        let dir = m.load_data(&[0xA5, 0xA5, 0xA5, 0x5A, 0x5A]);
        m.regs[RAX as usize] = 999;
        m.regs[RCX as usize] = dir;
        let m = run(m, 200_000);
        assert_eq!(m.read_u8_pub(dir + 3), 0x5A, "escribio un byte de mas");
        assert_eq!(m.read_u8_pub(dir + 4), 0x5A);
    }

    /// * La lectura emitida y la del anfitrion tienen que decir lo mismo.
    ///
    /// Se barren **todos** los patrones de 2 y 3 bytes, incluidos los nibbles
    /// que ningun emisor de aqui escribe (`A`, `E`, `B`): los datos de fuera los
    /// traen, y ahi es donde un visor y un programa se separarian.
    #[test]
    fn la_lectura_emitida_y_la_del_anfitrion_dicen_lo_mismo() {
        for a in 0u8..=255 {
            for b in 0u8..=255 {
                let bruto = [a, b];
                assert_eq!(
                    desempaqueta(&bruto),
                    desempaquetar_en_rust(&bruto),
                    "bruto {bruto:02X?}"
                );
            }
        }
        // Y una muestra de tres bytes con los signos raros incluidos.
        for signo in [0x0Au8, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F] {
            for alto in [0x00u8, 0x12, 0x99] {
                let bruto = [alto, 0x34, 0x50 | signo];
                assert_eq!(
                    desempaqueta(&bruto),
                    desempaquetar_en_rust(&bruto),
                    "bruto {bruto:02X?}"
                );
            }
        }
    }

    #[test]
    fn el_tamano_es_el_de_zos() {
        assert_eq!(bytes_para(1), 1);
        assert_eq!(bytes_para(2), 2);
        assert_eq!(bytes_para(5), 3);
        assert_eq!(bytes_para(7), 4); // S9(5)V99 -> 4 bytes, como en el mainframe
        assert_eq!(bytes_para(9), 5);
        assert_eq!(bytes_para(18), 10);
    }
}

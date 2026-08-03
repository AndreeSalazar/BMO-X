//! Decimal ZONADO (`DISPLAY`) — **librería, no puerta**.
//!
//! # Qué es
//!
//! **Un byte por dígito**, en ASCII, y el signo *sobrepunzado* en el último.
//! Es la otra mitad de [`crate::packed`]: las dos son cómo un número vive en un
//! fichero, y entre las dos cubren lo que un registro de banca trae dentro.
//!
//! ```text
//!   -1234 en PIC S9(5)  →  5 bytes
//!
//!   ┌────┬────┬────┬────┬────┐
//!   │'0' │'1' │'2' │'3' │ 't'│   ← 't' = 0x74 = el dígito 4 con el signo encima
//!   └────┴────┴────┴────┴────┘
//!     ▲                    ▲
//!     relleno         el SIGNO va DENTRO del dígito, no aparte
//! ```
//!
//! # ★ El sobrepunzado, y por qué no es una rareza
//!
//! Un `PIC S9(5)` mide **cinco** bytes, no seis: la `S` no ocupa posición. El
//! signo tiene que caber *dentro* de un dígito, y la forma de hacerlo es
//! cambiarle la zona alta al último byte. En EBCDIC eso es cambiar `F` por `D`;
//! en ASCII, la convención que usa todo el mundo (y GnuCOBOL) es la banda
//! `p`–`y` (`0x70`–`0x79`).
//!
//! Es feo y es de 1959. Pero es **lo que hay en los ficheros**, y un lector que
//! no lo entienda no ve un signo: ve una letra en medio de un importe.
//!
//! ⚠ Aquí se hace **ASCII**. Los ficheros que vienen de un mainframe son
//! EBCDIC, y ésa es otra tarea (`PLAN_BANCA.md` §1.6) — una tabla de 256
//! entradas, que en esta casa significa una tabla y no un cerebro.
//!
//! # Por qué vive aquí y no en el frontend de COBOL
//!
//! Por la regla de la cabecera de [`crate::fmt`]: se comparten **contratos y
//! librerías, nunca cerebros**. Escribir un entero como una tira de dígitos con
//! el signo encima del último es una REPRESENTACIÓN — el `Decimal` del Annex F
//! de Ada la pide igual. Lo que se queda en COBOL es *quién* es zonado y
//! *cuántos* dígitos tiene, que lo dice la PICTURE.

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, R11};

/// La banda ASCII del signo negativo: `p`(0x70) … `y`(0x79).
const NEGATIVO_BASE: u32 = 0x70;

/// Cuántos bytes ocupa un campo zonado de `digitos` dígitos. Uno cada uno — la
/// `S` no ocupa, porque va sobrepunzada.
pub const fn bytes_para(digitos: u32) -> usize {
    if digitos == 0 {
        1
    } else {
        digitos as usize
    }
}

/// Emite código que ESCRIBE `rax` como zonado en `[rcx]`.
///
/// - Entrada: `rax` = el entero escalado con signo, `rcx` = destino.
/// - Escribe exactamente `digitos` bytes.
/// - Ensucia `rax`, `rdx`, `rdi`, `rsi` y `r11`. `rcx` queda intacto.
///
/// Igual que en el empaquetado, un campo **sin `S` guarda el valor absoluto**:
/// es lo que dice el estándar, y es lo que impide que el mismo byte se lea
/// distinto según quién lo mire.
///
/// Y **trunca** por arriba lo que no cabe, como manda COBOL al mover a una PIC
/// más corta.
pub fn escribir(code: &mut Vec<u8>, digitos: u32, con_signo: bool) {
    let n = bytes_para(digitos);
    assert!(n <= 127, "campo zonado de {n} bytes fuera de rango");

    // r11 = 1 si era negativo. Se mira ANTES de quitarle el signo, porque
    // después ya no hay forma de saberlo.
    x86::zero_r32(code, R11);
    x86::test_r64_r64(code, RAX, RAX);
    let no_negativo = x86::emit_jump(code, Jump::IfNotSign);
    if con_signo {
        x86::mov_r32_imm32(code, R11, 1);
    }
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, no_negativo);

    x86::mov_r32_imm32(code, RDI, 10);

    // Los dígitos salen del `div` de menos a más peso, así que se escriben de
    // derecha a izquierda — que es justo el orden en el que están en el campo.
    for i in (0..n).rev() {
        x86::zero_r32(code, RDX);
        x86::div_r64(code, RDI); // rax = resto del numero, rdx = digito
        if i == n - 1 {
            // ★ El último byte lleva el signo encima. `'0'+d` si es positivo,
            // `0x70+d` si es negativo — la misma banda, otra zona alta.
            x86::mov_r64_r64(code, RSI, RDX);
            x86::test_r64_r64(code, R11, R11);
            let positivo = x86::emit_jump(code, Jump::IfZero);
            x86::add_r64_imm8(code, RSI, NEGATIVO_BASE as i8);
            let hecho = x86::emit_jump(code, Jump::Always);
            x86::patch_jump(code, positivo);
            x86::add_r64_imm8(code, RSI, b'0' as i8);
            x86::patch_jump(code, hecho);
            x86::mov_byte_at_reg_disp_from_low(code, RCX, i as u8, RSI);
        } else {
            x86::add_r64_imm8(code, RDX, b'0' as i8);
            x86::mov_byte_at_reg_disp_from_low(code, RCX, i as u8, RDX);
        }
    }
}

/// Emite código que LEE `digitos` bytes zonados de `[rcx]` a `rax`.
///
/// - Entrada: `rcx` = origen. Salida: `rax` = el entero escalado con signo.
/// - Ensucia `rax`, `rdx`, `rdi` y `r11`. `rcx` queda intacto.
///
/// ## Al leer se es GENEROSO con el último byte
///
/// Un compilador escribe `'0'`–`'9'` o `p`–`y`, pero los datos de fuera traen
/// también la banda `0x40`–`0x49` (el `+` explícito de algunas conversiones).
/// Se toma el **nibble bajo** como dígito siempre, y sólo la banda `0x70` marca
/// negativo. Leer un `p` como positivo convertiría un cargo en un abono sin que
/// saltara nada: no rompe, descuadra.
pub fn leer(code: &mut Vec<u8>, digitos: u32) {
    let n = bytes_para(digitos);
    assert!(n <= 127, "campo zonado de {n} bytes fuera de rango");

    x86::zero_r32(code, RAX);
    x86::mov_r32_imm32(code, RDI, 10);

    for i in 0..n {
        x86::movzx_r32_byte_at_reg_disp(code, RDX, RCX, i as u8);
        if i == n - 1 {
            // El último: guardar la zona alta antes de quedarse con el dígito.
            x86::mov_r64_r64(code, R11, RDX);
            x86::and_r64_imm32(code, R11, 0xF0);
        }
        x86::and_r64_imm32(code, RDX, 0x0F);
        x86::imul_r64_r64(code, RAX, RDI);
        x86::add_r64_r64(code, RAX, RDX);
    }

    // Y el signo, al final: sólo la banda 0x70 es negativa.
    x86::cmp_r64_imm8(code, R11, NEGATIVO_BASE as i8);
    let positivo = x86::emit_jump(code, Jump::IfNotZero);
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, positivo);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    fn escribe(valor: i64, digitos: u32, con_signo: bool) -> Vec<u8> {
        let n = bytes_para(digitos);
        let mut code = Vec::new();
        escribir(&mut code, digitos, con_signo);
        let mut m = Machine::new(code);
        let dir = m.load_data(&vec![0xA5u8; n]);
        m.regs[RAX as usize] = valor as u64;
        m.regs[RCX as usize] = dir;
        let m = run(m, 200_000);
        (0..n).map(|i| m.read_u8_pub(dir + i as u64)).collect()
    }

    fn lee(bruto: &[u8]) -> i64 {
        let mut code = Vec::new();
        leer(&mut code, bruto.len() as u32);
        let mut m = Machine::new(code);
        let dir = m.load_data(bruto);
        m.regs[RCX as usize] = dir;
        run(m, 200_000).regs[RAX as usize] as i64
    }

    /// ★ Los bytes EXACTOS. Un ida y vuelta no valdría: si los dos emisores
    /// compartieran el mismo error de sobrepunzado, cuadrarían entre ellos y no
    /// con el fichero de un banco.
    #[test]
    fn un_byte_por_digito_y_el_signo_encima_del_ultimo() {
        // Positivo: dígitos ASCII normales, y el último también.
        assert_eq!(escribe(1234, 5, true), b"01234".to_vec());
        // Negativo: los cuatro primeros IGUALES y sólo cambia el último —
        // `'4'`(0x34) pasa a `'t'`(0x74). El signo va DENTRO del dígito, que es
        // lo que hace que un `PIC S9(5)` mida cinco bytes y no seis.
        assert_eq!(escribe(-1234, 5, true), b"0123\x74".to_vec());
        assert_eq!(escribe(0, 3, true), b"000".to_vec());
        // El cero no es negativo, ni siquiera viniendo de un `-0`.
        assert_eq!(escribe(-0, 3, true), b"000".to_vec());
    }

    /// Sin `S` se guarda el valor absoluto y el último byte es un dígito
    /// normal. Lo contrario haría que el mismo byte se leyera distinto según
    /// quién lo mirase.
    #[test]
    fn sin_signo_guarda_el_valor_absoluto() {
        assert_eq!(escribe(-42, 4, false), b"0042".to_vec());
        assert_eq!(lee(b"0042"), 42);
    }

    #[test]
    fn ida_y_vuelta_con_signo() {
        for valor in [0i64, 1, -1, 7, -7, 99, -99, 12_345, -12_345, 99_999, -99_999] {
            let bruto = escribe(valor, 5, true);
            assert_eq!(lee(&bruto), valor, "valor {valor} bruto {bruto:02X?}");
        }
    }

    /// Al leer, la banda `0x40` (el `+` explícito de algunas conversiones) es
    /// POSITIVA y sólo la `0x70` es negativa.
    #[test]
    fn al_leer_solo_la_banda_de_las_letras_es_negativa() {
        assert_eq!(lee(b"0123\x34"), 1234); // '4' normal
        assert_eq!(lee(b"0123\x74"), -1234); // 't' → negativo
        assert_eq!(lee(b"0123\x44"), 1234); // 0x44 → el `+` explicito, positivo
    }

    /// Lo que no cabe se pierde por arriba, igual que en el empaquetado y que
    /// en el estándar.
    #[test]
    fn lo_que_no_cabe_se_trunca_por_arriba() {
        assert_eq!(escribe(1_234_567, 3, false), b"567".to_vec());
    }

    /// Ni un byte de más: el vecino de la derecha se queda como estaba. Un
    /// `off by one` aquí pisa el campo de al lado dentro del registro.
    #[test]
    fn no_se_sale_del_campo() {
        let mut code = Vec::new();
        escribir(&mut code, 3, true);
        let mut m = Machine::new(code);
        let dir = m.load_data(&[0xA5, 0xA5, 0xA5, 0x5A, 0x5A]);
        m.regs[RAX as usize] = 999;
        m.regs[RCX as usize] = dir;
        let m = run(m, 200_000);
        assert_eq!(m.read_u8_pub(dir + 3), 0x5A, "escribio un byte de mas");
        assert_eq!(m.read_u8_pub(dir + 4), 0x5A);
    }

    /// El ancho es el de la PICTURE: la `S` **no ocupa**, y ésa es la diferencia
    /// con el empaquetado, donde el signo sí se lleva medio byte.
    #[test]
    fn la_s_no_ocupa_posicion() {
        assert_eq!(bytes_para(5), 5);
        assert_eq!(bytes_para(9), 9);
        assert_eq!(bytes_para(18), 18);
    }
}

//! Formateo de valores a texto — **librería, no puerta**.
//!
//! # Dónde encaja
//!
//! La regla de L1 dice que la puerta (`console`) solo contiene lo expresable
//! en la superficie congelada por valor. Convertir un número a dígitos no es
//! eso: es un cálculo. Entonces, ¿por qué vive aquí y no en cada frontend?
//!
//! Porque el `forge` distingue dos cosas (ver `forge/README.md`): se
//! comparten **contratos y librerías**, nunca **cerebros**. Un conversor de
//! entero a decimal no tiene semántica de ningún lenguaje —el `%d` de C y el
//! `DISPLAY` de un numérico COBOL necesitan exactamente los mismos dígitos—,
//! así que duplicarlo sería copiar un bug en dos sitios en vez de tenerlo
//! bien en uno.
//!
//! Lo que **sí** se queda en cada frontend es la decisión de CUÁNDO y CON QUÉ
//! FORMA llamarlo: interpretar `%d` frente a `%x`, aplicar una PIC con
//! `ZZ9,99`, decidir el relleno. Eso es la esencia del lenguaje.
//!
//! Nada de esto es obligatorio: un frontend que quiera su propio formateo no
//! enlaza este módulo.
//!
//! # Cómo funciona
//!
//! Todo se construye en un buffer en la pila y se entrega a
//! [`crate::console::write_buffer`], la puerta de verdad. Ninguna función de
//! aquí habla con el kernel por su cuenta.

use crate::console;
use crate::x86::{self, Jump, RAX, RCX, RDX, RSP, R10, R8, R9};

/// Tamaño del buffer en pila. 20 dígitos (el máximo de un u64) + signo,
/// redondeado a 32 para mantener la pila alineada.
const BUFFER: i8 = 32;

/// Emite código que imprime `rax` como **entero decimal con signo**.
///
/// Registros que ensucia: los mismos que la puerta, más `r10`. La pila queda
/// como estaba.
pub fn write_i64(code: &mut Vec<u8>) {
    x86::sub_r64_imm8(code, RSP, BUFFER);

    // r8 apunta UNO MÁS ALLÁ del final: los dígitos salen al revés, así que
    // se escriben hacia atrás y al final r8 ya apunta al primero.
    x86::lea_r64_rsp_disp8(code, R8, BUFFER);
    x86::zero_r32(code, R9); // longitud
    x86::zero_r32(code, R10); // ¿era negativo?

    x86::test_r64_r64(code, RAX, RAX);
    let non_negative = x86::emit_jump(code, Jump::IfNotSign);
    x86::mov_r32_imm32(code, R10, 1);
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, non_negative);

    emit_digits(code, 10, false);

    // El signo va al final del bucle porque va DELANTE en el texto.
    x86::test_r64_r64(code, R10, R10);
    let no_sign = x86::emit_jump(code, Jump::IfZero);
    x86::dec_r64(code, R8);
    x86::mov_byte_at_reg_imm8(code, R8, b'-');
    x86::inc_r64(code, R9);
    x86::patch_jump(code, no_sign);

    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, BUFFER);
}

/// Emite código que imprime `rax` como entero **sin signo** en la base dada.
///
/// `radix` debe ser 10 o 16 — son las que produce un `printf`; cualquier
/// otra es un error del emisor, no del programa, y aborta la compilación.
pub fn write_u64_radix(code: &mut Vec<u8>, radix: u8) {
    assert!(radix == 10 || radix == 16, "base {radix} no soportada");

    x86::sub_r64_imm8(code, RSP, BUFFER);
    x86::lea_r64_rsp_disp8(code, R8, BUFFER);
    x86::zero_r32(code, R9);

    emit_digits(code, radix, true);

    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, BUFFER);
}

/// El bucle de dígitos: divide `rax` entre la base y guarda el resto como
/// carácter, hacia atrás desde `r8`, hasta agotar el valor.
///
/// Un `do…while`, no un `while`: el cero tiene que imprimir "0", y un bucle
/// que comprueba antes no imprimiría nada.
fn emit_digits(code: &mut Vec<u8>, radix: u8, unsigned: bool) {
    x86::mov_r32_imm32(code, RCX, radix as u32);

    let digit_loop = code.len();

    if unsigned {
        x86::zero_r32(code, RDX);
        x86::div_r64(code, RCX);
    } else {
        x86::cqo(code);
        x86::idiv_r64(code, RCX);
    }
    // rax = cociente, rdx = resto (el dígito).

    if radix == 16 {
        x86::cmp_r64_imm8(code, RDX, 10);
        let is_decimal_digit = x86::emit_jump(code, Jump::IfLess);
        x86::add_r64_imm8(code, RDX, (b'a' - 10) as i8);
        let stored = x86::emit_jump(code, Jump::Always);
        x86::patch_jump(code, is_decimal_digit);
        x86::add_r64_imm8(code, RDX, b'0' as i8);
        x86::patch_jump(code, stored);
    } else {
        x86::add_r64_imm8(code, RDX, b'0' as i8);
    }

    x86::dec_r64(code, R8);
    x86::mov_byte_at_reg_from_low(code, R8, RDX);
    x86::inc_r64(code, R9);

    x86::test_r64_r64(code, RAX, RAX);
    let again = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, again, digit_loop);
}

/// Emite código que imprime el byte bajo de `rax` como un carácter.
pub fn write_char(code: &mut Vec<u8>) {
    x86::sub_r64_imm8(code, RSP, 8);
    x86::mov_byte_at_reg_from_low(code, RSP, RAX);
    x86::mov_r64_r64(code, R8, RSP);
    x86::mov_r32_imm32(code, R9, 1);
    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, 8);
}

/// Emite código que imprime la cadena terminada en NUL apuntada por `rax`.
///
/// Mide primero y escribe después: la puerta trabaja por longitud, no por
/// terminador, así que el NUL nunca llega a cruzar (y no podría: corta la
/// palabra en el kernel).
pub fn write_cstr(code: &mut Vec<u8>) {
    x86::mov_r64_r64(code, R8, RAX);
    x86::zero_r32(code, R9);

    let scan = code.len();
    x86::cmp_byte_base_index_imm8(code, R8, R9, 0);
    let end = x86::emit_jump(code, Jump::IfZero);
    x86::inc_r64(code, R9);
    let again = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, again, scan);
    x86::patch_jump(code, end);

    console::write_buffer(code);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    fn run_with_rax(build: fn(&mut Vec<u8>), value: u64) -> String {
        let mut code = Vec::new();
        build(&mut code);
        let mut m = Machine::new(code);
        m.regs[RAX as usize] = value;
        run(m, 200_000).console
    }

    #[test]
    fn signed_decimal_covers_sign_zero_and_limits() {
        for value in [0i64, 7, 42, -1, -42, 1_000_000, i64::MAX, i64::MIN + 1] {
            let out = run_with_rax(write_i64, value as u64);
            assert_eq!(out, value.to_string(), "valor {value}");
        }
    }

    #[test]
    fn unsigned_decimal_does_not_invent_a_sign() {
        // El mismo patrón de bits que -1: con signo sería "-1", sin signo es
        // el máximo. Es justo la diferencia entre %d y %u.
        let out = run_with_rax(|c| write_u64_radix(c, 10), u64::MAX);
        assert_eq!(out, u64::MAX.to_string());
    }

    #[test]
    fn hex_matches_the_usual_lowercase_form() {
        for value in [0u64, 9, 10, 15, 255, 0xDEAD_BEEF, u64::MAX] {
            let out = run_with_rax(|c| write_u64_radix(c, 16), value);
            assert_eq!(out, format!("{value:x}"), "valor {value:#x}");
        }
    }

    #[test]
    fn char_prints_one_byte() {
        assert_eq!(run_with_rax(write_char, b'A' as u64), "A");
    }

    #[test]
    fn cstr_stops_at_the_terminator() {
        let mut code = Vec::new();
        write_cstr(&mut code);
        let mut m = Machine::new(code);
        let addr = m.load_data(b"hola\0basura que no debe salir");
        m.regs[RAX as usize] = addr;
        assert_eq!(run(m, 200_000).console, "hola");
    }

    /// La pila debe quedar donde estaba: si no, el `ret` de la función que
    /// llamó a esto saltaría a cualquier parte.
    #[test]
    fn stack_is_balanced_afterwards() {
        for build in [write_i64 as fn(&mut Vec<u8>), write_char] {
            let mut code = Vec::new();
            build(&mut code);
            let mut m = Machine::new(code);
            m.regs[RAX as usize] = 12345;
            let before = m.regs[RSP as usize];
            let after = run(m, 200_000);
            assert_eq!(after.regs[RSP as usize], before, "la pila quedo desbalanceada");
        }
    }
}

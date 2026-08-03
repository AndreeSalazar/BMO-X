//! Operaciones sobre bloques de TEXTO — **librería, no puerta**.
//!
//! # Qué hay aquí
//!
//! Contar, sustituir y partir sobre un bloque de bytes **de largo conocido**.
//! Nada de esto sabe qué es un `PIC X`, ni un `INSPECT`, ni un `UNSTRING`: son
//! recorridos de bytes, y por eso viven aquí y no en el frontend.
//!
//! Es la misma frontera que [`crate::memoria`], que ya trae `copiar`,
//! `rellenar` y `comparar`. Aquello son los verbos de C (`memcpy`, `memset`);
//! esto son los que COBOL escribe `INSPECT` y Ada `Index`/`Replace_Slice`.
//! **La misma emisión.**
//!
//! # Por qué el largo va por REGISTRO y no hay NUL
//!
//! `memoria::comparar` recorre hasta el cero, que es lo que hace una cadena de
//! C. Un campo de COBOL **no termina en cero**: mide lo que dice su PICTURE y
//! el hueco va a espacios. Un cero en medio es un dato, no un final.
//!
//! Mezclar las dos convenciones es de donde salen los bugs de cadenas, así que
//! aquí el largo es explícito y no se busca ningún terminador.

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI};

/// Cuenta cuántas veces aparece un byte.
///
/// - Entrada: `rdi` = principio, `rcx` = cuántos bytes, `rdx` = el byte.
/// - Salida: `rax` = las veces que estaba.
/// - Ensucia `rax`, `rcx`, `rdi` y `rsi`.
///
/// Es lo que COBOL escribe `INSPECT … TALLYING n FOR ALL "x"`, y su uso más
/// corriente en banca es contar espacios para saber cuánto mide de verdad un
/// campo que viene rellenado.
pub fn contar_byte(code: &mut Vec<u8>) {
    x86::zero_r32(code, RAX);
    x86::test_r64_r64(code, RCX, RCX);
    let vacio = x86::emit_jump(code, Jump::IfZero);

    let bucle = code.len();
    x86::movzx_r32_byte_at_reg(code, RSI, RDI);
    x86::cmp_r64_r64(code, RSI, RDX);
    let no_es = x86::emit_jump(code, Jump::IfNotZero);
    x86::inc_r64(code, RAX);
    x86::patch_jump(code, no_es);
    x86::inc_r64(code, RDI);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let sigue = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, sigue, bucle);

    x86::patch_jump(code, vacio);
}

/// Cambia un byte por otro, **en todas** sus apariciones.
///
/// - Entrada: `rdi` = principio, `rcx` = cuántos, `rdx` = el viejo,
///   `rsi` = el nuevo.
/// - Ensucia `rax`, `rcx` y `rdi`.
///
/// `INSPECT … REPLACING ALL " " BY "0"` — así se rellena de ceros un importe
/// que viene con espacios, que es lo que trae medio fichero de intercambio.
pub fn reemplazar_byte(code: &mut Vec<u8>) {
    x86::test_r64_r64(code, RCX, RCX);
    let vacio = x86::emit_jump(code, Jump::IfZero);

    let bucle = code.len();
    x86::movzx_r32_byte_at_reg(code, RAX, RDI);
    x86::cmp_r64_r64(code, RAX, RDX);
    let no_es = x86::emit_jump(code, Jump::IfNotZero);
    x86::mov_byte_at_reg_from_low(code, RDI, RSI);
    x86::patch_jump(code, no_es);
    x86::inc_r64(code, RDI);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let sigue = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, sigue, bucle);

    x86::patch_jump(code, vacio);
}

/// Igual, pero **sólo por delante**: para en cuanto encuentra otro byte.
///
/// Mismos registros que [`reemplazar_byte`].
///
/// ★ Y no es un capricho tener las dos. `REPLACING LEADING " " BY "0"` sobre
/// `"  12 34"` da `"0012 34"`; con `ALL` daría `"0012034"`, que es otro número.
/// Elegir mal ahí cambia un importe sin que nada avise.
pub fn reemplazar_delante(code: &mut Vec<u8>) {
    x86::test_r64_r64(code, RCX, RCX);
    let vacio = x86::emit_jump(code, Jump::IfZero);

    let bucle = code.len();
    x86::movzx_r32_byte_at_reg(code, RAX, RDI);
    x86::cmp_r64_r64(code, RAX, RDX);
    // El primero que no coincide termina el trabajo.
    let se_acabo = x86::emit_jump(code, Jump::IfNotZero);
    x86::mov_byte_at_reg_from_low(code, RDI, RSI);
    x86::inc_r64(code, RDI);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let sigue = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, sigue, bucle);

    x86::patch_jump(code, se_acabo);
    x86::patch_jump(code, vacio);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    fn contar(texto: &[u8], byte: u8) -> u64 {
        let mut code = Vec::new();
        contar_byte(&mut code);
        let mut m = Machine::new(code);
        let dir = m.load_data(texto);
        m.regs[RDI as usize] = dir;
        m.regs[RCX as usize] = texto.len() as u64;
        m.regs[RDX as usize] = byte as u64;
        run(m, 200_000).regs[RAX as usize]
    }

    fn reemplazar(texto: &[u8], viejo: u8, nuevo: u8, solo_delante: bool) -> Vec<u8> {
        let mut code = Vec::new();
        if solo_delante {
            reemplazar_delante(&mut code);
        } else {
            reemplazar_byte(&mut code);
        }
        let mut m = Machine::new(code);
        let dir = m.load_data(texto);
        m.regs[RDI as usize] = dir;
        m.regs[RCX as usize] = texto.len() as u64;
        m.regs[RDX as usize] = viejo as u64;
        m.regs[RSI as usize] = nuevo as u64;
        let m = run(m, 200_000);
        (0..texto.len()).map(|i| m.read_u8_pub(dir + i as u64)).collect()
    }

    #[test]
    fn contar_cuenta_todas_las_veces() {
        assert_eq!(contar(b"  12 34  ", b' '), 5);
        assert_eq!(contar(b"AAAA", b'A'), 4);
        assert_eq!(contar(b"AAAA", b'B'), 0);
        assert_eq!(contar(b"", b'A'), 0);
    }

    #[test]
    fn reemplazar_cambia_todas() {
        assert_eq!(reemplazar(b"  12 34", b' ', b'0', false), b"0012034".to_vec());
        assert_eq!(reemplazar(b"AAA", b'A', b'B', false), b"BBB".to_vec());
    }

    /// ★ La diferencia entre `ALL` y `LEADING`, que sobre un importe es otro
    /// número. Ésta es la razón por la que existen los dos emisores.
    #[test]
    fn delante_para_en_el_primero_que_no_coincide() {
        assert_eq!(reemplazar(b"  12 34", b' ', b'0', true), b"0012 34".to_vec());
        // Y si el primero ya no coincide, no toca nada.
        assert_eq!(reemplazar(b"12  34", b' ', b'0', true), b"12  34".to_vec());
        // Un campo entero de espacios se llena del todo.
        assert_eq!(reemplazar(b"    ", b' ', b'0', true), b"0000".to_vec());
    }

    /// Ni un byte de más: el vecino de la derecha se queda como estaba. Dentro
    /// de un registro, pasarse un byte pisa el campo de al lado.
    #[test]
    fn no_se_sale_del_bloque() {
        let mut code = Vec::new();
        reemplazar_byte(&mut code);
        let mut m = Machine::new(code);
        let dir = m.load_data(b"AAA\x5A\x5A");
        m.regs[RDI as usize] = dir;
        m.regs[RCX as usize] = 3;
        m.regs[RDX as usize] = b'A' as u64;
        m.regs[RSI as usize] = b'B' as u64;
        let m = run(m, 200_000);
        assert_eq!(m.read_u8_pub(dir + 3), 0x5A, "escribio un byte de mas");
        assert_eq!(m.read_u8_pub(dir + 4), 0x5A);
    }
}

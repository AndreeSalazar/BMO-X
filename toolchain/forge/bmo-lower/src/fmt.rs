//! Formateo de valores a texto -- **libreria, no puerta**.
//!
//! # Donde encaja
//!
//! La regla de L1 dice que la puerta (`console`) solo contiene lo expresable
//! en la superficie congelada por valor. Convertir un numero a digitos no es
//! eso: es un calculo. Entonces, por que vive aqui y no en cada frontend?
//!
//! Porque el `forge` distingue dos cosas (ver `forge/README.md`): se
//! comparten **contratos y librerias**, nunca **cerebros**. Un conversor de
//! entero a decimal no tiene semantica de ningun lenguaje --el `%d` de C y el
//! `DISPLAY` de un numerico COBOL necesitan exactamente los mismos digitos--,
//! asi que duplicarlo seria copiar un bug en dos sitios en vez de tenerlo
//! bien en uno.
//!
//! Lo que **si** se queda en cada frontend es la decision de CUANDO y CON QUE
//! FORMA llamarlo: interpretar `%d` frente a `%x`, aplicar una PIC con
//! `ZZ9,99`, decidir el relleno. Eso es la esencia del lenguaje.
//!
//! Nada de esto es obligatorio: un frontend que quiera su propio formateo no
//! enlaza este modulo.
//!
//! # Como funciona
//!
//! Todo se construye en un buffer en la pila y se entrega a
//! [`crate::console::write_buffer`], la puerta de verdad. Ninguna funcion de
//! aqui habla con el kernel por su cuenta.

use crate::console;
use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, RSP, R10, R11, R8, R9};

/// Tamano del buffer en pila. 20 digitos (el maximo de un u64) + signo,
/// redondeado a 32 para mantener la pila alineada.
pub const BUFFER: i8 = 32;

/// Emite codigo que imprime `rax` como **entero decimal con signo**.
///
/// Registros que ensucia: los mismos que la puerta, mas `r10`. La pila queda
/// como estaba.
pub fn write_i64(code: &mut Vec<u8>) {
    formatear_i64(code);
    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, BUFFER);
}

/// Igual, pero deja el texto en un buffer de PILA en vez de imprimirlo.
///
/// - Salida: `r8` = puntero al primer caracter, `r9` = largo.
/// - **El llamante DEBE devolver la pila** con `add rsp, `[`BUFFER`].
///
/// Existe porque un numero no siempre va a la consola. `WRITE` de COBOL lo
/// manda al disco por otra puerta, y sin esto habria que reescribir el
/// formateador entero para cambiar solo su ultimo paso -- que es justo el
/// reparto que este modulo dice tener: formatear es una cosa y publicar es
/// otra.
pub fn formatear_i64(code: &mut Vec<u8>) {
    x86::sub_r64_imm8(code, RSP, BUFFER);

    // r8 apunta UNO MAS ALLA del final: los digitos salen al reves, asi que
    // se escriben hacia atras y al final r8 ya apunta al primero.
    x86::lea_r64_rsp_disp8(code, R8, BUFFER);
    x86::zero_r32(code, R9); // longitud
    x86::zero_r32(code, R10); // era negativo?

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
}

/// Emite codigo que imprime `rax` como **decimal con escala fija**: el entero
/// lleva `escala` digitos de parte fraccionaria.
///
/// `rax = 5997` con `escala = 2` imprime `59.97`. Es exactamente como COBOL
/// guarda el dinero --centavos en un entero, sin coma flotante-- y como hay que
/// devolverlo a la vista de una persona.
///
/// ## Por que vive aqui y no en el frontend de COBOL
///
/// Por la regla de la cabecera de este modulo: **librerias, no cerebros**. Un
/// entero escalado no tiene semantica de ningun lenguaje -- el `PIC 9(5)V99` de
/// COBOL y un punto fijo de C necesitan los mismos digitos y el mismo punto.
/// Lo que si es del lenguaje es *decidir* la escala y, despues, aplicarle una
/// mascara de edicion (`ZZ9.99`, `$$$,$$9.99`): eso se queda en COBOL.
///
/// ## Como
///
/// El buffer se escribe **hacia atras**, asi que el orden natural del emisor
/// es el inverso del texto: primero los decimales, luego el punto, luego la
/// parte entera y por ultimo el signo. Eso evita tener que reservar hueco y
/// volver a rellenarlo.
///
/// Los decimales salen con **cuenta fija**: `5` con escala 2 es `0.05`, no
/// `0.5`. Un cero de relleno que falta convierte cinco centimos en cincuenta,
/// y ese es el error que este modulo entero existe para no cometer.
pub fn write_decimal_scaled(code: &mut Vec<u8>, escala: u32) {
    formatear_decimal_scaled(code, escala);
    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, BUFFER);
}

/// Igual, pero deja el texto en un buffer de PILA en vez de imprimirlo.
///
/// - Salida: `r8` = puntero al primer caracter, `r9` = largo.
/// - **El llamante DEBE devolver la pila** con `add rsp, `[`BUFFER`].
///
/// Es lo que usa `WRITE` de COBOL: el mismo numero, otra puerta.
pub fn formatear_decimal_scaled(code: &mut Vec<u8>, escala: u32) {
    if escala == 0 {
        formatear_i64(code);
        return;
    }

    x86::sub_r64_imm8(code, RSP, BUFFER);
    x86::lea_r64_rsp_disp8(code, R8, BUFFER);
    x86::zero_r32(code, R9);
    x86::zero_r32(code, R10);

    // El signo se aparta ya: dividir con signo daria restos negativos y el
    // digito saldria del reves.
    x86::test_r64_r64(code, RAX, RAX);
    let no_negativo = x86::emit_jump(code, Jump::IfNotSign);
    x86::mov_r32_imm32(code, R10, 1);
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, no_negativo);

    // Partir en entero y fraccion: rax = valor / 10^escala, rdx = resto.
    let potencia = 10u64.pow(escala);
    x86::mov_r64_imm64(code, RCX, potencia);
    x86::zero_r32(code, RDX);
    x86::div_r64(code, RCX);
    // rax = parte entera, rdx = fraccion. La entera se aparta en `r11` --
    // caller-saved y sin uso fijo en la ABI -- y se trabaja con la fraccion,
    // porque el buffer se llena al reves. En un registro y no en la pila: el
    // `div` de abajo se come rax y rdx, pero r11 sobrevive, y asi esta funcion
    // no tiene un push que pueda quedarse sin su pop.
    x86::mov_r64_r64(code, R11, RAX);
    x86::mov_r64_r64(code, RAX, RDX);

    // Los `escala` digitos de la fraccion, CUENTA FIJA. Sin bucle de
    // "hasta que el cociente sea cero": eso se comeria los ceros de la
    // izquierda y 0.05 saldria como 0.5.
    x86::mov_r32_imm32(code, RCX, 10);
    for _ in 0..escala {
        x86::zero_r32(code, RDX);
        x86::div_r64(code, RCX);
        x86::add_r64_imm8(code, RDX, b'0' as i8);
        x86::dec_r64(code, R8);
        x86::mov_byte_at_reg_from_low(code, R8, RDX);
        x86::inc_r64(code, R9);
    }

    // El punto.
    x86::dec_r64(code, R8);
    x86::mov_byte_at_reg_imm8(code, R8, b'.');
    x86::inc_r64(code, R9);

    // La parte entera, con al menos un digito (el `0` de `0.05`).
    x86::mov_r64_r64(code, RAX, R11);
    emit_digits(code, 10, true);

    // Y el signo, que va delante del todo en el texto y por eso al final aqui.
    x86::test_r64_r64(code, R10, R10);
    let sin_signo = x86::emit_jump(code, Jump::IfZero);
    x86::dec_r64(code, R8);
    x86::mov_byte_at_reg_imm8(code, R8, b'-');
    x86::inc_r64(code, R9);
    x86::patch_jump(code, sin_signo);
}

/// Emite codigo que imprime `rax` como entero **sin signo** en la base dada.
///
/// `radix` debe ser 10 o 16 -- son las que produce un `printf`; cualquier
/// otra es un error del emisor, no del programa, y aborta la compilacion.
pub fn write_u64_radix(code: &mut Vec<u8>, radix: u8) {
    assert!(radix == 10 || radix == 16, "base {radix} no soportada");

    x86::sub_r64_imm8(code, RSP, BUFFER);
    x86::lea_r64_rsp_disp8(code, R8, BUFFER);
    x86::zero_r32(code, R9);

    emit_digits(code, radix, true);

    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, BUFFER);
}

/// El bucle de digitos: divide `rax` entre la base y guarda el resto como
/// caracter, hacia atras desde `r8`, hasta agotar el valor.
///
/// Un `do...while`, no un `while`: el cero tiene que imprimir "0", y un bucle
/// que comprueba antes no imprimiria nada.
/// Emite codigo que LEE un decimal con escala de un buffer (`r8` = puntero,
/// `r9` = longitud) y deja el entero escalado en `rax`.
///
/// `"19.99"` con escala 2 da `1999`. Es la pareja exacta de
/// [`write_decimal_scaled`] y vive aqui por lo mismo: leer digitos no tiene
/// semantica de ningun lenguaje.
///
/// ## Lo que hace y lo que NO
///
/// - Se salta lo que no sea digito, signo o punto. Un `ACCEPT` recibe lo que
///   una persona teclee, y una persona mete espacios.
/// - **Trunca** los decimales que sobren: `19.999` en escala 2 es `1999`. La
///   misma regla que al escribir, y la misma razon -- COBOL no redondea sin que
///   se lo pidan.
/// - **Rellena** los que falten: `19.9` en escala 2 es `1990`, no `199`. Este
///   es el que convierte 19,90 EUR en 1,99 EUR si se olvida.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rsi`, `rdi`, `r10`, `r11`.
pub fn parse_decimal_scaled(code: &mut Vec<u8>, escala: u32) {
    x86::zero_r32(code, RAX); // acumulador
    x86::zero_r32(code, RCX); // indice
    x86::zero_r32(code, R10); // negativo?
    x86::zero_r32(code, R11); // ya pasamos el punto?
    x86::mov_r32_imm32(code, RSI, escala); // decimales que faltan por leer
    x86::mov_r32_imm32(code, RDI, 10); // la base, para el imul

    let top = code.len();
    x86::cmp_r64_r64(code, RCX, R9);
    let fin = x86::emit_jump(code, Jump::IfAboveOrEqual);
    x86::movzx_r32_byte_base_index(code, RDX, R8, RCX);
    x86::inc_r64(code, RCX);

    // Signo.
    x86::cmp_r64_imm8(code, RDX, b'-' as i8);
    let es_menos = x86::emit_jump(code, Jump::IfEqual);
    // Punto decimal.
    x86::cmp_r64_imm8(code, RDX, b'.' as i8);
    let es_punto = x86::emit_jump(code, Jump::IfEqual);

    // Digito? `c - '0' > 9` sin signo lo resuelve con UNA comparacion: las
    // letras y los espacios se van por arriba al restar.
    x86::sub_r64_imm8(code, RDX, b'0' as i8);
    x86::cmp_r64_imm8(code, RDX, 9);
    let no_es_digito = x86::emit_jump(code, Jump::IfAbove);
    x86::patch_jump_to(code, no_es_digito, top);

    // Si ya estamos en la fraccion y no queda sitio, se TRUNCA.
    x86::test_r64_r64(code, R11, R11);
    let en_entero = x86::emit_jump(code, Jump::IfZero);
    x86::test_r64_r64(code, RSI, RSI);
    let sin_sitio = x86::emit_jump(code, Jump::IfZero);
    x86::patch_jump_to(code, sin_sitio, top);
    x86::dec_r64(code, RSI);
    x86::patch_jump(code, en_entero);

    // acumulador = acumulador * 10 + digito
    x86::imul_r64_r64(code, RAX, RDI);
    x86::add_r64_r64(code, RAX, RDX);
    let sigue = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, sigue, top);

    x86::patch_jump(code, es_menos);
    x86::mov_r32_imm32(code, R10, 1);
    let tras_menos = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, tras_menos, top);

    x86::patch_jump(code, es_punto);
    x86::mov_r32_imm32(code, R11, 1);
    let tras_punto = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, tras_punto, top);

    x86::patch_jump(code, fin);

    // Los decimales que NO llegaron: `19.9` en escala 2 tiene que valer 1990.
    let relleno = code.len();
    x86::test_r64_r64(code, RSI, RSI);
    let ya_esta = x86::emit_jump(code, Jump::IfZero);
    x86::imul_r64_r64(code, RAX, RDI);
    x86::dec_r64(code, RSI);
    let otra = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, otra, relleno);
    x86::patch_jump(code, ya_esta);

    // Y el signo, al final: negar antes habria estropeado el acumulado.
    x86::test_r64_r64(code, R10, R10);
    let positivo = x86::emit_jump(code, Jump::IfZero);
    x86::neg_r64(code, RAX);
    x86::patch_jump(code, positivo);
}

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
    // rax = cociente, rdx = resto (el digito).

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

/// Emite codigo que imprime el byte bajo de `rax` como un caracter.
pub fn write_char(code: &mut Vec<u8>) {
    x86::sub_r64_imm8(code, RSP, 8);
    x86::mov_byte_at_reg_from_low(code, RSP, RAX);
    x86::mov_r64_r64(code, R8, RSP);
    x86::mov_r32_imm32(code, R9, 1);
    console::write_buffer(code);
    x86::add_r64_imm8(code, RSP, 8);
}

/// Emite codigo que imprime la cadena terminada en NUL apuntada por `rax`.
///
/// Mide primero y escribe despues: la puerta trabaja por longitud, no por
/// terminador, asi que el NUL nunca llega a cruzar (y no podria: corta la
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

    /// El caso del dinero: centavos dentro, importe fuera.
    #[test]
    fn decimal_escalado_pone_el_punto_donde_toca() {
        let casos: &[(i64, u32, &str)] = &[
            (5997, 2, "59.97"),
            (5, 2, "0.05"),        // <- el cero de relleno: 5 centavos, no 50
            (50, 2, "0.50"),
            (0, 2, "0.00"),
            (100000, 2, "1000.00"),
            (-1999, 2, "-19.99"),
            (-5, 2, "-0.05"),
            (7, 1, "0.7"),
            (123456, 3, "123.456"),
            (42, 0, "42"),          // escala 0 = entero de toda la vida
        ];
        for &(valor, escala, esperado) in casos {
            let mut code = Vec::new();
            write_decimal_scaled(&mut code, escala);
            let mut m = Machine::new(code);
            m.regs[RAX as usize] = valor as u64;
            assert_eq!(run(m, 200_000).console, esperado, "valor {valor} escala {escala}");
        }
    }

    /// La pila tiene que quedar donde estaba tambien en el camino escalado:
    /// lleva un push/pop en medio y un desequilibrio ahi manda el `ret` a
    /// cualquier parte.
    #[test]
    fn decimal_escalado_deja_la_pila_donde_estaba() {
        let mut code = Vec::new();
        write_decimal_scaled(&mut code, 2);
        let mut m = Machine::new(code);
        m.regs[RAX as usize] = 5997;
        let antes = m.regs[RSP as usize];
        let despues = run(m, 200_000);
        assert_eq!(despues.regs[RSP as usize], antes);
    }

    #[test]
    fn unsigned_decimal_does_not_invent_a_sign() {
        // El mismo patron de bits que -1: con signo seria "-1", sin signo es
        // el maximo. Es justo la diferencia entre %d y %u.
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

    /// Leer es la vuelta de escribir, y tiene que cuadrar con ella.
    #[test]
    fn leer_decimal_escalado() {
        let casos: &[(&str, u32, i64)] = &[
            ("19.99", 2, 1999),
            ("0.05", 2, 5),
            ("59.97", 2, 5997),
            ("-19.99", 2, -1999),
            ("100", 2, 10000),      // sin punto: los decimales se rellenan
            ("19.9", 2, 1990),      // <- el que convierte 19,90 en 1,99 si falta
            ("19.999", 2, 1999),    // sobra un decimal: se TRUNCA, no redondea
            ("  42  ", 0, 42),      // espacios: los mete una persona
            ("7", 1, 70),
            ("0", 2, 0),
        ];
        for &(texto, escala, esperado) in casos {
            let mut code = Vec::new();
            parse_decimal_scaled(&mut code, escala);
            // Devolver rax por la consola para poder comprobarlo.
            write_i64(&mut code);
            let mut m = Machine::new(code);
            let addr = m.load_data(texto.as_bytes());
            m.regs[R8 as usize] = addr;
            m.regs[R9 as usize] = texto.len() as u64;
            assert_eq!(
                run(m, 200_000).console,
                esperado.to_string(),
                "texto {texto:?} escala {escala}"
            );
        }
    }

    /// Ida y vuelta: lo que se escribe se tiene que poder volver a leer.
    #[test]
    fn escribir_y_leer_cuadran() {
        for valor in [0i64, 5, 1999, 5997, 100000, -1999, -5] {
            let mut code = Vec::new();
            write_decimal_scaled(&mut code, 2);
            let mut m = Machine::new(code);
            m.regs[RAX as usize] = valor as u64;
            let texto = run(m, 200_000).console;

            let mut code2 = Vec::new();
            parse_decimal_scaled(&mut code2, 2);
            write_i64(&mut code2);
            let mut m2 = Machine::new(code2);
            let addr = m2.load_data(texto.as_bytes());
            m2.regs[R8 as usize] = addr;
            m2.regs[R9 as usize] = texto.len() as u64;
            assert_eq!(run(m2, 200_000).console, valor.to_string(), "ida y vuelta de {valor}");
        }
    }

    /// La pila debe quedar donde estaba: si no, el `ret` de la funcion que
    /// llamo a esto saltaria a cualquier parte.
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

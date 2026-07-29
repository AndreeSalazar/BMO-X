//! La puerta de ARCHIVOS — L1.
//!
//! Lo mismo que `console` pero sobre `KIND_ARCHIVO`: abrir, mover bytes,
//! cerrar. L1 no sabe qué es un registro, ni un `FD`, ni una PICTURE — sólo
//! mueve bytes por la puerta congelada. Quien decide que una línea es un
//! registro es la L2 (`lang/cobol`), y quien decide que ese registro son
//! centavos es su PICTURE.
//!
//! ## Todo por valor, también aquí
//!
//! La ruta viaja de 8 en 8 por `TASK_OP_RUTA` y los datos de 7 en 7 con su
//! cuenta en el byte alto. Siete y no ocho porque el octavo lleva el contador
//! — y aquí ese contador no es un lujo: un archivo **no es texto** y cortar en
//! el primer cero corrompería cualquier binario. La consola puede permitirse
//! el NUL-stop; esto no.
//!
//! ## Escribir es de dos pasos
//!
//! Nada llega al disco hasta [`cerrar`]. Un programa que muere a medias no
//! deja medio archivo: no deja ninguno. Para un fichero de movimientos eso es
//! lo correcto — un extracto truncado se parece demasiado a uno completo.

use bmo_abi::syscalls::surface::{
    ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, ARCH_OP_LEER_LINEA, CURRENT_TASK, NR_INVOKE,
    TASK_OP_ARCHIVO_ABRIR, TASK_OP_ARCHIVO_CREAR, TASK_OP_RUTA,
};

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, R10, R8, R9};

/// Emite la apertura de un archivo cuya ruta se conoce al COMPILAR.
///
/// Es el caso de COBOL entero: `SELECT … ASSIGN TO "datos/movim.txt"` fija la
/// ruta en el fuente. Viaja como inmediatos dentro del código, igual que
/// `console::write_const` — sin sección de datos y sin relocations.
///
/// Deja el handle en `rax`. Cero = no se pudo abrir; el llamante decide qué
/// hacer con eso (COBOL levanta su bandera de fin y sigue).
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r11`.
pub fn abrir_const(code: &mut Vec<u8>, ruta: &[u8], escribe: bool) {
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_RUTA as u32);
    for trozo in ruta.chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        // rdi/rsi se recargan en cada vuelta por la misma razón que en
        // `console::write_const`: mantenerlos vivos exigiría saber que nadie
        // salta al medio de esto.
        x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
        x86::mov_r32_imm32(code, RSI, TASK_OP_RUTA as u32);
        x86::mov_r32_imm32(code, RAX, NR_INVOKE);
        x86::mov_r64_imm64(code, RDX, u64::from_le_bytes(w));
        x86::syscall(code);
    }
    let op = if escribe { TASK_OP_ARCHIVO_CREAR } else { TASK_OP_ARCHIVO_ABRIR };
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, op as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);
    // El valor vuelve en rdx (`BmoStatus` = {code, flags, value}); rax trae el
    // código. Se pasa a rax porque un handle es lo único que el llamante
    // quiere de aquí, y `rdx` lo pisa cualquier cosa.
    //
    // Si la apertura falló, `code != 0` y `value` es 0: el cero hace de "no
    // hay handle" sin necesidad de mirar dos registros.
    x86::mov_r64_r64(code, RAX, RDX);
}

/// Emite la lectura de UNA LÍNEA del archivo cuyo handle está en `r10`.
///
/// - Entrada: `r10` = handle, `r8` = buffer del llamante.
/// - Salida: `r9` = largo de la línea (sin el salto), `rax` = 1 si hubo
///   registro y **0 si se acabó el archivo**.
///
/// El handle vive en `r10` y no en `rdi` porque `rdi` es el argumento de la
/// puerta y hay que recargarlo en cada vuelta. `r10` no lo toca el `syscall`
/// —a diferencia de `rcx` y `r11`, que el silicio destruye— así que sobrevive
/// al bucle sin salvarlo en la pila.
///
/// `tope` es un INMEDIATO por la misma lección que dejó `console::read_line`:
/// allí el límite viajaba en `r11` y el `syscall` lo pisaba con RFLAGS, así que
/// el guarda del buffer estaba muerto. Aquí el tope lo sabe el compilador.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r9`, `r11`.
/// `r8` avanza hasta el final de lo leído.
pub fn leer_linea(code: &mut Vec<u8>, tope: u8) {
    let tope = tope.min(127) as i8;
    x86::zero_r32(code, R9);
    // `r11` lleva el "hubo registro". Se pisa en cada `syscall` con RFLAGS
    // —por eso NO puede llevar un límite, como enseñó `console::read_line`—
    // pero aquí se vuelve a poner después de cada llamada, así que sirve.
    x86::zero_r32(code, x86::R11);

    let otra_vez = code.len();
    x86::mov_r64_r64(code, RDI, R10);
    x86::mov_r32_imm32(code, RSI, ARCH_OP_LEER_LINEA as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);
    // `rax` guarda la palabra entera: `rdx` se va desmontando byte a byte y
    // el bit de fin vive arriba del todo.
    x86::mov_r64_r64(code, RAX, RDX);

    // rcx = cuántos bytes trae el paquete (bits 56..62).
    x86::mov_r64_r64(code, RCX, RDX);
    x86::shr_r64_imm8(code, RCX, 56);
    x86::and_r64_imm32(code, RCX, 0x7F);

    // ¿Vino algo, o el kernel dice que la línea acabó? Cualquiera de las dos
    // hace de esto un registro. Una línea VACÍA es un registro: darla por fin
    // de archivo se comería el último renglón de un fichero con doble salto.
    x86::test_r64_r64(code, RCX, RCX);
    let hay_bytes = x86::emit_jump(code, Jump::IfNotZero);
    // Sin bytes: mirar el bit de fin. Si tampoco está, se acabó el archivo.
    x86::shr_r64_imm8(code, RAX, 63);
    x86::test_r64_r64(code, RAX, RAX);
    let fin_de_linea_vacia = x86::emit_jump(code, Jump::IfNotZero);
    // Fin de archivo. **No se cede el turno ni se reintenta**, al revés que la
    // consola: un archivo que se acabó no va a crecer porque esperemos.
    // Insistir aquí colgaría el programa para siempre.
    let fin_de_archivo = x86::emit_jump(code, Jump::Always);

    x86::patch_jump(code, fin_de_linea_vacia);
    x86::mov_r32_imm32(code, x86::R11, 1);
    let fin_linea_a = x86::emit_jump(code, Jump::Always);

    x86::patch_jump(code, hay_bytes);
    x86::mov_r32_imm32(code, x86::R11, 1);

    // Desempaquetar byte a byte; el primero está en el byte BAJO. Aquí NO hay
    // que buscar el salto: el kernel ya no lo entrega.
    let byte_loop = code.len();
    x86::mov_r64_r64(code, RSI, RDX);
    x86::and_r64_imm32(code, RSI, 0xFF);
    // El retorno de carro se tira: un archivo escrito desde el anfitrión trae
    // "\r\n" y ese `\r` acabaría dentro del número.
    x86::cmp_r64_imm8(code, RSI, b'\r' as i8);
    let es_cr = x86::emit_jump(code, Jump::IfEqual);
    // Guardar si cabe. Lo que no cabe se descarta: recortar en silencio es
    // peor que perder el resto de una línea que no debía ser tan larga.
    x86::cmp_r64_imm8(code, R9, tope);
    let lleno = x86::emit_jump(code, Jump::IfAboveOrEqual);
    x86::mov_byte_at_reg_from_low(code, R8, RSI);
    x86::inc_r64(code, R8);
    x86::inc_r64(code, R9);
    x86::patch_jump(code, lleno);
    x86::patch_jump(code, es_cr);
    x86::shr_r64_imm8(code, RDX, 8);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let mas_bytes = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, mas_bytes, byte_loop);

    // Paquete agotado. Si el kernel no marcó el fin, la línea sigue: otra
    // vuelta. `rax` conserva la palabra original con su bit de arriba.
    x86::shr_r64_imm8(code, RAX, 63);
    x86::test_r64_r64(code, RAX, RAX);
    let acabo = x86::emit_jump(code, Jump::IfNotZero);
    let sigue = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, sigue, otra_vez);

    // `rax` = 1 si esto fue un registro, 0 si el archivo se acabó.
    x86::patch_jump(code, acabo);
    x86::patch_jump(code, fin_linea_a);
    x86::mov_r64_r64(code, RAX, x86::R11);
    let listo = x86::emit_jump(code, Jump::Always);

    // El fin de archivo pone el CERO a mano y no pasa por `r11`: a ese camino
    // se llega justo despues de un `syscall`, y el silicio deja ahi RFLAGS.
    // Leerlo devolveria un numero enorme que se lee como "si hubo registro" —
    // y un `PERFORM UNTIL FIN` no terminaria jamas.
    x86::patch_jump(code, fin_de_archivo);
    x86::zero_r32(code, RAX);
    x86::patch_jump(code, listo);
}

/// Emite la escritura de un buffer en el archivo cuyo handle está en `r10`.
///
/// - Entrada: `r10` = handle, `r8` = puntero, `r9` = largo.
/// - `r8`/`r9` quedan consumidos.
///
/// Los bytes van de 7 en 7 con la cuenta en el byte alto — **no** cortando en
/// el primer cero. Ver la nota de cabecera.
pub fn escribir_buffer(code: &mut Vec<u8>) {
    let loop_top = code.len();
    x86::test_r64_r64(code, R9, R9);
    let done = x86::emit_jump(code, Jump::IfZero);

    // rcx = min(r9, 7) — el tamaño de este trozo.
    x86::mov_r64_r64(code, RCX, R9);
    x86::cmp_r64_imm8(code, RCX, 7);
    let tengo_n = x86::emit_jump(code, Jump::IfBelowOrEqual);
    x86::mov_r32_imm32(code, RCX, 7);
    x86::patch_jump(code, tengo_n);

    // Empaquetar de atrás hacia adelante: el primer byte del texto acaba en el
    // byte BAJO, que es el que el kernel desempaqueta primero.
    x86::zero_r32(code, RDX);
    x86::mov_r64_r64(code, RAX, RCX);
    let byte_loop = code.len();
    x86::dec_r64(code, RAX);
    x86::shl_r64_imm8(code, RDX, 8);
    x86::movzx_r32_byte_base_index(code, RSI, R8, RAX);
    x86::or_r64_r64(code, RDX, RSI);
    x86::test_r64_r64(code, RAX, RAX);
    let otra = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, otra, byte_loop);

    // La cuenta en el byte alto. Se mete ANTES de avanzar, que es cuando
    // `rcx` todavía vale — el `syscall` lo destruye.
    x86::mov_r64_r64(code, RAX, RCX);
    x86::shl_r64_imm8(code, RAX, 56);
    x86::or_r64_r64(code, RDX, RAX);

    x86::add_r64_r64(code, R8, RCX);
    x86::sub_r64_r64(code, R9, RCX);

    x86::mov_r64_r64(code, RDI, R10);
    x86::mov_r32_imm32(code, RSI, ARCH_OP_ESCRIBIR as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);

    let back = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, back, loop_top);
    x86::patch_jump(code, done);
}

/// Emite el cierre del archivo cuyo handle está en `r10`.
///
/// **En uno de escritura es donde el contenido llega al disco.** Deja en `rax`
/// el 1/0 que contesta el kernel: `0` significa que no se guardó NADA, no que
/// se guardara a medias.
pub fn cerrar(code: &mut Vec<u8>) {
    x86::mov_r64_r64(code, RDI, R10);
    x86::mov_r32_imm32(code, RSI, ARCH_OP_CERRAR as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);
    x86::mov_r64_r64(code, RAX, RDX);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    /// Abre un archivo sembrado, lee una línea y devuelve `(hubo, largo,
    /// bytes)`.
    fn leer_una(contenido: &str, tope: u8) -> (u64, u64, Vec<u8>) {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/mov.txt", false);
        // El handle a r10, que es donde lo quieren las demás puertas.
        x86::mov_r64_r64(&mut code, R10, RAX);
        leer_linea(&mut code, tope);

        let mut m = Machine::new(code);
        m.poner_archivo("datos/mov.txt", contenido.as_bytes());
        let base = m.load_data(&vec![0u8; 64]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);

        let n = m.regs[R9 as usize];
        // r8 acabó al final de lo leído: se retrocede para mirar el principio.
        let mut visto = Vec::new();
        for i in 0..n {
            visto.push(m.read_u8_pub(base + i));
        }
        (m.regs[RAX as usize], n, visto)
    }

    #[test]
    fn lee_la_primera_linea() {
        let (hubo, n, b) = leer_una("1050\n2075\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 4);
        assert_eq!(b, b"1050");
    }

    /// Un archivo que se acabó da `rax = 0`. Es lo que COBOL convierte en
    /// `AT END`, y es la única forma de que un `PERFORM UNTIL` termine.
    #[test]
    fn un_archivo_vacio_es_fin_de_archivo() {
        let (hubo, n, _) = leer_una("", 32);
        assert_eq!(hubo, 0, "sin bytes no hay registro");
        assert_eq!(n, 0);
    }

    /// Una línea VACÍA sí es un registro. Darla por fin de archivo se comería
    /// el último renglón de un fichero que acaba en doble salto.
    #[test]
    fn una_linea_vacia_sigue_siendo_un_registro() {
        let (hubo, n, _) = leer_una("\n1050\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 0);
    }

    /// El `\r` de un archivo escrito desde Windows no entra en el registro:
    /// acabaría dentro del número y lo convertiría en otro.
    #[test]
    fn el_retorno_de_carro_no_entra_en_el_registro() {
        let (hubo, n, b) = leer_una("1050\r\n2075\r\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 4);
        assert_eq!(b, b"1050");
    }

    /// Una línea más larga que el buffer se recorta, y NO se sale de él.
    #[test]
    fn no_escribe_pasado_el_tope() {
        let (_, n, b) = leer_una("0123456789012345\n", 8);
        assert_eq!(n, 8);
        assert_eq!(b, b"01234567");
    }

    /// Leer dos veces da dos registros distintos: el cursor avanza en el
    /// kernel y no en el emisor.
    #[test]
    fn dos_lecturas_dan_dos_registros() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/mov.txt", false);
        x86::mov_r64_r64(&mut code, R10, RAX);
        leer_linea(&mut code, 32);
        // `r8` quedó al final de la primera línea. Se retrocede su largo para
        // volver al principio y se avanza 32: la segunda cae en otro sitio.
        x86::sub_r64_r64(&mut code, R8, R9);
        x86::add_r64_imm8(&mut code, R8, 32);
        leer_linea(&mut code, 32);

        let mut m = Machine::new(code);
        m.poner_archivo("datos/mov.txt", b"1050\n2075\n");
        let base = m.load_data(&vec![0u8; 128]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);

        let leer = |off: u64, n: u64| -> Vec<u8> {
            (0..n).map(|i| m.read_u8_pub(base + off + i)).collect()
        };
        assert_eq!(leer(0, 4), b"1050");
        assert_eq!(leer(32, 4), b"2075");
    }

    /// ★ El ciclo entero: crear, escribir, cerrar — y que en el disco quede
    /// exactamente eso. Sin el `cerrar`, el kernel no guarda nada, así que
    /// esto prueba las tres puertas a la vez.
    #[test]
    fn escribir_y_cerrar_deja_el_archivo_en_el_disco() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/salida.txt", true);
        x86::mov_r64_r64(&mut code, R10, RAX);
        escribir_buffer(&mut code);
        cerrar(&mut code);

        let mut m = Machine::new(code);
        let texto = b"59.97\n";
        let base = m.load_data(texto);
        m.regs[R8 as usize] = base;
        m.regs[R9 as usize] = texto.len() as u64;
        let m = run(m, 500_000);

        assert_eq!(m.regs[RAX as usize], 1, "cerrar debe confirmar el guardado");
        assert_eq!(m.archivo_texto("datos/salida.txt").as_deref(), Some("59.97\n"));
    }

    /// Sin `cerrar`, el disco no cambia. Es el contrato de dos pasos, y hay
    /// que probarlo: si el emulador guardara sobre la marcha, un programa que
    /// se olvida del `CLOSE` pasaría los tests y perdería el fichero en la
    /// máquina.
    #[test]
    fn sin_cerrar_no_hay_nada_en_el_disco() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/salida.txt", true);
        x86::mov_r64_r64(&mut code, R10, RAX);
        escribir_buffer(&mut code);

        let mut m = Machine::new(code);
        let texto = b"59.97\n";
        let base = m.load_data(texto);
        m.regs[R8 as usize] = base;
        m.regs[R9 as usize] = texto.len() as u64;
        let m = run(m, 500_000);

        assert_eq!(m.archivo("datos/salida.txt"), None);
    }

    /// Un archivo que no existe da handle 0 al abrir para LEER. COBOL lo
    /// convierte en "fin de archivo desde el principio" en vez de reventar.
    #[test]
    fn abrir_lo_que_no_esta_da_handle_cero() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/nada.txt", false);
        let m = run(Machine::new(code), 100_000);
        assert_eq!(m.regs[RAX as usize], 0);
    }

    /// Los bytes van con su CUENTA, no cortados en el primer cero: un archivo
    /// no es texto. Si esto se rompiera, cualquier binario se truncaría en su
    /// primer `\0`.
    #[test]
    fn el_nul_viaja() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/bin.dat", true);
        x86::mov_r64_r64(&mut code, R10, RAX);
        escribir_buffer(&mut code);
        cerrar(&mut code);

        let mut m = Machine::new(code);
        let datos = [0x41u8, 0x00, 0x42, 0x00, 0x00, 0x43];
        let base = m.load_data(&datos);
        m.regs[R8 as usize] = base;
        m.regs[R9 as usize] = datos.len() as u64;
        let m = run(m, 500_000);

        assert_eq!(m.archivo("datos/bin.dat"), Some(&datos[..]));
    }
}

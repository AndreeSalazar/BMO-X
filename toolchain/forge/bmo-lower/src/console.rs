//! La puerta de consola -- L1.
//!
//! Un unico subsyscall detras de un unico syscall:
//!
//! ```text
//! INVOKE(CURRENT_TASK, CONSOLE_WRITE, packed)
//!   rax = NR_INVOKE (0)
//!   rdi = CURRENT_TASK
//!   rsi = TASK_OP_CONSOLE_WRITE (0x06)
//!   rdx = hasta 8 bytes empaquetados little-endian, NUL-stop
//! ```
//!
//! Todo pasa **por valor**. La superficie congelada no acepta punteros, y no
//! es una limitacion temporal: es lo que hace que Ring 0 no tenga que
//! validar memoria ajena en la ruta de impresion.
//!
//! # NUL no viaja por esta puerta
//!
//! El kernel corta la palabra en el primer byte cero (asi el productor puede
//! rellenar con ceros un chunk final corto). Un `\0` incrustado en el texto
//! por tanto **no es transmisible**: `write_const` lo omite y sigue con el
//! resto, que para texto es lo correcto y lo predecible. Si algun dia hace
//! falta emitir binario crudo, sera otra puerta, no un parche a esta.

use bmo_abi::syscalls::surface::{CURRENT_TASK, NR_INVOKE, TASK_OP_CONSOLE_READ, TASK_OP_CONSOLE_WRITE};

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, R10, R8, R9};

/// Carga el numero de syscall en `rax`.
///
/// `NR_INVOKE` vale 0 hoy, y `xor eax,eax` cuesta 2 bytes en vez de 5. La
/// condicion es sobre una constante del ABI --el compilador la resuelve-- y
/// deja el emisor correcto si algun dia ese numero deja de ser cero. El
/// `xor` pisa flags, cosa que aqui no importa: en ambas puertas no hay
/// ninguna condicion viva en ese punto.
fn load_syscall_nr(code: &mut Vec<u8>) {
    if NR_INVOKE == 0 {
        x86::zero_r32(code, RAX);
    } else {
        x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    }
}

/// Empaqueta hasta 8 bytes en la palabra que espera el kernel.
fn pack(chunk: &[u8]) -> u64 {
    let mut word = [0u8; 8];
    word[..chunk.len()].copy_from_slice(chunk);
    u64::from_le_bytes(word)
}

/// Emite la escritura de un texto **conocido en tiempo de compilacion**.
///
/// Este es el caso del 90% de las L2: `printf("hola\n")`, `DISPLAY "HOLA"`,
/// `cout << "hola"`. El texto viaja como inmediatos dentro del codigo, asi
/// que no toca la seccion de datos, no genera relocations y no depende del
/// cargador: es exactamente la secuencia que `tools/hello-bex` ya ejecuto en
/// el metal real.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r11` -- todos
/// caller-saved en SysV (`rcx`/`r11` los pisa el propio `syscall`).
pub fn write_const(code: &mut Vec<u8>, text: &[u8]) {
    // Los NUL no cruzan; se parten y descartan aqui, no en la L2.
    for run in text.split(|b| *b == 0) {
        if run.is_empty() {
            continue;
        }
        write_const_run(code, run);
    }
}

fn write_const_run(code: &mut Vec<u8>, text: &[u8]) {
    // rdi/rsi se recargan en CADA llamada a la puerta, aunque el syscall los
    // preserve. Mantenerlos vivos entre llamadas exigiria saber que no hay
    // un salto entrando en medio, y un codegen con `goto`, `PERFORM` o
    // `if/else` no puede prometer eso. 15 bytes es un precio ridiculo frente
    // a un bug de flujo de control.
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_CONSOLE_WRITE as u32);

    for chunk in text.chunks(8) {
        load_syscall_nr(code);
        x86::mov_r64_imm64(code, RDX, pack(chunk));
        x86::syscall(code);
    }
}

/// Emite la escritura de un buffer **calculado en tiempo de ejecucion**.
///
/// Contrato de entrada:
/// - `r8` = puntero a los bytes
/// - `r9` = longitud en bytes
///
/// Se eligieron `r8`/`r9` justamente porque no son argumentos de la puerta
/// (`rdi`/`rsi`/`rdx`) ni los pisa `syscall` (`rcx`/`r11`), asi que
/// sobreviven al bucle sin salvarlos en la pila.
///
/// Esta es la variante que necesitan las L2 cuando el texto no se conoce
/// hasta ejecutar: `printf("%d", x)` formatea a un buffer y llama aqui;
/// `DISPLAY saldo` aplica la edicion PIC y llama aqui. L1 no sabe nada de
/// `%d` ni de PIC -- solo mueve bytes.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r8`, `r9`,
/// `r10`, `r11`. Todos caller-saved. `r8`/`r9` quedan consumidos (apuntando
/// al final, longitud 0).
pub fn write_buffer(code: &mut Vec<u8>) {
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_CONSOLE_WRITE as u32);

    let loop_top = code.len();

    // Quedan bytes?
    x86::test_r64_r64(code, R9, R9);
    let done = x86::emit_jump(code, Jump::IfZero);

    // rcx = min(r9, 8) -- el tamano de este chunk.
    x86::mov_r64_r64(code, RCX, R9);
    x86::cmp_r64_imm8(code, RCX, 8);
    let have_n = x86::emit_jump(code, Jump::IfBelowOrEqual);
    x86::mov_r32_imm32(code, RCX, 8);
    x86::patch_jump(code, have_n);

    // Empaqueta el chunk en rdx recorriendolo de atras hacia adelante:
    //   word = (word << 8) | byte[i]   con i = n-1 ... 0
    // asi el primer byte del texto acaba en el byte BAJO de la palabra, que
    // es lo que el kernel desempaqueta primero.
    x86::zero_r32(code, RDX);
    x86::mov_r64_r64(code, RAX, RCX); // rax = indice, cuenta hacia atras
    let byte_loop = code.len();
    x86::dec_r64(code, RAX);
    x86::shl_r64_imm8(code, RDX, 8);
    x86::movzx_r32_byte_base_index(code, R10, R8, RAX);
    x86::or_r64_r64(code, RDX, R10);
    x86::test_r64_r64(code, RAX, RAX);
    let again = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, again, byte_loop);

    // Avanza ANTES del syscall: `syscall` destruye rcx (guarda el rip de
    // retorno ahi), asi que despues ya no sabriamos cuanto avanzar.
    x86::add_r64_r64(code, R8, RCX);
    x86::sub_r64_r64(code, R9, RCX);

    load_syscall_nr(code);
    x86::syscall(code);

    let back = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, back, loop_top);

    x86::patch_jump(code, done);
}

/// Emite codigo que lee UNA LINEA de la consola del proceso a `r8`, y deja su
/// longitud (sin el salto) en `r9`.
///
/// `r8` tiene que apuntar a un buffer del llamante y `tope` es su tamano.
///
/// ## El tope es un INMEDIATO, y esa es la correccion
///
/// Antes llegaba en `rcx` y se copiaba a `r11` una vez, ANTES del bucle. Pero
/// **`syscall` destruye `r11`**: el silicio guarda ahi RFLAGS. Desde la primera
/// vuelta, la comparacion de limite se hacia contra RFLAGS (~0x246 = 582), o
/// sea que el guarda del buffer estaba MUERTO -- una linea de mas de 64
/// caracteres tecleada en un `ACCEPT` se salia del buffer de pila.
///
/// No se ve en un volcado de bytes y no se veia ejecutando, porque el
/// emulador tampoco modelaba el valor de vuelta (lo ponia en `rax` cuando la
/// puerta lo devuelve en `rdx`), asi que esta funcion **no tenia ni un test**.
/// Dos mentiras que se tapaban la una a la otra.
///
/// El tope lo sabe el compilador --el buffer lo reserva el--, asi que va como
/// inmediato y no ocupa registro que nadie pueda pisar.
///
/// ## Por que cede el turno y no bloquea
///
/// La puerta no bloquea: devuelve `0` cuando no hay nada. Un bucle que
/// insistiera sin ceder se comeria el quantum entero y el terminal --que es
/// quien tiene que ESCRIBIR lo que esperamos-- no correria nunca. El programa
/// se quedaria esperando algo que el mismo impide que llegue.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r9`, `r10`,
/// `r11`. `r8` avanza hasta el final de lo leido.
pub fn read_line(code: &mut Vec<u8>, tope: u8) {
    // El tope cabe en un imm8 con signo: un buffer de linea de mas de 127
    // bytes no es una linea, es otro problema. Se recorta al emitir en vez de
    // emitir una comparacion que compara otra cosa.
    let tope = tope.min(127) as i8;
    x86::zero_r32(code, R9);

    let otra_vez = code.len();

    // INVOKE(CURRENT_TASK, CONSOLE_READ) -> rdx = (n << 56) | bytes
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_CONSOLE_READ as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);

    // rcx = cuantos bytes trae (bits 56..63).
    x86::mov_r64_r64(code, RCX, RDX);
    x86::shr_r64_imm8(code, RCX, 56);
    x86::test_r64_r64(code, RCX, RCX);
    let hay_algo = x86::emit_jump(code, Jump::IfNotZero);
    // Nada todavia: ceder el turno y volver a preguntar.
    crate::task::yield_now(code);
    let reintenta = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, reintenta, otra_vez);
    x86::patch_jump(code, hay_algo);

    // Desempaquetar byte a byte: el primero esta en el byte BAJO.
    let byte_loop = code.len();
    x86::mov_r64_r64(code, R10, RDX);
    x86::and_r64_imm32(code, R10, 0xFF);
    // Es el salto de linea? Entonces la linea esta completa.
    x86::cmp_r64_imm8(code, R10, b'\n' as i8);
    let fin = x86::emit_jump(code, Jump::IfEqual);
    // Guardar si cabe (si no cabe se descarta: recortar en silencio es peor).
    // Contra un INMEDIATO, no contra `r11` -- que el `syscall` de arriba pisa
    // con RFLAGS en cada vuelta. Ver la nota de la cabecera.
    x86::cmp_r64_imm8(code, R9, tope);
    let lleno = x86::emit_jump(code, Jump::IfAboveOrEqual);
    x86::mov_byte_at_reg_from_low(code, R8, R10);
    x86::inc_r64(code, R8);
    x86::inc_r64(code, R9);
    x86::patch_jump(code, lleno);
    // Siguiente byte del paquete.
    x86::shr_r64_imm8(code, RDX, 8);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let mas_bytes = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, mas_bytes, byte_loop);
    // Paquete agotado sin ver el salto: pedir otro.
    let sigue = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, sigue, otra_vez);

    x86::patch_jump(code, fin);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::emu::{run, Machine};

    /// El mensaje que ya se vio en pantalla en el Ryzen real (commit
    /// 179c19b1). Si la puerta deja de reproducir esta secuencia, algo se
    /// rompio respecto a lo unico que sabemos con certeza que funciona.
    const HELLO: &str = "BMO-X: hola mundo desde Ring 3\nCPL3 -> INVOKE -> CPL0 OK\n";

    /// Bytes esperados, escritos a mano segun `tools/hello-bex`.
    fn hello_bex_reference(text: &str) -> Vec<u8> {
        let mut code = Vec::new();
        code.extend_from_slice(&[0x48, 0xBF]);
        code.extend_from_slice(&CURRENT_TASK.to_le_bytes());
        code.push(0xBE);
        code.extend_from_slice(&(TASK_OP_CONSOLE_WRITE as u32).to_le_bytes());
        for chunk in text.as_bytes().chunks(8) {
            let mut word = [0u8; 8];
            word[..chunk.len()].copy_from_slice(chunk);
            code.extend_from_slice(&[0x31, 0xC0]); // xor eax, eax -- NR_INVOKE
            code.extend_from_slice(&[0x48, 0xBA]);
            code.extend_from_slice(&word);
            code.extend_from_slice(&[0x0F, 0x05]);
        }
        code
    }

    #[test]
    fn const_reproduces_the_sequence_that_ran_on_metal() {
        let mut code = Vec::new();
        write_const(&mut code, HELLO.as_bytes());
        assert_eq!(code, hello_bex_reference(HELLO));
    }

    #[test]
    fn const_emits_the_text_the_kernel_will_read() {
        let mut code = Vec::new();
        write_const(&mut code, b"hola\n");
        let m = run(Machine::new(code), 10_000);
        assert_eq!(m.console, "hola\n");
    }

    #[test]
    fn const_handles_every_chunk_boundary() {
        // Longitudes 0..24 cubren chunk exacto, chunk corto y varios chunks.
        for len in 0..24usize {
            let text: String = (0..len).map(|i| (b'a' + (i % 26) as u8) as char).collect();
            let mut code = Vec::new();
            write_const(&mut code, text.as_bytes());
            let m = run(Machine::new(code), 10_000);
            assert_eq!(m.console, text, "falló con longitud {len}");
        }
    }

    #[test]
    fn const_skips_embedded_nul_and_keeps_the_rest() {
        let mut code = Vec::new();
        write_const(&mut code, b"ab\0cd");
        let m = run(Machine::new(code), 10_000);
        assert_eq!(m.console, "abcd");
    }

    #[test]
    fn buffer_writes_the_bytes_it_was_pointed_at() {
        for len in 0..40usize {
            let text: String = (0..len).map(|i| (b'A' + (i % 26) as u8) as char).collect();
            let mut code = Vec::new();
            write_buffer(&mut code);

            let mut m = Machine::new(code);
            let addr = m.load_data(text.as_bytes());
            m.regs[R8 as usize] = addr;
            m.regs[R9 as usize] = len as u64;

            let m = run(m, 100_000);
            assert_eq!(m.console, text, "falló con longitud {len}");
            assert_eq!(m.regs[R9 as usize], 0, "el bucle debe consumir r9");
        }
    }

    /// Ejecuta `read_line` sobre una entrada sembrada y devuelve `(bytes
    /// leidos, contenido del buffer entero)`. El buffer se rodea de centinelas
    /// para poder ver si el emisor escribio fuera.
    fn run_read_line(entrada: &str, tope: u8, free_slot: usize) -> (u64, Vec<u8>) {
        const CENTINELA: u8 = 0xAA;
        let mut code = Vec::new();
        read_line(&mut code, tope);

        let mut m = Machine::new(code);
        // Buffer + 16 bytes de centinela detras.
        let relleno = vec![0u8; free_slot];
        let base = m.load_data(&relleno);
        let cent = m.load_data(&[CENTINELA; 16]);
        assert_eq!(cent, base + free_slot as u64, "el centinela va justo detras");

        m.poner_entrada(entrada);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);

        let mut visto = Vec::new();
        for i in 0..(free_slot + 16) {
            visto.push(m.read_u8_pub(base + i as u64));
        }
        (m.regs[R9 as usize], visto)
    }

    /// La puerta de `ACCEPT`, EJECUTADA. Hasta ahora no tenia ni un test: el
    /// emulador ponia el valor de vuelta en `rax` cuando la puerta lo devuelve
    /// en `rdx`, asi que esto habria girado para siempre.
    #[test]
    fn read_line_reads_what_the_terminal_typed() {
        let (n, buf) = run_read_line("19.99\n", 64, 64);
        assert_eq!(n, 5);
        assert_eq!(&buf[..5], b"19.99");
    }

    /// Para en el salto de linea y no se lleva lo de despues: dos `ACCEPT`
    /// seguidos tienen que ver dos valores distintos.
    #[test]
    fn read_line_stops_at_the_newline() {
        let (n, buf) = run_read_line("12\n34\n", 64, 64);
        assert_eq!(n, 2);
        assert_eq!(&buf[..2], b"12");
    }

    /// Una linea vacia es una linea, no "no hay nada". Un `ACCEPT` al que le
    /// dan un Enter a secas tiene que volver, no colgarse.
    #[test]
    fn read_line_accepts_an_empty_line() {
        let (n, _) = run_read_line("\n", 64, 64);
        assert_eq!(n, 0);
    }

    /// * El guarda del buffer, que estaba MUERTO.
    ///
    /// El tope se copiaba a `r11` antes del bucle y `syscall` pisa `r11` con
    /// RFLAGS, asi que desde la primera vuelta se comparaba contra ~582. Con
    /// un tope de 8 y una linea de 40 caracteres, la version anterior escribia
    /// los 40 -- 32 bytes fuera del buffer de pila que reserva `ACCEPT`.
    ///
    /// Aqui se comprueba sobre los CENTINELAS: si alguno cambia, el emisor
    /// escribio donde no debia.
    #[test]
    fn read_line_never_writes_past_the_buffer() {
        const CENTINELA: u8 = 0xAA;
        let larga = "0123456789012345678901234567890123456789\n";
        let (n, buf) = run_read_line(larga, 8, 8);
        assert_eq!(n, 8, "se guardan 8 y el resto se descarta");
        assert_eq!(&buf[..8], b"01234567");
        for (i, &b) in buf[8..].iter().enumerate() {
            assert_eq!(b, CENTINELA, "byte {i} DETRAS del buffer pisado");
        }
    }

    #[test]
    fn buffer_leaves_the_door_registers_where_the_kernel_expects_them() {
        let mut code = Vec::new();
        write_buffer(&mut code);
        let mut m = Machine::new(code);
        let addr = m.load_data(b"xyz");
        m.regs[R8 as usize] = addr;
        m.regs[R9 as usize] = 3;
        let m = run(m, 100_000);
        // Cada syscall observado tenia capability y operacion correctas: lo
        // verifica el emulador en cada `0F 05` (ver emu.rs).
        assert_eq!(m.syscalls.len(), 1);
    }
}

/// Emite codigo que lee **UN byte** de la consola, bloqueando hasta que llegue.
///
/// Precondicion: `rdi` apunta a un buffer de **9 bytes** que pertenece al
/// llamante y que sobrevive entre llamadas. Deja el byte en `rax`.
///
/// ## Por que hace falta un buffer, y por que es del llamante
///
/// La puerta entrega **hasta 7 bytes de una vez** y los CONSUME: lo que no se
/// guarde se pierde. Escribiendo rapido llegan varios en el mismo paquete, asi
/// que un lector de un byte sin buffer se comeria seis de cada siete
/// pulsaciones -- y pareceria un teclado que pierde letras.
///
/// El buffer lo pone el llamante porque L1 no tiene memoria propia: no sabe de
/// secciones ni de `.data`. Es la misma disciplina que [`read_line`], que
/// recibe el suyo en `r8`.
///
/// Disposicion: `[0..8]` los bytes pendientes empaquetados, `[8]` cuantos
/// quedan. Lo pone a cero quien lo reserva.
///
/// ## Por que cede el turno
///
/// La puerta no bloquea: devuelve `0` si no hay nada. Insistir sin ceder se
/// come el quantum y el terminal --que es quien tiene que ESCRIBIR lo que
/// esperamos-- no correria nunca. El programa esperaria algo que el mismo
/// impide que llegue.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rsi`, `r10`, `r11`. `rdi` se
/// conserva.
pub fn read_char(code: &mut Vec<u8>) {
    // rcx = pendientes del paquete anterior.
    x86::movzx_r32_byte_at_reg_disp(code, RCX, RDI, 8);
    x86::test_r64_r64(code, RCX, RCX);
    let hay_guardados = x86::emit_jump(code, Jump::IfNotZero);

    // -- Pedir un paquete nuevo --
    let pide = code.len();
    // `rdi` lleva el buffer y la puerta lo necesita: se aparca.
    x86::push_r64(code, RDI);
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_CONSOLE_READ as u32);
    load_syscall_nr(code);
    x86::syscall(code);
    x86::pop_r64(code, RDI);

    x86::mov_r64_r64(code, RCX, RDX);
    x86::shr_r64_imm8(code, RCX, 56);
    x86::test_r64_r64(code, RCX, RCX);
    let llego = x86::emit_jump(code, Jump::IfNotZero);
    crate::task::yield_now(code);
    let reintenta = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, reintenta, pide);
    x86::patch_jump(code, llego);

    // Guardar el paquete. El contador viaja en el byte alto de `rdx` y se
    // quita: si se dejara, al llegar al septimo desplazamiento saldria como si
    // fuera texto tecleado.
    x86::shl_r64_imm8(code, RDX, 8);
    x86::shr_r64_imm8(code, RDX, 8);
    x86::mov_at_reg_from_r64(code, RDI, RDX);
    x86::mov_byte_at_reg_disp_from_low(code, RDI, 8, RCX);

    x86::patch_jump(code, hay_guardados);

    // -- Sacar el primero --
    x86::movzx_r32_byte_at_reg(code, RAX, RDI);
    x86::mov_r64_at_reg(code, RDX, RDI);
    x86::shr_r64_imm8(code, RDX, 8);
    x86::mov_at_reg_from_r64(code, RDI, RDX);
    x86::movzx_r32_byte_at_reg_disp(code, RCX, RDI, 8);
    x86::dec_r64(code, RCX);
    x86::mov_byte_at_reg_disp_from_low(code, RDI, 8, RCX);
}

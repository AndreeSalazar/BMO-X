//! La puerta de ARCHIVOS -- L1.
//!
//! Lo mismo que `console` pero sobre `KIND_ARCHIVO`: abrir, mover bytes,
//! cerrar. L1 no sabe que es un registro, ni un `FD`, ni una PICTURE -- solo
//! mueve bytes por la puerta congelada. Quien decide que una linea es un
//! registro es la L2 (`lang/cobol`), y quien decide que ese registro son
//! centavos es su PICTURE.
//!
//! ## Todo por valor, tambien aqui
//!
//! La ruta viaja de 8 en 8 por `TASK_OP_RUTA` y los datos de 7 en 7 con su
//! cuenta en el byte alto. Siete y no ocho porque el octavo lleva el contador
//! -- y aqui ese contador no es un lujo: un archivo **no es texto** y cortar en
//! el primer cero corromperia cualquier binario. La consola puede permitirse
//! el NUL-stop; esto no.
//!
//! ## Escribir es de dos pasos
//!
//! Nada llega al disco hasta [`close`]. Un programa que muere a medias no
//! deja medio archivo: no deja ninguno. Para un fichero de movimientos eso es
//! lo correcto -- un extracto truncado se parece demasiado a uno completo.

use bmo_abi::syscalls::surface::{
    ARCH_OP_CERRAR, ARCH_OP_ESCRIBIR, ARCH_OP_LEER, ARCH_OP_LEER_LINEA, CURRENT_TASK, NR_INVOKE,
    TASK_OP_ARCHIVO_ABRIR, TASK_OP_ARCHIVO_CREAR, TASK_OP_RUTA,
};

use crate::x86::{self, Jump, RAX, RCX, RDI, RDX, RSI, R10, R8, R9};

/// Emite la apertura de un archivo cuya ruta se conoce al COMPILAR.
///
/// Es el caso de COBOL entero: `SELECT ... ASSIGN TO "datos/movim.txt"` fija la
/// ruta en el fuente. Viaja como inmediatos dentro del codigo, igual que
/// `console::write_const` -- sin seccion de datos y sin relocations.
///
/// Deja el handle en `rax`. Cero = no se pudo abrir; el llamante decide que
/// hacer con eso (COBOL levanta su bandera de fin y sigue).
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r11`.
pub fn abrir_const(code: &mut Vec<u8>, ruta: &[u8], escribe: bool) {
    x86::mov_r64_imm64(code, RDI, CURRENT_TASK);
    x86::mov_r32_imm32(code, RSI, TASK_OP_RUTA as u32);
    for trozo in ruta.chunks(8) {
        let mut w = [0u8; 8];
        w[..trozo.len()].copy_from_slice(trozo);
        // rdi/rsi se recargan en cada vuelta por la misma razon que en
        // `console::write_const`: mantenerlos vivos exigiria saber que nadie
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
    // codigo. Se pasa a rax porque un handle es lo unico que el llamante
    // quiere de aqui, y `rdx` lo pisa cualquier cosa.
    //
    // Si la apertura fallo, `code != 0` y `value` es 0: el cero hace de "no
    // hay handle" sin necesidad de mirar dos registros.
    x86::mov_r64_r64(code, RAX, RDX);
}

/// Emite la lectura de UNA LINEA del archivo cuyo handle esta en `r10`.
///
/// - Entrada: `r10` = handle, `r8` = buffer del llamante.
/// - Salida: `r9` = largo de la linea (sin el salto), `rax` = 1 si hubo
///   registro y **0 si se acabo el archivo**.
///
/// El handle vive en `r10` y no en `rdi` porque `rdi` es el argumento de la
/// puerta y hay que recargarlo en cada vuelta. `r10` no lo toca el `syscall`
/// --a diferencia de `rcx` y `r11`, que el silicio destruye-- asi que sobrevive
/// al bucle sin salvarlo en la pila.
///
/// `tope` es un INMEDIATO por la misma leccion que dejo `console::read_line`:
/// alli el limite viajaba en `r11` y el `syscall` lo pisaba con RFLAGS, asi que
/// el guarda del buffer estaba muerto. Aqui el tope lo sabe el compilador.
///
/// Registros que ensucia: `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r9`, `r11`.
/// `r8` avanza hasta el final de lo leido.
pub fn read_line(code: &mut Vec<u8>, tope: u8) {
    let tope = tope.min(127) as i8;
    x86::zero_r32(code, R9);

    let otra_vez = code.len();
    x86::mov_r64_r64(code, RDI, R10);
    x86::mov_r32_imm32(code, RSI, ARCH_OP_LEER_LINEA as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);
    // `rax` guarda la palabra entera: `rdx` se va desmontando byte a byte y
    // el bit de fin vive arriba del todo.
    x86::mov_r64_r64(code, RAX, RDX);

    // rcx = cuantos bytes trae el paquete (bits 56..62).
    x86::mov_r64_r64(code, RCX, RDX);
    x86::shr_r64_imm8(code, RCX, 56);
    x86::and_r64_imm32(code, RCX, 0x7F);

    // Vino algo, o el kernel dice que la linea acabo? Cualquiera de las dos
    // hace de esto un registro. Una linea VACIA es un registro: darla por fin
    // de archivo se comeria el ultimo renglon de un fichero con doble salto.
    x86::test_r64_r64(code, RCX, RCX);
    let hay_bytes = x86::emit_jump(code, Jump::IfNotZero);
    // Sin bytes: mirar el bit de fin. Si tampoco esta, se acabo el archivo.
    x86::shr_r64_imm8(code, RAX, 63);
    x86::test_r64_r64(code, RAX, RAX);
    let fin_de_linea_vacia = x86::emit_jump(code, Jump::IfNotZero);
    // Fin de archivo. **No se cede el turno ni se reintenta**, al reves que la
    // consola: un archivo que se acabo no va a crecer porque esperemos.
    // Insistir aqui colgaria el programa para siempre.
    let fin_de_archivo = x86::emit_jump(code, Jump::Always);

    x86::patch_jump(code, fin_de_linea_vacia);
    let fin_linea_a = x86::emit_jump(code, Jump::Always);

    x86::patch_jump(code, hay_bytes);

    // Desempaquetar byte a byte; el primero esta en el byte BAJO. Aqui NO hay
    // que buscar el salto: el kernel ya no lo entrega.
    let byte_loop = code.len();
    x86::mov_r64_r64(code, RSI, RDX);
    x86::and_r64_imm32(code, RSI, 0xFF);
    // El retorno de carro se tira: un archivo escrito desde el anfitrion trae
    // "\r\n" y ese `\r` acabaria dentro del numero.
    x86::cmp_r64_imm8(code, RSI, b'\r' as i8);
    let es_cr = x86::emit_jump(code, Jump::IfEqual);
    // Guardar si cabe. Lo que no cabe se descarta: recortar en silencio es
    // peor que perder el resto de una linea que no debia ser tan larga.
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

    // Paquete agotado. Si el kernel no marco el fin, la linea sigue: otra
    // vuelta. `rax` conserva la palabra original con su bit de arriba.
    x86::shr_r64_imm8(code, RAX, 63);
    x86::test_r64_r64(code, RAX, RAX);
    let acabo = x86::emit_jump(code, Jump::IfNotZero);
    let sigue = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, sigue, otra_vez);

    // `rax` = 1 si esto fue un registro, 0 si el archivo se acabo. A estos dos
    // caminos se llega sabiendo que SI lo fue: por uno porque el kernel marco
    // el fin de linea, por el otro porque trajo bytes.
    x86::patch_jump(code, acabo);
    x86::patch_jump(code, fin_linea_a);
    x86::mov_r32_imm32(code, RAX, 1);
    let listo = x86::emit_jump(code, Jump::Always);

    // -- Fin de archivo, y el ULTIMO RENGLON --
    //
    // Aqui se llega cuando el kernel no trajo bytes y tampoco marco fin de
    // linea. Eso NO siempre es "no hubo registro": si `r9` ya trae bytes, son
    // los del ultimo renglon de un fichero que acaba SIN salto -- y ese es el
    // clasico que se come el movimiento de mas valor, el ultimo.
    //
    // La cuenta se saca de `r9` y no de una bandera guardada en `r11`, porque
    // a este camino se llega justo despues de un `syscall` y el silicio deja
    // ahi RFLAGS: la bandera estaba muerta antes de leerla. `r9` es la misma
    // verdad y nadie la pisa.
    x86::patch_jump(code, fin_de_archivo);
    x86::zero_r32(code, RAX);
    x86::test_r64_r64(code, R9, R9);
    let sin_nada = x86::emit_jump(code, Jump::IfZero);
    x86::mov_r32_imm32(code, RAX, 1);
    x86::patch_jump(code, sin_nada);
    x86::patch_jump(code, listo);
}

/// Emite la lectura de un REGISTRO DE LARGO FIJO -- `n` bytes crudos.
///
/// - Entrada: `r10` = handle, `r8` = el area del registro.
/// - Salida: `rax` = 1 si se leyo el registro entero, 0 si se acabo el archivo.
/// - Ensucia `rax`, `rcx`, `rdx`, `rdi`, `rsi`, `r9` y `r11`. `r8` queda donde
///   estaba.
///
/// # * El resto de siete bytes, y por que el llamante tiene que guardarlo
///
/// `ARCH_OP_LEER` entrega **hasta siete bytes por paquete** y adelanta el
/// cursor exactamente los que entrega -- eso esta comprobado en
/// `ring0/obj/archivo.rs`. Pero un registro de banca no mide un multiplo de
/// siete: mide 5, o 16, o 47.
///
/// Asi que la ultima tirada de cada registro trae bytes de MAS, y esos bytes
/// **son del registro siguiente**. No se pueden devolver: el cursor es del
/// kernel y nadie de fuera puede retroceder.
///
/// Por eso el llamante reserva **16 bytes detras del area**:
///
/// ```text
///   [ area: n bytes ][ palabra pendiente: 8 ][ cuantos quedan: 8 ]
///   ^                ^
///   r8               r8+n
/// ```
///
/// La sobra se guarda ahi y la consume el registro siguiente antes de pedir
/// nada al kernel. Sin eso, un fichero de registros de 5 bytes daria bien el
/// primero y basura todos los demas -- que es exactamente el fallo que este
/// comentario existe para no dejar suelto.
///
/// [!] Un registro a medias al final del fichero se trata como **fin de
/// archivo**. Distinguir "se acabo" de "el fichero esta truncado" es lo que
/// hace el `FILE STATUS` del estandar, y esa es otra tarea.
pub fn leer_bytes(code: &mut Vec<u8>, n: u32) {
    assert!(n > 0, "un registro de cero bytes no es un registro");
    let pal = n as i32; // [r8 + n]     = palabra pendiente
    let cnt = n as i32 + 8; // [r8 + n + 8] = cuantos quedan en ella

    // r9 = por donde va la escritura. El area empieza en r8 y no se mueve.
    x86::mov_r64_r64(code, R9, R8);

    let drenar = code.len();
    // -- 1) Gastar lo que sobro del registro anterior --
    x86::mov_r64_at_reg_disp32(code, RCX, R8, cnt);
    x86::test_r64_r64(code, RCX, RCX);
    let nada_pendiente = x86::emit_jump(code, Jump::IfZero);
    x86::mov_r64_at_reg_disp32(code, RDX, R8, pal);

    let copia = code.len();
    // Ya esta lleno el registro? Entonces lo que quede se guarda para el
    // siguiente y no se toca mas.
    x86::mov_r64_r64(code, RAX, R9);
    x86::sub_r64_r64(code, RAX, R8);
    x86::cmp_r64_imm32(code, RAX, n as i32);
    let lleno = x86::emit_jump(code, Jump::IfAboveOrEqual);
    x86::mov_byte_at_reg_from_low(code, R9, RDX);
    x86::inc_r64(code, R9);
    x86::shr_r64_imm8(code, RDX, 8);
    x86::dec_r64(code, RCX);
    x86::test_r64_r64(code, RCX, RCX);
    let sigue_copiando = x86::emit_jump(code, Jump::IfNotZero);
    x86::patch_jump_to(code, sigue_copiando, copia);

    x86::patch_jump(code, lleno);
    // Lo que no cupo queda apuntado para el registro de despues.
    x86::mov_at_reg_disp32_from_r64(code, R8, pal, RDX);
    x86::mov_at_reg_disp32_from_r64(code, R8, cnt, RCX);

    x86::patch_jump(code, nada_pendiente);
    // -- 2) Esta completo el registro? --
    x86::mov_r64_r64(code, RAX, R9);
    x86::sub_r64_r64(code, RAX, R8);
    x86::cmp_r64_imm32(code, RAX, n as i32);
    let completo = x86::emit_jump(code, Jump::IfAboveOrEqual);

    // -- 3) Pedir otro paquete al kernel --
    x86::mov_r64_r64(code, RDI, R10);
    x86::mov_r32_imm32(code, RSI, ARCH_OP_LEER as u32);
    x86::mov_r32_imm32(code, RAX, NR_INVOKE);
    x86::syscall(code);
    // rcx = cuantos trae (byte alto), rdx = los bytes.
    x86::mov_r64_r64(code, RCX, RDX);
    x86::shr_r64_imm8(code, RCX, 56);
    x86::and_r64_imm32(code, RCX, 0xFF);
    x86::test_r64_r64(code, RCX, RCX);
    let se_acabo = x86::emit_jump(code, Jump::IfZero);
    // Quitar la cuenta del byte alto para quedarse con los datos.
    x86::shl_r64_imm8(code, RDX, 8);
    x86::shr_r64_imm8(code, RDX, 8);
    x86::mov_at_reg_disp32_from_r64(code, R8, pal, RDX);
    x86::mov_at_reg_disp32_from_r64(code, R8, cnt, RCX);
    let otra_vuelta = x86::emit_jump(code, Jump::Always);
    x86::patch_jump_to(code, otra_vuelta, drenar);

    x86::patch_jump(code, completo);
    x86::mov_r32_imm32(code, RAX, 1);
    let listo = x86::emit_jump(code, Jump::Always);

    x86::patch_jump(code, se_acabo);
    x86::zero_r32(code, RAX);
    x86::patch_jump(code, listo);
}

/// Emite la escritura de un buffer en el archivo cuyo handle esta en `r10`.
///
/// - Entrada: `r10` = handle, `r8` = puntero, `r9` = largo.
/// - `r8`/`r9` quedan consumidos.
///
/// Los bytes van de 7 en 7 con la cuenta en el byte alto -- **no** cortando en
/// el primer cero. Ver la nota de cabecera.
pub fn escribir_buffer(code: &mut Vec<u8>) {
    let loop_top = code.len();
    x86::test_r64_r64(code, R9, R9);
    let done = x86::emit_jump(code, Jump::IfZero);

    // rcx = min(r9, 7) -- el tamano de este trozo.
    x86::mov_r64_r64(code, RCX, R9);
    x86::cmp_r64_imm8(code, RCX, 7);
    let tengo_n = x86::emit_jump(code, Jump::IfBelowOrEqual);
    x86::mov_r32_imm32(code, RCX, 7);
    x86::patch_jump(code, tengo_n);

    // Empaquetar de atras hacia adelante: el primer byte del texto acaba en el
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
    // `rcx` todavia vale -- el `syscall` lo destruye.
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

/// Emite el cierre del archivo cuyo handle esta en `r10`.
///
/// **En uno de escritura es donde el contenido llega al disco.** Deja en `rax`
/// el 1/0 que contesta el kernel: `0` significa que no se guardo NADA, no que
/// se guardara a medias.
pub fn close(code: &mut Vec<u8>) {
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

    /// Abre un archivo sembrado, lee una linea y devuelve `(hubo, largo,
    /// bytes)`.
    fn leer_una(contenido: &str, tope: u8) -> (u64, u64, Vec<u8>) {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/mov.txt", false);
        // El handle a r10, que es donde lo quieren las demas puertas.
        x86::mov_r64_r64(&mut code, R10, RAX);
        read_line(&mut code, tope);

        let mut m = Machine::new(code);
        m.poner_archivo("datos/mov.txt", contenido.as_bytes());
        let base = m.load_data(&vec![0u8; 64]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);

        let n = m.regs[R9 as usize];
        // r8 acabo al final de lo leido: se retrocede para mirar el principio.
        let mut visto = Vec::new();
        for i in 0..n {
            visto.push(m.read_u8_pub(base + i));
        }
        (m.regs[RAX as usize], n, visto)
    }

    /// Lee `cuantos` registros SEGUIDOS de `n` bytes y devuelve lo que cayo en
    /// cada uno, mas el `rax` de la ultima lectura.
    ///
    /// Leer varios de una tirada es el punto: el fallo del resto de siete bytes
    /// **no se ve en el primero**, se ve en el segundo.
    fn leer_registros(contenido: &[u8], n: u32, cuantos: usize) -> (Vec<Vec<u8>>, u64) {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/reg.bin", false);
        x86::mov_r64_r64(&mut code, R10, RAX);
        // Un area por registro, seguidas, con sus 16 bytes de estado detras.
        // El estado se comparte: `leer_bytes` lo busca en `r8+n`, asi que cada
        // vuelta tiene que apuntar `r8` a la MISMA area para no perderlo.
        for _ in 0..cuantos {
            leer_bytes(&mut code, n);
            // Guardar lo leido en la zona de resultados y devolver r8 a su
            // sitio lo hace el anfitrion mirando la memoria; aqui solo se
            // repite la lectura sobre la misma area, y el test lee entre medias
            // usando areas distintas -- ver abajo.
        }
        let mut m = Machine::new(code);
        m.poner_archivo("datos/reg.bin", contenido);
        let base = m.load_data(&vec![0u8; (n as usize) + 16]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);
        // Con una sola area, lo que queda es el ULTIMO registro leido.
        let ultimo: Vec<u8> = (0..n as u64).map(|i| m.read_u8_pub(base + i)).collect();
        (vec![ultimo], m.regs[RAX as usize])
    }

    /// Un registro de largo fijo, leido entero.
    #[test]
    fn lee_un_registro_de_largo_fijo() {
        let (regs, hubo) = leer_registros(b"ABCDE", 5, 1);
        assert_eq!(hubo, 1);
        assert_eq!(regs[0], b"ABCDE".to_vec());
    }

    /// * EL RESTO DE SIETE BYTES. Registros de **5** bytes: el paquete del
    /// kernel trae 7, asi que el primero deja DOS bytes que son del segundo.
    ///
    /// Si esa sobra se tirara, el segundo registro saldria corrido y todos los
    /// de detras tambien. Este test lee tres seguidos y mira el TERCERO, que es
    /// donde el error ya se ha acumulado dos veces.
    #[test]
    fn el_resto_de_siete_bytes_no_corre_los_registros() {
        // "AAAAABBBBBCCCCC" -> tres registros de cinco.
        let (regs, hubo) = leer_registros(b"AAAAABBBBBCCCCC", 5, 3);
        assert_eq!(hubo, 1, "el tercer registro tenia que existir");
        assert_eq!(
            regs[0],
            b"CCCCC".to_vec(),
            "los registros se corrieron: la sobra del paquete de 7 se perdio"
        );
    }

    /// Un registro mas grande que un paquete: hacen falta varias tiradas.
    #[test]
    fn un_registro_mas_grande_que_el_paquete() {
        let (regs, hubo) = leer_registros(b"0123456789ABCDEF", 16, 1);
        assert_eq!(hubo, 1);
        assert_eq!(regs[0], b"0123456789ABCDEF".to_vec());
    }

    /// Y varios de esos seguidos, que es donde el resto y el troceado se
    /// pisan a la vez: 16 no es multiplo de 7.
    #[test]
    fn varios_registros_grandes_seguidos() {
        let mut datos = Vec::new();
        datos.extend_from_slice(b"0123456789ABCDEF");
        datos.extend_from_slice(b"GHIJKLMNOPQRSTUV");
        datos.extend_from_slice(b"WXYZabcdefghijkl");
        let (regs, hubo) = leer_registros(&datos, 16, 3);
        assert_eq!(hubo, 1);
        assert_eq!(regs[0], b"WXYZabcdefghijkl".to_vec());
    }

    /// Cuando se acaba el fichero, `rax` = 0. Sin eso un `PERFORM UNTIL` sobre
    /// un fichero no tendria forma de parar.
    #[test]
    fn al_acabarse_el_fichero_avisa() {
        let (_, hubo) = leer_registros(b"AAAAA", 5, 2);
        assert_eq!(hubo, 0, "el segundo registro no existia y no lo dijo");
    }

    /// Un registro a medias al final se trata como fin de archivo. Esta dicho
    /// en la cabecera: distinguirlo de un fichero truncado es lo que hace el
    /// `FILE STATUS`, y esa es otra tarea.
    #[test]
    fn un_registro_a_medias_cuenta_como_fin() {
        let (_, hubo) = leer_registros(b"AAAAABB", 5, 2);
        assert_eq!(hubo, 0);
    }

    #[test]
    fn lee_la_primera_linea() {
        let (hubo, n, b) = leer_una("1050\n2075\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 4);
        assert_eq!(b, b"1050");
    }

    /// Un archivo que se acabo da `rax = 0`. Es lo que COBOL convierte en
    /// `AT END`, y es la unica forma de que un `PERFORM UNTIL` termine.
    #[test]
    fn un_archivo_vacio_es_fin_de_archivo() {
        let (hubo, n, _) = leer_una("", 32);
        assert_eq!(hubo, 0, "sin bytes no hay registro");
        assert_eq!(n, 0);
    }

    /// Una linea VACIA si es un registro. Darla por fin de archivo se comeria
    /// el ultimo renglon de un fichero que acaba en doble salto.
    #[test]
    fn una_linea_vacia_sigue_siendo_un_registro() {
        let (hubo, n, _) = leer_una("\n1050\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 0);
    }

    /// * El ULTIMO renglon cuenta aunque el archivo no acabe en salto de
    /// linea. Es el clasico que se come el movimiento de mas valor: el ultimo.
    ///
    /// El kernel solo marca "fin de linea" cuando encuentra el `\n`, asi que en
    /// esta lectura no lo marca y tampoco trae bytes en el ultimo paquete: la
    /// unica prueba de que hubo registro es que `r9` ya trae los bytes de la
    /// linea. Cuando esto lo llevaba una bandera en `r11`, el `syscall` la
    /// mataba y el registro se perdia en silencio.
    #[test]
    fn el_ultimo_renglon_sin_salto_es_un_registro() {
        let (hubo, n, b) = leer_una("2075", 32);
        assert_eq!(hubo, 1, "un renglon sin salto final SIGUE siendo un registro");
        assert_eq!(n, 4);
        assert_eq!(b, b"2075");
    }

    /// Y el de despues si es fin de archivo: el renglon sin salto se entrega
    /// UNA vez, no en cada vuelta. Si se repitiera, un `PERFORM UNTIL` sumaria
    /// el mismo importe para siempre.
    #[test]
    fn tras_el_ultimo_renglon_sin_salto_se_acaba() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/mov.txt", false);
        x86::mov_r64_r64(&mut code, R10, RAX);
        read_line(&mut code, 32);
        // La primera lectura se tira; solo interesa la SEGUNDA. `r8` acabo al
        // final de lo leido, asi que se devuelve al principio del buffer.
        x86::sub_r64_r64(&mut code, R8, R9);
        read_line(&mut code, 32);

        let mut m = Machine::new(code);
        m.poner_archivo("datos/mov.txt", b"2075");
        let base = m.load_data(&vec![0u8; 64]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);
        assert_eq!(m.regs[RAX as usize], 0, "el renglon sin salto no se entrega dos veces");
    }

    /// El `\r` de un archivo escrito desde Windows no entra en el registro:
    /// acabaria dentro del numero y lo convertiria en otro.
    #[test]
    fn el_retorno_de_carro_no_entra_en_el_registro() {
        let (hubo, n, b) = leer_una("1050\r\n2075\r\n", 32);
        assert_eq!(hubo, 1);
        assert_eq!(n, 4);
        assert_eq!(b, b"1050");
    }

    /// Una linea mas larga que el buffer se recorta, y NO se sale de el.
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
        read_line(&mut code, 32);
        // `r8` quedo al final de la primera linea. Se retrocede su largo para
        // volver al principio y se avanza 32: la segunda cae en otro sitio.
        x86::sub_r64_r64(&mut code, R8, R9);
        x86::add_r64_imm8(&mut code, R8, 32);
        read_line(&mut code, 32);

        let mut m = Machine::new(code);
        m.poner_archivo("datos/mov.txt", b"1050\n2075\n");
        let base = m.load_data(&vec![0u8; 128]);
        m.regs[R8 as usize] = base;
        let m = run(m, 500_000);

        let read = |off: u64, n: u64| -> Vec<u8> {
            (0..n).map(|i| m.read_u8_pub(base + off + i)).collect()
        };
        assert_eq!(read(0, 4), b"1050");
        assert_eq!(read(32, 4), b"2075");
    }

    /// * El ciclo entero: crear, escribir, cerrar -- y que en el disco quede
    /// exactamente eso. Sin el `close`, el kernel no guarda nada, asi que
    /// esto prueba las tres puertas a la vez.
    #[test]
    fn escribir_y_cerrar_deja_el_archivo_en_el_disco() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/salida.txt", true);
        x86::mov_r64_r64(&mut code, R10, RAX);
        escribir_buffer(&mut code);
        close(&mut code);

        let mut m = Machine::new(code);
        let texto = b"59.97\n";
        let base = m.load_data(texto);
        m.regs[R8 as usize] = base;
        m.regs[R9 as usize] = texto.len() as u64;
        let m = run(m, 500_000);

        assert_eq!(m.regs[RAX as usize], 1, "cerrar debe confirmar el guardado");
        assert_eq!(m.archivo_texto("datos/salida.txt").as_deref(), Some("59.97\n"));
    }

    /// Sin `close`, el disco no cambia. Es el contrato de dos pasos, y hay
    /// que probarlo: si el emulador guardara sobre la marcha, un programa que
    /// se olvida del `CLOSE` pasaria los tests y perderia el fichero en la
    /// maquina.
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
    /// no es texto. Si esto se rompiera, cualquier binario se truncaria en su
    /// primer `\0`.
    #[test]
    fn el_nul_viaja() {
        let mut code = Vec::new();
        abrir_const(&mut code, b"datos/bin.dat", true);
        x86::mov_r64_r64(&mut code, R10, RAX);
        escribir_buffer(&mut code);
        close(&mut code);

        let mut m = Machine::new(code);
        let datos = [0x41u8, 0x00, 0x42, 0x00, 0x00, 0x43];
        let base = m.load_data(&datos);
        m.regs[R8 as usize] = base;
        m.regs[R9 as usize] = datos.len() as u64;
        let m = run(m, 500_000);

        assert_eq!(m.archivo("datos/bin.dat"), Some(&datos[..]));
    }
}

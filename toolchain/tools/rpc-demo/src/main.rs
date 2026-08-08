//! Genera el par **cliente/servidor** que prueba Endpoint RPC en hardware.
//!
//! Un IPC bloqueante que compila no demuestra nada: lo que hay que ver es el
//! viaje completo -- el cliente llama y se queda parado, el kernel lleva la
//! llamada, el servidor la atiende y responde, y el cliente despierta con la
//! respuesta. Si alguno de los tres guardias del kernel (el one-shot, la
//! generacion, la ranura preparada antes de despertar) estuviera mal, se ve
//! aqui y no en un comentario.
//!
//! ## Por que a mano y no en BMO C
//!
//! El frontend de C mapea los argumentos de `syscall` a rdi/rsi/rdx/r10/r8/r9,
//! que es justo la convencion correcta -- pero estos dos programas necesitan
//! **bucles con condicion** sobre el valor devuelto (el cliente reintenta
//! hasta que el servidor exista) y guardar handles entre llamadas. Escribirlos
//! a mano es unas pocas decenas de instrucciones y no arrastra al frontend a
//! una funcion que todavia no tiene. Cuando C sepa expresarlo, se reescriben.

use std::path::PathBuf;
use bmo_abi::bef::writer::{BefBuilder, BefSection};

const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;

// Los tres numeros de syscall. La superficie congelada.
const NR_INVOKE: u32 = 0;
const NR_WAIT: u32 = 2;

// Operaciones sobre CURRENT_TASK.
const OP_YIELD: u32 = 3;
const OP_EXIT: u32 = 4;
const OP_CHANNEL_OPEN: u32 = 5;
const OP_CONSOLE_WRITE: u32 = 6;
const OP_ENDPOINT_CREATE: u32 = 7;
const OP_ENDPOINT_CONNECT: u32 = 8;

/// Registros que usamos. Codificacion de x86-64 en el orden de la ISA.
#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum R { Rax = 0, Rcx = 1, Rdx = 2, Rbx = 3, Rsp = 4, Rbp = 5, Rsi = 6, Rdi = 7 }

/// Emisor minimo. Solo lo que hacen falta estos dos programas.
struct Asm { c: Vec<u8> }

impl Asm {
    fn new() -> Self { Self { c: Vec::new() } }

    /// `mov <r64>, imm64`  -- REX.W B8+rd
    fn mov_imm64(&mut self, r: R, v: u64) {
        self.c.push(0x48);
        self.c.push(0xB8 + r as u8);
        self.c.extend_from_slice(&v.to_le_bytes());
    }

    /// `mov <e32>, imm32` -- pone a cero la mitad alta, que es lo que queremos
    /// para los numeros de operacion.
    fn mov_imm32(&mut self, r: R, v: u32) {
        self.c.push(0xB8 + r as u8);
        self.c.extend_from_slice(&v.to_le_bytes());
    }

    /// `mov dst, src` (64 bits) -- REX.W 89 /r
    fn mov_reg(&mut self, dst: R, src: R) {
        self.c.push(0x48);
        self.c.push(0x89);
        self.c.push(0xC0 | ((src as u8) << 3) | dst as u8);
    }

    /// `xor <e32>, <e32>` -- pone el registro a cero.
    fn zero(&mut self, r: R) {
        self.c.push(0x31);
        self.c.push(0xC0 | ((r as u8) << 3) | r as u8);
    }

    fn syscall(&mut self) { self.c.extend_from_slice(&[0x0F, 0x05]); }

    /// `test eax, eax` -- pone ZF si el codigo de estado es 0 (OK).
    fn test_eax(&mut self) { self.c.extend_from_slice(&[0x85, 0xC0]); }

    /// `test rdx, rdx` -- ZF si el valor devuelto es 0.
    fn test_rdx(&mut self) { self.c.extend_from_slice(&[0x48, 0x85, 0xD2]); }

    /// Salto condicional hacia una etiqueta ya emitida.
    fn jcc_atras(&mut self, cc: u8, destino: usize) {
        self.c.push(0x0F);
        self.c.push(cc);
        let desde = (self.c.len() + 4) as i32;
        let rel = destino as i32 - desde;
        self.c.extend_from_slice(&rel.to_le_bytes());
    }

    /// Salto condicional/incondicional de 32 bits, con destino por parchear.
    /// Devuelve la posicion del desplazamiento.
    fn jmp_placeholder(&mut self, cond: Option<u8>) -> usize {
        match cond {
            Some(cc) => { self.c.push(0x0F); self.c.push(cc); }
            None => self.c.push(0xE9),
        }
        let at = self.c.len();
        self.c.extend_from_slice(&[0, 0, 0, 0]);
        at
    }

    /// Ata un salto pendiente a la posicion actual.
    ///
    /// El desplazamiento es relativo al final de la instruccion, no a su
    /// principio. Equivocarse en eso es el bug que este proyecto ya se comio
    /// tres veces --el `IF` de COBOL, `PERFORM`, los bucles de C-- siempre igual:
    /// un salto que compila y no salta a donde dice.
    fn atar(&mut self, at: usize) {
        let destino = self.c.len() as i32;
        let desde = (at + 4) as i32;
        let rel = destino - desde;
        self.c[at..at + 4].copy_from_slice(&rel.to_le_bytes());
    }

    fn aqui(&self) -> usize { self.c.len() }

    fn saltar_atras(&mut self, destino: usize) {
        self.c.push(0xE9);
        let desde = (self.c.len() + 4) as i32;
        let rel = destino as i32 - desde;
        self.c.extend_from_slice(&rel.to_le_bytes());
    }

    // -- Envoltorios de la superficie --

    /// `INVOKE(CURRENT_TASK, op, arg0)`.
    fn invoke_task(&mut self, op: u32, arg0: u64) {
        self.mov_imm64(R::Rdi, CURRENT_TASK);
        self.mov_imm32(R::Rsi, op);
        self.mov_imm64(R::Rdx, arg0);
        self.mov_imm32(R::Rax, NR_INVOKE);
        self.syscall();
    }

    /// Escribe texto por la puerta de consola: 8 bytes por llamada, LE, con
    /// NUL de parada. Es la misma que ya usan los tres demos.
    fn imprimir(&mut self, texto: &str) {
        for trozo in texto.as_bytes().chunks(8) {
            let mut w = [0u8; 8];
            w[..trozo.len()].copy_from_slice(trozo);
            self.invoke_task(OP_CONSOLE_WRITE, u64::from_le_bytes(w));
        }
    }

    fn salir(&mut self) {
        self.mov_imm64(R::Rdi, CURRENT_TASK);
        self.mov_imm32(R::Rsi, OP_EXIT);
        self.mov_imm32(R::Rax, NR_INVOKE);
        self.syscall();
    }
}

/// El SERVIDOR: abre su estuario, crea el endpoint y atiende para siempre.
fn servidor() -> Vec<u8> {
    let mut a = Asm::new();

    // Estuario 0: por ahi le llegaran las llamadas.
    a.invoke_task(OP_CHANNEL_OPEN, 0);
    // Endpoint atado a ese estuario. rdx = handle.
    a.invoke_task(OP_ENDPOINT_CREATE, 0);
    a.mov_reg(R::Rbx, R::Rdx); // rbx = endpoint. Sobrevive a los syscall.

    a.imprimir("ventanilla abierta\n");

    // -- bucle de atencion --
    let bucle = a.aqui();
    // WAIT(endpoint, 0, 0) -> rax = code, rdx = handle de respuesta
    a.mov_reg(R::Rdi, R::Rbx);
    a.zero(R::Rsi);
    a.zero(R::Rdx);
    a.mov_imm32(R::Rax, NR_WAIT);
    a.syscall();
    a.test_eax();
    let al_final = a.jmp_placeholder(Some(0x85)); // jnz: code != 0 -> se acabo
    // value = 0 significa "todavia nada, vuelve a preguntar". El kernel deja
    // la espera puesta y devuelve; quien reintenta es este bucle, desde Ring 3.
    // Que el reintento viva AQUI y no dentro del kernel es lo que impide el
    // bucle infinito en Ring 0 que reinicio la maquina la primera vez.
    a.test_rdx();
    a.jcc_atras(0x84, bucle); // jz -> otra vez a esperar

    // Responder: INVOKE(reply, code=0, value=0xBEEF)
    a.mov_reg(R::Rdi, R::Rdx);
    a.zero(R::Rsi);                 // code 0 = OK
    a.mov_imm64(R::Rdx, 0xBEEF);    // el valor que el cliente debe recibir
    a.mov_imm32(R::Rax, NR_INVOKE);
    a.syscall();

    // La etiqueta del proceso ya dice quien habla ("srv>"), asi que el mensaje
    // no la repite: en pantalla salia "srv> srv: ...".
    a.imprimir("atendida\n");
    a.saltar_atras(bucle);

    a.atar(al_final);
    a.imprimir("cierro\n");
    a.salir();
    a.c
}

/// El CLIENTE: busca la ventanilla, llama y cuenta lo que le respondieron.
fn cliente() -> Vec<u8> {
    let mut a = Asm::new();

    // El servidor puede no haber creado el endpoint todavia: el planificador
    // es round-robin y no hay garantia de orden. Reintentar cediendo el turno
    // es la forma honesta de esperarle sin quemar CPU.
    let reintentar = a.aqui();
    a.invoke_task(OP_ENDPOINT_CONNECT, 0);
    a.test_eax();
    let conectado = a.jmp_placeholder(Some(0x84)); // jz: code == 0 -> listo
    a.mov_imm64(R::Rdi, CURRENT_TASK);
    a.mov_imm32(R::Rsi, OP_YIELD);
    a.mov_imm32(R::Rax, NR_INVOKE);
    a.syscall();
    a.saltar_atras(reintentar);

    a.atar(conectado);
    a.mov_reg(R::Rbx, R::Rdx); // rbx = handle del endpoint

    a.imprimir("llamando\n");

    // INVOKE(endpoint, op=1, arg0=7). AQUI SE BLOQUEA.
    a.mov_reg(R::Rdi, R::Rbx);
    a.mov_imm32(R::Rsi, 1);
    a.mov_imm64(R::Rdx, 7);
    a.mov_imm32(R::Rax, NR_INVOKE);
    a.syscall();

    a.test_eax();
    let fallo = a.jmp_placeholder(Some(0x85)); // jnz: code != 0
    a.imprimir("respondido, viaje completo\n");
    let fin = a.jmp_placeholder(None);

    a.atar(fallo);
    a.imprimir("la llamada fallo\n");

    a.atar(fin);
    a.salir();
    a.c
}

fn escribir(nombre: &str, code: Vec<u8>, destino: &PathBuf) {
    let n = code.len();
    let mut b = BefBuilder::new();
    b.add_section(BefSection::code(code));
    let bytes = b.build().expect("construyendo el BEF");
    std::fs::write(destino, &bytes).expect("escribiendo el .bex");
    println!("  {:<10} {:>5} B de codigo  ->  {} ({} B)", nombre, n, destino.display(), bytes.len());
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        eprintln!("uso: rpc-demo <servidor.bex> <cliente.bex>");
        std::process::exit(2);
    }
    println!("== rpc-demo ==");
    escribir("servidor", servidor(), &PathBuf::from(&args[0]));
    escribir("cliente", cliente(), &PathBuf::from(&args[1]));
}

//! Genera el programa Ring 3 que **reclama la pantalla y la pinta**.
//!
//! Es la prueba de que `KIND_FRAMEBUFFER` hace lo que dice. Un mapeo que
//! compila no demuestra nada; lo que hay que ver es que un proceso Ring 3
//! escribe píxeles con `mov` —sin un solo syscall por píxel— y que el kernel
//! se calla mientras tanto.
//!
//! El programa hace exactamente esto:
//!
//! 1. `FRAMEBUFFER_CLAIM` → handle. Con él, la pantalla mapeada en su espacio.
//! 2. Pregunta base, tamaño y stride: cuatro llamadas, una vez, al arrancar.
//! 3. `rep stosd` sobre toda la pantalla — fondo.
//! 4. `rep stosd` sobre las primeras filas — barra superior.
//! 5. **No sale.** Cede el turno para siempre.
//!
//! El punto 5 no es pereza: si saliera, `revoke_all` le quitaría la pantalla,
//! el kernel la recuperaría y repintaría su panel encima. Un escritorio es un
//! proceso que VIVE. Y de paso es la prueba de estrés honesta del cambio de
//! contexto — el proceso entra y sale del CPU miles de veces sin morirse.
//!
//! ## Por qué a mano y no en BMO C
//!
//! Por lo mismo que el par de RPC: hace falta `rep stosd` y aritmética sobre
//! el valor devuelto por un syscall. Son treinta instrucciones. Cuando el
//! frontend de C sepa expresarlo, se reescribe y este generador se jubila.

use std::path::PathBuf;

use bmo_abi::bef::writer::{BefBuilder, BefSection};

const CURRENT_TASK: u64 = 0xFFFF_FFFF_FFFF_FFFE;

const NR_INVOKE: u32 = 0;

const OP_YIELD: u32 = 3;
const OP_EXIT: u32 = 4;
const OP_CONSOLE_WRITE: u32 = 6;
const OP_FRAMEBUFFER_CLAIM: u32 = 9;

// Operaciones sobre el handle de la pantalla (espejo de ring0/fb.rs).
const FB_OP_BASE: u32 = 1;
const FB_OP_STRIDE: u32 = 3;
const FB_OP_BYTES: u32 = 4;

/// Alto de la barra superior, en filas.
const BARRA_FILAS: u32 = 44;

// XRGB-8888. El formato real lo dice `FB_OP_STRIDE`, pero en esta máquina el
// GOP entrega BGR y estos dos colores se ven igual de bien en ambos: son
// grises azulados, no primarios.
const COLOR_FONDO: u32 = 0x0014_1C2B;
const COLOR_BARRA: u32 = 0x0028_3448;

#[derive(Clone, Copy, PartialEq)]
#[allow(dead_code)]
enum R {
    Rax = 0,
    Rcx = 1,
    Rdx = 2,
    Rbx = 3,
    Rsp = 4,
    Rbp = 5,
    Rsi = 6,
    Rdi = 7,
}

struct Asm {
    c: Vec<u8>,
}

impl Asm {
    fn new() -> Self {
        Self { c: Vec::new() }
    }

    /// `mov <r64>, imm64` — REX.W B8+rd
    fn mov_imm64(&mut self, r: R, v: u64) {
        self.c.push(0x48);
        self.c.push(0xB8 + r as u8);
        self.c.extend_from_slice(&v.to_le_bytes());
    }

    /// `mov <e32>, imm32` — pone a cero la mitad alta.
    fn mov_imm32(&mut self, r: R, v: u32) {
        self.c.push(0xB8 + r as u8);
        self.c.extend_from_slice(&v.to_le_bytes());
    }

    /// `mov dst, src` (64 bits) — REX.W 89 /r
    fn mov_reg(&mut self, dst: R, src: R) {
        self.c.push(0x48);
        self.c.push(0x89);
        self.c.push(0xC0 | ((src as u8) << 3) | dst as u8);
    }

    /// `shr <r64>, imm8` — REX.W C1 /5 ib
    fn shr_imm(&mut self, r: R, n: u8) {
        self.c.push(0x48);
        self.c.push(0xC1);
        self.c.push(0xE8 | r as u8);
        self.c.push(n);
    }

    /// `imul <r64>, <r64>, imm32` — REX.W 69 /r id
    fn imul_imm(&mut self, dst: R, src: R, v: u32) {
        self.c.push(0x48);
        self.c.push(0x69);
        self.c.push(0xC0 | ((dst as u8) << 3) | src as u8);
        self.c.extend_from_slice(&v.to_le_bytes());
    }

    /// `rep stosd` — escribe `ecx` dwords de `eax` a partir de `rdi`.
    ///
    /// Aquí está el asunto entero: **esto no es un syscall**. Son dos bytes
    /// que llenan la pantalla porque la pantalla ES memoria del proceso. DF
    /// vale 0 porque el contexto Ring 3 arranca con RFLAGS = IF|0x2.
    fn rep_stosd(&mut self) {
        self.c.extend_from_slice(&[0xF3, 0xAB]);
    }

    fn syscall(&mut self) {
        self.c.extend_from_slice(&[0x0F, 0x05]);
    }

    fn test_eax(&mut self) {
        self.c.extend_from_slice(&[0x85, 0xC0]);
    }

    fn jmp_placeholder(&mut self, cond: Option<u8>) -> usize {
        match cond {
            Some(cc) => {
                self.c.push(0x0F);
                self.c.push(cc);
            }
            None => self.c.push(0xE9),
        }
        let at = self.c.len();
        self.c.extend_from_slice(&[0, 0, 0, 0]);
        at
    }

    /// Ata un salto pendiente a la posición actual. El desplazamiento es
    /// relativo al FINAL de la instrucción — equivocarse aquí es el bug que
    /// este proyecto ya se comió tres veces.
    fn atar(&mut self, at: usize) {
        let destino = self.c.len() as i32;
        let desde = (at + 4) as i32;
        let rel = destino - desde;
        self.c[at..at + 4].copy_from_slice(&rel.to_le_bytes());
    }

    fn aqui(&self) -> usize {
        self.c.len()
    }

    fn saltar_atras(&mut self, destino: usize) {
        self.c.push(0xE9);
        let desde = (self.c.len() + 4) as i32;
        let rel = destino as i32 - desde;
        self.c.extend_from_slice(&rel.to_le_bytes());
    }

    // ── Envoltorios de la superficie ──

    fn invoke_task(&mut self, op: u32, arg0: u64) {
        self.mov_imm64(R::Rdi, CURRENT_TASK);
        self.mov_imm32(R::Rsi, op);
        self.mov_imm64(R::Rdx, arg0);
        self.mov_imm32(R::Rax, NR_INVOKE);
        self.syscall();
    }

    /// `INVOKE(<handle en rbx>, op)` — pregunta a la pantalla.
    fn invoke_fb(&mut self, op: u32) {
        self.mov_reg(R::Rdi, R::Rbx);
        self.mov_imm32(R::Rsi, op);
        self.mov_imm32(R::Rax, NR_INVOKE);
        self.syscall();
    }

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

fn escritorio() -> Vec<u8> {
    let mut a = Asm::new();

    // ── 1. Reclamar la pantalla ──
    //
    // El aviso va ANTES de reclamar, y no es cosmético: en cuanto la cesión se
    // consuma, el kernel deja de dibujar y nada de lo que se imprima después
    // llega al panel. Ésta es la última línea que se ve, así que tiene que
    // decir qué está a punto de pasar.
    a.imprimir("reclamo la pantalla\n");

    a.invoke_task(OP_FRAMEBUFFER_CLAIM, 0);
    a.test_eax();
    let sin_pantalla = a.jmp_placeholder(Some(0x85)); // jnz: code != 0
    a.mov_reg(R::Rbx, R::Rdx); // rbx = handle. Sobrevive a los syscall.

    // ── 2. Geometría: base y bytes totales ──
    // rbp = base (callee-saved por convención, pero aquí lo que importa es que
    // ningún syscall lo pisa: el kernel restaura los 15 GPR del frame).
    a.invoke_fb(FB_OP_BASE);
    a.mov_reg(R::Rbp, R::Rdx);
    a.invoke_fb(FB_OP_BYTES);
    a.mov_reg(R::Rsi, R::Rdx); // rsi = bytes

    // ── 3. Fondo: toda la pantalla de una pasada ──
    a.mov_reg(R::Rdi, R::Rbp);
    a.mov_reg(R::Rcx, R::Rsi);
    a.shr_imm(R::Rcx, 2); // bytes -> dwords
    a.mov_imm32(R::Rax, COLOR_FONDO);
    a.rep_stosd();

    // ── 4. Barra superior: stride * BARRA_FILAS píxeles desde la base ──
    a.invoke_fb(FB_OP_STRIDE);
    a.shr_imm(R::Rdx, 32); // la mitad alta es el stride en píxeles
    a.imul_imm(R::Rcx, R::Rdx, BARRA_FILAS);
    a.mov_reg(R::Rdi, R::Rbp);
    a.mov_imm32(R::Rax, COLOR_BARRA);
    a.rep_stosd();

    a.imprimir("escritorio pintado\n");

    // ── 5. Vivir. Un escritorio no termina. ──
    //
    // Si saliera, `revoke_all` le quitaria la pantalla y el kernel repintaria
    // su panel encima: no se veria nada. Ceder el turno en bucle mantiene el
    // escritorio en pantalla Y ejerce el cambio de contexto miles de veces,
    // que es justo la prueba que faltaba.
    let vivir = a.aqui();
    a.mov_imm64(R::Rdi, CURRENT_TASK);
    a.mov_imm32(R::Rsi, OP_YIELD);
    a.mov_imm32(R::Rax, NR_INVOKE);
    a.syscall();
    a.saltar_atras(vivir);

    a.atar(sin_pantalla);
    a.imprimir("sin pantalla que reclamar\n");
    a.salir();
    a.c
}

fn escribir(nombre: &str, code: Vec<u8>, destino: &PathBuf) {
    let n = code.len();
    let mut b = BefBuilder::new();
    b.add_section(BefSection::code(code));
    let bytes = b.build().expect("construyendo el BEF");
    std::fs::write(destino, &bytes).expect("escribiendo el .bex");
    println!(
        "  {:<12} {:>5} B de codigo  ->  {} ({} B)",
        nombre,
        n,
        destino.display(),
        bytes.len()
    );
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        eprintln!("uso: fb-demo <escritorio.bex>");
        std::process::exit(2);
    }
    println!("== fb-demo ==");
    escribir("escritorio", escritorio(), &PathBuf::from(&args[0]));
}

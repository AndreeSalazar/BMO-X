//! Emulador x86-64 mínimo — **solo para tests**.
//!
//! # Por qué existe
//!
//! Un emisor de código máquina que solo se testea comparando bytes contra
//! bytes escritos a mano no prueba nada: si el autor entendió mal una
//! codificación, el test la repite y pasa igual de mal. Y aquí el costo de
//! equivocarse no es un `assert` rojo, es flashear un USB, arrancar el
//! Ryzen y mirar una pantalla negra sin saber por qué.
//!
//! Así que en vez de comparar bytes, este módulo los **ejecuta**: decodifica
//! exactamente las formas que emite `x86.rs`, corre el bucle, y modela la
//! puerta del kernel (`uconsole::write_packed`: 8 bytes LE, NUL-stop) para
//! reconstruir el texto que aparecería en pantalla. El test compara ese
//! texto con el original. Si una codificación está mal, el emulador se
//! atraganta o el texto sale distinto.
//!
//! Modela también dos cosas que el silicio hace y es fácil olvidar:
//! - `syscall` destruye `rcx` y `r11` → aquí se llenan de veneno, para que
//!   cualquier código que dependa de ellos falle en el test y no en el metal.
//! - Escribir un registro de 32 bits pone a cero la mitad alta del de 64.
//!
//! No pretende ser un emulador general: si aparece un opcode que `x86.rs` no
//! emite, hace panic con el byte, que es la respuesta correcta.

const DATA_BASE: u64 = 0x1_0000;
const POISON: u64 = 0xDEAD_BEEF_DEAD_BEEF;

pub struct Machine {
    pub regs: [u64; 16],
    pub code: Vec<u8>,
    pub rip: usize,
    /// Texto que el kernel habría pintado.
    pub console: String,
    /// Cuántas veces se cruzó CPL3→CPL0.
    pub syscalls: usize,
    data: Vec<u8>,
    zf: bool,
    cf: bool,
}

impl Machine {
    pub fn new(code: Vec<u8>) -> Self {
        Self {
            regs: [0; 16],
            code,
            rip: 0,
            console: String::new(),
            syscalls: 0,
            data: Vec::new(),
            zf: false,
            cf: false,
        }
    }

    /// Coloca bytes en memoria y devuelve su dirección.
    pub fn load_data(&mut self, bytes: &[u8]) -> u64 {
        let addr = DATA_BASE + self.data.len() as u64;
        self.data.extend_from_slice(bytes);
        addr
    }

    fn read_u8(&self, addr: u64) -> u8 {
        let idx = addr
            .checked_sub(DATA_BASE)
            .expect("lectura fuera del área de datos") as usize;
        *self
            .data
            .get(idx)
            .unwrap_or_else(|| panic!("lectura fuera de rango en {addr:#x}"))
    }

    fn fetch_u8(&mut self) -> u8 {
        let b = self.code[self.rip];
        self.rip += 1;
        b
    }

    fn fetch_u32(&mut self) -> u32 {
        let mut buf = [0u8; 4];
        buf.copy_from_slice(&self.code[self.rip..self.rip + 4]);
        self.rip += 4;
        u32::from_le_bytes(buf)
    }

    fn fetch_u64(&mut self) -> u64 {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&self.code[self.rip..self.rip + 8]);
        self.rip += 8;
        u64::from_le_bytes(buf)
    }

    /// Escribe respetando el ancho: 32 bits pone a cero la mitad alta.
    fn write_reg(&mut self, reg: usize, value: u64, wide: bool) {
        self.regs[reg] = if wide { value } else { value as u32 as u64 };
    }

    fn read_reg(&self, reg: usize, wide: bool) -> u64 {
        if wide {
            self.regs[reg]
        } else {
            self.regs[reg] as u32 as u64
        }
    }

    /// La puerta del kernel, modelada: `uconsole::write_packed`.
    fn do_syscall(&mut self) {
        use bmo_abi::syscalls::surface::{CURRENT_TASK, NR_INVOKE, TASK_OP_CONSOLE_WRITE};

        assert_eq!(self.regs[0], NR_INVOKE as u64, "rax debe ser NR_INVOKE");
        assert_eq!(self.regs[7], CURRENT_TASK, "rdi debe ser CURRENT_TASK");
        assert_eq!(
            self.regs[6], TASK_OP_CONSOLE_WRITE,
            "rsi debe ser CONSOLE_WRITE"
        );

        let packed = self.regs[2];
        for i in 0..8 {
            let b = ((packed >> (i * 8)) & 0xFF) as u8;
            if b == 0 {
                break; // NUL-stop: idéntico al kernel
            }
            self.console.push(b as char);
        }
        self.syscalls += 1;

        // El silicio destruye estos dos. Envenenarlos convierte "funcionó de
        // milagro" en un test rojo.
        self.regs[1] = POISON; // rcx
        self.regs[11] = POISON; // r11
        self.regs[0] = 0; // rax = estado devuelto
    }

    fn step(&mut self) {
        let mut byte = self.fetch_u8();
        let mut rex = 0u8;
        if (0x40..=0x4F).contains(&byte) {
            rex = byte;
            byte = self.fetch_u8();
        }
        let wide = rex & 0x08 != 0;
        let rex_r = ((rex >> 2) & 1) as usize;
        let rex_x = ((rex >> 1) & 1) as usize;
        let rex_b = (rex & 1) as usize;

        match byte {
            // mov <reg>, imm
            0xB8..=0xBF => {
                let reg = ((byte & 7) as usize) | (rex_b << 3);
                let imm = if wide {
                    self.fetch_u64()
                } else {
                    self.fetch_u32() as u64
                };
                self.write_reg(reg, imm, wide);
            }
            // ALU r/m, r  (mod=11 siempre en lo que emitimos)
            0x89 | 0x09 | 0x01 | 0x29 | 0x85 | 0x31 => {
                let modrm = self.fetch_u8();
                assert_eq!(modrm & 0xC0, 0xC0, "solo se emite mod=11 aquí");
                let reg = (((modrm >> 3) & 7) as usize) | (rex_r << 3);
                let rm = ((modrm & 7) as usize) | (rex_b << 3);
                let a = self.read_reg(rm, wide);
                let b = self.read_reg(reg, wide);
                match byte {
                    0x89 => self.write_reg(rm, b, wide), // mov
                    0x09 => {
                        let r = a | b;
                        self.zf = r == 0;
                        self.write_reg(rm, r, wide);
                    }
                    0x01 => {
                        let r = a.wrapping_add(b);
                        self.zf = r == 0;
                        self.write_reg(rm, r, wide);
                    }
                    0x29 => {
                        let r = a.wrapping_sub(b);
                        self.zf = r == 0;
                        self.cf = a < b;
                        self.write_reg(rm, r, wide);
                    }
                    0x85 => {
                        self.zf = (a & b) == 0;
                        self.cf = false;
                    }
                    0x31 => {
                        let r = a ^ b;
                        self.zf = r == 0;
                        self.write_reg(rm, r, wide);
                    }
                    _ => unreachable!(),
                }
            }
            // grupo 1 con imm8: /7 = cmp
            0x83 => {
                let modrm = self.fetch_u8();
                let ext = (modrm >> 3) & 7;
                let rm = ((modrm & 7) as usize) | (rex_b << 3);
                let imm = self.fetch_u8() as i8 as i64 as u64;
                let a = self.read_reg(rm, wide);
                assert_eq!(ext, 7, "solo se emite CMP de este grupo");
                let r = a.wrapping_sub(imm);
                self.zf = r == 0;
                self.cf = a < imm;
            }
            // grupo 5 con imm8: /4 = shl
            0xC1 => {
                let modrm = self.fetch_u8();
                let ext = (modrm >> 3) & 7;
                let rm = ((modrm & 7) as usize) | (rex_b << 3);
                let imm = self.fetch_u8();
                assert_eq!(ext, 4, "solo se emite SHL de este grupo");
                let r = self.read_reg(rm, wide) << imm;
                self.zf = r == 0;
                self.write_reg(rm, r, wide);
            }
            // grupo 3: /1 = dec
            0xFF => {
                let modrm = self.fetch_u8();
                let ext = (modrm >> 3) & 7;
                let rm = ((modrm & 7) as usize) | (rex_b << 3);
                assert_eq!(ext, 1, "solo se emite DEC de este grupo");
                let r = self.read_reg(rm, wide).wrapping_sub(1);
                self.zf = r == 0;
                self.write_reg(rm, r, wide);
            }
            0xE9 => {
                let rel = self.fetch_u32() as i32;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            0xEB => {
                let rel = self.fetch_u8() as i8;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            0xF3 => {
                let nop = self.fetch_u8();
                assert_eq!(nop, 0x90, "solo se emite PAUSE con prefijo F3");
            }
            0x0F => {
                let second = self.fetch_u8();
                match second {
                    0x05 => self.do_syscall(),
                    // movzx r32, byte [base + index]
                    0xB6 => {
                        let modrm = self.fetch_u8();
                        assert_eq!(modrm & 0xC0, 0x00, "solo se emite mod=00");
                        assert_eq!(modrm & 7, 0b100, "solo se emite la forma con SIB");
                        let dst = (((modrm >> 3) & 7) as usize) | (rex_r << 3);
                        let sib = self.fetch_u8();
                        let scale = 1u64 << (sib >> 6);
                        let index = (((sib >> 3) & 7) as usize) | (rex_x << 3);
                        let base = ((sib & 7) as usize) | (rex_b << 3);
                        let addr = self.regs[base] + self.regs[index] * scale;
                        let value = self.read_u8(addr) as u64;
                        self.write_reg(dst, value, false);
                    }
                    // jcc rel32
                    0x84 | 0x85 | 0x86 => {
                        let rel = self.fetch_u32() as i32;
                        let taken = match second {
                            0x84 => self.zf,
                            0x85 => !self.zf,
                            0x86 => self.cf || self.zf,
                            _ => unreachable!(),
                        };
                        if taken {
                            self.rip = (self.rip as i64 + rel as i64) as usize;
                        }
                    }
                    other => panic!("opcode 0F {other:#04X} no emitido por x86.rs"),
                }
            }
            other => panic!("opcode {other:#04X} no emitido por x86.rs"),
        }
    }
}

/// Ejecuta hasta caer del final del código o agotar el presupuesto de pasos
/// (un bucle que no termina es un bug, y colgar el test lo esconde).
pub fn run(mut m: Machine, max_steps: usize) -> Machine {
    let mut steps = 0;
    while m.rip < m.code.len() {
        m.step();
        steps += 1;
        assert!(steps < max_steps, "el código emitido no termina");
    }
    m
}

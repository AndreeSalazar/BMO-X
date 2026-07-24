//! Emulador x86-64 mínimo — el banco de pruebas del toolchain.
//!
//! # Por qué existe
//!
//! Un emisor de código máquina que solo se testea comparando bytes contra
//! bytes escritos a mano no prueba nada: si el autor entendió mal una
//! codificación, el test la repite y pasa igual de mal. Peor aún: un
//! `IF` que emite un salto con desplazamiento cero **parece** código
//! correcto en un volcado de bytes, compila, valida el BEF, y ejecuta las
//! dos ramas en hardware. Esa clase de mentira solo la caza la ejecución.
//!
//! Así que este módulo no compara: **ejecuta**. Corre el código emitido y
//! modela la puerta del kernel (`uconsole::write_packed`: 8 bytes LE,
//! NUL-stop) para reconstruir el texto que aparecería en pantalla. El test
//! compara ese texto con lo que el programa debería imprimir.
//!
//! Modela también dos cosas que el silicio hace y es fácil olvidar:
//! - `syscall` destruye `rcx` y `r11` → aquí se llenan de veneno, para que
//!   cualquier código que dependa de ellos falle en el test y no en el metal.
//! - Escribir un registro de 32 bits pone a cero la mitad alta del de 64.
//!
//! # Alcance
//!
//! Cubre el subconjunto que emiten los frontends de BMO: movimientos,
//! aritmética entera con signo, `imul`/`idiv`, pila, direccionamiento
//! `[rbp+disp]` y `[rsp]`, comparaciones, saltos condicionales e
//! incondicionales, y `syscall`. **No** es un emulador general: ante un
//! opcode que ningún emisor de BMO produce hace panic con el byte, que es
//! la respuesta correcta — significa que alguien emitió algo sin pensar en
//! cómo lo iba a verificar.
//!
//! Se activa con la feature `emulator` para que no viaje en las builds
//! normales del toolchain.

use std::collections::HashMap;

/// Dirección base del área de datos que carga el test.
pub const DATA_BASE: u64 = 0x1_0000;
/// Tope de pila inicial. Alineado a 64 como pide el contrato de BMO.
pub const STACK_TOP: u64 = 0x7000_0000;

const POISON: u64 = 0xDEAD_BEEF_DEAD_BEEF;

const RAX: usize = 0;
const RCX: usize = 1;
const RDX: usize = 2;
const RSP: usize = 4;
const RSI: usize = 6;
const RDI: usize = 7;
const R11: usize = 11;

/// Una llamada observada cruzando CPL3→CPL0.
#[derive(Debug, Clone, Copy)]
pub struct ObservedSyscall {
    pub nr: u64,
    pub capability: u64,
    pub operation: u64,
    pub arg0: u64,
}

pub struct Machine {
    pub regs: [u64; 16],
    pub code: Vec<u8>,
    pub rip: usize,
    /// Texto que el kernel habría pintado.
    pub console: String,
    /// Toda llamada observada, en orden.
    pub syscalls: Vec<ObservedSyscall>,
    /// True cuando el programa invocó `TASK_OP_EXIT`.
    pub exited: bool,
    mem: HashMap<u64, u8>,
    data_len: u64,
    zf: bool,
    sf: bool,
    of: bool,
    cf: bool,
}

impl Machine {
    pub fn new(code: Vec<u8>) -> Self {
        let mut m = Self {
            regs: [0; 16],
            code,
            rip: 0,
            console: String::new(),
            syscalls: Vec::new(),
            exited: false,
            mem: HashMap::new(),
            data_len: 0,
            zf: false,
            sf: false,
            of: false,
            cf: false,
        };
        m.regs[RSP] = STACK_TOP;
        m
    }

    /// Coloca bytes en memoria y devuelve su dirección.
    pub fn load_data(&mut self, bytes: &[u8]) -> u64 {
        let addr = DATA_BASE + self.data_len;
        for (i, b) in bytes.iter().enumerate() {
            self.mem.insert(addr + i as u64, *b);
        }
        self.data_len += bytes.len() as u64;
        addr
    }

    /// Lee 8 bytes de memoria.
    pub fn read_u64(&self, addr: u64) -> u64 {
        let mut v = 0u64;
        for i in 0..8 {
            v |= (self.read_u8_mem(addr + i) as u64) << (i * 8);
        }
        v
    }

    fn write_u64(&mut self, addr: u64, value: u64) {
        for i in 0..8 {
            self.mem.insert(addr + i, ((value >> (i * 8)) & 0xFF) as u8);
        }
    }

    /// Lee un byte de memoria.
    ///
    /// Si nadie escribió ahí, cae a la propia imagen: los frontends colocan
    /// las cadenas y los globales DENTRO de la sección de código, justo
    /// detrás de las instrucciones, y los alcanzan con `lea [rip+disp]`. Un
    /// `%s` leería ceros si el emulador no modelara eso. Fuera de la imagen
    /// devuelve cero, que es lo que hace el kernel con una página nueva.
    fn read_u8_mem(&self, addr: u64) -> u8 {
        if let Some(b) = self.mem.get(&addr) {
            return *b;
        }
        self.code.get(addr as usize).copied().unwrap_or(0)
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

    fn push(&mut self, value: u64) {
        self.regs[RSP] = self.regs[RSP].wrapping_sub(8);
        let sp = self.regs[RSP];
        self.write_u64(sp, value);
    }

    fn pop(&mut self) -> u64 {
        let sp = self.regs[RSP];
        let v = self.read_u64(sp);
        self.regs[RSP] = sp.wrapping_add(8);
        v
    }

    /// Flags de una resta `a - b`, que es lo que produce `cmp`.
    fn flags_sub(&mut self, a: u64, b: u64) {
        let r = a.wrapping_sub(b);
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = a < b;
        // Overflow con signo: los operandos difieren en signo y el
        // resultado toma el del sustraendo.
        self.of = ((a ^ b) & (a ^ r)) >> 63 != 0;
    }

    fn flags_logic(&mut self, r: u64) {
        self.zf = r == 0;
        self.sf = (r as i64) < 0;
        self.cf = false;
        self.of = false;
    }

    /// La puerta del kernel, modelada.
    fn do_syscall(&mut self) {
        use bmo_abi::syscalls::surface::{
            CURRENT_TASK, NR_INVOKE, TASK_OP_CONSOLE_WRITE, TASK_OP_EXIT,
        };

        let call = ObservedSyscall {
            nr: self.regs[RAX],
            capability: self.regs[RDI],
            operation: self.regs[RSI],
            arg0: self.regs[RDX],
        };
        self.syscalls.push(call);

        assert_eq!(
            call.nr, NR_INVOKE as u64,
            "solo INVOKE cruza esta puerta (rax={:#x})",
            call.nr
        );

        if call.capability == CURRENT_TASK {
            match call.operation {
                op if op == TASK_OP_CONSOLE_WRITE => {
                    for i in 0..8 {
                        let b = ((call.arg0 >> (i * 8)) & 0xFF) as u8;
                        if b == 0 {
                            break; // NUL-stop: idéntico al kernel
                        }
                        self.console.push(b as char);
                    }
                }
                op if op == TASK_OP_EXIT => self.exited = true,
                _ => {}
            }
        }

        // El silicio destruye estos dos.
        self.regs[RCX] = POISON;
        self.regs[R11] = POISON;
        self.regs[RAX] = 0;
    }

    /// Decodifica un ModRM y devuelve `(reg, destino)`.
    fn modrm(&mut self, rex_r: usize, rex_x: usize, rex_b: usize) -> (usize, Operand) {
        let modrm = self.fetch_u8();
        let md = modrm >> 6;
        let reg = (((modrm >> 3) & 7) as usize) | (rex_r << 3);
        let rm = (modrm & 7) as usize;

        if md == 3 {
            return (reg, Operand::Reg(rm | (rex_b << 3)));
        }

        // mod=00 con rm=101 NO es "[rbp]": en 64 bits es direccionamiento
        // RELATIVO A RIP con disp32. Es como los frontends alcanzan sus
        // cadenas y variables globales (`lea rax, [rip+disp]`), así que sin
        // esto el emulador se comía los 4 bytes del desplazamiento como si
        // fueran instrucciones y descarrilaba.
        if md == 0 && rm == 0b101 {
            let disp = self.fetch_u32() as i32 as i64;
            let addr = (self.rip as i64 + disp) as u64;
            return (reg, Operand::Mem(addr));
        }

        // Base (+ índice si hay SIB).
        let (base, index, scale) = if rm == 0b100 {
            let sib = self.fetch_u8();
            let idx = (((sib >> 3) & 7) as usize) | (rex_x << 3);
            let base = ((sib & 7) as usize) | (rex_b << 3);
            // índice 4 sin REX.X significa "sin índice".
            let idx = if idx == 4 { None } else { Some(idx) };
            (base, idx, 1u64 << (sib >> 6))
        } else {
            (rm | (rex_b << 3), None, 1)
        };

        let disp = match md {
            0 => 0i64,
            1 => self.fetch_u8() as i8 as i64,
            2 => self.fetch_u32() as i32 as i64,
            _ => unreachable!(),
        };

        let mut addr = (self.regs[base] as i64 + disp) as u64;
        if let Some(i) = index {
            addr = addr.wrapping_add(self.regs[i].wrapping_mul(scale));
        }
        (reg, Operand::Mem(addr))
    }

    fn load(&self, op: Operand, wide: bool) -> u64 {
        match op {
            Operand::Reg(r) => self.read_reg(r, wide),
            Operand::Mem(a) => {
                let v = self.read_u64(a);
                if wide {
                    v
                } else {
                    v as u32 as u64
                }
            }
        }
    }

    /// Lee un solo byte del operando. En registro es el byte BAJO — con
    /// REX presente `dl`/`sil` son eso y no los registros altos heredados.
    fn load_u8(&self, op: Operand) -> u64 {
        match op {
            Operand::Reg(r) => self.regs[r] & 0xFF,
            Operand::Mem(a) => self.read_u8_mem(a) as u64,
        }
    }

    fn store_u8(&mut self, op: Operand, value: u64) {
        match op {
            Operand::Reg(r) => self.regs[r] = (self.regs[r] & !0xFF) | (value & 0xFF),
            Operand::Mem(a) => {
                self.mem.insert(a, (value & 0xFF) as u8);
            }
        }
    }

    fn store(&mut self, op: Operand, value: u64, wide: bool) {
        match op {
            Operand::Reg(r) => self.write_reg(r, value, wide),
            Operand::Mem(a) => {
                if wide {
                    self.write_u64(a, value);
                } else {
                    self.write_u64(a, value as u32 as u64);
                }
            }
        }
    }

    fn step(&mut self) {
        let mut byte = self.fetch_u8();
        let mut rex = 0u8;
        // Prefijos que emitimos: F3 (pause) se trata aparte más abajo.
        if (0x40..=0x4F).contains(&byte) {
            rex = byte;
            byte = self.fetch_u8();
        }
        let wide = rex & 0x08 != 0;
        let rex_r = ((rex >> 2) & 1) as usize;
        let rex_x = ((rex >> 1) & 1) as usize;
        let rex_b = (rex & 1) as usize;

        match byte {
            // push <reg> / pop <reg>
            0x50..=0x57 => {
                let r = ((byte & 7) as usize) | (rex_b << 3);
                let v = self.regs[r];
                self.push(v);
            }
            0x58..=0x5F => {
                let r = ((byte & 7) as usize) | (rex_b << 3);
                let v = self.pop();
                self.regs[r] = v;
            }
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
            // ALU  r/m, reg
            0x89 | 0x09 | 0x01 | 0x29 | 0x85 | 0x31 | 0x39 | 0x21 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.load(dst, wide);
                let b = self.read_reg(reg, wide);
                match byte {
                    0x89 => self.store(dst, b, wide),
                    0x09 => {
                        let r = a | b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x21 => {
                        let r = a & b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x31 => {
                        let r = a ^ b;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x01 => {
                        let r = a.wrapping_add(b);
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    0x29 => {
                        self.flags_sub(a, b);
                        let r = a.wrapping_sub(b);
                        self.store(dst, r, wide);
                    }
                    0x39 => self.flags_sub(a, b), // cmp
                    0x85 => self.flags_logic(a & b), // test
                    _ => unreachable!(),
                }
            }
            // ALU  reg, r/m  (dirección contraria)
            0x8B | 0x0B | 0x03 | 0x2B | 0x3B => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let a = self.read_reg(reg, wide);
                let b = self.load(src, wide);
                match byte {
                    0x8B => self.write_reg(reg, b, wide),
                    0x0B => {
                        let r = a | b;
                        self.flags_logic(r);
                        self.write_reg(reg, r, wide);
                    }
                    0x03 => {
                        let r = a.wrapping_add(b);
                        self.flags_logic(r);
                        self.write_reg(reg, r, wide);
                    }
                    0x2B => {
                        self.flags_sub(a, b);
                        let r = a.wrapping_sub(b);
                        self.write_reg(reg, r, wide);
                    }
                    0x3B => self.flags_sub(a, b),
                    _ => unreachable!(),
                }
            }
            // movsxd reg64, r/m32 — carga un int CON SIGNO
            0x63 => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.load(src, false) as u32 as i32 as i64 as u64;
                self.write_reg(reg, v, true);
            }
            // lea reg, [mem]
            0x8D => {
                let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                match src {
                    Operand::Mem(a) => self.write_reg(reg, a, wide),
                    Operand::Reg(_) => panic!("lea con operando registro es inválido"),
                }
            }
            // grupo 1 con imm8: /0 add, /5 sub, /7 cmp, /4 and
            0x83 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as i8 as i64 as u64;
                let a = self.load(dst, wide);
                match ext & 7 {
                    0 => {
                        let r = a.wrapping_add(imm);
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    4 => {
                        let r = a & imm;
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, wide);
                    }
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 83 /{other} no emitido por BMO"),
                }
            }
            // grupo 1 con imm32
            0x81 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u32() as i32 as i64 as u64;
                let a = self.load(dst, wide);
                match ext & 7 {
                    0 => {
                        let r = a.wrapping_add(imm);
                        self.flags_logic(r);
                        self.store(dst, r, wide);
                    }
                    5 => {
                        self.flags_sub(a, imm);
                        let r = a.wrapping_sub(imm);
                        self.store(dst, r, wide);
                    }
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 81 /{other} no emitido por BMO"),
                }
            }
            // mov r/m, imm32
            0xC7 => {
                let (_, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u32() as i32 as i64 as u64;
                self.store(dst, imm, wide);
            }
            // desplazamientos con imm8: /4 shl, /5 shr, /7 sar
            0xC1 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u32;
                let a = self.load(dst, wide);
                let r = match ext & 7 {
                    4 => a << imm,
                    5 => a >> imm,
                    7 => ((a as i64) >> imm) as u64,
                    other => panic!("grupo C1 /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, wide);
            }
            // mov r/m8, r8  — guarda el byte bajo de un registro
            0x88 => {
                let (reg, dst) = self.modrm(rex_r, rex_x, rex_b);
                let v = self.regs[reg] & 0xFF;
                self.store_u8(dst, v);
            }
            // mov r/m8, imm8
            0xC6 => {
                let (_, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u64;
                self.store_u8(dst, imm);
            }
            // grupo 1 sobre BYTE con imm8: /7 cmp
            0x80 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let imm = self.fetch_u8() as u64;
                let a = self.load_u8(dst);
                match ext & 7 {
                    7 => self.flags_sub(a, imm),
                    other => panic!("grupo 80 /{other} no emitido por BMO"),
                }
            }
            // desplazamientos por `cl`: /4 shl, /5 shr, /7 sar
            0xD3 => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let count = (self.regs[RCX] & 0x3F) as u32; // el CPU enmascara a 6 bits
                let a = self.load(dst, wide);
                let r = match ext & 7 {
                    4 => a << count,
                    5 => a >> count,
                    7 => ((a as i64) >> count) as u64,
                    other => panic!("grupo D3 /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, wide);
            }
            // grupo 3: /3 neg, /6 div, /7 idiv
            0xF7 => {
                let (ext, src) = self.modrm(0, rex_x, rex_b);
                let v = self.load(src, wide);
                match ext & 7 {
                    3 => {
                        let r = (self.load(src, wide) as i64).wrapping_neg() as u64;
                        self.flags_logic(r);
                        self.store(src, r, wide);
                    }
                    // div SIN signo: rdx:rax entre el operando. El emisor
                    // siempre pone rdx=0 antes, así que basta con rax.
                    6 => {
                        assert_ne!(v, 0, "division por cero en el codigo emitido");
                        assert_eq!(
                            self.regs[RDX], 0,
                            "div de 128 bits: el emisor debe poner rdx=0 antes"
                        );
                        let dividend = self.regs[RAX];
                        self.regs[RAX] = dividend / v;
                        self.regs[RDX] = dividend % v;
                    }
                    7 => {
                        // idiv: dividendo en rdx:rax; aquí basta rax con signo
                        // extendido por cqo, que es lo único que emitimos.
                        let divisor = v as i64;
                        assert_ne!(divisor, 0, "division por cero en el codigo emitido");
                        let dividend = self.regs[RAX] as i64;
                        self.regs[RAX] = dividend.wrapping_div(divisor) as u64;
                        self.regs[RDX] = dividend.wrapping_rem(divisor) as u64;
                    }
                    other => panic!("grupo F7 /{other} no emitido por BMO"),
                }
            }
            // grupo 5: /0 inc, /1 dec
            0xFF => {
                let (ext, dst) = self.modrm(0, rex_x, rex_b);
                let a = self.load(dst, wide);
                let r = match ext & 7 {
                    0 => a.wrapping_add(1),
                    1 => a.wrapping_sub(1),
                    other => panic!("grupo FF /{other} no emitido por BMO"),
                };
                self.flags_logic(r);
                self.store(dst, r, wide);
            }
            // cqo — extiende el signo de rax a rdx
            0x99 => {
                self.regs[RDX] = if (self.regs[RAX] as i64) < 0 {
                    u64::MAX
                } else {
                    0
                };
            }
            0x90 => {} // nop
            0xE9 => {
                let rel = self.fetch_u32() as i32;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            0xEB => {
                let rel = self.fetch_u8() as i8;
                self.rip = (self.rip as i64 + rel as i64) as usize;
            }
            // jcc rel8
            0x70..=0x7F => {
                let rel = self.fetch_u8() as i8;
                if self.cond(byte & 0x0F) {
                    self.rip = (self.rip as i64 + rel as i64) as usize;
                }
            }
            0xF3 => {
                let nop = self.fetch_u8();
                assert_eq!(nop, 0x90, "solo se emite PAUSE con prefijo F3");
            }
            0x0F => {
                let second = self.fetch_u8();
                match second {
                    0x05 => self.do_syscall(),
                    // movzx reg, r/m8
                    0xB6 => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let v = match src {
                            Operand::Mem(a) => self.read_u8_mem(a) as u64,
                            Operand::Reg(r) => self.regs[r] & 0xFF,
                        };
                        self.write_reg(reg, v, false);
                    }
                    // imul reg, r/m
                    0xAF => {
                        let (reg, src) = self.modrm(rex_r, rex_x, rex_b);
                        let a = self.read_reg(reg, wide) as i64;
                        let b = self.load(src, wide) as i64;
                        let r = a.wrapping_mul(b) as u64;
                        self.write_reg(reg, r, wide);
                    }
                    // setcc r/m8 — deja 0 o 1 según la condición
                    0x90..=0x9F => {
                        let (_, dst) = self.modrm(0, rex_x, rex_b);
                        let value = u64::from(self.cond(second & 0x0F));
                        self.store_u8(dst, value);
                    }
                    // jcc rel32
                    0x80..=0x8F => {
                        let rel = self.fetch_u32() as i32;
                        if self.cond(second & 0x0F) {
                            self.rip = (self.rip as i64 + rel as i64) as usize;
                        }
                    }
                    other => panic!("opcode 0F {other:#04X} no emitido por BMO"),
                }
            }
            other => panic!("opcode {other:#04X} no emitido por BMO"),
        }
    }

    /// Evalúa el código de condición de un `jcc` (el nibble bajo del opcode).
    fn cond(&self, cc: u8) -> bool {
        match cc {
            0x0 => self.of,
            0x1 => !self.of,
            0x2 => self.cf,
            0x3 => !self.cf,
            0x4 => self.zf,
            0x5 => !self.zf,
            0x6 => self.cf || self.zf,
            0x7 => !self.cf && !self.zf,
            0x8 => self.sf,
            0x9 => !self.sf,
            0xC => self.sf != self.of,
            0xD => self.sf == self.of,
            0xE => self.zf || (self.sf != self.of),
            0xF => !self.zf && (self.sf == self.of),
            other => panic!("condicion {other:#x} no emitida por BMO"),
        }
    }
}

#[derive(Clone, Copy)]
enum Operand {
    Reg(usize),
    Mem(u64),
}

/// Ejecuta hasta caer del final del código, hasta `EXIT`, o hasta agotar el
/// presupuesto de pasos (un bucle que no termina es un bug, y colgar el test
/// lo esconde en vez de reportarlo).
pub fn run(mut m: Machine, max_steps: usize) -> Machine {
    let mut steps = 0;
    while m.rip < m.code.len() && !m.exited {
        m.step();
        steps += 1;
        assert!(
            steps < max_steps,
            "el codigo emitido no termina (>{max_steps} instrucciones)"
        );
    }
    m
}

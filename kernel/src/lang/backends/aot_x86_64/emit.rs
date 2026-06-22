//! `backends::aot_x86_64::emit` — Emitter de bytes x86-64.
//!
//! Emite instrucciones x86-64 reales a un buffer de bytes. La
//! codificación sigue el manual Intel SDM Vol 2.
//!
//! ## Codificación típica
//!
//! ```text
//! [Prefixes 1B] [REX 1B] [Opcode 1-3B] [ModR/M 1B] [SIB 1B] [Displacement 0/1/2/4B] [Immediate 0/1/2/4/8B]
//! ```
//!
//! ## OperandSize
//!
//! Algunas instrucciones operan sobre 8/16/32/64 bits. La mayoría usa
//! el parámetro `OpSize` para decidir el REX.W y el prefijo 0x66.

#![allow(dead_code)]

use super::abi::Reg;

const CODE_BUF_SIZE: usize = 64 * 1024;
const RODATA_BUF_SIZE: usize = 16 * 1024;

/// Tamaño del operando: 8/16/32/64 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpSize {
    S8  = 1,
    S16 = 2,
    S32 = 4,
    S64 = 8,
}

impl OpSize {
    pub fn bytes(self) -> u8 { self as u8 }
}

/// Operando: registro, memoria `[base+disp]`, o inmediato.
#[derive(Clone, Copy, Debug)]
pub enum Operand {
    Reg(Reg),
    /// `[reg + disp]`. disp es signed 32-bit.
    Mem { base: Reg, disp: i32 },
    /// Inmediato (64-bit, se trunca según contexto).
    Imm(i64),
    /// RIP-relative (disp desde la siguiente instrucción).
    RipRel(i32),
}

/// Emite bytes x86-64 a un buffer interno.
pub struct Emitter {
    code: [u8; CODE_BUF_SIZE],
    pub code_len: usize,
    /// Buffer separado para datos de solo lectura (strings, constants).
    rodata: [u8; RODATA_BUF_SIZE],
    pub rodata_len: usize,
    /// Mapa de strings ya emitidos (deduplicación).
    string_map: [Option<(*const u8, usize)>; 64],
    string_count: usize,
}

impl Emitter {
    pub const fn new() -> Self {
        Self {
            code: [0; CODE_BUF_SIZE],
            code_len: 0,
            rodata: [0; RODATA_BUF_SIZE],
            rodata_len: 0,
            string_map: [None; 64],
            string_count: 0,
        }
    }

    // ─── Buffer access ───────────────────────────────────────────
    pub fn bytes(&self) -> &[u8] { &self.code[..self.code_len] }
    pub fn rodata(&self) -> &[u8] { &self.rodata[..self.rodata_len] }

    // ─── Low-level emit ──────────────────────────────────────────
    fn emit_byte_to(&mut self, buf: &mut usize, b: u8) {
        if *buf < CODE_BUF_SIZE.max(RODATA_BUF_SIZE) {
            if buf == &mut self.code_len {
                if *buf < CODE_BUF_SIZE {
                    self.code[*buf] = b;
                    *buf += 1;
                }
            } else {
                if *buf < RODATA_BUF_SIZE {
                    self.rodata[*buf] = b;
                    *buf += 1;
                }
            }
        }
    }

    pub fn cb(&mut self, b: u8) {
        if self.code_len < CODE_BUF_SIZE {
            self.code[self.code_len] = b;
            self.code_len += 1;
        }
    }
    pub fn cs(&mut self, bs: &[u8]) {
        for &b in bs { self.cb(b); }
    }

    // ─── REX prefix ──────────────────────────────────────────────
    /// Emite REX prefix: `0100 WRXB`.
    /// `w` = 64-bit operand, `r` = ext reg, `x` = ext index, `b` = ext rm.
    pub fn rex(&mut self, w: bool, r: Reg, x: Reg, b: Reg) {
        let mut rex = 0x40u8;
        if w { rex |= 0x08; }
        if r.needs_rex_r() { rex |= 0x04; }
        if x.needs_rex_b() { rex |= 0x02; }
        if b.needs_rex_b() { rex |= 0x01; }
        self.cb(rex);
    }

    /// ModR/M byte: mod(2) | reg(3) | rm(3).
    pub fn modrm(&mut self, mod_: u8, reg: u8, rm: u8) {
        self.cb((mod_ << 6) | (reg << 3) | rm);
    }

    /// ModR/M con base [reg+disp8/32]. Requiere SIB si base = RSP/R12.
    pub fn modrm_mem(&mut self, reg: Reg, base: Reg, disp: i32) {
        let b = base.code3();
        let r = reg.code3();
        if disp == 0 && base != Reg::Rsp && base != Reg::R12 {
            // [reg]
            self.modrm(0, r, b);
        } else if (disp as i32) >= -128 && disp <= 127 {
            // [reg + disp8]
            self.modrm(1, r, b);
            self.cb(disp as u8);
        } else {
            // [reg + disp32]
            self.modrm(2, r, b);
            self.cs(&disp.to_le_bytes());
        }
        // Si base es RSP/R12, agregar SIB byte
        if base == Reg::Rsp || base == Reg::R12 {
            self.cb(0x24); // SIB: scale=0, index=4 (none), base=4
        }
    }

    /// ModR/M con RIP-relative [rip + disp32].
    pub fn modrm_rip(&mut self, reg: Reg) {
        let r = reg.code3();
        self.modrm(0, r, 5); // mod=00, rm=101 (RIP-relative)
    }

    // ─── MOV ─────────────────────────────────────────────────────
    /// `mov rax, imm64`.
    pub fn mov_rax_imm64(&mut self, imm: u64) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0xB8);
        self.cs(&imm.to_le_bytes());
    }

    /// `mov reg, imm64`.
    pub fn mov_imm64(&mut self, dst: Reg, imm: u64) {
        self.rex(true, Reg::Rax, Reg::Rax, dst);
        self.cb(0xB8 | dst.code3());
        self.cs(&imm.to_le_bytes());
    }

    /// `mov reg, imm32` (sign-extended to 64).
    pub fn mov_imm32(&mut self, dst: Reg, imm: i32) {
        self.rex(true, Reg::Rax, Reg::Rax, dst);
        self.cb(0xC7);
        self.modrm(3, 0, dst.code3());
        self.cs(&imm.to_le_bytes());
    }

    /// `mov reg, reg`.
    pub fn mov_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x89);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `mov [base+disp], reg` (store).
    pub fn mov_mr(&mut self, base: Reg, disp: i32, src: Reg) {
        self.rex(true, src, Reg::Rax, base);
        self.cb(0x89);
        self.modrm_mem(src, base, disp);
    }

    /// `mov reg, [base+disp]` (load).
    pub fn mov_rm(&mut self, dst: Reg, base: Reg, disp: i32) {
        self.rex(true, dst, Reg::Rax, base);
        self.cb(0x8B);
        self.modrm_mem(dst, base, disp);
    }

    /// `mov [rip+disp32], src` (store a rodata desde code).
    pub fn mov_mr_rip(&mut self, disp: i32, src: Reg) {
        self.rex(true, src, Reg::Rax, Reg::Rax);
        self.cb(0x89);
        self.modrm_rip(src);
        self.cs(&disp.to_le_bytes());
    }

    /// `mov dst, [rip+disp32]` (load desde rodata).
    pub fn mov_rm_rip(&mut self, dst: Reg, disp: i32) {
        self.rex(true, dst, Reg::Rax, Reg::Rax);
        self.cb(0x8B);
        self.modrm_rip(dst);
        self.cs(&disp.to_le_bytes());
    }

    /// `lea reg, [base+disp]`.
    pub fn lea(&mut self, dst: Reg, base: Reg, disp: i32) {
        self.rex(true, dst, Reg::Rax, base);
        self.cb(0x8D);
        self.modrm_mem(dst, base, disp);
    }

    /// `lea reg, [rip+disp32]`.
    pub fn lea_rip(&mut self, dst: Reg, disp: i32) {
        self.rex(true, dst, Reg::Rax, Reg::Rax);
        self.cb(0x8D);
        self.modrm_rip(dst);
        self.cs(&disp.to_le_bytes());
    }

    // ─── Aritmética ──────────────────────────────────────────────
    /// `add dst, imm32`.
    pub fn add_imm(&mut self, dst: Reg, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x83);
            self.modrm(3, 0, dst.code3());
            self.cb(imm as u8);
        } else {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x81);
            self.modrm(3, 0, dst.code3());
            self.cs(&imm.to_le_bytes());
        }
    }

    /// `add dst, src`.
    pub fn add_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x01);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `sub dst, imm32`.
    pub fn sub_imm(&mut self, dst: Reg, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x83);
            self.modrm(3, 5, dst.code3());
            self.cb(imm as u8);
        } else {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x81);
            self.modrm(3, 5, dst.code3());
            self.cs(&imm.to_le_bytes());
        }
    }

    /// `sub dst, src`.
    pub fn sub_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x29);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `imul dst, src` (2-operand, signed).
    pub fn imul_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, dst, Reg::Rax, src);
        self.cb(0x0F);
        self.cb(0xAF);
        self.modrm(3, dst.code3(), src.code3());
    }

    /// `idiv src` (signed, RAX = quotient, RDX = remainder).
    pub fn idiv(&mut self, src: Reg) {
        self.rex(true, Reg::Rax, Reg::Rax, src);
        self.cb(0xF7);
        self.modrm(3, 7, src.code3());
    }

    /// `cdq` / `cqo` (sign-extend EAX→EDX:EAX or RAX→RDX:RAX).
    pub fn cqo(&mut self) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0x99);
    }

    /// `xor reg, reg` (zero reg).
    pub fn xor_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x31);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `and dst, src`.
    pub fn and_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x21);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `or dst, src`.
    pub fn or_rr(&mut self, dst: Reg, src: Reg) {
        self.rex(true, src, Reg::Rax, dst);
        self.cb(0x09);
        self.modrm(3, src.code3(), dst.code3());
    }

    /// `shl rax, cl` (shift left by CL).
    pub fn shl_cl(&mut self) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0xD3);
        self.modrm(3, 4, Reg::Rax.code3());
    }

    /// `shr rax, cl` (shift right logical by CL).
    pub fn shr_cl(&mut self) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0xD3);
        self.modrm(3, 5, Reg::Rax.code3());
    }

    /// `neg rax`.
    pub fn neg_rax(&mut self) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0xF7);
        self.modrm(3, 3, Reg::Rax.code3());
    }

    /// `not rax`.
    pub fn not_rax(&mut self) {
        self.rex(true, Reg::Rax, Reg::Rax, Reg::Rax);
        self.cb(0xF7);
        self.modrm(3, 2, Reg::Rax.code3());
    }

    // ─── Comparación ─────────────────────────────────────────────
    /// `cmp reg, imm32`.
    pub fn cmp_imm(&mut self, dst: Reg, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x83);
            self.modrm(3, 7, dst.code3());
            self.cb(imm as u8);
        } else {
            self.rex(true, Reg::Rax, Reg::Rax, dst);
            self.cb(0x81);
            self.modrm(3, 7, dst.code3());
            self.cs(&imm.to_le_bytes());
        }
    }

    /// `cmp a, b`.
    pub fn cmp_rr(&mut self, a: Reg, b: Reg) {
        self.rex(true, b, Reg::Rax, a);
        self.cb(0x39);
        self.modrm(3, b.code3(), a.code3());
    }

    /// `test reg, reg`.
    pub fn test_rr(&mut self, a: Reg, b: Reg) {
        self.rex(true, b, Reg::Rax, a);
        self.cb(0x85);
        self.modrm(3, b.code3(), a.code3());
    }

    /// `setcc al` (set byte if condition).
    pub fn setcc_al(&mut self, cc: CondCode) {
        // 0F 9X C0
        self.cb(0x0F);
        self.cb(0x90 | cc as u8);
        self.modrm(3, 0, Reg::Rax.code3());
    }

    /// `movzx rax, al` (zero-extend byte to qword).
    pub fn movzx_byte(&mut self, dst: Reg) {
        self.rex(true, Reg::Rax, Reg::Rax, dst);
        self.cb(0x0F);
        self.cb(0xB6);
        self.modrm(3, dst.code3(), Reg::Rax.code3());
    }

    // ─── Stack ───────────────────────────────────────────────────
    /// `push reg`.
    pub fn push(&mut self, r: Reg) {
        if r.needs_rex_b() { self.cb(0x41); }
        self.cb(0x50 | r.code3());
    }

    /// `pop reg`.
    pub fn pop(&mut self, r: Reg) {
        if r.needs_rex_b() { self.cb(0x41); }
        self.cb(0x58 | r.code3());
    }

    /// `push imm8` / `push imm32`.
    pub fn push_imm(&mut self, imm: i32) {
        if imm >= -128 && imm <= 127 {
            self.cb(0x6A);
            self.cb(imm as u8);
        } else {
            self.cb(0x68);
            self.cs(&imm.to_le_bytes());
        }
    }

    /// `mov rbp, rsp` (frame setup).
    pub fn mov_rbp_rsp(&mut self) {
        self.mov_rr(Reg::Rbp, Reg::Rsp);
    }

    /// `sub rsp, imm32` (alloc stack frame).
    pub fn sub_rsp_imm(&mut self, imm: i32) { self.sub_imm(Reg::Rsp, imm); }
    /// `add rsp, imm32` (dealloc stack frame).
    pub fn add_rsp_imm(&mut self, imm: i32) { self.add_imm(Reg::Rsp, imm); }

    // ─── Control flow ────────────────────────────────────────────
    /// `call rel32` (relative to next instruction).
    pub fn call_rel32(&mut self, rel: i32) {
        self.cb(0xE8);
        self.cs(&rel.to_le_bytes());
    }

    /// `ret`.
    pub fn ret(&mut self) { self.cb(0xC3); }

    /// `syscall` (ring 0 transition via BMO ABI).
    pub fn syscall(&mut self) { self.cs(&[0x0F, 0x05]); }

    /// `jmp rel32`.
    pub fn jmp_rel32(&mut self, rel: i32) {
        self.cb(0xE9);
        self.cs(&rel.to_le_bytes());
    }

    /// `jcc rel32` (conditional jump).
    pub fn jcc(&mut self, cc: CondCode, rel: i32) {
        self.cb(0x0F);
        self.cb(0x80 | cc as u8);
        self.cs(&rel.to_le_bytes());
    }

    /// `leave` (rsp = rbp; pop rbp).
    pub fn leave(&mut self) { self.cb(0xC9); }

    // ─── Patching ────────────────────────────────────────────────
    /// Posición actual de emisión.
    pub fn pos(&self) -> usize { self.code_len }

    /// Reserva 4 bytes para un `rel32` y devuelve la posición.
    pub fn reserve_rel32(&mut self) -> usize {
        let p = self.code_len;
        self.cs(&[0; 4]);
        p
    }

    /// Patchea un `rel32` que ya fue emitido.
    pub fn patch_rel32(&mut self, at: usize, target: usize) {
        let cur = at + 4;
        let rel = (target as isize - cur as isize) as i32;
        let bytes = rel.to_le_bytes();
        self.code[at..at+4].copy_from_slice(&bytes);
    }

    // ─── Rodata ──────────────────────────────────────────────────
    /// Emite un string en .rodata, devuelve el offset.
    /// El string incluye el null terminator.
    pub fn add_string(&mut self, s: &[u8]) -> u32 {
        // Buscar si ya existe (deduplicación simple)
        for i in 0..self.string_count {
            if let Some((ptr, len)) = self.string_map[i] {
                if len == s.len() {
                    let mut same = true;
                    for j in 0..s.len() {
                        unsafe {
                            if *ptr.add(j) != s[j] { same = false; break; }
                        }
                    }
                    if same {
                        // Calcular offset
                        let mut off = 0u32;
                        for k in 0..i {
                            if let Some((_, l)) = self.string_map[k] {
                                off += l as u32 + 1; // +1 null terminator
                            }
                        }
                        return off;
                    }
                }
            }
        }

        // Emitir
        let offset = self.rodata_len as u32;
        for &b in s {
            if self.rodata_len < RODATA_BUF_SIZE {
                self.rodata[self.rodata_len] = b;
                self.rodata_len += 1;
            }
        }
        // Null terminator
        if self.rodata_len < RODATA_BUF_SIZE {
            self.rodata[self.rodata_len] = 0;
            self.rodata_len += 1;
        }

        // Guardar
        if self.string_count < 64 {
            self.string_map[self.string_count] = Some((self.rodata.as_ptr() as *const u8, s.len()));
            self.string_count += 1;
        }
        offset
    }
}

/// Condition codes para `jcc` y `setcc`.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CondCode {
    /// je / jz
    E   = 0x4,
    /// jne / jnz
    Ne  = 0x5,
    /// jl / jnge (signed)
    L   = 0xC,
    /// jle / jng
    Le  = 0xE,
    /// jg / jnle
    G   = 0xF,
    /// jge / jnl
    Ge  = 0xD,
    /// jb / jnae / jc (unsigned)
    B   = 0x2,
    /// jbe / jna
    Be  = 0x6,
    /// ja / jnbe
    A   = 0x7,
    /// jae / jnb / jnc
    Ae  = 0x3,
}

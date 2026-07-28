//! Codificación x86-64 mínima — solo las formas que la puerta necesita.
//!
//! Deliberadamente NO usa `bmo-sem-asm`: la puerta emite una secuencia fija
//! y auditable byte a byte (el mismo criterio que `tools/hello-bex`, el
//! programa que ya corre en el metal). Un encoder por tablas es la
//! herramienta correcta para un codegen general, no para 20 instrucciones
//! que deben poder leerse a ojo cuando algo falle en hardware.
//!
//! Convención de nombres: `r64` = registro de 64 bits por su índice
//! arquitectural (rax=0 … rdi=7, r8=8 … r15=15).

pub const RAX: u8 = 0;
pub const RCX: u8 = 1;
pub const RDX: u8 = 2;
pub const RSP: u8 = 4;
pub const RSI: u8 = 6;
pub const RDI: u8 = 7;
pub const R8: u8 = 8;
pub const R9: u8 = 9;
pub const R10: u8 = 10;
/// Caller-saved y sin uso fijo en la ABI: el sitio natural para guardar algo
/// que tiene que sobrevivir a un `div`, que se come rax y rdx.
pub const R11: u8 = 11;

/// `mov <r64>, imm64` — 10 bytes (REX.W + B8+rd + imm64).
pub fn mov_r64_imm64(out: &mut Vec<u8>, reg: u8, imm: u64) {
    out.push(0x48 | ((reg >> 3) & 1)); // REX.W (+REX.B para r8..r15)
    out.push(0xB8 + (reg & 7));
    out.extend_from_slice(&imm.to_le_bytes());
}

/// `mov <r32>, imm32` — 5 bytes. Escribir el registro de 32 bits pone a cero
/// la mitad alta del de 64, que es justo lo que queremos para constantes
/// pequeñas (una operación, un contador) sin pagar el `mov` de 10 bytes.
pub fn mov_r32_imm32(out: &mut Vec<u8>, reg: u8, imm: u32) {
    if reg >= 8 {
        out.push(0x41); // REX.B
    }
    out.push(0xB8 + (reg & 7));
    out.extend_from_slice(&imm.to_le_bytes());
}

/// REX.W para una instrucción `op r/m64, r64` con modrm mod=11.
fn rex_w(reg: u8, rm: u8) -> u8 {
    0x48 | (((reg >> 3) & 1) << 2) | ((rm >> 3) & 1)
}

fn modrm_reg_direct(reg: u8, rm: u8) -> u8 {
    0xC0 | ((reg & 7) << 3) | (rm & 7)
}

/// Emite `<op> <rm64>, <reg64>` con el opcode de la forma `/r` indicada.
fn alu_rm_r(out: &mut Vec<u8>, opcode: u8, rm: u8, reg: u8) {
    out.push(rex_w(reg, rm));
    out.push(opcode);
    out.push(modrm_reg_direct(reg, rm));
}

/// `mov <dst>, <src>` (64 bits).
pub fn mov_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    alu_rm_r(out, 0x89, dst, src);
}

/// `or <dst>, <src>` (64 bits).
pub fn or_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    alu_rm_r(out, 0x09, dst, src);
}

/// `add <dst>, <src>` (64 bits).
pub fn add_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    alu_rm_r(out, 0x01, dst, src);
}

/// `sub <dst>, <src>` (64 bits).
pub fn sub_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    alu_rm_r(out, 0x29, dst, src);
}

/// `test <a>, <b>` (64 bits) — pone ZF si el AND es cero.
pub fn test_r64_r64(out: &mut Vec<u8>, a: u8, b: u8) {
    alu_rm_r(out, 0x85, a, b);
}

/// `xor <r32>, <r32>` — el idioma de 2 bytes para poner un registro a cero.
pub fn zero_r32(out: &mut Vec<u8>, reg: u8) {
    if reg >= 8 {
        out.push(0x45); // REX.R + REX.B (mismo registro en ambos campos)
    }
    out.push(0x31);
    out.push(modrm_reg_direct(reg, reg));
}

/// `cmp <r64>, imm8` (signo-extendido).
pub fn cmp_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: i8) {
    out.push(rex_w(0, reg));
    out.push(0x83);
    out.push(0xC0 | (7 << 3) | (reg & 7)); // /7 = CMP
    out.push(imm as u8);
}

/// `dec <r64>`.
pub fn dec_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xFF);
    out.push(0xC0 | (1 << 3) | (reg & 7)); // /1 = DEC
}

/// `shl <r64>, imm8`.
pub fn shl_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: u8) {
    out.push(rex_w(0, reg));
    out.push(0xC1);
    out.push(0xC0 | (4 << 3) | (reg & 7)); // /4 = SHL
    out.push(imm);
}

/// `shr <reg>, <imm>` — desplazamiento a la DERECHA sin signo. Es el que saca
/// campos empaquetados de una palabra: el contador de bytes vive en los bits
/// altos y hay que bajarlo sin arrastrar el signo.
pub fn shr_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: u8) {
    out.push(rex_w(0, reg));
    out.push(0xC1);
    out.push(0xC0 | (5 << 3) | (reg & 7)); // /5 = SHR
    out.push(imm);
}

/// `and <reg>, <imm32>` — quedarse con los bits bajos.
pub fn and_r64_imm32(out: &mut Vec<u8>, reg: u8, imm: u32) {
    out.push(rex_w(0, reg));
    out.push(0x81);
    out.push(0xC0 | (4 << 3) | (reg & 7)); // /4 = AND
    out.extend_from_slice(&imm.to_le_bytes());
}

/// `imul <dst>, <src>` — producto con signo entre registros.
pub fn imul_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(rex_w(dst, src));
    out.extend_from_slice(&[0x0F, 0xAF]);
    out.push(0xC0 | ((dst & 7) << 3) | (src & 7));
}

/// `cmp <a>, <b>` entre registros.
pub fn cmp_r64_r64(out: &mut Vec<u8>, a: u8, b: u8) {
    out.push(rex_w(b, a));
    out.push(0x39);
    out.push(0xC0 | ((b & 7) << 3) | (a & 7));
}

/// `movzx <dst32>, byte [<base> + <index>]` — carga un byte del buffer sin
/// arrastrar basura en los bits altos.
pub fn movzx_r32_byte_base_index(out: &mut Vec<u8>, dst: u8, base: u8, index: u8) {
    let rex = 0x40
        | (((dst >> 3) & 1) << 2)   // REX.R
        | (((index >> 3) & 1) << 1) // REX.X
        | ((base >> 3) & 1); // REX.B
    if rex != 0x40 {
        out.push(rex);
    }
    out.extend_from_slice(&[0x0F, 0xB6]);
    out.push(((dst & 7) << 3) | 0b100); // mod=00, rm=100 → SIB
    out.push((index & 7) << 3 | (base & 7)); // scale=1
}

/// `inc <r64>`.
pub fn inc_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xFF);
    out.push(0xC0 | (reg & 7)); // /0 = INC
}

/// `neg <r64>`.
pub fn neg_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xF7);
    out.push(0xC0 | (3 << 3) | (reg & 7)); // /3 = NEG
}

/// `add <r64>, imm8` (signo-extendido).
pub fn add_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: i8) {
    out.push(rex_w(0, reg));
    out.push(0x83);
    out.push(0xC0 | (reg & 7)); // /0 = ADD
    out.push(imm as u8);
}

/// `sub <r64>, imm8` (signo-extendido).
pub fn sub_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: i8) {
    out.push(rex_w(0, reg));
    out.push(0x83);
    out.push(0xC0 | (5 << 3) | (reg & 7)); // /5 = SUB
    out.push(imm as u8);
}

/// `cqo` — extiende el signo de `rax` a `rdx:rax`, lo que `idiv` espera.
pub fn cqo(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x48, 0x99]);
}

/// `idiv <r64>` — divide `rdx:rax` con signo.
pub fn idiv_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xF7);
    out.push(0xC0 | (7 << 3) | (reg & 7)); // /7 = IDIV
}

/// `div <r64>` — divide `rdx:rax` SIN signo.
pub fn div_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xF7);
    out.push(0xC0 | (6 << 3) | (reg & 7)); // /6 = DIV
}

/// `lea <dst>, [rsp + disp8]`.
pub fn lea_r64_rsp_disp8(out: &mut Vec<u8>, dst: u8, disp: i8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2)); // REX.W (+R)
    out.push(0x8D);
    out.push(0x40 | ((dst & 7) << 3) | 0b100); // mod=01, rm=100 → SIB
    out.push(0x24); // SIB: base=rsp, sin índice
    out.push(disp as u8);
}

/// Emite el ModRM (+SIB) de `[<base>]` sin desplazamiento, para el campo
/// `reg`/extensión dado.
///
/// Los dos casos que no se pueden escribir "directo", y que hay que tratar
/// aparte o el CPU decodifica otra cosa:
/// - `rsp`/`r12` (rm=100) exigen un byte SIB.
/// - `rbp`/`r13` (rm=101) con mod=00 significan RIP-relativo, así que se
///   codifican como mod=01 con desplazamiento 0.
fn modrm_at_base(out: &mut Vec<u8>, reg_field: u8, base: u8) {
    let rm = base & 7;
    if rm == 0b100 {
        out.push((reg_field << 3) | 0b100); // mod=00, rm=100 → SIB
        out.push(0x24); // SIB: base=rsp/r12, sin índice
    } else if rm == 0b101 {
        out.push(0x40 | (reg_field << 3) | rm); // mod=01
        out.push(0x00); // disp8 = 0
    } else {
        out.push((reg_field << 3) | rm); // mod=00
    }
}

/// `mov byte [<base>], <src8>` — guarda el byte bajo de un registro.
pub fn mov_byte_at_reg_from_low(out: &mut Vec<u8>, base: u8, src: u8) {
    // REX obligatorio si el origen es spl/bpl/sil/dil o r8..r15, para que
    // `dl`/`sil` no se confundan con los registros altos heredados (ah, ch…).
    let rex = 0x40 | (((src >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 || src >= 4 {
        out.push(rex);
    }
    out.push(0x88);
    modrm_at_base(out, src & 7, base);
}

/// `mov byte [<base>], imm8`.
pub fn mov_byte_at_reg_imm8(out: &mut Vec<u8>, base: u8, imm: u8) {
    if base >= 8 {
        out.push(0x41); // REX.B
    }
    out.push(0xC6);
    modrm_at_base(out, 0, base); // /0
    out.push(imm);
}

/// `cmp byte [<base> + <index>], imm8`.
pub fn cmp_byte_base_index_imm8(out: &mut Vec<u8>, base: u8, index: u8, imm: u8) {
    let rex = 0x40 | (((index >> 3) & 1) << 1) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.push(0x80);
    out.push((7 << 3) | 0b100); // mod=00, /7 = CMP, rm=100 → SIB
    out.push((index & 7) << 3 | (base & 7)); // scale=1
    out.push(imm);
}

/// `syscall`.
pub fn syscall(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x0F, 0x05]);
}

/// Salto condicional/incondicional con destino a parchear.
pub enum Jump {
    /// `jmp rel32`.
    Always,
    /// `jz rel32` (salta si ZF).
    IfZero,
    /// `jnz rel32`.
    IfNotZero,
    /// `jbe rel32` (sin signo: menor o igual).
    IfBelowOrEqual,
    /// `jns rel32` (salta si NO hay bit de signo: valor ≥ 0).
    IfNotSign,
    /// `jl rel32` (con signo: menor).
    IfLess,
    /// `je rel32` (salta si iguales — el mismo ZF que `IfZero`, con otro
    /// nombre porque leer `IfZero` tras un `cmp` confunde).
    IfEqual,
    /// `jae rel32` (sin signo: mayor o igual).
    IfAboveOrEqual,
    /// `ja rel32` (sin signo: mayor). El truco de `c - '0' > 9` para saber si
    /// un byte es un digito con UNA comparacion en vez de dos.
    IfAbove,
}

/// Emite el salto y devuelve el offset del campo rel32, para `patch_jump`.
///
/// Todos los saltos son rel32 aunque el cuerpo quepa en rel8: el tamaño de
/// la secuencia deja de depender de la distancia, así el emisor es
/// determinista y no hay forma de que un cambio futuro rompa un rango.
#[must_use]
pub fn emit_jump(out: &mut Vec<u8>, kind: Jump) -> usize {
    match kind {
        Jump::Always => out.push(0xE9),
        Jump::IfZero => out.extend_from_slice(&[0x0F, 0x84]),
        Jump::IfNotZero => out.extend_from_slice(&[0x0F, 0x85]),
        Jump::IfBelowOrEqual => out.extend_from_slice(&[0x0F, 0x86]),
        Jump::IfNotSign => out.extend_from_slice(&[0x0F, 0x89]),
        Jump::IfLess => out.extend_from_slice(&[0x0F, 0x8C]),
        Jump::IfEqual => out.extend_from_slice(&[0x0F, 0x84]),
        Jump::IfAboveOrEqual => out.extend_from_slice(&[0x0F, 0x83]),
        Jump::IfAbove => out.extend_from_slice(&[0x0F, 0x87]),
    }
    let field = out.len();
    out.extend_from_slice(&[0, 0, 0, 0]);
    field
}

/// Apunta un salto ya emitido a la posición actual del buffer.
pub fn patch_jump(out: &mut [u8], field: usize) {
    let next_insn = field + 4;
    let rel = (out.len() as i64 - next_insn as i64) as i32;
    out[field..field + 4].copy_from_slice(&rel.to_le_bytes());
}

/// Apunta un salto ya emitido a un destino concreto (para saltos atrás).
pub fn patch_jump_to(out: &mut [u8], field: usize, target: usize) {
    let next_insn = field + 4;
    let rel = (target as i64 - next_insn as i64) as i32;
    out[field..field + 4].copy_from_slice(&rel.to_le_bytes());
}

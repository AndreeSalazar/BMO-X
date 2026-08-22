//! Codificacion x86-64 minima -- solo las formas que la puerta necesita.
//!
//! Deliberadamente NO usa `bmo-sem-asm`: la puerta emite una secuencia fija
//! y auditable byte a byte (el mismo criterio que `tools/hello-bex`, el
//! programa que ya corre en el metal). Un encoder por tablas es la
//! herramienta correcta para un codegen general, no para 20 instrucciones
//! que deben poder leerse a ojo cuando algo falle en hardware.
//!
//! Convencion de nombres: `r64` = registro de 64 bits por su indice
//! arquitectural (rax=0 ... rdi=7, r8=8 ... r15=15).

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

/// `mov <r64>, imm64` -- 10 bytes (REX.W + B8+rd + imm64).
pub fn mov_r64_imm64(out: &mut Vec<u8>, reg: u8, imm: u64) {
    out.push(0x48 | ((reg >> 3) & 1)); // REX.W (+REX.B para r8..r15)
    out.push(0xB8 + (reg & 7));
    out.extend_from_slice(&imm.to_le_bytes());
}

/// `mov <r32>, imm32` -- 5 bytes. Escribir el registro de 32 bits pone a cero
/// la mitad alta del de 64, que es justo lo que queremos para constantes
/// pequenas (una operacion, un contador) sin pagar el `mov` de 10 bytes.
pub fn mov_r32_imm32(out: &mut Vec<u8>, reg: u8, imm: u32) {
    if reg >= 8 {
        out.push(0x41); // REX.B
    }
    out.push(0xB8 + (reg & 7));
    out.extend_from_slice(&imm.to_le_bytes());
}

/// REX.W para una instruccion `op r/m64, r64` con modrm mod=11.
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
/// `xor <dst64>, <src64>`. Lo usa la comparacion de TEXTO: dos trozos iguales
/// dan cero, y acumulando con `or` se sabe si toda la cadena coincide sin un
/// solo salto por dentro.
pub fn xor_r64_r64(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(rex_w(src, dst));
    out.push(0x31);
    out.push(0xC0 | ((src & 7) << 3) | (dst & 7));
}

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

/// `test <a>, <b>` (64 bits) -- pone ZF si el AND es cero.
pub fn test_r64_r64(out: &mut Vec<u8>, a: u8, b: u8) {
    alu_rm_r(out, 0x85, a, b);
}

/// `xor <r32>, <r32>` -- el idioma de 2 bytes para poner un registro a cero.
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

/// `shr <reg>, <imm>` -- desplazamiento a la DERECHA sin signo. Es el que saca
/// campos empaquetados de una palabra: el contador de bytes vive en los bits
/// altos y hay que bajarlo sin arrastrar el signo.
pub fn shr_r64_imm8(out: &mut Vec<u8>, reg: u8, imm: u8) {
    out.push(rex_w(0, reg));
    out.push(0xC1);
    out.push(0xC0 | (5 << 3) | (reg & 7)); // /5 = SHR
    out.push(imm);
}

/// `and <reg>, <imm32>` -- quedarse con los bits bajos.
pub fn and_r64_imm32(out: &mut Vec<u8>, reg: u8, imm: u32) {
    out.push(rex_w(0, reg));
    out.push(0x81);
    out.push(0xC0 | (4 << 3) | (reg & 7)); // /4 = AND
    out.extend_from_slice(&imm.to_le_bytes());
}

/// `imul <dst>, <src>` -- producto con signo entre registros.
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

/// `movzx <dst32>, byte [<base> + <index>]` -- carga un byte del buffer sin
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
    out.push(((dst & 7) << 3) | 0b100); // mod=00, rm=100 -> SIB
    out.push((index & 7) << 3 | (base & 7)); // scale=1
}

/// `movzx <dst32>, byte [<base>]` -- la variante sin indice.
///
/// Existe aparte de `movzx_r32_byte_base_index` porque recorrer un buffer con
/// un puntero que avanza es distinto de indexarlo: no hay registro de indice
/// que reservar, y forzar uno a cero solo para poder usar el SIB gasta un
/// registro en un emisor que no tiene de sobra.
pub fn movzx_r32_byte_at_reg(out: &mut Vec<u8>, dst: u8, base: u8) {
    let rex = 0x40 | (((dst >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.extend_from_slice(&[0x0F, 0xB6]);
    modrm_at_base(out, dst & 7, base);
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

/// `cqo` -- extiende el signo de `rax` a `rdx:rax`, lo que `idiv` espera.
pub fn cqo(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x48, 0x99]);
}

/// `idiv <r64>` -- divide `rdx:rax` con signo.
pub fn idiv_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xF7);
    out.push(0xC0 | (7 << 3) | (reg & 7)); // /7 = IDIV
}

/// `div <r64>` -- divide `rdx:rax` SIN signo.
pub fn div_r64(out: &mut Vec<u8>, reg: u8) {
    out.push(rex_w(0, reg));
    out.push(0xF7);
    out.push(0xC0 | (6 << 3) | (reg & 7)); // /6 = DIV
}

/// `lea <dst>, [rsp + disp8]`.
pub fn lea_r64_rsp_disp8(out: &mut Vec<u8>, dst: u8, disp: i8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2)); // REX.W (+R)
    out.push(0x8D);
    out.push(0x40 | ((dst & 7) << 3) | 0b100); // mod=01, rm=100 -> SIB
    out.push(0x24); // SIB: base=rsp, sin indice
    out.push(disp as u8);
}

/// Emite el ModRM (+SIB) de `[<base>]` sin desplazamiento, para el campo
/// `reg`/extension dado.
///
/// Los dos casos que no se pueden escribir "directo", y que hay que tratar
/// aparte o el CPU decodifica otra cosa:
/// - `rsp`/`r12` (rm=100) exigen un byte SIB.
/// - `rbp`/`r13` (rm=101) con mod=00 significan RIP-relativo, asi que se
///   codifican como mod=01 con desplazamiento 0.
fn modrm_at_base(out: &mut Vec<u8>, reg_field: u8, base: u8) {
    let rm = base & 7;
    if rm == 0b100 {
        out.push((reg_field << 3) | 0b100); // mod=00, rm=100 -> SIB
        out.push(0x24); // SIB: base=rsp/r12, sin indice
    } else if rm == 0b101 {
        out.push(0x40 | (reg_field << 3) | rm); // mod=01
        out.push(0x00); // disp8 = 0
    } else {
        out.push((reg_field << 3) | rm); // mod=00
    }
}

/// `mov byte [<base>], <src8>` -- guarda el byte bajo de un registro.
pub fn mov_byte_at_reg_from_low(out: &mut Vec<u8>, base: u8, src: u8) {
    // REX obligatorio si el origen es spl/bpl/sil/dil o r8..r15, para que
    // `dl`/`sil` no se confundan con los registros altos heredados (ah, ch...).
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
    out.push((7 << 3) | 0b100); // mod=00, /7 = CMP, rm=100 -> SIB
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
    /// `jns rel32` (salta si NO hay bit de signo: valor >= 0).
    IfNotSign,
    /// `jl rel32` (con signo: menor).
    IfLess,
    /// `je rel32` (salta si iguales -- el mismo ZF que `IfZero`, con otro
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
/// Todos los saltos son rel32 aunque el cuerpo quepa en rel8: el tamano de
/// la secuencia deja de depender de la distancia, asi el emisor es
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

/// Apunta un salto ya emitido a la posicion actual del buffer.
pub fn patch_jump(out: &mut [u8], field: usize) {
    let next_insn = field + 4;
    let rel = (out.len() as i64 - next_insn as i64) as i32;
    out[field..field + 4].copy_from_slice(&rel.to_le_bytes());
}

/// Apunta un salto ya emitido a un destino concreto (para saltos atras).
pub fn patch_jump_to(out: &mut [u8], field: usize, target: usize) {
    let next_insn = field + 4;
    let rel = (target as i64 - next_insn as i64) as i32;
    out[field..field + 4].copy_from_slice(&rel.to_le_bytes());
}

// -- Acceso a un buffer con desplazamiento constante ------------------------
//
// Los emisores de arriba direccionan `[base]` a secas, que basta para recorrer
// un buffer con un puntero que avanza. Un buffer con CAMPOS --bytes por un lado,
// contador por otro-- necesita `[base+disp]`, y falsearlo sumando al puntero
// obligaria a restar despues: dos instrucciones y un estado mas que recordar.

/// El ModRM de `[<base>+disp8]`. `rsp`/`r12` necesitan SIB tambien aqui.
fn modrm_base_disp8(out: &mut Vec<u8>, reg_field: u8, base: u8, disp: u8) {
    let rm = base & 7;
    out.push(0x40 | (reg_field << 3) | rm); // mod=01
    if rm == 0b100 {
        out.push(0x24); // SIB: base=rsp/r12, sin indice
    }
    out.push(disp);
}

/// `movzx <dst32>, byte [<base>+disp8]`.
pub fn movzx_r32_byte_at_reg_disp(out: &mut Vec<u8>, dst: u8, base: u8, disp: u8) {
    let rex = 0x40 | (((dst >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.extend_from_slice(&[0x0F, 0xB6]);
    modrm_base_disp8(out, dst & 7, base, disp);
}

/// `mov byte [<base>+disp8], <src_low8>`.
pub fn mov_byte_at_reg_disp_from_low(out: &mut Vec<u8>, base: u8, disp: u8, src: u8) {
    // * El REX hace falta AUNQUE los dos registros sean bajos cuando la fuente
    // es `sil`, `dil`, `spl` o `bpl`: sin el, `88 /r` con reg=110 significa
    // `dh`, no `sil`. Es la trampa clasica de los bytes altos heredados del
    // 8086, y produce un valor de otro registro sin ningun aviso.
    let rex = 0x40 | (((src >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 || matches!(src, 4..=7) {
        out.push(rex);
    }
    out.push(0x88);
    modrm_base_disp8(out, src & 7, base, disp);
}

/// El ModRM de `[<base>+disp32]`. Como el de `disp8` pero con mod=10, para
/// cuando el desplazamiento no cabe en un byte con signo -- que es el caso en
/// cuanto un registro pasa de 127 bytes.
fn modrm_base_disp32(out: &mut Vec<u8>, reg_field: u8, base: u8, disp: i32) {
    let rm = base & 7;
    out.push(0x80 | (reg_field << 3) | rm); // mod=10
    if rm == 0b100 {
        out.push(0x24); // SIB: base=rsp/r12, sin indice
    }
    out.extend_from_slice(&disp.to_le_bytes());
}

/// `mov <dst64>, [<base>+disp32]`.
pub fn mov_r64_at_reg_disp32(out: &mut Vec<u8>, dst: u8, base: u8, disp: i32) {
    out.push(rex_w(dst, base));
    out.push(0x8B);
    modrm_base_disp32(out, dst & 7, base, disp);
}

/// `mov [<base>+disp32], <src64>`.
pub fn mov_at_reg_disp32_from_r64(out: &mut Vec<u8>, base: u8, disp: i32, src: u8) {
    out.push(rex_w(src, base));
    out.push(0x89);
    modrm_base_disp32(out, src & 7, base, disp);
}

/// `cmp <reg64>, imm32`. La hermana de [`cmp_r64_imm8`] para cuando el numero
/// no cabe en un byte -- el tamano de un registro, por ejemplo.
pub fn cmp_r64_imm32(out: &mut Vec<u8>, reg: u8, imm: i32) {
    out.push(rex_w(0, reg));
    out.push(0x81);
    out.push(0xF8 | (reg & 7)); // /7 = cmp, mod=11
    out.extend_from_slice(&imm.to_le_bytes());
}

/// `mov <dst64>, [<base>]`.
pub fn mov_r64_at_reg(out: &mut Vec<u8>, dst: u8, base: u8) {
    out.push(rex_w(dst, base));
    out.push(0x8B);
    modrm_at_base(out, dst & 7, base);
}

/// `mov [<base>], <src64>`.
pub fn mov_at_reg_from_r64(out: &mut Vec<u8>, base: u8, src: u8) {
    out.push(rex_w(src, base));
    out.push(0x89);
    modrm_at_base(out, src & 7, base);
}

/// `mov <dst32>, [<base>]` -- lee cuatro bytes.
///
/// ** Escribir la mitad baja de un registro **pone a cero la mitad alta** en
/// 64 bits, asi que esto ya deja el valor extendido sin ceros a mano. Es la
/// razon de que no haga falta un `movzx` de 32, y de que si haga falta uno de
/// 16 y otro de 8: por debajo de 32 el silicio **conserva** lo que hubiera.
pub fn mov_r32_at_reg(out: &mut Vec<u8>, dst: u8, base: u8) {
    let rex = 0x40 | (((dst >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.push(0x8B);
    modrm_at_base(out, dst & 7, base);
}

/// `mov [<base>], <src32>` -- escribe cuatro bytes.
///
/// Es el que escribe un pixel de 32 bits en un framebuffer, que es el motivo
/// concreto por el que existe.
pub fn mov_at_reg_from_r32(out: &mut Vec<u8>, base: u8, src: u8) {
    let rex = 0x40 | (((src >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.push(0x89);
    modrm_at_base(out, src & 7, base);
}

/// `movzx <dst32>, word [<base>]` -- lee dos bytes y pone el resto a cero.
pub fn movzx_r32_word_at_reg(out: &mut Vec<u8>, dst: u8, base: u8) {
    let rex = 0x40 | (((dst >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.extend_from_slice(&[0x0F, 0xB7]);
    modrm_at_base(out, dst & 7, base);
}

/// `mov word [<base>], <src16>` -- escribe dos bytes.
///
/// El `0x66` de delante es el prefijo de tamano de operando: la misma
/// instruccion que escribe cuatro bytes escribe dos cuando lo lleva.
pub fn mov_word_at_reg_from_r16(out: &mut Vec<u8>, base: u8, src: u8) {
    out.push(0x66);
    let rex = 0x40 | (((src >> 3) & 1) << 2) | ((base >> 3) & 1);
    if rex != 0x40 {
        out.push(rex);
    }
    out.push(0x89);
    modrm_at_base(out, src & 7, base);
}

// ===================================================================
//  ** COMA FLOTANTE ESCALAR (SSE)
// ===================================================================
//
//  Estos bytes ya se emitian, pero desde DENTRO del generador de BMO C, escritos
//  a mano en cada sitio. Aqui estan una vez, con nombre, para que el segundo
//  lenguaje que los necesite no los vuelva a escribir -- que es exactamente lo
//  que paso con los anchos de 16 y 32 bits.
//
//  ** El modelo de INTI: los valores viven en registros normales como PATRON DE
//  BITS, y solo cruzan a `xmm` para la operacion. Cuesta dos `movq` por
//  operacion y a cambio **el asignador de registros, el marco y la convencion de
//  llamada no cambian ni una linea**.
//
//  No es la version rapida y no pretende serlo. Es la version que se puede
//  escribir entera hoy y medir manana: el dia que haya reparto de `xmm`, lo que
//  cambia es donde viven los valores, no que operacion se emite.

/// `movq <xmm>, <r64>` -- el patron de bits, tal cual, al registro de coma
/// flotante. **No convierte**: 5 no se vuelve 5.0.
pub fn movq_xmm_de_r64(out: &mut Vec<u8>, xmm: u8, reg: u8) {
    out.extend_from_slice(&[0x66, 0x48 | ((reg >> 3) & 1) | (((xmm >> 3) & 1) << 2)]);
    out.extend_from_slice(&[0x0F, 0x6E]);
    out.push(modrm_reg_direct(xmm & 7, reg & 7));
}

/// `movq <r64>, <xmm>` -- y de vuelta.
pub fn movq_r64_de_xmm(out: &mut Vec<u8>, reg: u8, xmm: u8) {
    out.extend_from_slice(&[0x66, 0x48 | ((reg >> 3) & 1) | (((xmm >> 3) & 1) << 2)]);
    out.extend_from_slice(&[0x0F, 0x7E]);
    out.push(modrm_reg_direct(xmm & 7, reg & 7));
}

/// Las cuatro operaciones de doble precision, sobre `xmm0` y `xmm1`.
///
/// ** Y fijate en lo que NO llevan detras: ninguna comprobacion.
///
/// No es un olvido ni una excepcion a "INTI no tiene comportamiento indefinido".
/// Es que **IEEE-754 define el desbordamiento y la division por cero**: dan
/// infinito y NaN, que son valores. La Regla 1 y la Regla 3 existen porque en
/// los ENTEROS esos dos casos no tienen respuesta; aqui la tienen, y esta
/// escrita en una norma de 1985.
pub fn addsd(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xF2, 0x0F, 0x58, 0xC1]);
}

pub fn subsd(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xF2, 0x0F, 0x5C, 0xC1]);
}

pub fn mulsd(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xF2, 0x0F, 0x59, 0xC1]);
}

pub fn divsd(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0xF2, 0x0F, 0x5E, 0xC1]);
}

/// `comisd xmm0, xmm1` -- compara y deja las banderas como un `cmp` de enteros.
///
/// ** Eso es lo que deja que las comparaciones de coma flotante reutilicen los
/// mismos `setcc` que las de enteros: el silicio ya tradujo. Lo que NO traduce
/// es el NaN, que sale "no comparable" y pone las tres banderas -- por eso una
/// comparacion con NaN es falsa mire por donde se mire.
pub fn comisd(out: &mut Vec<u8>) {
    out.extend_from_slice(&[0x66, 0x0F, 0x2F, 0xC1]);
}

/// `cvtsi2sd <xmm0>, <r64>` -- un entero se convierte de verdad. 5 -> 5.0.
pub fn cvtsi2sd_de_r64(out: &mut Vec<u8>, reg: u8) {
    out.extend_from_slice(&[0xF2, 0x48 | ((reg >> 3) & 1), 0x0F, 0x2A]);
    out.push(modrm_reg_direct(0, reg & 7));
}

/// `cvttsd2si <r64>, <xmm0>` -- y de vuelta, TRUNCANDO.
///
/// ** La doble `t` no es un adorno: es "truncate", y elige el redondeo. 2,9 da
/// 2 y -2,9 da -2, que es lo que hace un lenguaje de sistema al convertir. La
/// version sin la segunda `t` redondea al par mas cercano, que es correcto para
/// aritmetica y sorprendente para una conversion escrita a mano.
///
/// OJO con lo que devuelve cuando el numero NO CABE: el valor mas negativo del
/// entero, como centinela, y sin levantar nada que se pueda mirar despues. Por
/// eso la Regla 12 no se puede cumplir con esta instruccion sola.
pub fn cvttsd2si_r64(out: &mut Vec<u8>, reg: u8) {
    out.extend_from_slice(&[0xF2, 0x48 | (((reg >> 3) & 1) << 2), 0x0F, 0x2C]);
    out.push(modrm_reg_direct(reg & 7, 0));
}

// ===================================================================
//  ** CABE ESTO EN MENOS BYTES? -- la pregunta de la Regla 12
// ===================================================================
//
//  Extender con SIGNO los bytes bajos y comparar con el original es el modo
//  clasico de preguntar *"cabia?"*, y funciona por una razon bonita: si el
//  valor cabia en n bytes, extenderlo devuelve el mismo numero; si no cabia, la
//  extension inventa unos bits altos distintos de los que habia.
//
//  ** Y es una pregunta que hay que hacer explicita porque la maquina NO la
//  hace: escribir un registro de 32 bits tira los otros 32 sin quejarse. Ese
//  silencio es exactamente el comportamiento indefinido del que INTI se escapa.

/// `movsxd <r64>, <low32(src)>` -- los 32 bajos, con su signo, a 64.
pub fn movsxd_r64_r32(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2) | ((src >> 3) & 1));
    out.push(0x63);
    out.push(modrm_reg_direct(dst & 7, src & 7));
}

/// `movsx <r64>, <low16(src)>`.
pub fn movsx_r64_r16(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2) | ((src >> 3) & 1));
    out.extend_from_slice(&[0x0F, 0xBF]);
    out.push(modrm_reg_direct(dst & 7, src & 7));
}

/// `movsx <r64>, <low8(src)>`.
pub fn movsx_r64_r8(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2) | ((src >> 3) & 1));
    out.extend_from_slice(&[0x0F, 0xBE]);
    out.push(modrm_reg_direct(dst & 7, src & 7));
}

/// Un salto CORTO hacia delante, con el hueco sin rellenar.
///
/// Devuelve la posicion del byte de desplazamiento, que hay que cerrar con
/// [`cierra_salto_corto`] cuando se sepa el destino.
///
/// ** Corto y no largo porque estos saltos son de una comprobacion a la
/// siguiente linea: caben de sobra en un byte, y usar cuatro donde caben uno es
/// engordar el camino que SIEMPRE se recorre para ahorrar en el que casi nunca.
pub fn salto_corto(out: &mut Vec<u8>, cc: u8) -> usize {
    out.extend_from_slice(&[cc, 0]);
    out.len() - 1
}

/// Cierra un [`salto_corto`] para que caiga en el final actual del codigo.
pub fn cierra_salto_corto(out: &mut Vec<u8>, hueco: usize) {
    let destino = out.len();
    let rel = destino as i64 - (hueco as i64 + 1);
    debug_assert!(
        (-128..=127).contains(&rel),
        "un salto corto no llega: {} bytes",
        rel
    );
    out[hueco] = rel as u8;
}

// ===================================================================
//  ** DE BANDERAS A 0/1 -- la plomeria de toda comparacion
// ===================================================================
//
//  Estos bytes tambien se escribian a mano en cada sitio que comparaba. Estan
//  aqui por el mismo motivo que los de arriba: **el segundo lenguaje que
//  compare no tiene que volver a escribirlos**.
//
//  Y hay una razon mas fuerte que la comodidad. Una comparacion de coma
//  flotante necesita mirar DOS banderas --el resultado y la de "no comparable"--
//  y combinarlas. Con los bytes sueltos por el emisor, esa combinacion se
//  escribe distinta cada vez que hace falta; con nombres, se escribe una.

/// `setcc <low(reg)>` -- pone el byte bajo a 1 o a 0 segun la bandera.
///
/// OJO al orden, que costo un test en su dia: `setcc` va PRIMERO y la extension
/// despues. Poner el registro a cero antes con un `xor` **destruye las banderas
/// que la comparacion acaba de dejar**, y entonces contesta siempre lo mismo.
pub fn setcc_low(out: &mut Vec<u8>, cc: u8, reg: u8) {
    if reg >= 4 {
        out.push(0x40 | ((reg >> 3) & 1));
    }
    out.extend_from_slice(&[0x0F, cc]);
    out.push(modrm_reg_direct(0, reg & 7));
}

/// `movzx <r64>, <low(src)>` -- el byte de `setcc`, extendido con ceros.
pub fn movzx_r64_low(out: &mut Vec<u8>, dst: u8, src: u8) {
    out.push(0x48 | (((dst >> 3) & 1) << 2) | ((src >> 3) & 1));
    out.extend_from_slice(&[0x0F, 0xB6]);
    out.push(modrm_reg_direct(dst & 7, src & 7));
}

/// `and <low(dst)>, <low(src)>` -- dos condiciones que tienen que darse las dos.
pub fn and_low_low(out: &mut Vec<u8>, dst: u8, src: u8) {
    if dst >= 4 || src >= 4 {
        out.push(0x40 | (((src >> 3) & 1) << 2) | ((dst >> 3) & 1));
    }
    out.push(0x20);
    out.push(modrm_reg_direct(src & 7, dst & 7));
}

/// `or <low(dst)>, <low(src)>` -- o una o la otra.
pub fn or_low_low(out: &mut Vec<u8>, dst: u8, src: u8) {
    if dst >= 4 || src >= 4 {
        out.push(0x40 | (((src >> 3) & 1) << 2) | ((dst >> 3) & 1));
    }
    out.push(0x08);
    out.push(modrm_reg_direct(src & 7, dst & 7));
}

/// `push <r64>` / `pop <r64>`.
pub fn push_r64(out: &mut Vec<u8>, reg: u8) {
    if reg >= 8 {
        out.push(0x41);
    }
    out.push(0x50 | (reg & 7));
}

pub fn pop_r64(out: &mut Vec<u8>, reg: u8) {
    if reg >= 8 {
        out.push(0x41);
    }
    out.push(0x58 | (reg & 7));
}

//! **FLOATING POINT**: the only value that does not travel in `rax`.
//!
//! === Why this is a file of its own ===
//!
//! Because a `double` breaks the assumption everything else is built on. The
//! rest of the codegen has a one-line rule --*the value of an expression ends
//! up in `rax`*-- and here it ends up in `xmm0`, loads with different
//! instructions, compares with `comisd` instead of `cmp`, and its `setcc` codes
//! are the UNORDERED ones rather than the signed ones.
//!
//! Mixed in with the integer path, each of those differences looked like a
//! corner case. Together they are **a second register bank with its own
//! rules**.
//!
//! ** And the unordered `setcc` detail is not trivia: they are the same codes
//! an UNSIGNED integer comparison needs. The float arm had been using them from
//! the start and the integer arm had not -- which is exactly how the signedness
//! defect managed to hide for so long.

use super::*;

impl Codegen {
    /// cvtsi2sd xmm0, rax -- entero (rax) -> double (xmm0).
    pub(super) fn emit_int_to_double(&mut self) {
        self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2A, 0xC0]);
    }

    /// modrm+disp para `<sse> xmm0, [rbp+off]` / `[rbp+off], xmm0` (reg field = 0).
    pub(super) fn emit_rbp_disp(&mut self, off: i32) {
        if off >= -128 && off <= 127 {
            self.code.push(0x45);           // mod=01, reg=0, rm=101 (rbp) + disp8
            self.code.push(off as u8);
        } else {
            self.code.push(0x85);           // mod=10 + disp32
            self.code.extend_from_slice(&off.to_le_bytes());
        }
    }

    /// Carga una variable float/double del stack a xmm0 (siempre como double).
    pub(super) fn emit_load_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x10]); // movss xmm0,[rbp+off]
                self.emit_rbp_disp(off);
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd xmm0,xmm0
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x10]); // movsd xmm0,[rbp+off]
                self.emit_rbp_disp(off);
            }
        } else if let Some(&(_, ref typ)) = self.global_offsets.get(name) {
            // * UN GLOBAL DE COMA FLOTANTE, leido donde vive.
            //
            // Antes esto ponia `xmm0` a cero y decia *"usa locales"*. El dato
            // ya estaba bien guardado --su patron IEEE-- y lo unico que
            // faltaba era ir a buscarlo: la direccion sale de la misma
            // `lea rip-relativa` con la que se leen los globales enteros, y de
            // ahi un `movss`/`movsd` en vez de un `mov`.
            //
            // Lo pidio `float mouse_acceleration = 2.0;` de `i_video.c`.
            let is_f32 = matches!(typ, TypeSpec::Float);
            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]); // lea rax,[rip+g]
            self.global_fixups.push((self.code.len() - 4, name.to_string()));
            if is_f32 {
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x00]); // movss xmm0,[rax]
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x00]); // movsd xmm0,[rax]
            }
        } else {
            self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC0]); // xorpd xmm0,xmm0
            self.errors.push(format!("variable float '{name}' no esta declarada"));
        }
    }

    /// Guarda xmm0 (double) en una variable float/double del stack.
    pub(super) fn store_float_var(&mut self, name: &str) {
        if let Some(&(off, ref typ)) = self.var_offsets.get(name) {
            let is_f32 = matches!(typ, TypeSpec::Float);
            let off = off;
            if is_f32 {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]); // cvtsd2ss xmm0,xmm0
                self.code.extend_from_slice(&[0xF3, 0x0F, 0x11]);       // movss [rbp+off],xmm0
                self.emit_rbp_disp(off);
            } else {
                self.code.extend_from_slice(&[0xF2, 0x0F, 0x11]);       // movsd [rbp+off],xmm0
                self.emit_rbp_disp(off);
            }
        } else {
            self.errors.push(format!("variable float global '{name}' aun no soportada (usa locales)"));
        }
    }

    /// Evalua `e` a xmm0 como double, convirtiendo enteros si hace falta.
    pub(super) fn emit_fexpr_operand(&mut self, e: &Expr) {
        if self.expr_is_float(e) {
            self.emit_fexpr(e);
        } else {
            self.emit_expr(e);          // rax = valor entero
            self.emit_int_to_double();  // xmm0 = (double) rax
        }
    }

    /// a OP b en double: resultado en xmm0. `op` = bytes de `<opsd> xmm0,xmm1`.
    pub(super) fn emit_fbinop(&mut self, a: &Expr, b: &Expr, op: &[u8]) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0  (spill a)
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0  (xmm1=b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (xmm0=a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(op);                             // op xmm0,xmm1
    }

    /// Evalua una expresion FLOTANTE dejando el resultado (double) en xmm0.
    pub(super) fn emit_fexpr(&mut self, e: &Expr) {
        match e {
            // Una llamada que devuelve un double **ya deja el valor en xmm0**:
            // no hay nada que convertir, solo que emitirla. Se pide por el
            // camino entero porque ahi vive todo el trabajo de una llamada
            // --los argumentos, las relocs, los agregados-- y duplicarlo aqui
            // seria tener dos sitios donde equivocarse.
            Expr::Call(_, _) => {
                self.sin_guarda_float = true;
                self.emit_expr(e);
            }
            Expr::FloatLit(f) => {
                let bits = f.to_bits();
                self.code.extend_from_slice(&[0x48, 0xB8]);            // mov rax, imm64
                self.code.extend_from_slice(&bits.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC0]); // movq xmm0, rax
            }
            Expr::Var(n) => self.emit_load_float_var(n),
            Expr::Cast(t, inner) if Self::is_float_ty(t) => {
                // (double)algo -- si algo ya es float, no-op; si es entero, convierte
                self.emit_fexpr_operand(inner);
            }
            Expr::Neg(a) => {
                self.emit_fexpr(a);
                // xorpd xmm0, sign-bit -> negacion
                self.code.extend_from_slice(&[0x48, 0xB8]);
                self.code.extend_from_slice(&0x8000_0000_0000_0000u64.to_le_bytes());
                self.code.extend_from_slice(&[0x66, 0x48, 0x0F, 0x6E, 0xC8]); // movq xmm1, rax
                self.code.extend_from_slice(&[0x66, 0x0F, 0x57, 0xC1]);       // xorpd xmm0, xmm1
            }
            Expr::Add(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x58, 0xC1]), // addsd
            Expr::Sub(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5C, 0xC1]), // subsd
            Expr::Mul(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x59, 0xC1]), // mulsd
            Expr::Div(a, b) => self.emit_fbinop(a, b, &[0xF2, 0x0F, 0x5E, 0xC1]), // divsd
            // cualquier otra cosa: es entera -> convertir a double
            _ => self.emit_fexpr_operand(e),
        }
    }

    /// Comparacion de floats: a CMP b -> 0/1 en rax. `setcc` es el opcode
    /// SETcc estilo UNSIGNED (comisd fija CF/ZF como comparacion sin signo).
    pub(super) fn emit_fcmp(&mut self, a: &Expr, b: &Expr, setcc: u8) {
        self.emit_fexpr_operand(a);
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);       // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
        self.emit_fexpr_operand(b);
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0xC8]);       // movsd xmm1,xmm0 (b)
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp] (a)
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);       // add rsp,8
        self.code.extend_from_slice(&[0x66, 0x0F, 0x2F, 0xC1]);       // comisd xmm0,xmm1
        self.code.extend_from_slice(&[0x0F, setcc, 0xC0]);            // setcc al
        self.code.extend_from_slice(&[0x0F, 0xB6, 0xC0]);            // movzx eax, al
    }
}

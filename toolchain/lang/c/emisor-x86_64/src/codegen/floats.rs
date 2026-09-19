//! **FLOATING POINT**: the only value that does not travel in `rax`.
//!
//! [fase]     EMISION
//!
//! [aparece]  DENTRO -- [!] y con una cicatriz: NUEVE pruebas de coma flotante
//!            estan en verde y NINGUNA ejecuta. Aqui el banco no protege
//!
//! [carril]   ROJO     -- nadie te sujeta: compila, pasa el banco, y el sintoma sale LEJOS
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/
//!
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

    /// **GUARDAR UN FLOTANTE EN UN LUGAR.** `true` si lo hizo.
    ///
    /// # Por que hacia falta, y por que es UNA funcion y no cinco
    ///
    /// `Expr::Assign` --asignar a una VARIABLE-- ya tenia su ruta SSE desde
    /// siempre. Los otros cinco destinos no:
    ///
    /// ```text
    ///    f = 3.5           variable      ruta SSE        bien
    ///    t[0] = 3.5        subindice     ruta ENTERA     guarda 0
    ///    v.f = 3.5         campo         ruta ENTERA     guarda 0
    ///    p->f = 3.5        flecha        ruta ENTERA     guarda 0
    ///    *p = 3.5          indireccion   ruta ENTERA     guarda 0
    /// ```
    ///
    /// ** Y no guardaba basura: guardaba CERO. El valor se calculaba por el
    /// camino entero, donde un `FloatLit` no deja nada en `rax`, y el store
    /// escribia ese `rax`. Un programa que reparte flotantes en una tabla o en
    /// un `struct` los leia todos a cero **sin un solo error de compilacion**.
    ///
    /// *** Es UNA funcion y no cinco brazos copiados porque los cinco hacen lo
    /// mismo: valor a `xmm0`, direccion a `rax`, store del ancho que toque. La
    /// direccion la calcula `emit_lvalue_addr`, que ya conoce las cinco
    /// formas -- cinco copias serian cinco sitios donde olvidarse del `cvtsd2ss`.
    ///
    /// [!] El VALOR se calcula ANTES que la direccion, igual que en los brazos
    /// enteros de al lado. El orden no lo fija C --es indeterminado-- pero si
    /// lo fija esta casa: que los dos caminos hagan lo mismo es lo que impide
    /// que un `t[i++] = f(x)` se comporte distinto segun el tipo del elemento.
    pub(super) fn emit_guardar_flotante(&mut self, lvalue: &Expr, val: &Expr) -> bool {
        let elem = self.tipo_del_lvalue(lvalue);
        if !Self::is_float_ty(&elem) {
            return false;
        }
        self.emit_fexpr_operand(val);                                // xmm0 = valor
        self.code.extend_from_slice(&[0x48, 0x83, 0xEC, 0x08]);      // sub rsp,8
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x04, 0x24]); // movsd [rsp],xmm0
        self.emit_lvalue_addr(lvalue);                               // rax = direccion
        self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x04, 0x24]); // movsd xmm0,[rsp]
        self.code.extend_from_slice(&[0x48, 0x83, 0xC4, 0x08]);      // add rsp,8
        if matches!(elem, TypeSpec::Float) {
            self.code.extend_from_slice(&[0xF2, 0x0F, 0x5A, 0xC0]);  // cvtsd2ss xmm0,xmm0
            self.code.extend_from_slice(&[0xF3, 0x0F, 0x11, 0x00]);  // movss [rax],xmm0
        } else {
            self.code.extend_from_slice(&[0xF2, 0x0F, 0x11, 0x00]);  // movsd [rax],xmm0
        }
        true
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
            // ** Un INTRINSECO que devuelve `xmm0` ya deja el valor donde esta
            // ruta lo espera: no hay nada que convertir, solo que emitirlo.
            //
            // Y tiene que estar ESCRITO. Sin este brazo caia en el `_ =>` del
            // final --*"cualquier otra cosa: es entera"*--, que llama a
            // `emit_fexpr_operand`, que pregunta `expr_is_float`, que ahora dice
            // que SI, que vuelve a llamar aqui: **la pila se desborda antes de
            // emitir un byte**. El comodin no se equivocaba de respuesta, se
            // equivocaba de pregunta.
            //
            // [!] La invariante que lo hace correcto: aqui solo llegan las filas
            // con `returns = "xmm0"`, porque `expr_is_float` es quien abre la
            // puerta y solo la abre para esas.
            Expr::Intrinsic(n, args) => self.emit_intrinsic(n, args),
            Expr::Var(n) => self.emit_load_float_var(n),
            // *** LOS LUGARES: `*p`, `p[i]`, `t[i]`, `v.f`, `p->f`.
            //
            // ** SIN ESTE BRAZO, TRES DE ELLOS TUMBABAN EL COMPILADOR, y el
            // mecanismo estaba escrito veinte lineas mas arriba en este mismo
            // fichero: `expr_is_float` decia que SI para `Field`, `Arrow` e
            // `IndexPtr`, aqui no habia brazo, caia en el `_ =>` del final,
            // que llama a `emit_fexpr_operand`, que pregunta `expr_is_float`,
            // que dice que si, que vuelve a llamar aqui.
            //
            // ```text
            //    struct s { float f; };  v.f   ->  la pila se desborda
            // ```
            //
            // *** Un `struct` con un campo `float` no compilaba: MATABA al
            // compilador. Y el aviso de que esto pasaria lleva escrito ahi
            // arriba desde el dia de los intrinsecos, con estas palabras: *el
            // comodin no se equivocaba de respuesta, se equivocaba de
            // pregunta*.
            //
            //   > Una nota que explica un fallo y no lo cierra es una nota que
            //   > describe el fallo siguiente.
            //
            // La direccion la calcula `emit_lvalue_addr`, el mismo emisor que
            // usa todo el resto: aqui no hay un segundo camino que mantener.
            Expr::Deref(_)
            | Expr::IndexPtr(_, _)
            | Expr::Subscript(_, _)
            | Expr::Field(_, _)
            | Expr::Arrow(_, _) => {
                // El ancho lo dice el juez unico de tipos. Si no lo sabe se
                // usa `double`, que es el ancho del camino: equivocarse hacia
                // el ancho GRANDE lee de mas, pero no convierte un valor en
                // otro -- y el `_` de aqui solo se alcanza si `expr_is_float`
                // ya dijo que si.
                let ancho = crate::tipos::tipo_de(self, e).unwrap_or(TypeSpec::Double);
                self.emit_lvalue_addr(e);
                if matches!(ancho, TypeSpec::Float) {
                    self.code.extend_from_slice(&[0xF3, 0x0F, 0x10, 0x00]); // movss xmm0,[rax]
                    self.code.extend_from_slice(&[0xF3, 0x0F, 0x5A, 0xC0]); // cvtss2sd
                } else {
                    self.code.extend_from_slice(&[0xF2, 0x0F, 0x10, 0x00]); // movsd xmm0,[rax]
                }
            }
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

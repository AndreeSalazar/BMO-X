//! **OPERAR EN SITIO SOBRE LA MATRIZ**: `i = i + 1` con `i` en `r12` es
//! `add r12, 1`, no cargar, sumar, recortar y guardar.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- un recorte mal hecho aqui deja en el registro un valor
//!            distinto del que dejaria la pila, y el banco lo ve en la primera
//!            fila que desborde un `int` o cuente con un `char`
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # Lo que dijo el metro (2026-09-18)
//!
//! Con el troquel POR VARIABLE, `i` entro en `r12` y el metro contesto que el
//! programa ejecutaba **las mismas instrucciones**: `movsxd rax, [rbp-0x68]`
//! habia pasado a `mov rax, r12`, una por una. El registro solo paga si se
//! opera EN el:
//!
//! ```text
//!    por la pila      movsxd rax,[i] ; add rax,1 ; movsxd rax,eax ; mov [i],eax     4
//!    en la matriz     mov rax,r12    ; add rax,1 ; movsxd rax,eax ; mov r12,rax     4
//!    EN SITIO         add r12,1 ; movsxd r12,r12d                                   2
//! ```
//!
//! # *** LA INVARIANTE QUE ESTO NO PUEDE ROMPER
//!
//! `frame.rs::emit_guardar_en_registro` deja en `rN` **exactamente** lo que
//! dejaria guardar en la pila y releer: un `int` extendido con signo, un
//! `unsigned` con ceros arriba, un `char` con signo desde el bit 7. Cada
//! operacion de aqui termina con el MISMO recorte, hecho sobre `rN`
//! (`emit_recorte_en_registro`), y por eso los tipos son los mismos seis
//! brazos, en el mismo orden. Si aquel cambia, este cambia.
//!
//! # Que se hace en sitio, y que no
//!
//! ```text
//!    x = x <op> imm    add sub and or xor, con inmediato    -> `op rN, imm`
//!    x++ x-- ++x --x   con su paso de puntero                -> `add/sub rN, paso`
//!    p = p + 1         p puntero: el paso multiplica el imm  -> `add rN, 8`
//!    x = x * imm       NO: `imul rN, rN, imm` es otro opcode; el dia que el
//!                      metro lo pida
//!    x = y + 1         NO: el derecho no es la misma variable
//! ```
//!
//! Y hay DOS contextos, porque una asignacion en C es una expresion:
//!
//! ```text
//!    SENTENCIA   `i = i + 1;`  `for (...; i++)`   no hace falta dejar nada en rax
//!    VALOR       `x = (i = i + 1)`                el nuevo valor va a rax: `mov rax, rN`
//!                `x = i++`                        el VIEJO: por el camino de siempre
//! ```

use crate::ast::{Expr, TypeSpec};

use super::decidir::inmediato::{inmediato_de, Inmediato};
use super::Codegen;

impl Codegen {
    /// **En contexto de SENTENCIA.** `true` si `e` se hizo en sitio y no hay
    /// que emitirla; `rax` queda con cualquier cosa.
    pub(super) fn emit_en_sitio(&mut self, e: &Expr) -> bool {
        match e {
            Expr::Assign(name, val) => {
                self.emit_constante_en_sitio(name, val) || self.emit_asignacion_en_sitio(name, val)
            }
            Expr::PreInc(name) | Expr::PostInc(name) => self.emit_paso_en_sitio(name, false),
            Expr::PreDec(name) | Expr::PostDec(name) => self.emit_paso_en_sitio(name, true),
            _ => false,
        }
    }

    /// **En contexto de VALOR.** Como la de arriba, pero deja el valor NUEVO
    /// en `rax`. `x++` y `x--` no entran: su valor es el viejo.
    pub(super) fn emit_en_sitio_con_valor(&mut self, e: &Expr) -> bool {
        let name = match e {
            Expr::Assign(name, _) | Expr::PreInc(name) | Expr::PreDec(name) => name,
            _ => return false,
        };
        let Some(&r) = self.var_regs.get(name) else { return false };
        if !self.emit_en_sitio(e) {
            return false;
        }
        self.code.extend_from_slice(&[0x49, 0x8B, 0xC0 + (r - 8)]); // mov rax, rN
        true
    }

    /// **`name = constante`, sin pasar por rax** (2026-09-19): `mov [x], 5` o
    /// `mov r12, 5`. Era el 3 % de lo que ejecutaba el banco de C: cada `x =
    /// 0`, cada `int x;` (BMO C pone las locales a cero) y cada contador que
    /// arranca hacian `mov rax, imm` y despues el guardado.
    ///
    /// La constante se RECORTA aqui, al compilar, a la anchura del tipo: un
    /// `char c = 200` guarda -56, igual que por la pila. Los `short` y los
    /// globales siguen por el camino de siempre (el inmediato de 16 bits y el
    /// `lea` del global no ganan nada limpio), y un `long` que no cabe en 32
    /// bits con signo va a la matriz con `movabs` y al marco por la pila.
    pub(super) fn emit_constante_en_sitio(&mut self, name: &str, val: &Expr) -> bool {
        let v = match val {
            Expr::Int(n) => *n,
            Expr::CharLit(c) => *c as i64,
            _ => match super::decidir::plegado::constante_para_emitir(val) {
                Some(v) => v,
                None => return false,
            },
        };
        let Some(tipo) = self.var_type_of(name) else { return false };
        if Self::is_float_ty(&tipo) || self.es_agregado(&tipo) || self.var_is_array(name) {
            return false;
        }
        // la constante como la dejaria la pila al releerla
        let v = match tipo {
            TypeSpec::Char => v as i8 as i64,
            TypeSpec::UnsignedChar => v as u8 as i64,
            TypeSpec::Short | TypeSpec::UnsignedShort => return false,
            TypeSpec::Int => v as i32 as i64,
            TypeSpec::UnsignedInt => v as u32 as i64,
            _ => v,
        };
        if let Some(&r) = self.var_regs.get(name) {
            if matches!(tipo, TypeSpec::UnsignedInt) || (v > i32::MAX as i64 && v <= u32::MAX as i64) {
                // mov rNd, imm32: pone a cero la mitad alta
                self.code.extend_from_slice(&[0x41, 0xB8 + (r - 8)]);
                self.code.extend_from_slice(&(v as u32).to_le_bytes());
            } else if let Ok(i) = i32::try_from(v) {
                self.code.extend_from_slice(&[0x49, 0xC7, 0xC0 | (r - 8)]); // mov rN, imm32 (con signo)
                self.code.extend_from_slice(&i.to_le_bytes());
            } else {
                self.code.extend_from_slice(&[0x49, 0xB8 + (r - 8)]); // movabs rN, imm64
                self.code.extend_from_slice(&v.to_le_bytes());
            }
            return true;
        }
        let Some(&(off, _)) = self.var_offsets.get(name) else { return false };
        let corto = (-128..=127).contains(&off);
        let modrm = if corto { 0x45 } else { 0x85 };
        match self.type_stack_size(&tipo) {
            1 => self.code.push(0xC6),
            4 => self.code.push(0xC7),
            8 => {
                if i32::try_from(v).is_err() {
                    return false;
                }
                self.code.extend_from_slice(&[0x48, 0xC7]);
            }
            _ => return false,
        }
        self.code.push(modrm);
        if corto {
            self.code.push(off as u8);
        } else {
            self.code.extend_from_slice(&off.to_le_bytes());
        }
        if self.type_stack_size(&tipo) == 1 {
            self.code.push(v as u8);
        } else {
            self.code.extend_from_slice(&(v as i32).to_le_bytes());
        }
        true
    }

    /// `name = name <op> imm` con `name` en la matriz.
    fn emit_asignacion_en_sitio(&mut self, name: &str, val: &Expr) -> bool {
        let Some(&r) = self.var_regs.get(name) else { return false };
        let (a, b, ext) = match val {
            Expr::Add(a, b) => (a, b, 0u8),
            Expr::Sub(a, b) => (a, b, 5),
            Expr::BitAnd(a, b) => (a, b, 4),
            Expr::BitOr(a, b) => (a, b, 1),
            Expr::BitXor(a, b) => (a, b, 6),
            _ => return false,
        };
        if !matches!(a.as_ref(), Expr::Var(v) if v == name) {
            return false;
        }
        let Some(tipo) = self.var_type_of(name) else { return false };
        // Un puntero avanza ELEMENTOS: el inmediato se multiplica por el paso,
        // y solo para sumar y restar -- una mascara sobre un puntero no escala.
        let imm = match (&tipo, ext) {
            (TypeSpec::Ptr(_), 0 | 5) => {
                let paso = self.paso_de_puntero(name) as i64;
                let v = match inmediato_de(b) {
                    Some(Inmediato::Corto(c)) => c as i64,
                    Some(Inmediato::Largo(l)) => l as i64,
                    None => return false,
                };
                match i32::try_from(v.wrapping_mul(paso)) {
                    Ok(x) if i8::try_from(x).is_ok() => Inmediato::Corto(x as i8),
                    Ok(x) => Inmediato::Largo(x),
                    Err(_) => return false,
                }
            }
            (TypeSpec::Ptr(_), _) => return false,
            _ => match inmediato_de(b) {
                Some(i) => i,
                None => return false,
            },
        };
        self.emit_alu_en_registro(r, ext, imm);
        self.emit_recorte_en_registro(r, &tipo);
        true
    }

    /// `name++` / `name--` con `name` en la matriz: su paso, en sitio.
    fn emit_paso_en_sitio(&mut self, name: &str, restar: bool) -> bool {
        let Some(&r) = self.var_regs.get(name) else { return false };
        let Some(tipo) = self.var_type_of(name) else { return false };
        let paso = self.paso_de_puntero(name);
        let imm = match i32::try_from(paso) {
            Ok(p) if i8::try_from(p).is_ok() => Inmediato::Corto(p as i8),
            Ok(p) => Inmediato::Largo(p),
            Err(_) => return false,
        };
        self.emit_alu_en_registro(r, if restar { 5 } else { 0 }, imm);
        self.emit_recorte_en_registro(r, &tipo);
        true
    }

    /// `op rN, imm` -- el grupo 1 sobre un registro extendido: REX.W + REX.B.
    fn emit_alu_en_registro(&mut self, r: u8, ext: u8, imm: Inmediato) {
        let modrm = 0xC0 | (ext << 3) | (r - 8);
        match imm {
            Inmediato::Corto(c) => self.code.extend_from_slice(&[0x49, 0x83, modrm, c as u8]),
            Inmediato::Largo(l) => {
                self.code.extend_from_slice(&[0x49, 0x81, modrm]);
                self.code.extend_from_slice(&l.to_le_bytes());
            }
        }
    }

    /// El recorte de `emit_guardar_en_registro`, hecho de `rN` a `rN`.
    ///
    /// `reg` = rN y `rm` = rN: REX.R y REX.B a la vez (`4D` con W, `45` sin).
    fn emit_recorte_en_registro(&mut self, r: u8, tipo: &TypeSpec) {
        let modrm = 0xC0 | ((r - 8) << 3) | (r - 8);
        match tipo {
            TypeSpec::Char => self.code.extend_from_slice(&[0x4D, 0x0F, 0xBE, modrm]),
            TypeSpec::UnsignedChar => self.code.extend_from_slice(&[0x4D, 0x0F, 0xB6, modrm]),
            TypeSpec::Short => self.code.extend_from_slice(&[0x4D, 0x0F, 0xBF, modrm]),
            TypeSpec::UnsignedShort => self.code.extend_from_slice(&[0x4D, 0x0F, 0xB7, modrm]),
            TypeSpec::Int => self.code.extend_from_slice(&[0x4D, 0x63, modrm]),
            TypeSpec::UnsignedInt => self.code.extend_from_slice(&[0x45, 0x89, modrm]),
            // Ocho bytes: no hay nada que ensanchar.
            _ => {}
        }
    }
}

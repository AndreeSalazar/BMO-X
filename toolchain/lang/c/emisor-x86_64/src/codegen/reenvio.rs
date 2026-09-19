//! **EMITIR UN REENVIO**: mover los registros y saltar. La decision vive en
//! `decidir/reenvio.rs`; esto son los bytes.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- si el `jmp` va a la funcion equivocada o el intrinseco
//!            no normaliza su retorno, la primera fila que reenvie lo ve
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! Sin prologo ni marco: la funcion no tiene locales, no toca la pila y no
//! guarda nada. Un reenvio a funcion es una LLAMADA DE COLA (`jmp`): el `ret`
//! del destino vuelve a quien nos llamo. Un reenvio a intrinseco suelta los
//! bytes, normaliza el retorno como cualquier intrinseco y hace `ret`.

use crate::ast::Function;

use super::decidir::llamada::REGISTROS;
use super::decidir::reenvio::{bailar, detectar, Destino, Paso, Reenvio};
use super::operando::registro_por_nombre;
use super::{CallReloc, Codegen, TargetProfile};

impl Codegen {
    /// Es `func` un reenvio? (ver `decidir/reenvio.rs`)
    pub(super) fn detectar_reenvio(&self, func: &Function) -> Option<Reenvio> {
        detectar(
            func,
            |i| {
                let t = &func.params[i].typ;
                !self.es_agregado(t) && !Self::is_float_ty(t)
            },
            |nombre, n| {
                if let Some(def) = self.intrinsics.get(nombre) {
                    if def.args.len() != n || def.args.iter().any(|r| registro_por_nombre(r).is_none()) {
                        return None;
                    }
                    return Some(Destino::Intrinseco(nombre.to_string()));
                }
                if self.variadicas.contains(nombre) || n > REGISTROS.len() {
                    return None;
                }
                // una funcion de verdad: definida aqui o con prototipo, y no
                // el nombre de un parametro (eso seria una llamada por puntero)
                if !(self.known_functions.contains(nombre) || self.solo_prototipo(nombre)) {
                    return None;
                }
                if func.params.iter().any(|p| p.name == nombre) {
                    return None;
                }
                let (params, _) = self.firmas.get(nombre)?;
                if params.len() != n || params.iter().any(|t| self.es_agregado(t) || Self::is_float_ty(t)) {
                    return None;
                }
                Some(Destino::Funcion(nombre.to_string()))
            },
            |e| match e {
                crate::ast::Expr::Int(v) => Some(*v),
                crate::ast::Expr::CharLit(c) => Some(*c as i64),
                _ => super::decidir::plegado::constante_para_emitir(e),
            },
        )
    }

    /// Los bytes del reenvio.
    pub(super) fn emit_reenvio(&mut self, r: &Reenvio) {
        let destinos: Vec<u8> = match &r.destino {
            Destino::Intrinseco(n) => self.intrinsics.get(n).expect("lo dio detectar").args.iter()
                .map(|s| registro_por_nombre(s).expect("lo comprobo detectar")).collect(),
            Destino::Funcion(_) => REGISTROS[..r.fuentes.len()].to_vec(),
        };
        for paso in bailar(&r.fuentes, &destinos) {
            match paso {
                Paso::Mover(d, s) => self.code.extend_from_slice(&[0x48 | ((d >> 3) << 2) | (s >> 3), 0x8B, 0xC0 | ((d & 7) << 3) | (s & 7)]),
                Paso::Poner(d, v) => self.emit_mov_reg_imm(d, v),
            }
        }
        match &r.destino {
            Destino::Intrinseco(n) => {
                let def = self.intrinsics.get(n).expect("lo dio detectar");
                let (bytes, returns) = (def.bytes.clone(), def.returns.clone());
                self.code.extend_from_slice(&bytes);
                self.emit_intrinsic_return(returns.as_deref());
                self.code.push(0xC3); // ret
            }
            Destino::Funcion(n) => {
                self.code.push(0xE9); // jmp rel32: la llamada de cola
                self.call_relocs.push(CallReloc { offset: self.code.len(), target: n.clone() });
                self.code.extend_from_slice(&[0, 0, 0, 0]);
                if self.target == TargetProfile::Ring3App && !self.function_offsets.contains_key(n) {
                    self.stdlib_imports.insert(n.clone());
                }
            }
        }
    }
}

//! **LA DIRECCION** -- variables, punteros, campos, indices.
//!
//! [fase]     EMISION
//!
//! [aparece]  DENTRO -- un desplazamiento de mas lee al vecino o lo pisa. A
//!            veces peta al momento --y entonces es barato-- y a veces no, y
//!            entonces el sintoma sale tres funciones despues
//!
//! [carril]   ROJO -- y NO se elige: sale de su `[aparece]` (DENTRO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//!
//! [cuesta]   DATO -- escribir en el sitio equivocado corrompe lo que hubiera
//!
//! [riesgo]   ESPEJO SILENCIO
//!            ESPEJO -- la disposicion de un agregado se calcula AQUI y tambien
//!                     en `parser/types.rs`. Dos aritmeticas sobre la misma
//!                     estructura, y si se separan el campo se lee corrido
//!            SILENCIO -- pisar memoria que nadie vuelve a mirar no se nota
//!
//! # Por que estos brazos y no otros
//!
//! Porque todos contestan la misma pregunta: **donde esta**. Lo que hagan con
//! ese sitio --leer, escribir, incrementar-- es secundario; lo que comparten es
//! que un error suyo es un error de DIRECCION, y esos se pagan en la memoria de
//! otro.

use crate::ast::*;

use super::super::indexing::Por;
use super::super::{Codegen, Fixup};

impl Codegen {
    /// Los brazos de este carril. El despacho vive en `emitir/mod.rs`
    /// y es EXHAUSTIVO: si manana nace una forma nueva de expresion,
    /// el compilador para alli y no aqui.
    pub(super) fn emitir_direccion(&mut self, expr: &Expr) {
        match expr {
            Expr::StringLit(s) => {
                // lea rax, [rip + disp] -- fixup patched in patch_string_fixups
                self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                let idx = self.strings.iter().position(|t| t == s).unwrap_or(0);
                self.fixups.push(Fixup { lea_offset: self.code.len() - 4, string_idx: idx });
            }
            Expr::Var(name) => {
                self.emit_load_var(name);
            }
            Expr::Assign(name, val) => {
                // `p = q` con `p` agregado: se copian sus BYTES, todos.
                //
                // Antes caia al camino normal --`mov rax,[q]` + `mov [p],rax`--
                // que se lleva ocho y deja el resto con lo que hubiera. Un
                // struct de 12 se copiaba a medias, en silencio.
                if let Some(t) = self.var_type_of(name) {
                    if self.es_agregado(&t) {
                        let bytes = self.type_stack_size(&t);
                        let destino = Expr::Var(name.clone());
                        self.emit_asigna_agregado(&destino, val, bytes);
                        return;
                    }
                }
                // Asignacion a variable float/double -> ruta SSE.
                if self.var_type_of(name).map_or(false, |t| Self::is_float_ty(&t)) {
                    self.emit_fexpr_operand(val);
                    self.store_float_var(name);
                } else {
                    self.emit_expr(val);
                    self.emit_store_var(name);
                }
            }
            Expr::PreInc(name) => {
                self.emit_inc_var(name);
                // rax already has new value
            }
            Expr::PreDec(name) => {
                self.emit_dec_var(name);
            }
            Expr::PostInc(name) => {
                self.emit_load_var(name);
                self.code.push(0x50); // push old value
                self.emit_inc_var(name);
                self.code.push(0x58); // pop rax (old value)
            }
            Expr::PostDec(name) => {
                self.emit_load_var(name);
                self.code.push(0x50);
                self.emit_dec_var(name);
                self.code.push(0x58);
            }
            // `*p` debe leer el TAMANO DEL APUNTADO, no siempre 8 bytes.
            // Antes `*(p+1)` con `int *p` leia 8 bytes desde la posicion
            // correcta, o sea dos enteros pegados: devolvia 504403158366158848
            // en vez de 6.
            Expr::Deref(a) => self.emit_leer_por_puntero(a),
            Expr::AddrOf(inner) => {
                match inner.as_ref() {
                    Expr::Var(name) => {
                        if let Some(&(offset, _)) = self.var_offsets.get(name) {
                            if offset >= -128 && offset <= 127 {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x45, offset as u8]);
                            } else {
                                self.code.extend_from_slice(&[0x48, 0x8D, 0x85]);
                                self.code.extend_from_slice(&(offset as i32).to_le_bytes());
                            }
                        } else if self.global_offsets.contains_key(name) {
                            self.code.extend_from_slice(&[0x48, 0x8D, 0x05, 0, 0, 0, 0]);
                            self.global_fixups.push((self.code.len() - 4, name.clone()));
                        } else if self.known_functions.contains(name) {
                            // &myfunc -- direccion de la funcion
                            self.emit_func_addr(name);
                        } else { self.emit_xor_eax(); }
                    }
                    Expr::Subscript(name, idx) => {
                        self.emit_subscript_addr(name, idx);
                    }
                    Expr::Deref(ptr) => {
                        self.emit_expr(ptr); // rax = address of the pointed-to data
                    }
                    // ** LAS TRES QUE FALTABAN, y las tres MATARON A DOOM.
                    //
                    // `&c->defaults[i]` es `AddrOf(IndexPtr(..))`, y hasta el
                    // 2026-08-13 caia en el `_` de abajo: la direccion salia
                    // CERO, en silencio, y `SearchCollection` de `m_config.c`
                    // devolvia `NULL` habiendo ENCONTRADO su entrada. DOOM se
                    // mataba con `I_Error` a 56.465 lineas de aqui.
                    //
                    // Las tres son la version SIN CARGA de los brazos que ya
                    // existen mas abajo: `Field`, `Arrow` e `IndexPtr` calculan
                    // la direccion y luego llaman a `emit_load_elem`. Tomar la
                    // direccion es exactamente eso menos el ultimo paso.
                    Expr::IndexPtr(base, index) => {
                        let elem = &self.exige_tipo(self.pointee_type(base), "a que apunta este puntero", "Declara el tipo del puntero, o pon un cast: `*(int*)p`.");
                        self.emit_index_ptr_addr(base, index, &elem.clone());
                    }
                    Expr::Field(base, _campo) => {
                        let offset = &self.offset_de_valor(base, _campo);
                        self.emit_expr_as_ptr(base);
                        self.emit_add_offset(*offset);
                    }
                    Expr::Arrow(ptr, _campo) => {
                        let offset = &self.offset_por_puntero(ptr, _campo);
                        self.emit_expr(ptr);
                        self.emit_add_offset(*offset);
                    }
                    // ** Y ESTE BRAZO YA NO RELLENA DE CEROS: GRITA.
                    //
                    // Era la tercera vez que el mismo `_ =>` mudo costaba un
                    // dia de fotos --el `char *mapa` del raycaster
                    // (`2bc13367`), las relocations que no existian
                    // (`46506e51`), y esta--. El patron es siempre el mismo:
                    // un brazo por defecto que produce un valor LEGITIMO (cero
                    // es una direccion valida de escribir en cualquier
                    // expresion) para el caso "no supe traducirlo".
                    //
                    // Un compilador que no sabe tomar una direccion tiene que
                    // decirlo AQUI, donde la frase esta entera, y no dejar que
                    // el programa lo descubra en metal.
                    otro => {
                        self.errors.push(format!(
                            "no se de que forma tomar la direccion de esta expresion: {otro:?}"
                        ));
                        self.emit_xor_eax();
                    }
                }
            }
            Expr::Subscript(name, index) => {
                // direccion exacta (array o puntero) + carga del TAMANO del elemento
                self.emit_subscript_addr(name, index);
                let elem = self.elem_type_of(name);
                self.emit_load_elem(&elem);
            }
            // ** `E1 op= E2` con la direccion de `E1` calculada UNA vez.
            Expr::AssignOp(lvalue, kind, rhs) => self.emit_assign_op(lvalue, *kind, rhs),
            Expr::AssignSubscript(name, index, val) => {
                // La puerta SSE: si el elemento es flotante, se guarda por xmm0.
                let lv = Expr::Subscript(name.clone(), index.clone());
                if self.emit_guardar_flotante(&lv, val) { return; }
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_subscript_addr(name, index); // rax = direccion
                self.code.push(0x5A);         // pop rdx = valor
                let elem = self.elem_type_of(name);
                self.emit_store_elem(&elem);  // [rax] = rdx (tamano exacto)
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]); // rax = valor (resultado del assign)
            }
            Expr::IndexPtr(base, index) => {
                let elem = &self.exige_tipo(self.pointee_type(base), "a que apunta este puntero", "Declara el tipo del puntero, o pon un cast: `*(int*)p`.");
                // p->arr[i]: direccion = base(puntero) + i*sizeof(elem), luego load
                self.emit_index_ptr_addr(base, index, elem);
                self.emit_load_elem(&elem.clone());
            }
            Expr::AssignIndexPtr(base, index, val) => {
                // La puerta SSE: si el elemento es flotante, se guarda por xmm0.
                let lv = Expr::IndexPtr(base.clone(), index.clone());
                if self.emit_guardar_flotante(&lv, val) { return; }
                let elem = &self.exige_tipo(self.pointee_type(base), "a que apunta este puntero", "Declara el tipo del puntero, o pon un cast: `*(int*)p`.");
                self.emit_expr(val);          // rax = valor
                self.code.push(0x50);         // push valor
                self.emit_index_ptr_addr(base, index, elem); // rax = direccion
                self.code.push(0x5A);         // pop rdx = valor
                self.emit_store_elem(&elem.clone());
                self.code.extend_from_slice(&[0x48, 0x89, 0xD0]);
            }
            Expr::Field(base, campo) => self.emit_leer_campo(base, campo, Por::Valor),
            Expr::Arrow(ptr, campo) => self.emit_leer_campo(ptr, campo, Por::Puntero),
            Expr::AssignField(base, campo, val) => {
                // La puerta SSE: si el elemento es flotante, se guarda por xmm0.
                let lv = Expr::Field(base.clone(), campo.clone());
                if self.emit_guardar_flotante(&lv, val) { return; }
                self.emit_guardar_campo(base, campo, Por::Valor, val)
            }
            Expr::AssignDeref(addr, val) => {
                // La puerta SSE: si el elemento es flotante, se guarda por xmm0.
                let lv = Expr::Deref(addr.clone());
                if self.emit_guardar_flotante(&lv, val) { return; }
                self.emit_guardar_por_puntero(addr, val)
            }
            Expr::AssignArrow(ptr, campo, val) => {
                // La puerta SSE: si el elemento es flotante, se guarda por xmm0.
                let lv = Expr::Arrow(ptr.clone(), campo.clone());
                if self.emit_guardar_flotante(&lv, val) { return; }
                self.emit_guardar_campo(ptr, campo, Por::Puntero, val)
            }
            _ => unreachable!("el despacho de emit_expr y este carril no dicen lo mismo"),
        }
    }
}

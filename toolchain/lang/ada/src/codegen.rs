//! De Ada a bytes x86-64. Sin IR, sin optimizador central, sin runtime.
//!
//! ## El decimal, que es la razón de todo
//!
//! Un `type Saldo is delta 0.01 digits 12` se guarda como **entero escalado**:
//! `19.99` es el entero `1999`. Sumar dos saldos es un `add` — suma decimal
//! exacta, sin coma flotante y sin redondeo. Multiplicar dos escalas de 2 da
//! escala 4, así que se divide entre 100 para volver; dividir hace lo
//! contrario, preescalar el dividendo.
//!
//! Es exactamente lo que hace el frontend de COBOL, y no es copia: **es que
//! Annex F de Ada copió las reglas de COBOL**. Dos lenguajes que dicen lo mismo
//! acaban en la misma aritmética.
//!
//! ## Lo que NO hay
//!
//! Ni una llamada a un runtime de Ada. `Put_Line` baja a `bmo_lower::console`
//! y de ahí al único syscall que existe. Un `.bex` de Ada no enlaza nada.

use std::collections::HashMap;

use bmo_abi::bef::{BefBuilder, BefSection};
use bmo_lower::x86;

use crate::ast::*;

/// Registros que usa este emisor. `rax` es el acumulador, `rdx` el segundo
/// operando y `rcx` el factor de escala.
const RAX: u8 = 0;
const RCX: u8 = 1;
const RDX: u8 = 2;

pub fn compilar(p: &Programa) -> Result<Vec<u8>, AdaError> {
    let mut c = Codegen::nuevo();
    c.programa(p)?;
    let mut b = BefBuilder::new();
    b.add_section(BefSection::code(core::mem::take(&mut c.code)));
    b.entry_offset = 0;
    Ok(b.build().unwrap_or_default())
}

struct Codegen {
    code: Vec<u8>,
    /// Dónde vive cada variable, respecto de `rbp`.
    huecos: HashMap<String, i32>,
    /// Y con cuántos decimales. Es la llave del decimal exacto.
    escalas: HashMap<String, u32>,
    pila: i32,
    errores: Vec<AdaError>,
    /// Saltos pendientes de resolver: (posición del campo, etiqueta).
    saltos: Vec<(usize, u32)>,
    etiquetas: HashMap<u32, usize>,
    siguiente: u32,
}

impl Codegen {
    fn nuevo() -> Self {
        Self {
            code: Vec::new(),
            huecos: HashMap::new(),
            escalas: HashMap::new(),
            pila: 0,
            errores: Vec::new(),
            saltos: Vec::new(),
            etiquetas: HashMap::new(),
            siguiente: 0,
        }
    }

    fn etiqueta(&mut self) -> u32 {
        self.siguiente += 1;
        self.siguiente
    }

    fn fijar(&mut self, l: u32) {
        let aqui = self.code.len();
        self.etiquetas.insert(l, aqui);
    }

    /// `jmp rel32`, pendiente de parchear.
    fn saltar(&mut self, l: u32) {
        self.code.push(0xE9);
        self.saltos.push((self.code.len(), l));
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    /// `jcc rel32` con el segundo byte del opcode. Siempre rel32: el cuerpo de
    /// un bucle puede pasar de 127 bytes, y un salto que se desborda en
    /// silencio es peor que uno largo de más.
    fn saltar_si(&mut self, cc: u8, l: u32) {
        self.code.extend_from_slice(&[0x0F, cc]);
        self.saltos.push((self.code.len(), l));
        self.code.extend_from_slice(&[0, 0, 0, 0]);
    }

    fn resolver_saltos(&mut self) {
        for (campo, l) in std::mem::take(&mut self.saltos) {
            let destino = match self.etiquetas.get(&l) {
                Some(&d) => d,
                // Una etiqueta usada y nunca fijada es un bug del emisor, no
                // del programa: se aborta en vez de saltar a ninguna parte.
                None => panic!("etiqueta {l} usada y nunca fijada"),
            };
            let rel = (destino as i64 - (campo as i64 + 4)) as i32;
            self.code[campo..campo + 4].copy_from_slice(&rel.to_le_bytes());
        }
    }

    // ── Memoria ─────────────────────────────────────────────────────────

    fn cargar(&mut self, nombre: &str) {
        match self.huecos.get(nombre).copied() {
            Some(off) => {
                // mov rax, [rbp+off]
                self.code.extend_from_slice(&[0x48, 0x8B, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
            None => self.errores.push(AdaError::nuevo(
                0,
                format!("'{}' no esta declarada", nombre.to_ascii_lowercase()),
            )),
        }
    }

    fn guardar(&mut self, nombre: &str) {
        match self.huecos.get(nombre).copied() {
            Some(off) => {
                // mov [rbp+off], rax
                self.code.extend_from_slice(&[0x48, 0x89, 0x85]);
                self.code.extend_from_slice(&off.to_le_bytes());
            }
            None => self.errores.push(AdaError::nuevo(
                0,
                format!("'{}' no esta declarada", nombre.to_ascii_lowercase()),
            )),
        }
    }

    fn escala_de(&self, nombre: &str) -> u32 {
        self.escalas.get(nombre).copied().unwrap_or(0)
    }

    /// Un literal escrito a su entero escalado. `"19.99"` con escala 2 → 1999.
    ///
    /// **Trunca** los decimales que sobran, que es lo que hace un tipo decimal
    /// cuando le das más precisión de la que declara.
    pub fn escalar(lit: &str, escala: u32) -> i64 {
        let t = lit.trim();
        let negativo = t.starts_with('-');
        let s = t.trim_start_matches(['+', '-']);
        let (ent, frac) = s.split_once('.').unwrap_or((s, ""));
        let entero: i64 = ent.parse().unwrap_or(0);
        let mut f = frac.to_string();
        while (f.len() as u32) < escala {
            f.push('0');
        }
        f.truncate(escala as usize);
        let dec: i64 = if f.is_empty() { 0 } else { f.parse().unwrap_or(0) };
        let v = entero * 10i64.pow(escala) + dec;
        if negativo {
            -v
        } else {
            v
        }
    }

    /// Lleva `rax` de una escala a otra multiplicando o dividiendo por 10^n.
    fn reescalar(&mut self, de: u32, a: u32) {
        if de == a {
            return;
        }
        if a > de {
            let f = 10u64.pow(a - de);
            x86::mov_r64_imm64(&mut self.code, RCX, f);
            x86::imul_r64_r64(&mut self.code, RAX, RCX);
        } else {
            let f = 10u64.pow(de - a);
            x86::mov_r64_imm64(&mut self.code, RCX, f);
            x86::cqo(&mut self.code);
            x86::idiv_r64(&mut self.code, RCX);
        }
    }

    // ── Expresiones ─────────────────────────────────────────────────────

    /// Deja el valor de `e` en `rax`, en la escala `destino`.
    fn expresion(&mut self, e: &Expr, destino: u32) {
        match e {
            Expr::Literal(n) => {
                let v = Self::escalar(n, destino);
                x86::mov_r64_imm64(&mut self.code, RAX, v as u64);
            }
            Expr::Nombre(n) => {
                let de = self.escala_de(n);
                self.cargar(n);
                self.reescalar(de, destino);
            }
            Expr::Binaria(a, op, b) => {
                match op {
                    '+' | '-' => {
                        // Los dos lados en la MISMA escala; entonces sumar es
                        // sumar céntimos.
                        self.expresion(a, destino);
                        self.code.push(0x50); // push rax
                        self.expresion(b, destino);
                        self.code.push(0x5A); // pop rdx
                        if *op == '+' {
                            x86::add_r64_r64(&mut self.code, RAX, RDX);
                        } else {
                            // rdx - rax, y el resultado a rax.
                            x86::sub_r64_r64(&mut self.code, RDX, RAX);
                            x86::mov_r64_r64(&mut self.code, RAX, RDX);
                        }
                    }
                    '*' => {
                        // Escala n × escala n = escala 2n; se vuelve dividiendo
                        // entre 10^n. $2.00 × 3 = $6.00, exacto.
                        self.expresion(a, destino);
                        self.code.push(0x50);
                        self.expresion(b, destino);
                        self.code.push(0x5A);
                        x86::imul_r64_r64(&mut self.code, RAX, RDX);
                        if destino > 0 {
                            x86::mov_r64_imm64(&mut self.code, RCX, 10u64.pow(destino));
                            x86::cqo(&mut self.code);
                            x86::idiv_r64(&mut self.code, RCX);
                        }
                    }
                    _ => {
                        // Dividir: se PREESCALA el dividendo, si no el
                        // resultado saldría en escala 0 y se perderían los
                        // céntimos. $10.00 / 4 = $2.50.
                        self.expresion(b, destino); // divisor
                        self.code.push(0x50);
                        self.expresion(a, destino); // dividendo
                        if destino > 0 {
                            x86::mov_r64_imm64(&mut self.code, RCX, 10u64.pow(destino));
                            x86::imul_r64_r64(&mut self.code, RAX, RCX);
                        }
                        self.code.push(0x59); // pop rcx (divisor)
                        x86::cqo(&mut self.code);
                        x86::idiv_r64(&mut self.code, RCX);
                    }
                }
            }
        }
    }

    /// La escala con la que hay que comparar dos expresiones: la mayor de las
    /// dos, para no perder decimales al comparar.
    fn escala_expr(&self, e: &Expr) -> u32 {
        match e {
            Expr::Literal(n) => match n.split_once('.') {
                Some((_, d)) => d.len() as u32,
                None => 0,
            },
            Expr::Nombre(n) => self.escala_de(n),
            Expr::Binaria(a, _, b) => self.escala_expr(a).max(self.escala_expr(b)),
        }
    }

    /// Salta a `destino` cuando la condición es FALSA.
    fn saltar_si_falsa(&mut self, c: &Condicion, destino: u32) {
        let escala = self.escala_expr(&c.izq).max(self.escala_expr(&c.der));
        self.expresion(&c.izq, escala);
        self.code.push(0x50); // push rax
        self.expresion(&c.der, escala);
        self.code.push(0x5A); // pop rdx  (el izquierdo)
        x86::cmp_r64_r64(&mut self.code, RDX, RAX);
        // El código de condición es el CONTRARIO: se salta cuando NO se cumple.
        let cc = match c.op.as_str() {
            "=" => 0x85,  // jne
            "/=" => 0x84, // je
            ">" => 0x8E,  // jle
            "<" => 0x8D,  // jge
            ">=" => 0x8C, // jl
            "<=" => 0x8F, // jg
            _ => 0x85,
        };
        self.saltar_si(cc, destino);
    }

    // ── Sentencias ──────────────────────────────────────────────────────

    fn sentencia(&mut self, s: &Sentencia) {
        match s {
            // `null;` no emite nada, y eso NO es un no-op silencioso: es lo
            // que la sentencia significa. La diferencia con un hueco es que
            // aquí alguien lo escribió.
            Sentencia::Nada => {}
            Sentencia::PutLiteral(t) => {
                let mut bytes = t.as_bytes().to_vec();
                bytes.push(b'\n');
                bmo_lower::console::write_const(&mut self.code, &bytes);
            }
            Sentencia::PutValor(n) => {
                if !self.huecos.contains_key(n) {
                    self.errores.push(AdaError::nuevo(
                        0,
                        format!("Put_Line({}): no esta declarada", n.to_ascii_lowercase()),
                    ));
                    return;
                }
                let escala = self.escala_de(n);
                self.cargar(n);
                bmo_lower::fmt::write_decimal_scaled(&mut self.code, escala);
                bmo_lower::console::write_const(&mut self.code, b"\n");
            }
            Sentencia::Asignar(n, e) => {
                if !self.huecos.contains_key(n) {
                    self.errores.push(AdaError::nuevo(
                        0,
                        format!("'{}' no esta declarada", n.to_ascii_lowercase()),
                    ));
                    return;
                }
                let escala = self.escala_de(n);
                self.expresion(e, escala);
                self.guardar(n);
            }
            Sentencia::Si(cond, entonces, si_no) => {
                let e_else = self.etiqueta();
                let e_fin = self.etiqueta();
                self.saltar_si_falsa(cond, e_else);
                for s in entonces {
                    self.sentencia(s);
                }
                self.saltar(e_fin);
                self.fijar(e_else);
                for s in si_no {
                    self.sentencia(s);
                }
                self.fijar(e_fin);
            }
            Sentencia::Mientras(cond, cuerpo) => {
                let e_top = self.etiqueta();
                let e_fin = self.etiqueta();
                self.fijar(e_top);
                self.saltar_si_falsa(cond, e_fin);
                for s in cuerpo {
                    self.sentencia(s);
                }
                self.saltar(e_top);
                self.fijar(e_fin);
            }
        }
    }

    // ── El programa entero ──────────────────────────────────────────────

    fn programa(&mut self, p: &Programa) -> Result<(), AdaError> {
        // Un hueco de 8 bytes por variable. Todo valor es un entero de 64 bits
        // con signo: la escala dice dónde cae la coma, no cuánto ocupa.
        for d in &p.declaraciones {
            if self.huecos.contains_key(&d.nombre) {
                return Err(AdaError::nuevo(
                    0,
                    format!("'{}' esta declarada dos veces", d.nombre.to_ascii_lowercase()),
                ));
            }
            self.pila += 8;
            self.huecos.insert(d.nombre.clone(), -self.pila);
            self.escalas.insert(d.nombre.clone(), d.escala);
        }

        // Prólogo. Se reserva y se alinea a 64 igual que los demás frontends:
        // a la entrada de un proceso BEF no se puede suponer nada del RSP.
        self.code.extend_from_slice(&[0x55]); // push rbp
        self.code.extend_from_slice(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
        self.code.extend_from_slice(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
        self.code.extend_from_slice(&((self.pila as u32) + 63).to_le_bytes());
        self.code.extend_from_slice(&[0x48, 0x83, 0xE4, 0xC0]); // and rsp, -64

        // Los valores iniciales. En Ada una variable sin `:=` no tiene valor
        // definido; aquí se pone a cero, que es lo único honesto que se puede
        // hacer sin inventar: leer basura de la pila sería peor.
        for d in &p.declaraciones {
            let v = match &d.inicial {
                Some(lit) => Self::escalar(lit, d.escala),
                None => 0,
            };
            x86::mov_r64_imm64(&mut self.code, RAX, v as u64);
            self.guardar(&d.nombre);
        }

        for s in &p.cuerpo {
            self.sentencia(s);
        }

        // Salir por la puerta. No hay `hlt`: es privilegiada, y en Ring 3 sería
        // un #GP — la red de seguridad provocando justo el fallo del que
        // protege. La puerta gira en `pause`.
        bmo_lower::task::exit(&mut self.code);
        self.resolver_saltos();

        if let Some(e) = self.errores.first() {
            return Err(e.clone());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::Codegen;

    /// El alma del asunto: un literal decimal es un entero exacto.
    #[test]
    fn los_literales_se_escalan_a_centimos_exactos() {
        assert_eq!(Codegen::escalar("19.99", 2), 1999);
        assert_eq!(Codegen::escalar("0.01", 2), 1);
        assert_eq!(Codegen::escalar("7", 2), 700);
        assert_eq!(Codegen::escalar("-120.00", 2), -12000);
        // Y sumar céntimos es exacto, que es todo lo que se pide:
        assert_eq!(Codegen::escalar("10.05", 2) + Codegen::escalar("3.20", 2), 1325);
    }

    /// Más decimales de los que declara el tipo: se truncan, no se redondean.
    #[test]
    fn los_decimales_que_sobran_se_truncan() {
        assert_eq!(Codegen::escalar("1.999", 2), 199);
    }

    #[test]
    fn escala_cero_es_un_entero_normal() {
        assert_eq!(Codegen::escalar("42", 0), 42);
        assert_eq!(Codegen::escalar("-7", 0), -7);
    }
}

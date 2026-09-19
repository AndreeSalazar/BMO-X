//! **LOS AMBITOS** -- una local que sombrea a otra deja de llamarse igual.
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- si esto resuelve un nombre al ambito equivocado, el
//!            programa lee OTRA variable, y el banco lo ve en la primera fila
//!            que declare la misma letra en dos bloques
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//! # El fallo que lo trajo (2026-09-18)
//!
//! El emisor guarda las locales POR NOMBRE, un hueco por funcion
//! (`frame.rs::build_var_map`: *"sombra: un solo slot"*). Dos declaraciones
//! con el mismo nombre en bloques hermanos compartian hueco Y TIPO -- el de la
//! primera:
//!
//! ```text
//!    { int a = 1; ... }  { unsigned a = 0xFFFFFFF0u; printf("%u", a); }
//!                                                      18446744073709551600
//! ```
//!
//! La segunda `a` se releia con `movsxd` porque el hueco decia `int`, y
//! `a < 3u` comparaba con signo. Salio al escribir una fila del banco, no en
//! un programa -- pero un programa con dos bucles `for (int i ...)` de tipos
//! distintos lo habria pagado igual, en silencio.
//!
//! # La solucion: renombrar, no reescribir el emisor
//!
//! El emisor entero es por nombre y funciona. Lo que falta es que **dos
//! variables distintas tengan dos nombres distintos**, y eso se resuelve ANTES
//! de emitir, sobre el arbol: la primera declaracion de `a` se queda `a`; la
//! segunda pasa a ser `a.2`, la tercera `a.3`, y cada uso se reescribe al que
//! le toca por las reglas de C (el mas interior visible, y solo desde su
//! declaracion hacia abajo). El punto es ilegal en un identificador de C, asi
//! que `a.2` no puede chocar con nada que el programa escriba -- el mismo
//! truco que `P.doble` en C++ y `funcion.variable`.
//!
//! ** Y es lo que hace que el resto del compilador no tenga que saber de
//! ambitos: el troquel, el escaner de direcciones, `var_type_of`, todos siguen
//! trabajando por nombre, y ahora el nombre ya es unico.
//!
//! # Lo que se renombra y lo que no
//!
//! ```text
//!    se renombra   Var, Assign, ++/--, Subscript, AssignSubscript, y el
//!                  nombre de Call (puede ser un puntero a funcion LOCAL)
//!    no            campos (Field/Arrow), intrinsecos, syscalls, etiquetas
//!                  de goto, y lo que no resuelve a una local: globales,
//!                  funciones y constantes de enum se quedan como estan
//! ```
//!
//! Los parametros son el ambito mas exterior: una local con el nombre de un
//! parametro lo sombrea y se renombra.

use std::collections::HashMap;

use crate::ast::{Case, Escritura, Expr, Stmt};

/// Renombra las sombras del cuerpo de una funcion. `params` son los nombres
/// de sus parametros, que ya ocupan el ambito exterior.
pub(in crate::codegen) fn renombrar_sombras(params: &[String], cuerpo: &[Stmt]) -> Vec<Stmt> {
    let mut a = Ambitos::default();
    a.abrir();
    for p in params {
        a.declarar(p);
    }
    a.abrir();
    let out = cuerpo.iter().map(|s| a.stmt(s)).collect();
    out
}

#[derive(Default)]
struct Ambitos {
    /// Un mapa por bloque abierto: nombre escrito -> nombre unico.
    pila: Vec<HashMap<String, String>>,
    /// Cuantas veces se ha declarado cada nombre en la funcion.
    vistos: HashMap<String, usize>,
}

impl Ambitos {
    fn abrir(&mut self) {
        self.pila.push(HashMap::new());
    }
    fn cerrar(&mut self) {
        self.pila.pop();
    }
    /// La primera declaracion se queda con su nombre; las demas, `nombre.N`.
    fn declarar(&mut self, nombre: &str) -> String {
        let n = self.vistos.entry(nombre.to_string()).or_insert(0);
        *n += 1;
        let unico = if *n == 1 { nombre.to_string() } else { format!("{nombre}.{n}") };
        self.pila.last_mut().expect("hay un ambito").insert(nombre.to_string(), unico.clone());
        unico
    }
    /// El mas interior visible; lo que no es local se queda como esta.
    fn resolver(&self, nombre: &str) -> String {
        self.pila
            .iter()
            .rev()
            .find_map(|m| m.get(nombre))
            .cloned()
            .unwrap_or_else(|| nombre.to_string())
    }

    fn bloque(&mut self, v: &[Stmt]) -> Vec<Stmt> {
        self.abrir();
        let out = v.iter().map(|s| self.stmt(s)).collect();
        self.cerrar();
        out
    }

    /// Un cuerpo de `if`/`while`/`for` es un ambito aunque no lleve llaves.
    fn cuerpo(&mut self, s: &Stmt) -> Box<Stmt> {
        self.abrir();
        let out = self.stmt(s);
        self.cerrar();
        Box::new(out)
    }

    fn stmt(&mut self, s: &Stmt) -> Stmt {
        match s {
            Stmt::Printf(t) => Stmt::Printf(t.clone()),
            Stmt::PrintfLn(t) => Stmt::PrintfLn(t.clone()),
            Stmt::If(c, a, b) => {
                let c = self.expr(c);
                let a = self.cuerpo(a);
                let b = b.as_ref().map(|x| self.cuerpo(x));
                Stmt::If(c, a, b)
            }
            Stmt::While(c, a) => {
                let c = self.expr(c);
                Stmt::While(c, self.cuerpo(a))
            }
            Stmt::DoWhile(a, c) => {
                let a = self.cuerpo(a);
                Stmt::DoWhile(a, self.expr(c))
            }
            Stmt::For(i, c, p, a) => {
                let i = i.as_ref().map(|e| self.expr(e));
                let c = c.as_ref().map(|e| self.expr(e));
                let p = p.as_ref().map(|e| self.expr(e));
                Stmt::For(i, c, p, self.cuerpo(a))
            }
            Stmt::Switch(e, casos) => {
                let e = self.expr(e);
                let casos = casos
                    .iter()
                    .map(|c| Case { value: c.value, stmts: self.bloque(&c.stmts) })
                    .collect();
                Stmt::Switch(e, casos)
            }
            Stmt::Break => Stmt::Break,
            Stmt::Continue => Stmt::Continue,
            Stmt::Return(e) => Stmt::Return(e.as_ref().map(|e| self.expr(e))),
            // C: la variable es visible desde su declarador, o sea que el
            // inicializador ya ve la NUEVA (`int a = a;` es la nueva, sin valor).
            Stmt::DeclAssign(t, n, init) => {
                let n = self.declarar(n);
                let init = init.as_ref().map(|e| self.expr(e));
                Stmt::DeclAssign(t.clone(), n, init)
            }
            Stmt::DeclInit(t, n, escrituras) => {
                let n = self.declarar(n);
                let escrituras = escrituras
                    .iter()
                    .map(|e| Escritura { offset: e.offset, tipo: e.tipo.clone(), valor: self.expr(&e.valor) })
                    .collect();
                Stmt::DeclInit(t.clone(), n, escrituras)
            }
            Stmt::Expr(e) => Stmt::Expr(self.expr(e)),
            Stmt::Block(v) => Stmt::Block(self.bloque(v)),
            Stmt::Goto(l) => Stmt::Goto(l.clone()),
            Stmt::Label(l) => Stmt::Label(l.clone()),
        }
    }

    fn caja(&mut self, e: &Expr) -> Box<Expr> {
        Box::new(self.expr(e))
    }

    fn lista(&mut self, v: &[Expr]) -> Vec<Expr> {
        v.iter().map(|e| self.expr(e)).collect()
    }

    fn expr(&mut self, e: &Expr) -> Expr {
        match e {
            Expr::Int(_) | Expr::FloatLit(_) | Expr::StringLit(_) | Expr::CharLit(_) => e.clone(),
            Expr::Var(n) => Expr::Var(self.resolver(n)),
            Expr::Call(n, args) => Expr::Call(self.resolver(n), self.lista(args)),
            Expr::Assign(n, v) => Expr::Assign(self.resolver(n), self.caja(v)),
            Expr::Neg(a) => Expr::Neg(self.caja(a)),
            Expr::Not(a) => Expr::Not(self.caja(a)),
            Expr::BitNot(a) => Expr::BitNot(self.caja(a)),
            Expr::PreInc(n) => Expr::PreInc(self.resolver(n)),
            Expr::PreDec(n) => Expr::PreDec(self.resolver(n)),
            Expr::PostInc(n) => Expr::PostInc(self.resolver(n)),
            Expr::PostDec(n) => Expr::PostDec(self.resolver(n)),
            Expr::Add(a, b) => Expr::Add(self.caja(a), self.caja(b)),
            Expr::Sub(a, b) => Expr::Sub(self.caja(a), self.caja(b)),
            Expr::Mul(a, b) => Expr::Mul(self.caja(a), self.caja(b)),
            Expr::Div(a, b) => Expr::Div(self.caja(a), self.caja(b)),
            Expr::Mod(a, b) => Expr::Mod(self.caja(a), self.caja(b)),
            Expr::Eq(a, b) => Expr::Eq(self.caja(a), self.caja(b)),
            Expr::Neq(a, b) => Expr::Neq(self.caja(a), self.caja(b)),
            Expr::Lt(a, b) => Expr::Lt(self.caja(a), self.caja(b)),
            Expr::Gt(a, b) => Expr::Gt(self.caja(a), self.caja(b)),
            Expr::Le(a, b) => Expr::Le(self.caja(a), self.caja(b)),
            Expr::Ge(a, b) => Expr::Ge(self.caja(a), self.caja(b)),
            Expr::BitAnd(a, b) => Expr::BitAnd(self.caja(a), self.caja(b)),
            Expr::BitXor(a, b) => Expr::BitXor(self.caja(a), self.caja(b)),
            Expr::BitOr(a, b) => Expr::BitOr(self.caja(a), self.caja(b)),
            Expr::LAnd(a, b) => Expr::LAnd(self.caja(a), self.caja(b)),
            Expr::LOr(a, b) => Expr::LOr(self.caja(a), self.caja(b)),
            Expr::Shl(a, b) => Expr::Shl(self.caja(a), self.caja(b)),
            Expr::Shr(a, b) => Expr::Shr(self.caja(a), self.caja(b)),
            Expr::Conditional(a, b, c) => Expr::Conditional(self.caja(a), self.caja(b), self.caja(c)),
            Expr::Comma(v) => Expr::Comma(self.lista(v)),
            Expr::Deref(a) => Expr::Deref(self.caja(a)),
            Expr::AddrOf(a) => Expr::AddrOf(self.caja(a)),
            Expr::Subscript(n, i) => Expr::Subscript(self.resolver(n), self.caja(i)),
            Expr::AssignSubscript(n, i, v) => Expr::AssignSubscript(self.resolver(n), self.caja(i), self.caja(v)),
            Expr::IndexPtr(a, b) => Expr::IndexPtr(self.caja(a), self.caja(b)),
            Expr::AssignIndexPtr(a, b, c) => Expr::AssignIndexPtr(self.caja(a), self.caja(b), self.caja(c)),
            Expr::CallPtr(f, args) => Expr::CallPtr(self.caja(f), self.lista(args)),
            Expr::Field(a, c) => Expr::Field(self.caja(a), c.clone()),
            Expr::Arrow(a, c) => Expr::Arrow(self.caja(a), c.clone()),
            Expr::AssignField(a, c, v) => Expr::AssignField(self.caja(a), c.clone(), self.caja(v)),
            Expr::AssignArrow(a, c, v) => Expr::AssignArrow(self.caja(a), c.clone(), self.caja(v)),
            Expr::AssignDeref(a, v) => Expr::AssignDeref(self.caja(a), self.caja(v)),
            Expr::AssignOp(a, op, v) => Expr::AssignOp(self.caja(a), *op, self.caja(v)),
            Expr::Cast(t, a) => Expr::Cast(t.clone(), self.caja(a)),
            Expr::Intrinsic(n, args) => Expr::Intrinsic(n.clone(), self.lista(args)),
            Expr::Syscall(d, args) => Expr::Syscall(d.clone(), self.lista(args)),
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    fn cuerpo(src: &str) -> (Vec<String>, Vec<Stmt>) {
        let p = crate::parse(&format!("int main(int n) {{ {src} return 0; }}")).expect("parsea");
        let f = p.functions.into_iter().find(|f| f.name == "main").unwrap();
        (f.params.iter().map(|p| p.name.clone()).collect(), f.body)
    }

    /// Los nombres declarados, en orden, tras renombrar.
    fn declarados(src: &str) -> Vec<String> {
        let (params, body) = cuerpo(src);
        let mut out = Vec::new();
        fn recoger(s: &Stmt, out: &mut Vec<String>) {
            match s {
                Stmt::DeclAssign(_, n, _) | Stmt::DeclInit(_, n, _) => out.push(n.clone()),
                Stmt::Block(v) => v.iter().for_each(|x| recoger(x, out)),
                Stmt::If(_, a, b) => {
                    recoger(a, out);
                    if let Some(b) = b {
                        recoger(b, out);
                    }
                }
                Stmt::While(_, a) | Stmt::DoWhile(a, _) | Stmt::For(_, _, _, a) => recoger(a, out),
                _ => {}
            }
        }
        for s in renombrar_sombras(&params, &body) {
            recoger(&s, &mut out);
        }
        out
    }

    #[test]
    fn la_primera_se_queda_y_las_sombras_se_numeran() {
        assert_eq!(declarados("int a; { int a; } { int a; }"), ["a", "a.2", "a.3"]);
        assert_eq!(declarados("int a; int b;"), ["a", "b"]);
        // un parametro es el ambito exterior: la local lo sombrea
        assert_eq!(declarados("int n;"), ["n.2"]);
    }

    #[test]
    fn el_uso_va_al_mas_interior_visible_y_solo_desde_su_declaracion() {
        let (params, body) = cuerpo("int a = 1; { a = 2; int a = 3; a = 4; } a = 5;");
        let r = renombrar_sombras(&params, &body);
        let Stmt::Block(interior) = &r[1] else { panic!("bloque") };
        assert_eq!(interior[0], Stmt::Expr(Expr::Assign("a".into(), Box::new(Expr::Int(2)))));
        assert!(matches!(&interior[1], Stmt::DeclAssign(_, n, _) if n == "a.2"));
        assert_eq!(interior[2], Stmt::Expr(Expr::Assign("a.2".into(), Box::new(Expr::Int(4)))));
        assert_eq!(r[2], Stmt::Expr(Expr::Assign("a".into(), Box::new(Expr::Int(5)))));
    }

    #[test]
    fn lo_que_no_es_local_no_se_toca() {
        let (params, body) = cuerpo("int a; a = g + f(a); goto fin; fin: a = 0;");
        let r = renombrar_sombras(&params, &body);
        // `g` y `f` no son locales; `fin` es una etiqueta
        assert_eq!(
            r[1],
            Stmt::Expr(Expr::Assign(
                "a".into(),
                Box::new(Expr::Add(Box::new(Expr::Var("g".into())), Box::new(Expr::Call("f".into(), vec![Expr::Var("a".into())]))))
            ))
        );
        assert_eq!(r[2], Stmt::Goto("fin".into()));
    }

    #[test]
    fn sin_sombras_el_arbol_sale_identico() {
        let (params, body) = cuerpo("int a = 1; int b = 2; if (a < b) { b = a; } while (a) a = a - 1;");
        assert_eq!(renombrar_sombras(&params, &body), body);
    }
}

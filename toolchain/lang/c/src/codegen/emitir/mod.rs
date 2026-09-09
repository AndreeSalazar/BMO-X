//! **EMITIR: los bytes, ya con la decision tomada.**
//!
//! [fase]     EMISION
//!
//! [aparece]  BANCO -- este fichero solo DESPACHA. Si se equivoca mandando un
//!            brazo al carril que no es, el `unreachable!` del otro lado lo
//!            dice al primer programa que lo use
//!
//! [carril]   VERDE -- y NO se elige: sale de su `[aparece]` (BANCO).
//!            Ver la tabla en toolchain/tools/fases/fases.py
//!
//!
//! [cuesta]   TAREA
//!
//! [riesgo]   ESPEJO
//!            ESPEJO -- la lista de formas esta aqui Y en el `match` de cada
//!                     carril. Son dos copias de lo mismo, y por eso el
//!                     `unreachable!` no es decorativo: es el juez de que no
//!                     se hayan separado
//!
//! # *** POR QUE SE PARTIO, Y LO PIDIO EL DUENO ASI
//!
//! > *"DIVIDIR todos los archivos que emiten, y por que, para evitar problemas
//! > y que cada uno cumpla su porque."*
//!
//! `emit_expr` eran **600 lineas y cincuenta formas** en un solo `match`: la
//! aritmetica que miente en silencio, los punteros que pisan memoria y las
//! llamadas que petan, todo mezclado y con el mismo aspecto en un diff.
//!
//! ```text
//!    roja.rs      EL VALOR      aritmetica, signo, ancho, comparaciones
//!                               -> contesta un numero. Si miente, nadie lo ve
//!    amarilla.rs  LA DIRECCION  variables, punteros, campos, indices
//!                               -> contesta un SITIO. A veces peta, a veces no
//!    verde.rs     EL ORDEN      llamadas, cortocircuitos, secuencia
//!                               -> decide QUE se ejecuta. Si falla, se ve
//! ```
//!
//! ** Los tres emiten bytes, asi que por `[cuesta]` los tres serian rojos. Lo
//! que los separa es **cuanto tarda en verse el fallo**, que es el eje
//! `[aparece]` que este compilador estreno el mismo dia. El color, aqui, es esa
//! escala con otro nombre.
//!
//! # *** EL DESPACHO SE QUEDA EXHAUSTIVO, Y ES LA UNICA DECISION DE DISENO
//!
//! El `match` de abajo cubre las cincuenta formas **sin brazo comodin**. Eso no
//! es estilo: es lo que hace que el dia que nazca una forma nueva de expresion,
//! **el compilador de Rust pare AQUI** y obligue a decidir de que color es.
//!
//! Partir en tres `if` que devuelvan `bool` habria sido mas corto y habria
//! perdido exactamente eso: la exhaustividad es un guardian que no cuesta nada
//! y esta casa no regala guardianes.

mod direccion;
mod orden;
mod valor;

use crate::ast::*;

use super::{decidir, Codegen};

impl Codegen {
    /// **Una expresion a `rax`.** Guardas, y despues el reparto por carriles.
    pub(super) fn emit_expr(&mut self, expr: &Expr) {
        // === *** LO QUE YA SE SABE NO SE CALCULA (2026-09-09) ================
        //
        // Antes que nada se le pregunta a `decidir/`. Si la expresion entera es
        // una constante que no depende del signo, sale UNA instruccion en vez
        // del arbol completo con sus `push`, sus `pop` y su operador.
        //
        // ** Y esto es lo que mata el `imul` de la suma de punteros: `p + 1`
        // construye `Mul(Int(1), Int(tamano))` en el arbol, y ese producto llega
        // aqui como una expresion suya. Plegar solo la raiz no lo habria visto
        // -- `p + (1*8)` no es constante, porque `p` no lo es.
        //
        // [!] `recortar_a_32` se llama IGUAL despues, y eso no es prudencia: es
        // lo unico que hace que esta rama y la larga sean la misma. Sin el, una
        // constante que no cabe en `int` se guardaria entera aqui y truncada
        // por el otro camino -- dos programas distintos segun por donde pase.
        if let Some(v) = decidir::plegado::constante_para_emitir(expr) {
            self.emit_mov_rax_imm(v);
            self.recortar_a_32(expr);
            return;
        }

        // Guard SSE: una expresion FLOTANTE que llega a la ruta entera esta en
        // contexto entero (int x = 1.5; return d;) -> calcular en xmm y truncar
        // a rax (cvttsd2si). Las comparaciones dan int 0/1 (no son float) y se
        // manejan abajo. emit_fexpr_operand solo llama aqui para NO-floats, asi
        // que no hay recursion infinita.
        let saltar_guarda = core::mem::take(&mut self.sin_guarda_float);
        if !saltar_guarda && self.expr_is_float(expr) {
            self.emit_fexpr(expr);
            self.code.extend_from_slice(&[0xF2, 0x48, 0x0F, 0x2C, 0xC0]); // cvttsd2si rax, xmm0
            return;
        }

        match expr {
            // -- ROJO: EL VALOR. Silencioso, y por eso primero.
            Expr::Int(..)
            | Expr::FloatLit(..)
            | Expr::CharLit(..)
            | Expr::Neg(..)
            | Expr::Not(..)
            | Expr::BitNot(..)
            | Expr::Add(..)
            | Expr::Sub(..)
            | Expr::Mul(..)
            | Expr::Div(..)
            | Expr::Mod(..)
            | Expr::Eq(..)
            | Expr::Neq(..)
            | Expr::Lt(..)
            | Expr::Gt(..)
            | Expr::Le(..)
            | Expr::Ge(..)
            | Expr::BitAnd(..)
            | Expr::BitXor(..)
            | Expr::BitOr(..)
            | Expr::Shl(..)
            | Expr::Shr(..)
            | Expr::Cast(..) => self.emitir_valor(expr),
            // -- AMARILLO: LA DIRECCION.
            Expr::StringLit(..)
            | Expr::Var(..)
            | Expr::Assign(..)
            | Expr::PreInc(..)
            | Expr::PreDec(..)
            | Expr::PostInc(..)
            | Expr::PostDec(..)
            | Expr::Deref(..)
            | Expr::AddrOf(..)
            | Expr::Subscript(..)
            | Expr::AssignOp(..)
            | Expr::AssignSubscript(..)
            | Expr::IndexPtr(..)
            | Expr::AssignIndexPtr(..)
            | Expr::Field(..)
            | Expr::Arrow(..)
            | Expr::AssignField(..)
            | Expr::AssignDeref(..)
            | Expr::AssignArrow(..) => self.emitir_direccion(expr),
            // -- VERDE: EL ORDEN.
            Expr::Call(..)
            | Expr::Syscall(..)
            | Expr::CallPtr(..)
            | Expr::LAnd(..)
            | Expr::LOr(..)
            | Expr::Conditional(..)
            | Expr::Intrinsic(..)
            | Expr::Comma(..) => self.emitir_orden(expr),        }
    }
}

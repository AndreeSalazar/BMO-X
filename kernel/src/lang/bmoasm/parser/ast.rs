//! AST mínimo. Owned (no zero-copy) para simplicidad inicial.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Byte,
    Num,
    Ptr,
    Arr,
    Ref,
    Void,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Suma, Resta, Mult, Div, Mod,
    Y, O, Xor, Shl, Shr,
    Igual, Mayor, Menor, MayIg, MenIg, Difer,
}

#[derive(Debug, Clone)]
pub enum Expr {
    LitInt(u64),
    LitByte(u8),
    LitNulo,
    LitStr(String),
    Ident(String),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// `no x` (unario).
    No(Box<Expr>),
    /// Acceso a registro directo: `reg rax`.
    Reg(String),
    /// `aloc N`.
    Aloc(Box<Expr>),
    /// Llamada a función: `nombre(arg1, arg2, ...)`.
    Call { name: String, args: Vec<Expr> },
}

impl Default for Expr {
    fn default() -> Self { Expr::LitNulo }
}

#[derive(Debug, Clone)]
pub enum Stmt {
    /// `def nombre(params) -> ret { body }`.
    Def {
        name: String,
        params: Vec<(String, Type)>,
        ret: Type,
        body: Vec<Stmt>,
    },
    /// `let nombre[: T] = expr`.
    Let { name: String, ty: Option<Type>, value: Expr },
    /// `retorna expr`.
    Retorna(Option<Expr>),
    /// `si cond { ... } sino { ... }`.
    Si { cond: Expr, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    /// `mientras cond { ... }`.
    Mientras { cond: Expr, body: Vec<Stmt> },
    /// `emit byte byte byte ...`.
    Emit(Vec<u8>),
    /// `reg name = expr`.
    RegAssign { reg: String, value: Expr },
    /// `libre ptr`.
    Libre(Expr),
    /// `rompe` / `continua`.
    Rompe,
    Continua,
    /// Expression statement (descarta resultado).
    ExprStmt(Expr),
    /// Declaración forward de función (para calls forward).
    FnForward { name: String, params: Vec<(String, Type)>, ret: Type },
    /// `match expr { caso pat => body, ... defecto => body }`.
    Match { expr: Expr, arms: Vec<(Expr, Vec<Stmt>)>, default: Option<Vec<Stmt>> },
    /// `para var desde expr hasta expr [paso expr] { body }`.
    Para { var: String, desde: Expr, hasta: Expr, paso: Option<Expr>, body: Vec<Stmt> },
    /// `bucle { body }` — infinite loop.
    Bucle(Vec<Stmt>),
    /// `etiqueta name` — label declaration.
    Etiqueta(String),
    /// `salto name` — goto.
    Salto(String),
}

#[derive(Debug, Clone, Default)]
pub struct Ast {
    /// Top-level definitions (solo `Stmt::Def` válidos).
    pub items: Vec<Stmt>,
}

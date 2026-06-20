//! AST — v0.3.0 → v0.4.0.
//!
//! Complete AST for the BMOasm IR. The translator at
//! `bmoasm::traductor` consumes this AST directly (no round-trip through
//! text) and emits x86_64/aarch64/riscv64 bytes.
//!
//! ## v0.4.0 additions
//!
//! The following were added so that the ÑEXO codegen (and the C
//! frontend) can express everything they need without losing info:
//!
//! - `Type::Struct(String)`, `Type::Enum(String)` — user types.
//! - `Stmt::Store { name, ty, value }` — rebind a local (was missing).
//! - `Stmt::CallStmt { name, args, ret }` — call as statement.
//! - `Stmt::TypeDecl { name, kind, fields_or_variants }` — type def.
//! - `Stmt::FieldAssign { obj, field, value }` — struct field assign.
//! - `Stmt::IndexAssign { obj, idx, value }` — array index assign.
//! - `Expr::Field { obj, name }` — struct field read.
//! - `Expr::Index { obj, idx }` — array index read.
//! - `Expr::AddrOf(inner)` — `&x` for references.
//! - `Expr::Deref(inner)` — `*x` for pointer deref.
//! - `Expr::Cast(inner, Type)` — type cast.

extern crate alloc;
use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// 8-bit byte.
    Byte,
    /// 64-bit integer.
    Num,
    /// 64-bit pointer.
    Ptr,
    /// Array (size inferred from context).
    Arr,
    /// Reference (pointer with borrow semantics).
    Ref,
    /// No return value.
    Void,
    /// Boolean (encoded as Num 0/1).
    Bool,
    /// User-defined struct type.
    Struct(String),
    /// User-defined enum type.
    Enum(String),
}

impl Type {
    /// Size in bytes for primitive types.
    pub fn size(&self) -> u8 {
        match self {
            Type::Byte | Type::Bool => 1,
            Type::Num | Type::Ptr | Type::Ref => 8,
            Type::Arr => 8,
            Type::Void => 0,
            Type::Struct(_) | Type::Enum(_) => 8,
        }
    }

    /// Convert to a copy-friendly primitive where possible.
    /// For user types, returns `Num` as the safe default.
    pub fn as_prim(&self) -> TypeCopy {
        match self {
            Type::Byte => TypeCopy::Byte,
            Type::Num => TypeCopy::Num,
            Type::Ptr => TypeCopy::Ptr,
            Type::Arr => TypeCopy::Arr,
            Type::Ref => TypeCopy::Ref,
            Type::Void => TypeCopy::Void,
            Type::Bool => TypeCopy::Bool,
            Type::Struct(_) | Type::Enum(_) => TypeCopy::Num,
        }
    }
}

/// Copy-friendly variant of `Type` for use in hot paths.
/// User-defined types are reduced to `Num`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeCopy {
    Byte,
    Num,
    Ptr,
    Arr,
    Ref,
    Void,
    Bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOp {
    Suma, Resta, Mult, Div, Mod,
    Y, O, Xor, Shl, Shr,
    Igual, Mayor, Menor, MayIg, MenIg, Difer,
}

impl BinOp {
    /// Reverse direction for swapping operands (e.g. `a < b` ↔ `b > a`).
    pub const fn reverse(self) -> Self {
        match self {
            BinOp::Mayor => BinOp::Menor,
            BinOp::Menor => BinOp::Mayor,
            BinOp::MayIg => BinOp::MenIg,
            BinOp::MenIg => BinOp::MayIg,
            other => other,
        }
    }
}

/// CPU flags — used with `cuando`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuFlag {
    Cf, Zf, Sf, Of, Pf, Df,
}

/// Memory ordering semantics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemOrder {
    Volatil,
    Acquire,
    Release,
    Relaxed,
    Fence,
}

#[derive(Debug, Clone)]
pub enum Expr {
    LitInt(u64),
    LitByte(u8),
    LitNulo,
    LitStr(String),
    Ident(String),
    Bin(BinOp, Box<Expr>, Box<Expr>),
    /// `no x` (unary).
    No(Box<Expr>),
    /// `&x` — address-of.
    AddrOf(Box<Expr>),
    /// `*x` — dereference.
    Deref(Box<Expr>),
    /// Type cast: `x as T`.
    Cast(Box<Expr>, Type),
    /// Acceso a registro directo: `reg rax`.
    Reg(String),
    /// `aloc N`.
    Aloc(Box<Expr>),
    /// Llamada a función: `nombre(arg1, arg2, ...)`.
    Call { name: String, args: Vec<Expr> },
    /// CPU flag access — `zf`, `cf`, etc.
    Flag(CpuFlag),
    /// Volatile/acquire/release memory access: `volatil expr`
    MemOrder(MemOrder, Box<Expr>),
    /// Struct field read: `obj.field`.
    Field { obj: Box<Expr>, name: String },
    /// Array index read: `obj[idx]`.
    Index { obj: Box<Expr>, idx: Box<Expr> },
}

impl Default for Expr {
    fn default() -> Self { Expr::LitNulo }
}

/// Type declaration kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeDeclKind {
    /// `tipo Nombre = estructura { campo1: T1, campo2: T2, ... }`
    Struct,
    /// `tipo Nombre = enumero { Var1, Var2, ... }`
    Enum,
}

/// Statement — top-level and body.
#[derive(Debug, Clone)]
pub enum Stmt {
    /// `def nombre(params) -> ret { body }`.
    Def { name: String, params: Vec<(String, Type)>, ret: Type, body: Vec<Stmt> },
    /// `let nombre[: T] = expr`.
    Let { name: String, ty: Option<Type>, value: Expr },
    /// `store nombre = expr` — reassign a local (was missing in v0.3.0).
    Store { name: String, ty: Option<Type>, value: Expr },
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
    /// Call used as statement (return value discarded).
    CallStmt { name: String, args: Vec<Expr> },
    /// `obj.field = value` — struct field assignment.
    FieldAssign { obj: Expr, field: String, value: Expr },
    /// `obj[idx] = value` — array index assignment.
    IndexAssign { obj: Expr, idx: Expr, value: Expr },
    /// Declaración de tipo: `tipo Nombre = estructura/enumero { ... }`.
    TypeDecl { name: String, kind: TypeDeclKind, fields: Vec<(String, Type)> },
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
    /// `incluye "file.bmo"` — multi-file.
    Incluye(String),
    /// `cuando zf { body }` — execute block when CPU flag is set.
    Cuando { flag: CpuFlag, body: Vec<Stmt> },
    /// `cuando zf sino { body }` — execute block when CPU flag is NOT set.
    CuandoSino { flag: CpuFlag, then_body: Vec<Stmt>, else_body: Option<Vec<Stmt>> },
    /// `atomico { body }` — LOCK prefix block.
    Atomico(Vec<Stmt>),
    /// `volatil expr` — volatile memory access.
    Volatil(Expr),
    /// `barr` — full memory barrier.
    Barr,
}

#[derive(Debug, Clone, Default)]
pub struct Ast {
    /// Top-level definitions.
    pub items: Vec<Stmt>,
}

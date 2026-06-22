//! `lang::common::ast` — BMO IR (Intermediate Representation).
//!
//! Es el AST **canónico** al que todos los frontends convierten su
//! lenguaje. El backend (AOT x86-64) opera solo sobre este AST.
//!
//! ## Por qué un IR común
//!
//! - **Reutilización**: 1 backend sirve para N lenguajes.
//! - **Testing**: testear el backend = testear todos los frontends.
//! - **Optimizaciones**: passes de optimización (constant folding,
//!   dead-code elimination) corren sobre el IR, no sobre cada lenguaje.
//!
//! ## Estructura
//!
//! ```text
//! Module (archivo)
//!   ├── items: [Item]
//!   │     ├── Function { name, params, ret, body, linkage }
//!   │     ├── Global   { name, ty, init, linkage }
//!   │     ├── TypeDecl { name, ty, kind }
//!   │     └── Extern   { name, kind }
//!   └── types: TypeTable
//! ```

#![allow(dead_code)]

use super::source::Span;
use super::types::{IrTypeId, NamedTypeId};
use core::fmt;

// ─── Módulo ─────────────────────────────────────────────────────────

/// Un módulo = un archivo de código fuente.
#[derive(Clone, Debug, Default)]
pub struct Module {
    pub name: String,
    pub items: Vec<Item>,
    /// Tabla de strings (interner) para identificadores.
    strings: Vec<String>,
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self { name: name.into(), items: Vec::new(), strings: Vec::new() }
    }

    /// Intern un string. Devuelve un ID opaco.
    pub fn intern(&mut self, s: &str) -> StrId {
        if let Some(idx) = self.strings.iter().position(|x| x == s) {
            return StrId(idx as u32);
        }
        let id = StrId(self.strings.len() as u32);
        self.strings.push(s.to_string());
        id
    }

    pub fn get_str(&self, id: StrId) -> &str {
        &self.strings[id.0 as usize]
    }

    pub fn add_item(&mut self, item: Item) { self.items.push(item); }
}

/// ID opaco de un string internado.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct StrId(pub u32);

// ─── Items ──────────────────────────────────────────────────────────

/// Un item de nivel superior (función, global, type, extern).
#[derive(Clone, Debug)]
pub enum Item {
    /// Declaración de función.
    Function {
        name: StrId,
        params: Vec<Param>,
        ret: IrTypeId,
        body: Block,
        linkage: Linkage,
        span: Span,
    },
    /// Variable global.
    Global {
        name: StrId,
        ty: IrTypeId,
        init: Option<Expr>,
        linkage: Linkage,
        span: Span,
    },
    /// Declaración de tipo (struct/union/enum/typedef).
    TypeDecl {
        name: StrId,
        kind: TypeDeclKind,
        span: Span,
    },
    /// Declaración extern (import de otro módulo).
    Extern {
        name: StrId,
        kind: ExternKind,
        span: Span,
    },
}

/// Parámetro de función.
#[derive(Clone, Debug)]
pub struct Param {
    pub name: StrId,
    pub ty: IrTypeId,
    pub span: Span,
}

/// Linkage de un símbolo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Linkage {
    /// Visible solo en este módulo.
    Internal,
    /// Visible a otros módulos (default).
    External,
    /// Función débil (puede ser sobreescrita).
    Weak,
}

impl Default for Linkage {
    fn default() -> Self { Self::External }
}

/// Clase de un type decl.
#[derive(Clone, Debug)]
pub enum TypeDeclKind {
    Struct { fields: Vec<Field> },
    Union  { fields: Vec<Field> },
    Enum   { variants: Vec<StrId> },
    Alias  { to: IrTypeId },
}

/// Campo de struct/union.
#[derive(Clone, Debug)]
pub struct Field {
    pub name: StrId,
    pub ty: IrTypeId,
    pub offset: u32,
    pub span: Span,
}

/// Clase de un extern.
#[derive(Clone, Debug)]
pub enum ExternKind {
    /// Función externa (para FFI / syscalls).
    Function { params: Vec<IrTypeId>, ret: IrTypeId },
    /// Variable externa.
    Global { ty: IrTypeId },
}

// ─── Statements ─────────────────────────────────────────────────────

/// Un bloque de statements.
#[derive(Clone, Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
    pub span: Span,
}

impl Block {
    pub fn new(span: Span) -> Self { Self { stmts: Vec::new(), span } }
    pub fn push(&mut self, s: Stmt) { self.stmts.push(s); }
}

/// Statement.
#[derive(Clone, Debug)]
pub enum Stmt {
    /// Expresión (resultado descartado).
    Expr(Expr, Span),
    /// `let name: type = init;`
    Let { name: StrId, ty: Option<IrTypeId>, init: Option<Expr>, span: Span },
    /// `name = value;` (asignación).
    Assign { target: Expr, value: Expr, span: Span },
    /// `if cond { then } else { else_ }`.
    If { cond: Expr, then_branch: Block, else_branch: Option<Block>, span: Span },
    /// `while cond { body }`.
    While { cond: Expr, body: Block, span: Span },
    /// `for init; cond; step { body }`.
    For { init: Box<Stmt>, cond: Expr, step: Box<Stmt>, body: Block, span: Span },
    /// `return value;` (value puede ser None).
    Return(Option<Expr>, Span),
    /// `break;`.
    Break(Span),
    /// `continue;`.
    Continue(Span),
    /// `label: stmt`.
    Label(StrId, Box<Stmt>, Span),
    /// `goto label;`.
    Goto(StrId, Span),
    /// `switch value { case ... default: ... }`.
    Switch { value: Expr, cases: Vec<SwitchCase>, default: Option<Block>, span: Span },
    /// Bloque anidado.
    Block(Block),
    /// `loop { body }` (infinito).
    Loop { body: Block, span: Span },
    /// Nulo (vacío, no-op).
    Empty(Span),
}

/// Caso de switch.
#[derive(Clone, Debug)]
pub struct SwitchCase {
    pub value: Expr,
    pub body: Block,
    pub span: Span,
}

// ─── Expresiones ────────────────────────────────────────────────────

/// Expresión. Cada frontend convierte sus expresiones a este formato.
#[derive(Clone, Debug)]
pub enum Expr {
    /// Literal integer.
    IntLit { value: i128, ty: IrTypeId, span: Span },
    /// Literal float.
    FloatLit { value: f64, ty: IrTypeId, span: Span },
    /// Literal string.
    StrLit { id: StrId, span: Span },
    /// Literal char.
    CharLit { value: u32, span: Span },
    /// Literal bool.
    BoolLit { value: bool, span: Span },
    /// `null` o `nullptr`.
    Null(Span),
    /// `undefined` / `void`.
    Undefined(Span),

    /// Variable local o global.
    Var { name: StrId, span: Span },

    /// Operación binaria.
    Bin { op: BinOp, lhs: Box<Expr>, rhs: Box<Expr>, span: Span },
    /// Operación unaria.
    Unary { op: UnaryOp, expr: Box<Expr>, span: Span },
    /// Llamada a función.
    Call { callee: Box<Expr>, args: Vec<Expr>, span: Span },
    /// Indexación `a[i]`.
    Index { base: Box<Expr>, index: Box<Expr>, span: Span },
    /// Acceso a campo `a.b`.
    Field { base: Box<Expr>, name: StrId, span: Span },
    /// Cast `x as T`.
    Cast { expr: Box<Expr>, to: IrTypeId, span: Span },
    /// Referencia `&x`.
    AddrOf { expr: Box<Expr>, span: Span },
    /// Dereferencia `*x`.
    Deref { expr: Box<Expr>, span: Span },
    /// Ternario `c ? a : b`.
    Ternary { cond: Box<Expr>, then: Box<Expr>, else_: Box<Expr>, span: Span },
    /// Tamaño de tipo/expresión: `sizeof(T)`.
    SizeOf { ty: IrTypeId, span: Span },

    /// Constructor de struct/array.
    Aggregate { ty: IrTypeId, fields: Vec<Expr>, span: Span },
    /// Cast a named type.
    NamedType { name: NamedTypeId, span: Span },
}

/// Operadores binarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BinOp {
    // Aritmética
    Add, Sub, Mul, Div, Mod,
    // Bitwise
    BitAnd, BitOr, BitXor, Shl, Shr,
    // Lógica
    And, Or,
    // Comparación
    Eq, Ne, Lt, Le, Gt, Ge,
    // Asignación compuesta
    AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign,
    // Otros
    Comma,
}

impl BinOp {
    pub fn is_assignment(self) -> bool {
        matches!(self,
            Self::AddAssign | Self::SubAssign | Self::MulAssign | Self::DivAssign | Self::ModAssign |
            Self::BitAndAssign | Self::BitOrAssign | Self::BitXorAssign | Self::ShlAssign | Self::ShrAssign
        )
    }
    pub fn base_op(self) -> Option<BinOp> {
        Some(match self {
            Self::AddAssign => Self::Add, Self::SubAssign => Self::Sub,
            Self::MulAssign => Self::Mul, Self::DivAssign => Self::Div,
            Self::ModAssign => Self::Mod, Self::BitAndAssign => Self::BitAnd,
            Self::BitOrAssign => Self::BitOr, Self::BitXorAssign => Self::BitXor,
            Self::ShlAssign => Self::Shl, Self::ShrAssign => Self::Shr,
            _ => return None,
        })
    }
}

/// Operadores unarios.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum UnaryOp {
    Neg, Not, BitNot, PreInc, PreDec, PostInc, PostDec,
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Add => "+", Self::Sub => "-", Self::Mul => "*", Self::Div => "/", Self::Mod => "%",
            Self::BitAnd => "&", Self::BitOr => "|", Self::BitXor => "^", Self::Shl => "<<", Self::Shr => ">>",
            Self::And => "&&", Self::Or => "||",
            Self::Eq => "==", Self::Ne => "!=", Self::Lt => "<", Self::Le => "<=", Self::Gt => ">", Self::Ge => ">=",
            Self::AddAssign => "+=", Self::SubAssign => "-=", Self::MulAssign => "*=", Self::DivAssign => "/=",
            Self::ModAssign => "%=", Self::BitAndAssign => "&=", Self::BitOrAssign => "|=",
            Self::BitXorAssign => "^=", Self::ShlAssign => "<<=", Self::ShrAssign => ">>=",
            Self::Comma => ",",
        })
    }
}

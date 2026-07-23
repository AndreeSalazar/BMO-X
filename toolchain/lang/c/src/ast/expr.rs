//! C Abstract Syntax Tree — expression nodes.

use super::types::*;

/// A named syscall definition loaded from sem-asm tables (.toml)
/// Args follow x86-64 SysV ABI convention: rdi, rsi, rdx, r10, r8, r9
/// Return value in rax
#[derive(Debug, Clone, PartialEq)]
pub struct SyscallDef {
    pub name: String,
    pub nr: u32,
    pub arg_count: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    StringLit(String),
    CharLit(u8),
    Var(String),
    Call(String, Vec<Expr>),
    Assign(String, Box<Expr>),
    Neg(Box<Expr>),
    Not(Box<Expr>),
    BitNot(Box<Expr>),
    PreInc(String),
    PreDec(String),
    PostInc(String),
    PostDec(String),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Neq(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    LAnd(Box<Expr>, Box<Expr>),
    LOr(Box<Expr>, Box<Expr>),
    Shl(Box<Expr>, Box<Expr>),
    Shr(Box<Expr>, Box<Expr>),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    Comma(Vec<Expr>),
    Deref(Box<Expr>),
    AddrOf(Box<Expr>),
    Subscript(String, Box<Expr>, u8),
    /// arr[i] = val — antes la asignación a subscript se DESCARTABA en silencio.
    AssignSubscript(String, Box<Expr>, u8, Box<Expr>),
    /// base.campo — (base, nombre, offset, TIPO del campo).
    /// El tipo viaja en el AST para que codegen cargue/guarde el tamaño EXACTO:
    /// antes pt.x=10 con x:int escribía 8 bytes y pisaba al campo siguiente.
    Field(Box<Expr>, String, u32, TypeSpec),
    Arrow(Box<Expr>, String, u32, TypeSpec),
    AssignField(Box<Expr>, String, u32, TypeSpec, Box<Expr>),
    AssignArrow(Box<Expr>, String, u32, TypeSpec, Box<Expr>),
    AssignDeref(Box<Expr>, Box<Expr>),
    /// (tipo)expr — cast REAL: trunca/extiende; antes era no-op silencioso.
    Cast(TypeSpec, Box<Expr>),
    /// __nombre(args...) — la FUSIÓN sem-asm↔C: instrucción de la tabla
    /// intrinsics.toml invocada como función. El nombre va SIN el prefijo __.
    /// Los argumentos van a los registros que dicta la tabla (dx, al, ecx...).
    Intrinsic(String, Vec<Expr>),
    Syscall(SyscallDef, Vec<Expr>),
}

//! C Abstract Syntax Tree -- expression nodes.

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
    /// Literal de punto flotante (1.5, 3.14). Se computa en doble precision
    /// por la ruta SSE (xmm), no por la ruta entera (rax).
    FloatLit(f64),
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
    Subscript(String, Box<Expr>),
    /// arr[i] = val -- antes la asignacion a subscript se DESCARTABA en silencio.
    AssignSubscript(String, Box<Expr>, Box<Expr>),
    /// base[i] donde base es una EXPRESION que da un puntero (p->arr[i],
    /// (a+1)[i]): (base, indice, tipo del elemento). Antes se rechazaba.
    IndexPtr(Box<Expr>, Box<Expr>),
    /// base[i] = val para bases compuestas.
    AssignIndexPtr(Box<Expr>, Box<Expr>, Box<Expr>),
    /// (*fp)(args) -- llamada a traves de un puntero a funcion CALCULADO
    /// (no una simple variable). callee da la direccion; args por la pila.
    CallPtr(Box<Expr>, Vec<Expr>),
    /// base.campo -- (base, nombre, offset, TIPO del campo).
    /// El tipo viaja en el AST para que codegen cargue/guarde el tamano EXACTO:
    /// antes pt.x=10 con x:int escribia 8 bytes y pisaba al campo siguiente.
    Field(Box<Expr>, String),
    Arrow(Box<Expr>, String),
    AssignField(Box<Expr>, String, Box<Expr>),
    AssignArrow(Box<Expr>, String, Box<Expr>),
    AssignDeref(Box<Expr>, Box<Expr>),
    /// **`E1 op= E2` con `E1` evaluado UNA SOLA VEZ.**
    ///
    /// === Por que existe esta variante, y no se desazucara ===
    ///
    /// Hasta el 2026-08-13 `a[i] += 7` se desazucaraba en el parser a
    /// `a[i] = a[i] + 7`, clonando el lvalue. Para un indice sin efectos eso es
    /// exacto y mas barato. Para uno con efectos es **incorrecto**:
    ///
    /// ```text
    ///   int g[4]; int i = 1;
    ///   g[i++] += 7;   ->  g[i++] = g[i++] + 7
    ///                      i avanza DOS veces y el 7 cae en g[2]
    /// ```
    ///
    /// C11 6.5.16.2p3 no deja lugar: *"the lvalue expression E1 is evaluated
    /// only once"*. Y no se puede arreglar desazucarando mejor -- una expresion
    /// no tiene donde poner un temporal-- asi que la DIRECCION tiene que
    /// calcularse una vez, y eso solo lo puede hacer el codegen.
    ///
    /// El operando izquierdo es el lvalue COMPLETO (`Subscript`, `Field`,
    /// `Arrow`, `IndexPtr`, `Deref`), no su direccion: el codegen ya sabe sacar
    /// la direccion de cada forma, y meter eso en el AST seria bajar el AST.
    AssignOp(Box<Expr>, AssignOpKind, Box<Expr>),

    /// (tipo)expr -- cast REAL: trunca/extiende; antes era no-op silencioso.
    Cast(TypeSpec, Box<Expr>),
    /// __nombre(args...) -- la FUSION sem-asm<->C: instruccion de la tabla
    /// intrinsics.toml invocada como funcion. El nombre va SIN el prefijo __.
    /// Los argumentos van a los registros que dicta la tabla (dx, al, ecx...).
    Intrinsic(String, Vec<Expr>),
    Syscall(SyscallDef, Vec<Expr>),
}

/// **Que operacion lleva un `op=`.**
///
/// Va como enum y no como los bytes de la instruccion porque el AST no debe
/// saber de codigo maquina -- y porque `/=` y `%=` necesitan mirar el SIGNO del
/// tipo antes de elegir instruccion, decision que se toma en el codegen con la
/// tabla de tipos delante (ver `expr_is_unsigned`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AssignOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Shl,
    Shr,
    BitAnd,
    BitOr,
    BitXor,
}

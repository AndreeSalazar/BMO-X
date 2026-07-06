#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDecl {
    Var(TypeSpec, String, Option<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub ret_type: TypeSpec,
    pub name: String,
    pub params: Vec<Param>,
    pub var_count: u32,
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub typ: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpec {
    Void,
    Char,
    Short,
    Int,
    Long,
    LongLong,
    UnsignedInt,
    UnsignedLong,
    UnsignedChar,
    UnsignedShort,
    UnsignedLongLong,
    Ptr(Box<TypeSpec>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Printf(String),
    PrintfLn(String),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    DoWhile(Box<Stmt>, Expr),
    For(Option<Expr>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Switch(Expr, Vec<Case>),
    Break,
    Continue,
    Return(Option<Expr>),
    DeclAssign(TypeSpec, String, Option<Expr>),
    Expr(Expr),
    Block(Vec<Stmt>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub value: Option<i64>,
    pub stmts: Vec<Stmt>,
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
    AddrOf(String),
    Subscript(String, Box<Expr>),
}

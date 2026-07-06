#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
    pub exported: Vec<String>,
}

impl Program {
    pub fn new() -> Self {
        Self { globals: Vec::new(), functions: Vec::new(), exported: Vec::new() }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    pub typ: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDecl {
    Var(TypeSpec, String, Option<Expr>),
    Struct(String, Vec<StructMember>),
    Union(String, Vec<StructMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub ret_type: TypeSpec,
    pub name: String,
    pub params: Vec<Param>,
    pub var_count: u32,
    pub var_names: Vec<String>,
    pub body: Vec<Stmt>,
}

impl TypeSpec {
    pub fn stack_size(&self) -> u32 {
        match self {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong | TypeSpec::UnsignedLongLong => 8,
            TypeSpec::Float => 4,
            TypeSpec::Double => 8,
            TypeSpec::Ptr(_) => 8,
            TypeSpec::StructRef(_) | TypeSpec::UnionRef(_) => 0,
        }
    }
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
    Float,
    Double,
    Ptr(Box<TypeSpec>),
    StructRef(String),
    UnionRef(String),
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
    Goto(String),
    Label(String),
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
    AddrOf(Box<Expr>),
    Subscript(String, Box<Expr>, u8),
    Field(Box<Expr>, String, u32), // base_expr, field_name, resolved_offset
    Arrow(Box<Expr>, String, u32), // ptr_expr, field_name, resolved_offset
    AssignField(Box<Expr>, String, u32, Box<Expr>), // base_expr, field_name, offset, val
    AssignArrow(Box<Expr>, String, u32, Box<Expr>), // ptr_expr, field_name, offset, val
}

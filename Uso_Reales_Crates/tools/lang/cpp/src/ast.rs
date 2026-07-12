//! C++ AST: classes, virtual functions, templates, inheritance.

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub includes: Vec<String>,
    pub namespaces: Vec<Namespace>,
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
    pub classes: Vec<Class>,
}

impl Program {
    pub fn new() -> Self {
        Self { includes: vec![], namespaces: vec![], globals: vec![], functions: vec![], classes: vec![] }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Namespace {
    pub name: String,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub name: String,
    pub bases: Vec<String>,
    pub members: Vec<MemberVar>,
    pub methods: Vec<Method>,
    pub constructor: Option<Method>,
    pub destructor: Option<Method>,
    pub vtable: bool, // true if any method is virtual
}

#[derive(Debug, Clone, PartialEq)]
pub struct MemberVar {
    pub typ: TypeSpec,
    pub name: String,
    pub offset: u32,
    pub access: Access,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Method {
    pub name: String,
    pub ret_type: TypeSpec,
    pub params: Vec<Param>,
    pub body: Vec<Stmt>,
    pub is_virtual: bool,
    pub is_override: bool,
    pub access: Access,
    pub class_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Public,
    Protected,
    Private,
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
    pub body: Vec<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub typ: TypeSpec,
    pub name: String,
    pub default: Option<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeSpec {
    Void,
    Char, Short, Int, Long, LongLong,
    UnsignedChar, UnsignedShort, UnsignedInt, UnsignedLong, UnsignedLongLong,
    Float, Double, Bool,
    Ptr(Box<TypeSpec>),
    Ref(Box<TypeSpec>),
    ClassRef(String),
    Template(String, Vec<TypeSpec>),
    Auto,
}

impl TypeSpec {
    pub fn size(&self) -> u32 {
        match self {
            TypeSpec::Void => 0,
            TypeSpec::Char | TypeSpec::UnsignedChar | TypeSpec::Bool => 1,
            TypeSpec::Short | TypeSpec::UnsignedShort => 2,
            TypeSpec::Int | TypeSpec::UnsignedInt | TypeSpec::Float => 4,
            TypeSpec::Long | TypeSpec::UnsignedLong | TypeSpec::LongLong
            | TypeSpec::UnsignedLongLong | TypeSpec::Double => 8,
            TypeSpec::Ptr(_) | TypeSpec::Ref(_) => 8,
            TypeSpec::ClassRef(_) => 8, // pointer to class in BMO
            TypeSpec::Template(_, _) => 0,
            TypeSpec::Auto => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Expr(Expr),
    Return(Option<Expr>),
    DeclVar(TypeSpec, String, Option<Expr>),
    Assign(String, Expr),
    If(Expr, Box<Stmt>, Option<Box<Stmt>>),
    While(Expr, Box<Stmt>),
    For(Option<Expr>, Option<Expr>, Option<Expr>, Box<Stmt>),
    Block(Vec<Stmt>),
    Break,
    Continue,
    Delete(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SyscallDef {
    pub name: String,
    pub nr: u32,
    pub arg_count: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Int(i64),
    FloatLit(f64),
    StringLit(String),
    CharLit(u8),
    BoolLit(bool),
    Var(String),
    Call(String, Vec<Expr>),
    MethodCall(Box<Expr>, String, Vec<Expr>),
    VirtualCall(Box<Expr>, String, u32, Vec<Expr>), // this, method_name, vtable_offset, args
    New(String, Vec<Expr>),
    Assign(String, Box<Expr>),
    MemberAccess(Box<Expr>, String, u32), // object, field, offset
    Arrow(Box<Expr>, String, u32), // ptr, field, offset
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
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    BitNot(Box<Expr>),
    Deref(Box<Expr>),
    AddrOf(Box<Expr>),
    PreInc(String),
    PreDec(String),
    PostInc(String),
    PostDec(String),
    Conditional(Box<Expr>, Box<Expr>, Box<Expr>),
    TemplateCall(String, Vec<TypeSpec>, Vec<Expr>),
    Syscall(SyscallDef, Vec<Expr>),
    This,
    NullPtr,
}

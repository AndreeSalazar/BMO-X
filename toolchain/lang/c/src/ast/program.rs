//! C Abstract Syntax Tree — top-level program nodes.

use super::expr::Expr;
use super::types::TypeSpec;
use super::stmt::Stmt;

#[derive(Debug, Clone, PartialEq)]
pub struct StructMember {
    pub typ: TypeSpec,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum GlobalDecl {
    Var(TypeSpec, String, Option<Expr>),
    /// ★ Un global con LISTA de inicialización: `int t[4] = {1,2,3,4}`,
    /// `struct P tabla[2] = {{1,2},{3,4}}`.
    ///
    /// Lleva las escrituras ya **aplanadas** —offset absoluto, tipo del
    /// subobjeto y valor— porque es exactamente lo que
    /// `parser::inicializador` produce para los locales desde que existen los
    /// inicializadores designados. Reusar esa salida en vez de inventar una
    /// representación para globales es lo que hace que `{[2].y = 8}` funcione
    /// igual en los dos sitios sin escribirlo dos veces.
    ///
    /// Es una variante aparte de [`GlobalDecl::Var`] y no un tercer campo
    /// porque son dos cosas distintas: `Var` lleva UNA expresión y ésta lleva
    /// N escrituras con su sitio. Meterlas en el mismo sitio obligaría a todo
    /// consumidor a preguntar cuál de las dos es.
    VarLista(TypeSpec, String, Vec<super::stmt::Escritura>),
    Struct(String, Vec<StructMember>),
    Union(String, Vec<StructMember>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub ret_type: TypeSpec,
    pub name: String,
    pub params: Vec<super::types::Param>,
    pub var_count: u32,
    pub var_names: Vec<String>,
    pub body: Vec<Stmt>,
    pub line: usize,
    /// ¿Declara `...`? Lo necesita el codegen para saber si `__va_arg()` tiene
    /// algo que leer — y para poder DECIRLO cuando no lo tiene.
    pub variadica: bool,
}

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

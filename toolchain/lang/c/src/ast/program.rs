//! C Abstract Syntax Tree -- top-level program nodes.

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
    /// * Un global con LISTA de inicializacion: `int t[4] = {1,2,3,4}`,
    /// `struct P tabla[2] = {{1,2},{3,4}}`.
    ///
    /// Lleva las escrituras ya **aplanadas** --offset absoluto, tipo del
    /// subobjeto y valor-- porque es exactamente lo que
    /// `parser::inicializador` produce para los locales desde que existen los
    /// inicializadores designados. Reusar esa salida en vez de inventar una
    /// representacion para globales es lo que hace que `{[2].y = 8}` funcione
    /// igual en los dos sitios sin escribirlo dos veces.
    ///
    /// Es una variante aparte de [`GlobalDecl::Var`] y no un tercer campo
    /// porque son dos cosas distintas: `Var` lleva UNA expresion y esta lleva
    /// N escrituras con su sitio. Meterlas en el mismo sitio obligaria a todo
    /// consumidor a preguntar cual de las dos es.
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
    /// Declara `...`? Lo necesita el codegen para saber si `__va_arg()` tiene
    /// algo que leer -- y para poder DECIRLO cuando no lo tiene.
    pub variadica: bool,
}

/// **La disposicion de UN agregado, tal y como la calculo el frontend.**
///
/// Viaja en el `Program` para que el codegen --que la recalcula por su cuenta--
/// tenga contra que compararla. Ver `codegen::cotejar_disposicion`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DisposicionAgregado {
    /// `(nombre, offset, tamano)` de cada campo, en orden de declaracion.
    pub campos: Vec<(String, u32, u32)>,
    pub tamano: u32,
    pub alineado: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub globals: Vec<GlobalDecl>,
    pub functions: Vec<Function>,
    pub exported: Vec<String>,
    /// **Lo que el frontend dice que mide y donde cae cada campo.**
    ///
    /// Vacio significa *"este frontend no lo declara"*, y entonces no hay nada
    /// que cotejar -- no es un fallo. Lo que SI es un fallo es declararlo y que
    /// no cuadre con lo que el codegen calcula por su cuenta.
    pub disposiciones: std::collections::HashMap<String, DisposicionAgregado>,
}

impl Program {
    pub fn new() -> Self {
        Self { globals: Vec::new(), functions: Vec::new(), exported: Vec::new(),
               disposiciones: std::collections::HashMap::new() }
    }
}

//! C Abstract Syntax Tree -- statement nodes.

use super::expr::Expr;
use super::types::TypeSpec;

#[derive(Debug, Clone, PartialEq)]
pub struct Case {
    pub value: Option<i64>,
    pub stmts: Vec<Stmt>,
}

/// Una escritura de una lista de inicializacion, ya resuelta.
///
/// **Este es el contrato entero entre el parser y el codegen para los
/// agregados**, y es a proposito lo mas tonto que se puede escribir: *en el
/// byte `offset` del objeto va `valor`, del tamano de `tipo`*.
///
/// Lo que NO viaja aqui: designadores, nombres de campo, corchetes, anidamiento.
/// El codegen no sabe que existe `.x = 1`, igual que no sabe que existe `%d` --
/// eso lo resolvio quien tenia delante el tipo y la sintaxis a la vez. Ver la
/// cabecera de `parser/inicializador.rs` para por que se eligio asi y que
/// hicieron GCC, Clang y los demas.
///
/// Una lista es un `Vec<Escritura>` en orden de aparicion. Si dos escrituras
/// caen en el mismo `offset` --que es legal: `{.x = 1, .x = 2}`-- gana la ultima,
/// y eso sale solo de emitirlas en orden.
#[derive(Debug, Clone, PartialEq)]
pub struct Escritura {
    /// Bytes desde el principio del objeto que se inicializa.
    pub offset: u32,
    /// El tipo del subobjeto: dice CUANTOS bytes se escriben.
    pub tipo: TypeSpec,
    pub valor: Expr,
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
    /// `T x = { ... }` -- declaracion con lista de inicializacion, ya aplanada.
    ///
    /// Variante propia y no un `DeclAssign` con una expresion rara: una lista
    /// no es un valor, es un **conjunto de escrituras**, y meterla en `Expr`
    /// obligaria a todo el que recorre expresiones a saltarsela.
    ///
    /// Lo no mencionado vale CERO (C99 section 6.7.9/21), y eso lo garantiza el
    /// codegen borrando el objeto entero antes de escribir nada.
    DeclInit(TypeSpec, String, Vec<Escritura>),
    Expr(Expr),
    Block(Vec<Stmt>),
    Goto(String),
    Label(String),
}

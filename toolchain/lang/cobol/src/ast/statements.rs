use crate::ast::SyscallDef;

#[derive(Debug, Clone, PartialEq)]
pub enum CobolStatement {
    Display(String),
    Accept(String),
    Move(String, String),
    Add(String, String),
    Subtract(String, String),
    Multiply(String, String),
    Divide(String, String),
    Compute(String, String),
    /// `IF <cond> ... [ELSE ...] END-IF`. Las condiciones se conjugan con
    /// AND (ver `CobolCondition`).
    If(Vec<CobolCondition>, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `PERFORM <n> TIMES ... END-PERFORM` — el cuerpo va en el AST, no
    /// como una cuenta suelta: sin cuerpo no hay nada que repetir.
    PerformTimes(u32, Vec<CobolStatement>),
    /// `PERFORM UNTIL <cond> ... END-PERFORM`. Prueba ANTES de cada
    /// iteración (`WITH TEST BEFORE`, el default del estándar).
    PerformUntil(Vec<CobolCondition>, Vec<CobolStatement>),
    Open(String, String),
    Close(String),
    Read(String, String),
    Write(String),
    StopRun,
    Syscall(SyscallDef, Vec<String>),
    Expr(String),
}

/// Una comparación simple. Una lista de ellas se evalúa como **AND**: es lo
/// que hoy sabe compilar el descenso. `OR` se rechaza en el parser con un
/// error explícito en vez de compilarse mal en silencio.
///
/// Cada operando es un nombre de dato o un literal; el codegen lo resuelve
/// mirando si está declarado en la DATA DIVISION.
#[derive(Debug, Clone, PartialEq)]
pub enum CobolCondition {
    Equal(String, String),
    NotEqual(String, String),
    Greater(String, String),
    Less(String, String),
    GreaterOrEqual(String, String),
    LessOrEqual(String, String),
}

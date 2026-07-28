use crate::ast::SyscallDef;

/// Que se imprime en un `DISPLAY`.
#[derive(Debug, Clone, PartialEq)]
pub enum DisplayArg {
    /// Entre comillas: sale tal cual.
    Literal(String),
    /// Un nombre de la DATA DIVISION: se formatea en EJECUCION con la escala
    /// de su PIC, porque el valor no se conoce al compilar.
    Variable(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CobolStatement {
    /// `DISPLAY "texto"` o `DISPLAY VARIABLE`.
    ///
    /// Eran lo mismo —una `String` que siempre se imprimia literal— y por eso
    /// el programa de ejemplo CALCULA 59.97 y luego imprime la cadena
    /// "total exacto: 59.97" escrita a mano. La aritmetica era de verdad; lo
    /// que se veia, no. Un `DISPLAY` que no sabe ensenar lo que acaba de
    /// calcular deja al lenguaje sin salida.
    Display(DisplayArg),
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

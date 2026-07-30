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
    /// `OPEN INPUT|OUTPUT <fichero>`. El modo decide si se abre para leer o
    /// se CREA para escribir, y son dos puertas distintas del kernel.
    Open(String, String),
    /// `CLOSE <fichero>`. En un fichero de salida **es donde el contenido
    /// llega al disco**: sin esto no se guarda nada.
    Close(String),
    /// `READ <fichero> AT END <stmts> [NOT AT END <stmts>] END-READ`.
    ///
    /// `AT END` no es un adorno de sintaxis: es la ÚNICA forma de que un
    /// `PERFORM UNTIL` sobre un fichero termine. Un `READ` que no lo lleva
    /// compilaba antes a un error explícito, y ahora compila a un bucle que no
    /// para — así que el parser lo exige.
    Read(String, Vec<CobolStatement>, Vec<CobolStatement>),
    /// `WRITE <registro>`. Escribe el valor del registro como una línea.
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
    /// Un NOMBRE DE CONDICIÓN a secas (`IF FIN-DE-FICHERO`), declarado con un
    /// nivel 88. El parser no puede resolverlo —no conoce los datos—, así que
    /// lo pasa por nombre y lo expande el codegen, que sí sabe de quién es y
    /// puede decirlo cuando no existe.
    Nombre(String),
    Equal(String, String),
    NotEqual(String, String),
    Greater(String, String),
    Less(String, String),
    GreaterOrEqual(String, String),
    LessOrEqual(String, String),
}

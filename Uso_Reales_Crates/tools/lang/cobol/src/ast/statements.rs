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
    If(Vec<CobolCondition>, Vec<CobolStatement>, Vec<CobolStatement>),
    Perform(u32),
    PerformUntil(String, String),
    Open(String, String),
    Close(String),
    Read(String, String),
    Write(String),
    StopRun,
    Syscall(SyscallDef, Vec<String>),
    Expr(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum CobolCondition {
    Equal(String, String),
    NotEqual(String, String),
    Greater(String, String),
    Less(String, String),
    GreaterOrEqual(String, String),
    LessOrEqual(String, String),
}

//! Parser sobre TOKENS — aplica la arquitectura (Lexer → Tokens → AST).
//!
//! Reemplaza incrementalmente al parser por-líneas (`parser.rs`). Empieza por
//! el nivel de SENTENCIA (el núcleo de COBOL) y produce el MISMO AST
//! (`CobolStatement`) que el codegen ya sabe compilar. Así crece sin romper
//! nada: el codegen decimal exacto sigue igual, solo cambia quién arma el AST.

use crate::ast::error::CobolError;
use crate::ast::CobolStatement;
use crate::lexer::{lex, Tok, Token};

/// Cursor sobre el flujo de tokens.
pub struct Cursor {
    toks: Vec<Token>,
    pos: usize,
}

impl Cursor {
    pub fn from_source(src: &str) -> Self {
        Self { toks: lex(src), pos: 0 }
    }
    pub fn new(toks: Vec<Token>) -> Self {
        Self { toks, pos: 0 }
    }

    fn peek(&self) -> Option<&Tok> {
        self.toks.get(self.pos).map(|t| &t.tok)
    }
    fn line(&self) -> usize {
        self.toks.get(self.pos).map(|t| t.line).unwrap_or(0)
    }
    fn bump(&mut self) -> Option<Tok> {
        let t = self.toks.get(self.pos).map(|t| t.tok.clone());
        if t.is_some() {
            self.pos += 1;
        }
        t
    }
    fn at_kw(&self, kw: &str) -> bool {
        matches!(self.peek(), Some(Tok::Keyword(w)) if w == kw)
    }
    fn eat_kw(&mut self, kw: &str) -> bool {
        if self.at_kw(kw) {
            self.pos += 1;
            true
        } else {
            false
        }
    }
    fn eat_periods(&mut self) {
        while matches!(self.peek(), Some(Tok::Period)) {
            self.pos += 1;
        }
    }
    pub fn done(&self) -> bool {
        self.pos >= self.toks.len()
    }
}

/// Un operando: número / cadena / identificador → su texto (como espera el
/// AST, que guarda `String`).
fn operand(c: &mut Cursor, line: usize, what: &str) -> Result<String, CobolError> {
    match c.bump() {
        Some(Tok::Number(n)) => Ok(n),
        Some(Tok::Str(s)) => Ok(s),
        Some(Tok::Ident(id)) => Ok(id),
        _ => Err(CobolError::new(line, format!("se esperaba un operando ({what})"))),
    }
}

/// Parsea UNA sentencia desde la posición actual del cursor.
pub fn parse_statement(c: &mut Cursor) -> Result<CobolStatement, CobolError> {
    let line = c.line();
    let kw = match c.peek() {
        Some(Tok::Keyword(w)) => w.clone(),
        _ => return Err(CobolError::new(line, "se esperaba un verbo COBOL")),
    };
    c.bump(); // consume el verbo

    let st = match kw.as_str() {
        "DISPLAY" => CobolStatement::Display(operand(c, line, "texto")?),
        "MOVE" => {
            let v = operand(c, line, "valor")?;
            if !c.eat_kw("TO") {
                return Err(CobolError::new(line, "MOVE requiere `TO`"));
            }
            CobolStatement::Move(v, operand(c, line, "destino")?)
        }
        "ADD" => {
            let v = operand(c, line, "valor")?;
            if !c.eat_kw("TO") {
                return Err(CobolError::new(line, "ADD requiere `TO`"));
            }
            CobolStatement::Add(v, operand(c, line, "destino")?)
        }
        "SUBTRACT" => {
            let v = operand(c, line, "valor")?;
            if !c.eat_kw("FROM") {
                return Err(CobolError::new(line, "SUBTRACT requiere `FROM`"));
            }
            CobolStatement::Subtract(v, operand(c, line, "destino")?)
        }
        "MULTIPLY" => {
            let v = operand(c, line, "valor")?;
            if !c.eat_kw("BY") {
                return Err(CobolError::new(line, "MULTIPLY requiere `BY`"));
            }
            CobolStatement::Multiply(v, operand(c, line, "destino")?)
        }
        "DIVIDE" => {
            let v = operand(c, line, "valor")?;
            if !c.eat_kw("BY") {
                return Err(CobolError::new(line, "DIVIDE requiere `BY`"));
            }
            CobolStatement::Divide(v, operand(c, line, "destino")?)
        }
        "STOP" => {
            c.eat_kw("RUN");
            CobolStatement::StopRun
        }
        other => {
            return Err(CobolError::new(
                line,
                format!("verbo aún no soportado en el token-parser: {other}"),
            ))
        }
    };
    c.eat_periods();
    Ok(st)
}

/// Parsea una secuencia de sentencias (cuerpo del PROCEDURE) desde el fuente.
pub fn parse_statements(src: &str) -> Result<Vec<CobolStatement>, CobolError> {
    let mut c = Cursor::from_source(src);
    let mut out = Vec::new();
    while !c.done() {
        if matches!(c.peek(), Some(Tok::Period)) {
            c.bump();
            continue;
        }
        out.push(parse_statement(&mut c)?);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_core_statements_from_tokens() {
        let src = "DISPLAY \"hola\". MOVE 10.05 TO SALDO. ADD 3.20 TO SALDO. STOP RUN.";
        let sts = parse_statements(src).unwrap();
        assert_eq!(sts.len(), 4);
        assert_eq!(sts[0], CobolStatement::Display("hola".into()));
        assert_eq!(sts[1], CobolStatement::Move("10.05".into(), "SALDO".into()));
        assert_eq!(sts[2], CobolStatement::Add("3.20".into(), "SALDO".into()));
        assert_eq!(sts[3], CobolStatement::StopRun);
    }

    #[test]
    fn multiline_and_decimal_period_disambiguation() {
        let src = "MULTIPLY 3 BY SALDO.\nDIVIDE 4 BY SALDO.\n";
        let sts = parse_statements(src).unwrap();
        assert_eq!(sts[0], CobolStatement::Multiply("3".into(), "SALDO".into()));
        assert_eq!(sts[1], CobolStatement::Divide("4".into(), "SALDO".into()));
    }

    #[test]
    fn errors_are_clear() {
        assert!(parse_statements("MOVE 5 SALDO.").is_err()); // falta TO
    }
}

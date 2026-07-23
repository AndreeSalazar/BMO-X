//! Parser sobre TOKENS — aplica la arquitectura (Lexer → Tokens → AST).
//!
//! Reemplaza incrementalmente al parser por-líneas (`parser.rs`). Empieza por
//! el nivel de SENTENCIA (el núcleo de COBOL) y produce el MISMO AST
//! (`CobolStatement`) que el codegen ya sabe compilar. Así crece sin romper
//! nada: el codegen decimal exacto sigue igual, solo cambia quién arma el AST.

use crate::ast::error::CobolError;
use crate::ast::{CobolProgram, CobolStatement, DataItem};
use crate::lexer::{lex, Tok, Token};

/// Texto fuente de un token — para reensamblar una cláusula PIC que el lexer
/// partió (`9(5)V99` → Number "9", `(`, "5", `)`, Ident "V99").
fn tok_text(t: &Tok) -> String {
    match t {
        Tok::Keyword(w) | Tok::Ident(w) | Tok::Number(w) => w.clone(),
        Tok::Str(s) => s.clone(),
        Tok::Period => ".".into(),
        Tok::Comma => ",".into(),
        Tok::LParen => "(".into(),
        Tok::RParen => ")".into(),
        Tok::Equal => "=".into(),
        Tok::Punct(c) => c.to_string(),
    }
}

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

// ── DATA DIVISION: records con PIC (la esencia de datos de COBOL) ────────

impl Cursor {
    /// Reensambla una cláusula PIC concatenando los textos de los tokens
    /// contiguos hasta un keyword (VALUE/…) o Period. `9(5)V99` vuelve a ser
    /// un solo string que `pic::parse_pic` entiende.
    fn read_pic(&mut self) -> String {
        let mut s = String::new();
        while let Some(t) = self.peek() {
            match t {
                Tok::Keyword(_) | Tok::Period => break,
                other => {
                    s.push_str(&tok_text(other));
                    self.pos += 1;
                }
            }
        }
        s
    }
}

/// Parsea UN data item: `NN NOMBRE [PIC[TURE] [IS] <pic>] [VALUE <lit>] .`
pub fn parse_data_item(c: &mut Cursor) -> Result<DataItem, CobolError> {
    let line = c.line();
    let level: u32 = match c.bump() {
        Some(Tok::Number(n)) => n.parse().map_err(|_| {
            CobolError::new(line, format!("nivel de data inválido: {n}"))
        })?,
        _ => return Err(CobolError::new(line, "se esperaba un número de nivel (01, 05…)")),
    };
    let name = match c.bump() {
        Some(Tok::Ident(id)) => id,
        Some(Tok::Keyword(k)) if k == "FILLER" => "FILLER".into(),
        _ => return Err(CobolError::new(line, "se esperaba el nombre del dato")),
    };

    let mut pic: Option<String> = None;
    let mut value: Option<String> = None;
    // Cláusulas en cualquier orden hasta el punto.
    loop {
        if c.eat_kw("PIC") || c.eat_kw("PICTURE") {
            c.eat_kw("IS"); // opcional
            pic = Some(c.read_pic());
        } else if c.eat_kw("VALUE") {
            c.eat_kw("IS"); // opcional
            value = c.bump().map(|t| tok_text(&t));
        } else if matches!(c.peek(), Some(Tok::Period)) || c.done() {
            break;
        } else {
            // Cláusula aún no soportada (USAGE, OCCURS…): sáltala.
            c.bump();
        }
    }
    c.eat_periods();
    Ok(DataItem::new(level, name, pic, value))
}

/// Parsea la WORKING-STORAGE / DATA DIVISION: items mientras empiece por un
/// número de nivel.
pub fn parse_data_items(src: &str) -> Result<Vec<DataItem>, CobolError> {
    let mut c = Cursor::from_source(src);
    let mut out = Vec::new();
    while matches!(c.peek(), Some(Tok::Number(_))) {
        out.push(parse_data_item(&mut c)?);
    }
    Ok(out)
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

// ── Programa completo (IDENTIFICATION / DATA / PROCEDURE) sobre tokens ────

/// Parsea un programa COBOL entero desde el fuente al AST `CobolProgram`.
/// Este es el camino que jubila al parser por-líneas: Source → lexer →
/// tparser → CobolProgram → codegen → BEF.
pub fn parse_program(src: &str) -> Result<CobolProgram, CobolError> {
    let mut c = Cursor::from_source(src);

    // 1. PROGRAM-ID. <nombre>.
    let mut program_id = String::from("PROGRAM");
    while !c.done() {
        if c.eat_kw("PROGRAM-ID") {
            if matches!(c.peek(), Some(Tok::Period)) {
                c.bump();
            }
            if let Some(t) = c.bump() {
                program_id = tok_text(&t);
            }
            c.eat_periods();
            break;
        }
        c.bump();
    }
    let mut prog = CobolProgram::new(program_id);

    // 2. Recorre divisiones. Los items de datos empiezan por número de nivel;
    //    al llegar a PROCEDURE DIVISION se parsean las sentencias.
    while !c.done() {
        if matches!(c.peek(), Some(Tok::Number(_))) {
            prog.data_items.push(parse_data_item(&mut c)?);
            continue;
        }
        if c.eat_kw("PROCEDURE") {
            c.eat_kw("DIVISION");
            // Cabecera (USING …) hasta el punto.
            while !matches!(c.peek(), Some(Tok::Period)) && !c.done() {
                c.bump();
            }
            c.eat_periods();
            // Sentencias hasta el final.
            while !c.done() {
                if matches!(c.peek(), Some(Tok::Period)) {
                    c.bump();
                    continue;
                }
                prog.statements.push(parse_statement(&mut c)?);
            }
            break;
        }
        // Cabeceras de división/sección y demás: saltar.
        c.bump();
    }
    Ok(prog)
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

    #[test]
    fn parses_data_item_with_pic_scale() {
        // El corazón de COBOL: un record con PIC decimal, desde tokens.
        let items = parse_data_items("01 SALDO PIC 9(5)V99 VALUE 0.").unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].level, 1);
        assert_eq!(items[0].name, "SALDO");
        assert_eq!(items[0].pic.as_deref(), Some("9(5)V99"));
        assert_eq!(items[0].scale(), 2); // ← centavos: la esencia bancaria
    }

    #[test]
    fn parses_several_items_and_text() {
        let src = "01 NOMBRE PIC X(20).\n01 EDAD PIC 9(3).\n01 PRECIO PICTURE IS S9(4)V99.";
        let items = parse_data_items(src).unwrap();
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].scale(), 0);   // texto
        assert_eq!(items[1].scale(), 0);   // entero
        assert_eq!(items[2].scale(), 2);   // dinero con signo
        assert!(items[2].pic_field.as_ref().unwrap().signed);
    }

    #[test]
    fn parses_whole_program_end_to_end() {
        let src = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. BANCO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 SALDO PIC 9(5)V99 VALUE 0.
PROCEDURE DIVISION.
MOVE 10.05 TO SALDO.
ADD 3.20 TO SALDO.
DISPLAY \"listo\".
STOP RUN.
";
        let prog = parse_program(src).unwrap();
        assert_eq!(prog.program_id, "BANCO");
        assert_eq!(prog.data_items.len(), 1);
        assert_eq!(prog.data_items[0].name, "SALDO");
        assert_eq!(prog.data_items[0].scale(), 2); // centavos
        assert_eq!(prog.statements.len(), 4);
        assert_eq!(prog.statements[0], CobolStatement::Move("10.05".into(), "SALDO".into()));

        // Pipeline NUEVO completo: tokens → AST → BEF (ejecutable real).
        let bef = crate::codegen::compile_to_bef_bytes(&prog).unwrap();
        assert!(bef.len() > 48, "el BEF debe tener cabecera + codigo");
        assert_eq!(&bef[..4], b"BEF1"); // magic del contenedor
    }
}

//! Minimal COBOL frontend for FastOS.
//!
//! This crate intentionally lives outside Ring 0. It parses a tiny, useful
//! COBOL subset and lowers it into a textual BMO-oriented IR. The next step is
//! replacing the textual emitter with real BEF generation through `bmo_abi`.

use bmo_abi::profile::BmoLanguageProfile;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CobolProgram {
    pub program_id: String,
    pub statements: Vec<CobolStatement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CobolStatement {
    Display(String),
    Accept(String),
    Move { value: String, target: String },
    StopRun,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CobolError {
    pub line: usize,
    pub message: String,
}

impl CobolError {
    fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::COBOL
}

pub fn parse(source: &str) -> Result<CobolProgram, CobolError> {
    let mut program_id = None;
    let mut in_procedure = false;
    let mut statements = Vec::new();

    for (idx, raw_line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        let normalized = line.trim_end_matches('.').trim();
        let upper = normalized.to_ascii_uppercase();

        if upper == "IDENTIFICATION DIVISION" || upper == "DATA DIVISION" {
            continue;
        }
        if upper == "PROCEDURE DIVISION" {
            in_procedure = true;
            continue;
        }
        if upper.starts_with("PROGRAM-ID") {
            let id = normalized
                .split_once('.')
                .map(|(_, rhs)| rhs)
                .or_else(|| normalized.split_once(' ').map(|(_, rhs)| rhs))
                .ok_or_else(|| CobolError::new(line_no, "PROGRAM-ID missing name"))?
                .trim()
                .trim_end_matches('.')
                .to_string();
            if id.is_empty() {
                return Err(CobolError::new(line_no, "PROGRAM-ID missing name"));
            }
            program_id = Some(id);
            continue;
        }

        if !in_procedure {
            continue;
        }

        if upper.starts_with("DISPLAY ") {
            statements.push(CobolStatement::Display(parse_operand(&normalized[8..])));
        } else if upper.starts_with("ACCEPT ") {
            let name = normalized[7..].trim();
            if name.is_empty() {
                return Err(CobolError::new(line_no, "ACCEPT missing target"));
            }
            statements.push(CobolStatement::Accept(name.to_string()));
        } else if upper.starts_with("MOVE ") {
            let rest = normalized[5..].trim();
            let upper_rest = rest.to_ascii_uppercase();
            let Some(to_pos) = upper_rest.find(" TO ") else {
                return Err(CobolError::new(line_no, "MOVE requires `TO`"));
            };
            let value = parse_operand(&rest[..to_pos]);
            let target = rest[to_pos + 4..].trim();
            if target.is_empty() {
                return Err(CobolError::new(line_no, "MOVE missing target"));
            }
            statements.push(CobolStatement::Move { value, target: target.to_string() });
        } else if upper == "STOP RUN" {
            statements.push(CobolStatement::StopRun);
        } else {
            return Err(CobolError::new(line_no, format!("unsupported COBOL statement: {normalized}")));
        }
    }

    let program_id = program_id.ok_or_else(|| CobolError::new(0, "missing PROGRAM-ID"))?;
    Ok(CobolProgram { program_id, statements })
}

pub fn compile_to_bmo_ir(program: &CobolProgram) -> String {
    let profile = profile();
    let mut out = String::new();
    out.push_str("; FastOS COBOL -> BMO IR v0\n");
    out.push_str(&format!(".profile {}\n", profile.name));
    out.push_str(&format!(".frontend {}\n", profile.frontend.name()));
    out.push_str(&format!(".runtime {}\n", profile.runtime.name()));
    out.push_str(&format!(".entry {}\n", sanitize_symbol(&program.program_id)));
    out.push('\n');

    for stmt in &program.statements {
        match stmt {
            CobolStatement::Display(value) => {
                out.push_str(&format!("  call bmo.debug.write_line, {}\n", quote(value)));
            }
            CobolStatement::Accept(target) => {
                out.push_str(&format!("  call bmo.input.read_line -> {}\n", sanitize_symbol(target)));
            }
            CobolStatement::Move { value, target } => {
                out.push_str(&format!("  mov {}, {}\n", sanitize_symbol(target), quote(value)));
            }
            CobolStatement::StopRun => out.push_str("  ret 0\n"),
        }
    }

    if !program.statements.iter().any(|s| matches!(s, CobolStatement::StopRun)) {
        out.push_str("  ret 0\n");
    }
    out
}

pub fn compile_source_to_bmo_ir(source: &str) -> Result<String, CobolError> {
    parse(source).map(|program| compile_to_bmo_ir(&program))
}

fn strip_comment(line: &str) -> &str {
    let trimmed = line.trim_start();
    if trimmed.starts_with('*') { "" } else { line }
}

fn parse_operand(value: &str) -> String {
    value.trim().trim_matches('"').trim_matches('\'').to_string()
}

fn sanitize_symbol(symbol: &str) -> String {
    symbol
        .chars()
        .map(|ch| if ch.is_ascii_alphanumeric() || ch == '_' { ch.to_ascii_lowercase() } else { '_' })
        .collect()
}

fn quote(value: &str) -> String {
    let mut escaped = String::from("\"");
    for ch in value.chars() {
        match ch {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            _ => escaped.push(ch),
        }
    }
    escaped.push('"');
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_display_program() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BMO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;

        let program = parse(src).unwrap();
        assert_eq!(program.program_id, "HELLO-BMO");
        assert_eq!(program.statements, vec![
            CobolStatement::Display("HOLA COBOL".into()),
            CobolStatement::StopRun,
        ]);
    }

    #[test]
    fn emits_bmo_ir_with_cobol_profile() {
        let ir = compile_source_to_bmo_ir(r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "FastOS".
"#).unwrap();

        assert!(ir.contains(".profile COBOL"));
        assert!(ir.contains(".frontend cobol"));
        assert!(ir.contains(".runtime cobol_core"));
        assert!(ir.contains("call bmo.debug.write_line, \"FastOS\""));
        assert!(ir.contains("ret 0"));
    }
}

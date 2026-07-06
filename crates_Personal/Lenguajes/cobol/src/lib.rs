pub mod codegen;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use bmo_abi::profile::BmoLanguageProfile;

#[derive(Debug, Clone, PartialEq)]
pub struct CobolProgram {
    pub program_id: String,
    pub data_items: Vec<DataItem>,
    pub statements: Vec<CobolStatement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataItem {
    pub level: u32,
    pub name: String,
    pub pic: Option<String>,
    pub value: Option<String>,
}

/// A named syscall definition loaded from Semantic_ASM .toml
#[derive(Debug, Clone, PartialEq)]
pub struct SyscallDef {
    pub name: String,
    pub nr: u32,
    pub arg_count: u8,
}

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
    /// Named syscall from Semantic_ASM definitions
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

#[derive(Debug, Clone, PartialEq)]
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
    let mut p = Parser::new(source);
    p.parse_program()
}

pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CobolError> {
    let program = parse(source)?;
    codegen::compile_to_bef_bytes(&program)
}

pub fn compile_source_to_bef_with_asm(
    source: &str,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CobolError> {
    let mut p = Parser::new(source);
    let program = p.parse_program_with_asm(asm_paths)?;
    codegen::compile_to_bef_bytes(&program)
}

struct Parser {
    lines: Vec<(usize, String)>,
    pos: usize,
    in_procedure: bool,
    syscalls: HashMap<String, SyscallDef>,
    usings: Vec<String>,
}

impl Parser {
    fn new(source: &str) -> Self {
        let lines: Vec<_> = source.lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.to_string()))
            .collect();
        Self { lines, pos: 0, in_procedure: false, syscalls: HashMap::new(), usings: Vec::new() }
    }

    /// Parse a simplified .toml file with lines: name = 0xNR, N
    fn load_asm_file(&mut self, path: &Path) -> Result<(), CobolError> {
        let content = fs::read_to_string(path)
            .map_err(|e| CobolError::new(0, format!("cannot read Semantic_ASM file {}: {e}", path.display())))?;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') { continue; }
            if let Some(eq_pos) = line.find('=') {
                let name = line[..eq_pos].trim().to_string();
                let rest = line[eq_pos + 1..].trim();
                let (val_str, arg_count) = if let Some(comma_pos) = rest.find(',') {
                    (rest[..comma_pos].trim(), rest[comma_pos + 1..].trim().parse::<u8>().unwrap_or(0))
                } else {
                    (rest, 0u8)
                };
                let nr = if val_str.starts_with("0x") || val_str.starts_with("0X") {
                    u32::from_str_radix(&val_str[2..], 16).unwrap_or(0)
                } else {
                    val_str.parse::<u32>().unwrap_or(0)
                };
                self.syscalls.insert(name.clone(), SyscallDef { name, nr, arg_count });
            }
        }
        Ok(())
    }

    fn parse_program_with_asm(&mut self, asm_paths: Vec<PathBuf>) -> Result<CobolProgram, CobolError> {
        // Preload ALL .toml files from asm_paths before parsing, so that
        // SYSCALL statements can resolve definitions at parse time.
        for asm_base in &asm_paths {
            self.preload_asm_dir(asm_base)?;
        }
        // Also load based on USE directives (harmless if already preloaded)
        let program = self.parse_program()?;
        let usings = std::mem::take(&mut self.usings);
        for path in &usings {
            for asm_base in &asm_paths {
                let asm_file = asm_base.join(path).with_extension("toml");
                if asm_file.exists() {
                    self.load_asm_file(&asm_file)?;
                }
            }
        }
        Ok(program)
    }

    /// Load all .toml files recursively from the given directory
    fn preload_asm_dir(&mut self, dir: &Path) -> Result<(), CobolError> {
        if !dir.is_dir() { return Ok(()); }
        if let Ok(entries) = fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    self.preload_asm_dir(&path)?;
                } else if path.extension().map_or(false, |e| e == "toml") {
                    self.load_asm_file(&path)?;
                }
            }
        }
        Ok(())
    }

    fn current(&self) -> Option<&(usize, String)> {
        self.lines.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn parse_program(&mut self) -> Result<CobolProgram, CobolError> {
        let mut program_id = String::from("DEFAULT");
        let mut data_items = Vec::new();
        let mut statements = Vec::new();
        let mut in_data = false;

        loop {
            let (line_no, raw) = match self.current() {
                Some(v) => (v.0, v.1.clone()),
                None => break,
            };
            let line = Self::strip_comment(&raw).trim().to_string();
            self.advance();
            if line.is_empty() { continue; }

            let normalized = line.trim_end_matches('.').trim().to_string();
            let upper = normalized.to_ascii_uppercase();

            if upper == "IDENTIFICATION DIVISION" || upper.starts_with("IDENTIFICATION") {
                continue;
            }
            if upper == "DATA DIVISION" || upper.starts_with("DATA") {
                in_data = true;
                continue;
            }
            if upper == "PROCEDURE DIVISION" || upper.starts_with("PROCEDURE") {
                in_data = false;
                self.in_procedure = true;
                let using = if let Some(up) = normalized.to_ascii_uppercase().find("USING") {
                    normalized[up + 5..].trim().to_string()
                } else { String::new() };
                _ = using;
                continue;
            }
            if upper.starts_with("END PROGRAM") || upper.starts_with("END") {
                break;
            }

            if in_data {
                if upper.contains("SECTION") || upper.starts_with("FD ") || upper.starts_with("FD.") {
                    continue;
                }
                if let Some(item) = self.parse_data_item(&normalized, line_no)? {
                    data_items.push(item);
                }
                continue;
            }

            if upper.starts_with("PROGRAM-ID") {
                program_id = self.extract_program_id(&normalized, line_no)?;
                continue;
            }

            if upper.starts_with("USE") {
                let path = normalized[3..].trim().trim_matches('"').to_string();
                if !path.is_empty() {
                    self.usings.push(path);
                }
                continue;
            }

            if !self.in_procedure { continue; }

            let stmt = self.parse_statement(&normalized, line_no)?;
            statements.push(stmt);
        }

        Ok(CobolProgram { program_id, data_items, statements })
    }

    fn extract_program_id(&self, line: &str, line_no: usize) -> Result<String, CobolError> {
        let id = line
            .split_once('.')
            .map(|(_, rhs)| rhs)
            .or_else(|| line.split_once(' ').map(|(_, rhs)| rhs))
            .ok_or_else(|| CobolError::new(line_no, "PROGRAM-ID missing name"))?
            .trim()
            .trim_end_matches('.')
            .to_string();
        if id.is_empty() { Err(CobolError::new(line_no, "PROGRAM-ID missing name")) } else { Ok(id) }
    }

    fn parse_data_item(&self, line: &str, _line_no: usize) -> Result<Option<DataItem>, CobolError> {
        let trimmed = line.trim();
        let first = trimmed.split_whitespace().next().unwrap_or("");
        let level: u32 = first.parse().unwrap_or(77);
        if level == 0 { return Ok(None); }
        let rest = trimmed.trim_start_matches(|c: char| c.is_ascii_digit());
        let rest = rest.trim();
        if rest.is_empty() { return Ok(None); }
        let parts: Vec<&str> = rest.split_whitespace().collect();
        if parts.is_empty() { return Ok(None); }
        let name = parts[0].trim_end_matches('.').to_string();
        let mut pic = None;
        let mut value = None;
        for w in &parts {
            let uw = w.to_ascii_uppercase();
            if uw == "PIC" || uw == "PICTURE" {}
            else if uw == "VALUE" {}
            else if uw.starts_with("X(") || uw.starts_with("9(") || uw.starts_with("S9(") || uw.starts_with("V9(") {
                pic = Some(w.to_string());
            }
            else if w.starts_with('"') || w.starts_with('\'') {
                value = Some(w.trim_matches('"').trim_matches('\'').to_string());
            }
            else if uw.starts_with("Z") || uw.chars().all(|c| c == '9' || c == 'V' || c == 'S' || c == 'Z') {
                pic = Some(w.to_string());
            }
        }
        Ok(Some(DataItem { level, name, pic, value }))
    }

    fn parse_statement(&self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let upper = line.trim().to_ascii_uppercase();

        if upper.starts_with("SYSCALL ") {
            let rest = line[8..].trim().trim_end_matches('.');
            let parts: Vec<&str> = rest.splitn(2, ' ').collect();
            let name = parts[0].to_string();
            let args = if parts.len() > 1 {
                parts[1].split(',').map(|a| a.trim().trim_matches('"').trim_matches('\'').to_string()).collect()
            } else { Vec::new() };
            if let Some(def) = self.syscalls.get(&name).cloned() {
                if args.len() != def.arg_count as usize {
                    return Err(CobolError::new(line_no, format!(
                        "syscall {}() expects {} arguments, got {}",
                        def.name, def.arg_count, args.len()
                    )));
                }
                Ok(CobolStatement::Syscall(def, args))
            } else {
                Err(CobolError::new(line_no, format!("unknown syscall: {name}")))
            }
        } else if upper.starts_with("DISPLAY ") {
            let val = Self::parse_operand(&line[8..]);
            Ok(CobolStatement::Display(val))
        } else if upper.starts_with("ACCEPT ") {
            let name = line[7..].trim().to_string();
            if name.is_empty() { return Err(CobolError::new(line_no, "ACCEPT missing target")); }
            Ok(CobolStatement::Accept(name))
        } else if upper.starts_with("MOVE ") {
            let rest = line[5..].trim();
            let up_rest = rest.to_ascii_uppercase();
            let Some(to_pos) = up_rest.find(" TO ") else {
                return Err(CobolError::new(line_no, "MOVE requires `TO`"));
            };
            let value = Self::parse_operand(&rest[..to_pos]);
            let target = rest[to_pos + 4..].trim().to_string();
            if target.is_empty() { return Err(CobolError::new(line_no, "MOVE missing target")); }
            Ok(CobolStatement::Move(value, target))
        } else if upper.starts_with("ADD ") {
            let rest = line[4..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(to_pos) = up.find(" TO ") else {
                return Err(CobolError::new(line_no, "ADD requires `TO`"));
            };
            let val = Self::parse_operand(&rest[..to_pos]);
            let target = rest[to_pos + 4..].trim().to_string();
            Ok(CobolStatement::Add(val, target))
        } else if upper.starts_with("SUBTRACT ") {
            let rest = line[9..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(from_pos) = up.find(" FROM ") else {
                return Err(CobolError::new(line_no, "SUBTRACT requires `FROM`"));
            };
            let val = Self::parse_operand(&rest[..from_pos]);
            let target = rest[from_pos + 6..].trim().to_string();
            Ok(CobolStatement::Subtract(val, target))
        } else if upper.starts_with("MULTIPLY ") {
            let rest = line[9..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(by_pos) = up.find(" BY ") else {
                return Err(CobolError::new(line_no, "MULTIPLY requires `BY`"));
            };
            let val = Self::parse_operand(&rest[..by_pos]);
            let target = rest[by_pos + 4..].trim().to_string();
            Ok(CobolStatement::Multiply(val, target))
        } else if upper.starts_with("DIVIDE ") {
            let rest = line[7..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(by_pos) = up.find(" BY ") else {
                return Err(CobolError::new(line_no, "DIVIDE requires `BY`"));
            };
            let val = Self::parse_operand(&rest[..by_pos]);
            let target = rest[by_pos + 4..].trim().to_string();
            Ok(CobolStatement::Divide(val, target))
        } else if upper.starts_with("COMPUTE ") {
            let rest = line[8..].trim();
            let eq_pos = rest.find('=').unwrap_or(0);
            if eq_pos == 0 { return Err(CobolError::new(line_no, "COMPUTE requires `=`")); }
            let target = rest[..eq_pos].trim().to_string();
            let expr = rest[eq_pos + 1..].trim().to_string();
            Ok(CobolStatement::Compute(target, expr))
        } else if upper.starts_with("OPEN ") {
            let rest = line[5..].trim();
            let parts: Vec<&str> = rest.splitn(2, |c: char| c.is_whitespace()).collect();
            if parts.len() < 2 { return Err(CobolError::new(line_no, "OPEN requires mode and file")); }
            Ok(CobolStatement::Open(parts[0].to_string(), parts[1].trim_end_matches('.').to_string()))
        } else if upper.starts_with("CLOSE ") {
            Ok(CobolStatement::Close(line[6..].trim().trim_end_matches('.').to_string()))
        } else if upper.starts_with("READ ") {
            let rest = line[5..].trim().trim_end_matches('.');
            let parts: Vec<&str> = if let Some(into_pos) = rest.to_ascii_uppercase().find(" INTO ") {
                let file = rest[..into_pos].trim();
                let into = rest[into_pos + 6..].trim();
                vec![file, into]
            } else { vec![rest, ""] };
            Ok(CobolStatement::Read(parts[0].to_string(), parts.get(1).unwrap_or(&"").to_string()))
        } else if upper.starts_with("WRITE ") {
            Ok(CobolStatement::Write(line[6..].trim().trim_end_matches('.').to_string()))
        } else if upper.starts_with("IF ") {
            self.parse_if(line, line_no)
        } else if upper.starts_with("PERFORM ") {
            let rest = line[8..].trim();
            if let Some(until_pos) = rest.to_ascii_uppercase().find(" UNTIL ") {
                let until_part = rest[until_pos + 7..].trim().trim_end_matches('.');
                let cond_parts: Vec<&str> = until_part.splitn(3, |c: char| c.is_whitespace()).collect();
                if cond_parts.len() >= 3 {
                    Ok(CobolStatement::PerformUntil(
                        Self::parse_operand(cond_parts[0]),
                        cond_parts[1..].join(" "),
                    ))
                } else { Ok(CobolStatement::Perform(1)) }
            } else {
                let times: u32 = rest.trim().trim_end_matches('.').parse().unwrap_or(1);
                Ok(CobolStatement::Perform(times))
            }
        } else if upper == "STOP RUN" || upper == "STOP RUN." {
            Ok(CobolStatement::StopRun)
        } else if upper.contains("END-IF") || upper.contains("END-PERFORM") {
            Ok(CobolStatement::Expr(line.to_string()))
        } else {
            Err(CobolError::new(line_no, format!("unsupported COBOL statement: {line}")))
        }
    }

    fn parse_if(&self, line: &str, _line_no: usize) -> Result<CobolStatement, CobolError> {
        let _rest = line[3..].trim_end_matches('.').trim();
        Ok(CobolStatement::If(Vec::new(), Vec::new(), Vec::new()))
    }

    fn parse_operand(value: &str) -> String {
        value.trim().trim_matches('"').trim_matches('\'').to_string()
    }

    fn strip_comment(line: &str) -> &str {
        let trimmed = line.trim_start();
        if trimmed.starts_with('*') || trimmed.starts_with(">>SOURCE") { "" } else { line }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn parses_display_program() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO-BMO.
DATA DIVISION.
WORKING-STORAGE SECTION.
01 WS-NAME PIC X(20).
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert_eq!(program.program_id, "HELLO-BMO");
        assert_eq!(program.data_items.len(), 1);
        assert_eq!(program.data_items[0].name, "WS-NAME");
    }

    #[test]
    fn emits_bef() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. HELLO.
PROCEDURE DIVISION.
DISPLAY "HOLA COBOL".
STOP RUN.
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
    }

    #[test]
    fn parses_arithmetic() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. ARITH.
PROCEDURE DIVISION.
MOVE 10 TO WS-NUM.
ADD 5 TO WS-NUM.
SUBTRACT 3 FROM WS-NUM.
MULTIPLY 2 BY WS-NUM.
DIVIDE 4 BY WS-NUM.
COMPUTE WS-NUM = 10 + 20.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert!(program.statements.len() >= 6);
    }

    #[test]
    fn parses_open_read_write_close() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. FILEIO.
PROCEDURE DIVISION.
OPEN INPUT INFILE.
READ INFILE INTO WS-REC.
WRITE OUTFILE.
CLOSE INFILE.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert_eq!(program.statements.len(), 5);
    }

    #[test]
    fn parses_perform() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. LOOP.
PROCEDURE DIVISION.
PERFORM 5.
PERFORM UNTIL WS-COUNT > 10.
STOP RUN.
"#;
        let program = parse(src).unwrap();
        assert!(program.statements.len() >= 2);
    }

    #[test]
    fn parses_cobol_use_and_syscall() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TEST.
USE "bmo/proc".
PROCEDURE DIVISION.
SYSCALL bmo_exit 0.
"#;
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let p = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
        assert!(p.len() > 48);
    }

    #[test]
    fn cobol_syscall_with_asm_path() {
        let src = r#"
IDENTIFICATION DIVISION.
PROGRAM-ID. TEST.
USE "bmo/proc".
PROCEDURE DIVISION.
SYSCALL bmo_exit 42.
"#;
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_asm(src, vec![asm]).unwrap();
        assert!(bef.len() > 48);
        // Should contain mov eax, 0x181 (bmo_exit nr)
        let mov_eax = &[0xB8u8, 0x81, 0x01, 0x00, 0x00];
        assert!(bef.windows(5).any(|w| w == mov_eax), "BEF should contain mov eax, 0x181 for bmo_exit");
    }
}

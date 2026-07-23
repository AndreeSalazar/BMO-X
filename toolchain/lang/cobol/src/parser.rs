use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast::{
    CobolError, CobolProgram, CobolStatement, DataItem, SyscallDef, SyscallMap,
};

pub struct Parser {
    lines: Vec<(usize, String)>,
    pos: usize,
    in_procedure: bool,
    syscalls: SyscallMap,
    usings: Vec<String>,
}

impl Parser {
    pub fn new(source: &str) -> Self {
        let mut syscalls = HashMap::new();
        for d in bmo_abi::asm::defs::syscalls() {
            syscalls.insert(d.name.clone(), SyscallDef { name: d.name, nr: d.nr, arg_count: d.arg_count });
        }
        let lines: Vec<_> = source.lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l.to_string()))
            .collect();
        Self { lines, pos: 0, in_procedure: false, syscalls, usings: Vec::new() }
    }

    pub fn parse_program(&mut self) -> Result<CobolProgram, CobolError> {
        let mut program = CobolProgram::new(String::from("DEFAULT"));
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
                    program.add_data_item(item);
                }
                continue;
            }

            if upper.starts_with("PROGRAM-ID") {
                program.program_id = self.extract_program_id(&normalized, line_no)?;
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
            program.add_statement(stmt);
        }

        Ok(program)
    }

    pub fn parse_program_with_asm(&mut self, _asm_paths: Vec<PathBuf>) -> Result<CobolProgram, CobolError> {
        self.parse_program()
    }

    fn current(&self) -> Option<&(usize, String)> {
        self.lines.get(self.pos)
    }

    fn advance(&mut self) {
        self.pos += 1;
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
        if id.is_empty() {
            Err(CobolError::new(line_no, "PROGRAM-ID missing name"))
        } else {
            Ok(id)
        }
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
        let mut i = 1;
        while i < parts.len() {
            let uw = parts[i].to_ascii_uppercase();
            if uw == "PIC" || uw == "PICTURE" {
                if i + 1 < parts.len() {
                    i += 1;
                    pic = Some(parts[i].trim_end_matches('.').to_string());
                }
            } else if uw == "VALUE" {
                if i + 1 < parts.len() {
                    i += 1;
                    value = Some(parts[i].trim_matches('"').trim_matches('\'').to_string());
                }
            }
            i += 1;
        }

        Ok(Some(DataItem::new(level, name, pic, value)))
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
            // Vocabulario COBOL COMPLETO vía las tablas generadas por Python
            // (cobol-gen): el parser distingue un verbo COBOL conocido pero
            // aún sin codegen, de una palabra reservada de cierto estándar,
            // de algo que sencillamente no es COBOL. Conoce todo el idioma
            // aunque todavía no compile cada forma.
            use crate::generated::words;
            let first = upper.split_whitespace().next().unwrap_or("");
            if let Some(kind) = words::verb_kind(first) {
                Err(CobolError::new(line_no, format!(
                    "verbo COBOL '{first}' (=> {kind}) reconocido, pero esta forma aún no se compila: {line}"
                )))
            } else if let Some(std) = words::reserved_since(first) {
                Err(CobolError::new(line_no, format!(
                    "'{first}' es palabra reservada COBOL ({std}); aún sin soporte como sentencia: {line}"
                )))
            } else {
                Err(CobolError::new(line_no, format!(
                    "no es COBOL reconocido: '{first}' desconocido en: {line}"
                )))
            }
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

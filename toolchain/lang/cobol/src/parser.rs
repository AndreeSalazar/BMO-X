use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast::{
    CobolCondition, CobolError, CobolProgram, CobolStatement, DataItem, SyscallDef,
    SyscallMap,
};

/// Cabecera de un PERFORM ya analizada, antes de leer el cuerpo.
enum PerformHeader {
    Times(u32),
    Until(Vec<CobolCondition>),
}

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

    fn parse_statement(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
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
            self.parse_perform(line, line_no)
        } else if upper == "STOP RUN" || upper == "STOP RUN." {
            Ok(CobolStatement::StopRun)
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

    /// `IF <cond> [THEN] … [ELSE …] END-IF`.
    ///
    /// Se exige `END-IF` (COBOL-85) en vez de aceptar el alcance por punto
    /// del COBOL clásico. No es pereza: el alcance por punto es ambiguo de
    /// leer y es una fuente clásica de bugs silenciosos —justo lo que este
    /// compilador acaba de dejar de hacer—. Si falta, el error lo dice.
    fn parse_if(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let head = line[3..].trim();
        let head = Self::strip_trailing_word(head, "THEN");
        let conditions = Self::parse_conditions(head, line_no)?;

        let mut then_branch = Vec::new();
        let mut else_branch = Vec::new();
        let mut in_else = false;

        loop {
            let (inner_no, raw) = match self.current() {
                Some(v) => (v.0, v.1.clone()),
                None => {
                    return Err(CobolError::new(
                        line_no,
                        "IF sin END-IF: esta implementacion exige el cierre explicito de COBOL-85",
                    ))
                }
            };
            let inner = Self::strip_comment(&raw).trim().to_string();
            self.advance();
            if inner.is_empty() {
                continue;
            }
            let up = inner.trim_end_matches('.').trim().to_ascii_uppercase();
            if up == "END-IF" {
                break;
            }
            if up == "ELSE" {
                if in_else {
                    return Err(CobolError::new(inner_no, "ELSE duplicado en el mismo IF"));
                }
                in_else = true;
                continue;
            }
            let stmt = self.parse_statement(inner.trim_end_matches('.').trim(), inner_no)?;
            if in_else {
                else_branch.push(stmt);
            } else {
                then_branch.push(stmt);
            }
        }

        Ok(CobolStatement::If(conditions, then_branch, else_branch))
    }

    /// `PERFORM <n> TIMES … END-PERFORM` o `PERFORM UNTIL <cond> … END-PERFORM`.
    fn parse_perform(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let rest = line[8..].trim().trim_end_matches('.').trim();
        let upper = rest.to_ascii_uppercase();

        let header = if let Some(pos) = upper.find("UNTIL ") {
            if pos == 0 {
                PerformHeader::Until(Self::parse_conditions(rest[6..].trim(), line_no)?)
            } else {
                return Err(CobolError::new(
                    line_no,
                    "solo se compila `PERFORM UNTIL <cond>` o `PERFORM <n> TIMES`",
                ));
            }
        } else {
            let count_text = Self::strip_trailing_word(rest, "TIMES");
            match count_text.trim().parse::<u32>() {
                Ok(n) => PerformHeader::Times(n),
                Err(_) => {
                    return Err(CobolError::new(
                        line_no,
                        format!(
                            "PERFORM sin forma compilable: '{rest}'. Hoy se compilan \
                             `PERFORM <n> TIMES` y `PERFORM UNTIL <cond>`; PERFORM de \
                             parrafo aun no (no hay parrafos)."
                        ),
                    ))
                }
            }
        };

        let mut body = Vec::new();
        loop {
            let (inner_no, raw) = match self.current() {
                Some(v) => (v.0, v.1.clone()),
                None => {
                    return Err(CobolError::new(
                        line_no,
                        "PERFORM sin END-PERFORM: esta implementacion exige el cierre explicito",
                    ))
                }
            };
            let inner = Self::strip_comment(&raw).trim().to_string();
            self.advance();
            if inner.is_empty() {
                continue;
            }
            if inner.trim_end_matches('.').trim().eq_ignore_ascii_case("END-PERFORM") {
                break;
            }
            body.push(self.parse_statement(inner.trim_end_matches('.').trim(), inner_no)?);
        }

        Ok(match header {
            PerformHeader::Times(n) => CobolStatement::PerformTimes(n, body),
            PerformHeader::Until(c) => CobolStatement::PerformUntil(c, body),
        })
    }

    /// Parsea una condición COBOL, con operadores simbólicos y con palabras.
    ///
    /// Acepta `A = B`, `A > B`, `A >= B`, `A NOT = B`, y las formas del
    /// estándar en palabras: `A IS EQUAL TO B`, `A IS GREATER THAN B`,
    /// `A IS NOT LESS THAN B`… Varias condiciones se unen con `AND`.
    ///
    /// `OR` se RECHAZA con un error explícito: mezclar AND y OR necesita un
    /// árbol de condiciones, y compilarlo como si fuera AND daría un
    /// programa que corre y decide mal.
    fn parse_conditions(text: &str, line_no: usize) -> Result<Vec<CobolCondition>, CobolError> {
        let normalized = Self::normalize_condition_words(text);
        if normalized.to_ascii_uppercase().split_whitespace().any(|w| w == "OR") {
            return Err(CobolError::new(
                line_no,
                "condiciones con OR aun no se compilan (haria falta un arbol \
                 AND/OR); reescribela con AND o con IF anidados",
            ));
        }

        let mut out = Vec::new();
        for part in Self::split_on_word(&normalized, "AND") {
            out.push(Self::parse_one_condition(part.trim(), line_no)?);
        }
        if out.is_empty() {
            return Err(CobolError::new(line_no, "condicion vacia"));
        }
        Ok(out)
    }

    /// Convierte las formas en palabras del estándar al operador simbólico
    /// equivalente, para que el análisis quede en un solo sitio.
    fn normalize_condition_words(text: &str) -> String {
        // El orden importa: las formas largas primero, si no `NOT LESS`
        // se comería el `LESS` de `NOT LESS THAN`.
        const REPLACEMENTS: &[(&str, &str)] = &[
            ("IS NOT GREATER THAN OR EQUAL TO", " < "),
            ("IS NOT LESS THAN OR EQUAL TO", " > "),
            ("IS GREATER THAN OR EQUAL TO", " >= "),
            ("IS LESS THAN OR EQUAL TO", " <= "),
            ("GREATER THAN OR EQUAL TO", " >= "),
            ("LESS THAN OR EQUAL TO", " <= "),
            ("IS NOT GREATER THAN", " <= "),
            ("IS NOT LESS THAN", " >= "),
            ("IS NOT EQUAL TO", " <> "),
            ("IS GREATER THAN", " > "),
            ("IS LESS THAN", " < "),
            ("IS EQUAL TO", " = "),
            ("NOT GREATER THAN", " <= "),
            ("NOT LESS THAN", " >= "),
            ("NOT EQUAL TO", " <> "),
            ("GREATER THAN", " > "),
            ("LESS THAN", " < "),
            ("EQUAL TO", " = "),
            ("IS NOT", " <> "),
            ("NOT =", " <> "),
            ("EQUALS", " = "),
            ("IS ", " "),
        ];

        let mut result = text.to_string();
        for (words, symbol) in REPLACEMENTS {
            loop {
                let upper = result.to_ascii_uppercase();
                let Some(pos) = upper.find(words) else { break };
                result.replace_range(pos..pos + words.len(), symbol);
            }
        }
        result
    }

    /// Parte por una palabra completa (no por subcadena: `AND` no debe
    /// cortar un dato llamado `BRANDING`).
    fn split_on_word<'a>(text: &'a str, word: &str) -> Vec<&'a str> {
        let mut parts = Vec::new();
        let mut start = 0usize;
        let upper = text.to_ascii_uppercase();
        let bytes = upper.as_bytes();
        let mut i = 0usize;
        while i + word.len() <= bytes.len() {
            let at_word = upper[i..].starts_with(word);
            let left_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
            let after = i + word.len();
            let right_ok = after == bytes.len() || bytes[after].is_ascii_whitespace();
            if at_word && left_ok && right_ok {
                parts.push(&text[start..i]);
                start = after;
                i = after;
            } else {
                i += 1;
            }
        }
        parts.push(&text[start..]);
        parts.into_iter().filter(|p| !p.trim().is_empty()).collect()
    }

    fn parse_one_condition(text: &str, line_no: usize) -> Result<CobolCondition, CobolError> {
        // Los de dos caracteres primero: `>=` contiene `>`.
        const OPERATORS: &[&str] = &[">=", "<=", "<>", "!=", "=", ">", "<"];
        for op in OPERATORS {
            let Some(pos) = text.find(op) else { continue };
            let left = Self::parse_operand(&text[..pos]);
            let right = Self::parse_operand(&text[pos + op.len()..]);
            if left.is_empty() || right.is_empty() {
                return Err(CobolError::new(
                    line_no,
                    format!("condicion incompleta: '{text}'"),
                ));
            }
            return Ok(match *op {
                "=" => CobolCondition::Equal(left, right),
                "<>" | "!=" => CobolCondition::NotEqual(left, right),
                ">" => CobolCondition::Greater(left, right),
                "<" => CobolCondition::Less(left, right),
                ">=" => CobolCondition::GreaterOrEqual(left, right),
                "<=" => CobolCondition::LessOrEqual(left, right),
                _ => unreachable!(),
            });
        }
        Err(CobolError::new(
            line_no,
            format!("no encuentro operador de comparacion en '{text}'"),
        ))
    }

    /// Quita una palabra final (`TIMES`, `THEN`) si está presente.
    fn strip_trailing_word<'a>(text: &'a str, word: &str) -> &'a str {
        let trimmed = text.trim();
        if trimmed.len() >= word.len() {
            let (head, tail) = trimmed.split_at(trimmed.len() - word.len());
            if tail.eq_ignore_ascii_case(word)
                && (head.is_empty() || head.ends_with(char::is_whitespace))
            {
                return head.trim();
            }
        }
        trimmed
    }

    fn parse_operand(value: &str) -> String {
        value.trim().trim_matches('"').trim_matches('\'').to_string()
    }

    fn strip_comment(line: &str) -> &str {
        let trimmed = line.trim_start();
        if trimmed.starts_with('*') || trimmed.starts_with(">>SOURCE") { "" } else { line }
    }
}

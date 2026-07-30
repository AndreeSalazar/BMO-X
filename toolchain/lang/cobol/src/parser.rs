use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast::{
    DisplayArg,
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
        // El `FD` cuyo registro todavia no ha aparecido.
        let mut fd_abierto = String::new();

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

            // `SELECT <nombre> ASSIGN TO "<ruta>"`. Vive en FILE-CONTROL, que
            // esta en la ENVIRONMENT DIVISION — antes de la DATA, asi que se
            // reconoce fuera del bloque de datos.
            if upper.starts_with("SELECT ") {
                program.files.push(Self::parse_select(&normalized, line_no)?);
                continue;
            }

            if in_data {
                if upper.starts_with("FD ") || upper.starts_with("FD.") {
                    // El `01` que venga DESPUES es el registro de este
                    // fichero. Se apunta el nombre y el siguiente dato lo
                    // reclama: es como COBOL lo escribe, y asi no hace falta
                    // una sintaxis nueva para decir "este 01 es de aquel FD".
                    fd_abierto = normalized[2..]
                        .trim()
                        .trim_end_matches('.')
                        .trim()
                        .to_ascii_uppercase();
                    continue;
                }
                if upper.contains("SECTION") {
                    // Cambiar de seccion cierra el FD: un `01` de
                    // WORKING-STORAGE no es el registro de nadie.
                    fd_abierto.clear();
                    continue;
                }
                if let Some(item) = self.parse_data_item(&normalized, line_no)? {
                    if !fd_abierto.is_empty() {
                        let nombre = item.name.clone();
                        match program.files.iter_mut().find(|f| f.name == fd_abierto) {
                            Some(f) => f.record = nombre,
                            None => {
                                return Err(CobolError::new(
                                    line_no,
                                    format!(
                                        "FD {fd_abierto} sin su SELECT: declara \
                                         `SELECT {fd_abierto} ASSIGN TO \"ruta\"` en FILE-CONTROL"
                                    ),
                                ))
                            }
                        }
                        fd_abierto.clear();
                    }
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

    fn parse_data_item(&self, line: &str, line_no: usize) -> Result<Option<DataItem>, CobolError> {
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
        let mut occurs = None;
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
            } else if uw == "OCCURS" {
                // `OCCURS <n> [TIMES]`. El `TIMES` es opcional en el estandar y
                // aqui tambien: lo que hace falta es el numero.
                let n = parts.get(i + 1).map(|s| s.trim_end_matches('.'));
                let n: u32 = match n.and_then(|s| s.parse().ok()) {
                    Some(n) if n > 0 => n,
                    _ => {
                        return Err(CobolError::new(
                            line_no,
                            format!(
                                "OCCURS de {name} sin cuantas veces: escribe \
                                 `OCCURS 12 TIMES`. Un OCCURS variable \
                                 (DEPENDING ON) todavia no se compila"
                            ),
                        ))
                    }
                };
                occurs = Some(n);
                i += 1;
            }
            i += 1;
        }

        let mut item = DataItem::new(level, name, pic, value);
        if let Some(n) = occurs {
            // Donde el estandar NO deja OCCURS: 01 (y 66/77/88). No es rigor
            // por rigor — un `01` es el registro entero, y repetir el registro
            // es otra cosa que repetir un campo de dentro. La forma buena es
            // el grupo, y es la que un banco escribe.
            if matches!(item.level, 1 | 66 | 77 | 88) {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "OCCURS en el nivel {:02} ({}) no existe: mete el campo en un grupo\n\
                         \x20      01 TABLA.\n\
                         \x20          05 {} PIC ... OCCURS {} TIMES.",
                        item.level, item.name, item.name, n
                    ),
                ));
            }
            if item.pic.is_none() {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "OCCURS de {} sin PIC: una tabla de grupos (cada elemento \
                         con varios campos) todavia no se compila",
                        item.name
                    ),
                ));
            }
            item.occurs = Some(n);
        }
        Ok(Some(item))
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
            // El parser por-lineas (el viejo) mira si venia entrecomillado:
            // `parse_operand` ya quita las comillas, asi que hay que
            // preguntarselo al texto CRUDO antes de que se pierda esa pista.
            let crudo = line[8..].trim();
            let val = Self::parse_operand(&line[8..]);
            let arg = if crudo.starts_with('"') || crudo.starts_with('\'') {
                DisplayArg::Literal(val)
            } else {
                DisplayArg::Variable(val)
            };
            Ok(CobolStatement::Display(arg))
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
            self.parse_read(line, line_no)
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
    /// `SELECT <nombre> ASSIGN TO "<ruta>"`.
    ///
    /// La ruta va entre comillas y es un literal: se resuelve al COMPILAR y
    /// viaja dentro del `.bex` como inmediatos. Un `ASSIGN TO` a una variable
    /// exigiría pasar la ruta byte a byte en ejecución, y eso es otra puerta.
    fn parse_select(line: &str, line_no: usize) -> Result<crate::ast::CobolFile, CobolError> {
        let resto = line[6..].trim().trim_end_matches('.').trim();
        let arriba = resto.to_ascii_uppercase();
        let corte = arriba.find("ASSIGN").ok_or_else(|| {
            CobolError::new(line_no, "SELECT sin ASSIGN TO: falta decir a que ruta")
        })?;
        let name = resto[..corte].trim().to_ascii_uppercase();
        if name.is_empty() {
            return Err(CobolError::new(line_no, "SELECT sin nombre de fichero"));
        }
        // Tras `ASSIGN` puede venir `TO` o no; el estándar lo permite.
        let cola = resto[corte + 6..].trim();
        let cola = Self::strip_leading_word(cola, "TO");
        let path = cola.trim().trim_matches('"').trim_matches('\'').to_string();
        if path.is_empty() {
            return Err(CobolError::new(
                line_no,
                "ASSIGN TO sin ruta: escribe `ASSIGN TO \"datos/movim.txt\"`",
            ));
        }
        // La ruta se comprueba AQUI y no en ejecucion. El volumen de datos es
        // FAT32 y su tabla guarda nombres 8.3: `apps/movimientos.txt` abriria
        // un handle nulo en la maquina, y COBOL lo leeria como "fichero vacio"
        // — o sea, un cierre a cero sin una sola queja. Es exactamente el fallo
        // que este compilador no comete: el nombre lo sabe al compilar, asi que
        // lo dice al compilar.
        if let Err(motivo) = Self::cabe_en_8_3(&path) {
            return Err(CobolError::new(line_no, format!("ASSIGN TO \"{path}\": {motivo}")));
        }
        Ok(crate::ast::CobolFile { name, path, record: String::new() })
    }

    /// ¿Cada tramo de la ruta cabe en un nombre 8.3 de FAT32?
    ///
    /// Ocho de nombre y tres de extension, por tramo. Es limite del volumen de
    /// hoy, no del lenguaje: cuando ESTRATOS acepte escritura y nombres largos,
    /// esta comprobacion se relaja — y hasta entonces mentir sale mas caro.
    fn cabe_en_8_3(ruta: &str) -> Result<(), String> {
        for tramo in ruta.split(['/', '\\']) {
            // La letra de unidad (`A:`) y los tramos vacios de `//` no cuentan.
            if tramo.is_empty() || tramo.ends_with(':') {
                continue;
            }
            let (base, ext) = match tramo.rfind('.') {
                Some(i) => (&tramo[..i], &tramo[i + 1..]),
                None => (tramo, ""),
            };
            if base.is_empty() {
                return Err(format!("el tramo '{tramo}' no tiene nombre"));
            }
            if base.len() > 8 || ext.len() > 3 {
                return Err(format!(
                    "'{tramo}' no cabe en 8.3 (FAT32): {} letras de nombre y {} de extension, \
                     el maximo es 8 y 3",
                    base.len(),
                    ext.len()
                ));
            }
        }
        Ok(())
    }

    fn strip_leading_word<'a>(s: &'a str, word: &str) -> &'a str {
        let arriba = s.to_ascii_uppercase();
        if arriba.starts_with(word) && arriba[word.len()..].starts_with(' ') {
            s[word.len()..].trim_start()
        } else {
            s
        }
    }

    /// `READ <fichero> AT END <stmts> [NOT AT END <stmts>] END-READ`.
    ///
    /// El `AT END` es OBLIGATORIO y no por rigor: es lo único que puede parar
    /// un `PERFORM UNTIL` sobre un fichero. Un `READ` sin él compila a un
    /// bucle infinito, y eso es peor que no compilar.
    fn parse_read(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let resto = line[4..].trim().trim_end_matches('.').trim();
        // El nombre es la primera palabra; lo que siga en la MISMA línea se
        // trata como si fuera la línea siguiente.
        let corte = resto.find(char::is_whitespace).unwrap_or(resto.len());
        let fichero = resto[..corte].trim().to_ascii_uppercase();
        if fichero.is_empty() {
            return Err(CobolError::new(line_no, "READ sin nombre de fichero"));
        }
        let mut cola = resto[corte..].trim().to_string();

        let mut al_final = Vec::new();
        let mut si_hay = Vec::new();
        // 0 = todavía no se ha visto ninguna cláusula.
        let mut rama = 0u8;
        let mut visto_at_end = false;

        loop {
            let (inner_no, texto) = if !cola.is_empty() {
                let t = core::mem::take(&mut cola);
                (line_no, t)
            } else {
                let (no, raw) = match self.current() {
                    Some(v) => (v.0, v.1.clone()),
                    None => {
                        return Err(CobolError::new(
                            line_no,
                            "READ sin END-READ: esta implementacion exige el cierre explicito",
                        ))
                    }
                };
                self.advance();
                (no, Self::strip_comment(&raw).trim().to_string())
            };
            if texto.is_empty() {
                continue;
            }
            let up = texto.trim_end_matches('.').trim().to_ascii_uppercase();
            if up == "END-READ" {
                break;
            }
            // Las cláusulas pueden traer su primera sentencia pegada:
            // `AT END MOVE 1 TO FIN`.
            let (etiqueta, sobra) = if up.starts_with("NOT AT END") {
                (2u8, texto.trim()[10..].trim().to_string())
            } else if up.starts_with("AT END") {
                (1u8, texto.trim()[6..].trim().to_string())
            } else {
                (0u8, String::new())
            };
            if etiqueta != 0 {
                rama = etiqueta;
                if etiqueta == 1 {
                    visto_at_end = true;
                }
                if sobra.is_empty() {
                    continue;
                }
                cola = sobra;
                continue;
            }
            if rama == 0 {
                return Err(CobolError::new(
                    inner_no,
                    "en un READ, lo primero tiene que ser AT END o NOT AT END",
                ));
            }
            let stmt = self.parse_statement(texto.trim_end_matches('.').trim(), inner_no)?;
            if rama == 1 {
                al_final.push(stmt);
            } else {
                si_hay.push(stmt);
            }
        }

        if !visto_at_end {
            return Err(CobolError::new(
                line_no,
                "READ sin AT END: sin el, un PERFORM UNTIL sobre este fichero no termina nunca",
            ));
        }
        Ok(CobolStatement::Read(fichero, al_final, si_hay))
    }

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

use std::collections::HashMap;
use std::path::PathBuf;

use crate::ast::{
    DisplayArg,
    CobolCondition, CobolError, CobolProgram, CobolStatement, Condicion, DataItem, Redondeo,
    SyscallDef,
    SyscallMap,
};

/// Cabecera de un PERFORM ya analizada, antes de leer el cuerpo.
enum PerformHeader {
    Times(u32),
    Until(Condicion),
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
                if let Some(mut item) = self.parse_data_item(&normalized, line_no)? {
                    // Un 88 se ata al dato que lo precede. Es lo que dice el
                    // estandar y lo que uno lee al mirarlo: el nombre de
                    // condicion cuelga del campo de arriba.
                    if item.level == 88 {
                        match program.data_items.iter().rev().find(|d| d.level != 88) {
                            Some(p) => item.padre = Some(p.name.clone()),
                            None => {
                                return Err(CobolError::new(
                                    line_no,
                                    format!(
                                        "{} es un nivel 88 y no hay ningun dato encima del que colgar",
                                        item.name
                                    ),
                                ))
                            }
                        }
                        program.add_data_item(item);
                        continue;
                    }
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

            // ★ Un NOMBRE DE PÁRRAFO abre uno nuevo. Todo lo que venga detrás
            // es suyo hasta el siguiente nombre.
            if let Some(nombre) = Self::nombre_de_parrafo(&line) {
                if program.parrafo(&nombre).is_some() {
                    return Err(CobolError::new(
                        line_no,
                        format!(
                            "el parrafo {nombre} ya existe: dos parrafos con el mismo nombre \
                             hacen que un PERFORM no sepa a cual va"
                        ),
                    ));
                }
                program.abrir_parrafo(nombre);
                continue;
            }

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

    /// `PERFORM <parrafo> [THRU <otro>] [<n> TIMES | UNTIL <cond>]`.
    ///
    /// El orden en el que se recorta importa: primero el `UNTIL`, que se lleva
    /// todo lo que queda a su derecha, y después el `THRU`. Al revés, un
    /// nombre de párrafo con la palabra dentro se partiría mal.
    fn parse_perform_fuera(rest: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let upper = rest.to_ascii_uppercase();

        // El `UNTIL` se lleva la cola entera.
        let (cabeza, hasta_que) = match Self::pos_palabra(&upper, "UNTIL") {
            Some(i) => {
                let cond = rest[i + 5..].trim();
                if cond.is_empty() {
                    return Err(CobolError::new(line_no, "PERFORM … UNTIL sin condicion"));
                }
                (rest[..i].trim(), Some(Self::parse_condicion(cond, line_no)?))
            }
            None => (rest, None),
        };

        // `<n> TIMES` al final de lo que queda.
        let upper_cabeza = cabeza.to_ascii_uppercase();
        let (cabeza, veces) = match Self::pos_palabra(&upper_cabeza, "TIMES") {
            Some(i) => {
                let antes = cabeza[..i].trim();
                let (resto, n) = antes.rsplit_once(char::is_whitespace).unwrap_or(("", antes));
                let n: u32 = n.trim().parse().map_err(|_| {
                    CobolError::new(
                        line_no,
                        format!("PERFORM … {n} TIMES: '{n}' no es un numero de veces"),
                    )
                })?;
                (resto.trim(), Some(n))
            }
            None => (cabeza, None),
        };

        // Y por fin `<parrafo> [THRU <otro>]`.
        let upper_cabeza = cabeza.to_ascii_uppercase();
        let (desde, hasta) = match Self::pos_palabra(&upper_cabeza, "THRU")
            .map(|i| (i, 4))
            .or_else(|| Self::pos_palabra(&upper_cabeza, "THROUGH").map(|i| (i, 7)))
        {
            Some((i, largo)) => {
                let a = cabeza[i + largo..].trim().to_ascii_uppercase();
                if a.is_empty() {
                    return Err(CobolError::new(line_no, "PERFORM … THRU sin el parrafo final"));
                }
                (cabeza[..i].trim().to_ascii_uppercase(), Some(a))
            }
            None => (cabeza.trim().to_ascii_uppercase(), None),
        };

        if desde.is_empty() || desde.split_whitespace().count() != 1 {
            return Err(CobolError::new(
                line_no,
                format!(
                    "PERFORM {desde}: se esperaba UN nombre de parrafo. Las formas que se \
                     compilan son `PERFORM <p>`, `PERFORM <p> THRU <q>`, `PERFORM <p> <n> TIMES` \
                     y `PERFORM <p> UNTIL <cond>`"
                ),
            ));
        }

        Ok(CobolStatement::PerformFuera { desde, hasta, veces, hasta_que })
    }

    /// Dónde empieza una palabra COMPLETA dentro de un texto ya en mayúsculas.
    ///
    /// Palabra completa y no subcadena: un párrafo llamado `9000-TIMES-UP` no
    /// puede activar el recorte del `TIMES`.
    fn pos_palabra(upper: &str, palabra: &str) -> Option<usize> {
        let bytes = upper.as_bytes();
        let mut i = 0usize;
        while i + palabra.len() <= bytes.len() {
            if upper[i..].starts_with(palabra) {
                let izq_ok = i == 0 || bytes[i - 1].is_ascii_whitespace();
                let fin = i + palabra.len();
                let der_ok = fin == bytes.len() || bytes[fin].is_ascii_whitespace();
                if izq_ok && der_ok {
                    return Some(i);
                }
            }
            i += 1;
        }
        None
    }

    /// ¿Esta línea de la PROCEDURE DIVISION es un NOMBRE DE PÁRRAFO?
    ///
    /// Un párrafo es **una palabra sola terminada en punto**, y ahí está toda
    /// la regla: `1000-INICIO.` lo es y `MOVE 0 TO A.` no, porque tiene cuatro.
    /// El punto es obligatorio —sin él no hay forma de distinguir un nombre de
    /// párrafo de un verbo mal escrito— y por eso se mira la línea CRUDA, antes
    /// de que el analizador de arriba se lo quite.
    ///
    /// Y no puede ser una palabra reservada: `EXIT.` es el verbo, no un
    /// párrafo llamado EXIT. Sin esa comprobación, un `EXIT.` suelto abriría un
    /// párrafo fantasma y se tragaría el resto del programa.
    ///
    /// `<nombre> SECTION.` se acepta como si fuera un párrafo: para lo que hoy
    /// hace falta —ser el destino de un `PERFORM`— una sección es un párrafo
    /// con otro nombre, y fingir lo contrario sería rechazar programas que
    /// funcionarían igual.
    fn nombre_de_parrafo(linea: &str) -> Option<String> {
        let t = linea.trim();
        if !t.ends_with('.') {
            return None;
        }
        let cuerpo = t.trim_end_matches('.').trim();
        let palabras: Vec<&str> = cuerpo.split_whitespace().collect();
        let nombre = match palabras.as_slice() {
            [uno] => *uno,
            [uno, dos] if dos.eq_ignore_ascii_case("SECTION") => *uno,
            _ => return None,
        };
        if nombre.is_empty() {
            return None;
        }
        let arriba = nombre.to_ascii_uppercase();
        if crate::generated::words::is_reserved(&arriba) {
            return None;
        }
        // Un nombre de COBOL: letras, dígitos y guiones. Sin esto, un `.` de
        // más en cualquier sitio abriría un párrafo.
        if !nombre.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return None;
        }
        Some(arriba)
    }

    /// Los valores de un `88`, tal cual los escribió quien lo declaró.
    ///
    /// `rest` es la línea entera sin el nivel: `NOMBRE VALUE 1 THRU 5` o
    /// `NOMBRE VALUES 6, 7`. Se busca el `VALUE`/`VALUES` y se parte lo que
    /// venga detrás por comas; cada trozo puede ser un valor o un rango.
    ///
    /// La coma es separador Y puede ser parte de un número en algunas
    /// convenciones — aquí no: BMO COBOL usa el punto decimal, así que una coma
    /// separa siempre. Está dicho para que nadie lo descubra por su cuenta.
    fn parse_valores_88(
        rest: &str,
        name: &str,
        line_no: usize,
    ) -> Result<Vec<crate::ast::Valor88>, CobolError> {
        use crate::ast::Valor88;

        let arriba = rest.to_ascii_uppercase();
        let corte = ["VALUES", "VALUE"]
            .iter()
            .find_map(|p| arriba.find(p).map(|i| i + p.len()))
            .ok_or_else(|| {
                CobolError::new(line_no, format!("{name}: falta el VALUE del nombre de condicion"))
            })?;
        let cola = rest[corte..].trim().trim_end_matches('.').trim();
        // `VALUE IS 1` es legal y el `IS` sobra.
        let cola = Self::strip_leading_word(cola, "IS");

        let out = Self::partir_valores(cola, line_no)?;
        if out.is_empty() {
            return Err(CobolError::new(
                line_no,
                format!("{name}: el VALUE esta vacio y no compara nada"),
            ));
        }
        Ok(out)
    }

    /// Una lista de valores separados por comas, cada uno suelto o un rango.
    ///
    /// La comparten el `VALUE` de un nivel 88 y el `WHEN` de un `EVALUATE` con
    /// sujeto. Son **la misma lista**: `1, 3 THRU 5, 9` significa lo mismo en
    /// los dos sitios, y tenerla dos veces sería tener dos gramáticas para una
    /// coma.
    ///
    /// La coma separa SIEMPRE: BMO COBOL usa el punto decimal, así que nunca
    /// forma parte de un número. Está dicho para que nadie lo descubra solo.
    fn partir_valores(texto: &str, line_no: usize) -> Result<Vec<crate::ast::Valor88>, CobolError> {
        use crate::ast::Valor88;
        let mut out = Vec::new();
        for trozo in texto.split(',') {
            let t = trozo.trim();
            if t.is_empty() {
                continue;
            }
            let arriba = t.to_ascii_uppercase();
            // El rango: `1 THRU 5`, con los dos extremos incluidos.
            let separador = ["THROUGH", "THRU"]
                .iter()
                .find_map(|p| arriba.find(&format!(" {p} ")).map(|i| (i, p.len() + 2)));
            match separador {
                Some((i, largo)) => {
                    let desde = Self::normalizar_figurativa(t[..i].trim());
                    let hasta = Self::normalizar_figurativa(t[i + largo..].trim());
                    if desde.is_empty() || hasta.is_empty() {
                        return Err(CobolError::new(
                            line_no,
                            "un THRU necesita los dos extremos",
                        ));
                    }
                    out.push(Valor88::Rango(desde, hasta));
                }
                None => out.push(Valor88::Uno(Self::normalizar_figurativa(t))),
            }
        }
        Ok(out)
    }

    /// Las constantes figurativas que hoy tienen sentido en un campo numerico.
    ///
    /// `SPACE`, `HIGH-VALUE`, `LOW-VALUE` y `QUOTE` se dejan pasar TAL CUAL a
    /// proposito: son de texto, y el sitio donde se rechazan es la comprobacion
    /// de "esto no es un numero", que da un mensaje mejor —dice cuales SI
    /// valen— que uno generico de aqui.
    fn normalizar_figurativa(v: &str) -> String {
        match v.trim().to_ascii_uppercase().as_str() {
            "ZERO" | "ZEROS" | "ZEROES" => "0".to_string(),
            _ => v.to_string(),
        }
    }

    /// ¿Es un literal numerico de COBOL? Signo opcional, digitos, y como mucho
    /// un punto decimal.
    fn es_numero_cobol(v: &str) -> bool {
        let s = v.trim().trim_start_matches(['+', '-']);
        if s.is_empty() {
            return false;
        }
        let mut puntos = 0;
        for c in s.chars() {
            if c == '.' {
                puntos += 1;
                if puntos > 1 {
                    return false;
                }
            } else if !c.is_ascii_digit() {
                return false;
            }
        }
        // Un punto solo no es un numero, y `9.` tampoco lo es en COBOL.
        s.chars().any(|c| c.is_ascii_digit()) && !s.ends_with('.')
    }

    /// ¿Esta palabra es un `USAGE` escrito sin la palabra `USAGE` delante?
    ///
    /// Incluye las que NO se compilan, y a proposito: reconocerlas aqui es lo
    /// que permite decir "COMP-1 es coma flotante y la banca no lo usa" en vez
    /// de "no es COBOL reconocido", que manda a buscar donde no es.
    fn es_palabra_de_usage(w: &str) -> bool {
        matches!(
            w,
            "COMP-3" | "COMPUTATIONAL-3" | "PACKED-DECIMAL" | "DISPLAY"
                | "COMP" | "COMPUTATIONAL" | "COMP-4" | "COMPUTATIONAL-4" | "BINARY"
                | "COMP-5" | "COMPUTATIONAL-5"
                | "COMP-1" | "COMPUTATIONAL-1" | "COMP-2" | "COMPUTATIONAL-2"
                | "INDEX" | "POINTER"
        )
    }

    /// Traduce la palabra a la representacion, o dice por que no.
    ///
    /// Solo hay DOS que se compilan: `DISPLAY` (lo de siempre) y el
    /// empaquetado. Las demas se rechazan CON SU MOTIVO — aceptar `COMP` y
    /// guardar exactamente lo mismo que un `DISPLAY` seria compilar una palabra
    /// que promete un formato y no lo da, que es el fallo que este compilador
    /// no comete.
    fn parse_usage(w: &str, name: &str, line_no: usize) -> Result<crate::pic::Usage, CobolError> {
        let w = w.trim_end_matches('.').to_ascii_uppercase();
        match w.as_str() {
            "DISPLAY" => Ok(crate::pic::Usage::Display),
            "COMP-3" | "COMPUTATIONAL-3" | "PACKED-DECIMAL" => Ok(crate::pic::Usage::Comp3),
            "COMP-1" | "COMPUTATIONAL-1" | "COMP-2" | "COMPUTATIONAL-2" => Err(CobolError::new(
                line_no,
                format!(
                    "{name} {w}: eso es COMA FLOTANTE, y no se compila a proposito. \
                     Un importe en binario flotante no puede representar 19.99, y de ahi \
                     salen los descuadres de un centimo. Usa PIC con V y COMP-3"
                ),
            )),
            "COMP" | "COMPUTATIONAL" | "COMP-4" | "COMPUTATIONAL-4" | "BINARY" | "COMP-5"
            | "COMPUTATIONAL-5" => Err(CobolError::new(
                line_no,
                format!(
                    "{name} {w}: el binario todavia no se guarda distinto de un DISPLAY. \
                     Se dice en vez de aceptarlo y guardar otra cosa; hoy quita el {w} \
                     o usa COMP-3, que si empaqueta de verdad"
                ),
            )),
            otro => Err(CobolError::new(
                line_no,
                format!("{name} USAGE {otro}: todavia no se compila"),
            )),
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
        let mut usage = None;
        let mut i = 1;
        while i < parts.len() {
            let uw = parts[i].to_ascii_uppercase();
            let uw = uw.trim_end_matches('.');
            // `USAGE [IS] <cual>` y tambien el `<cual>` a secas, que es como lo
            // escribe todo el mundo: `05 IMPORTE PIC S9(7)V99 COMP-3.`
            if uw == "USAGE" {
                let mut j = i + 1;
                if parts.get(j).map(|s| s.trim_end_matches('.').eq_ignore_ascii_case("IS")) == Some(true) {
                    j += 1;
                }
                let Some(cual) = parts.get(j) else {
                    return Err(CobolError::new(line_no, format!("{name}: USAGE sin decir cual")));
                };
                usage = Some(Self::parse_usage(cual, &name, line_no)?);
                i = j;
            } else if Self::es_palabra_de_usage(uw) {
                usage = Some(Self::parse_usage(uw, &name, line_no)?);
            } else if uw == "PIC" || uw == "PICTURE" {
                if i + 1 < parts.len() {
                    i += 1;
                    pic = Some(parts[i].trim_end_matches('.').to_string());
                }
            } else if uw == "VALUE" {
                if i + 1 < parts.len() {
                    // ★ Un literal ENTRECOMILLADO se saca de la LÍNEA CRUDA, no
                    // de los trozos: el troceado es por espacios, así que
                    // `VALUE "  12 34"` ya venía sin los dos de delante y sin
                    // saber cuántos había en medio.
                    //
                    // No es un detalle: en un `PIC X` los espacios son datos.
                    // Un `INSPECT … REPLACING LEADING SPACE BY ZERO` sobre un
                    // campo al que le faltan espacios da otro número.
                    if let Some(t) = Self::literal_entrecomillado(rest) {
                        value = Some(t);
                        // Saltar los trozos que formaban el literal.
                        while i + 1 < parts.len()
                            && !parts[i].ends_with('"')
                            && !parts[i].ends_with("\".")
                            && !parts[i].ends_with('\'')
                        {
                            i += 1;
                        }
                    } else {
                        i += 1;
                        value = Some(
                            parts[i]
                                .trim_end_matches('.')
                                .trim_matches('"')
                                .trim_matches('\'')
                                .to_string(),
                        );
                    }
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

        let usage = usage.unwrap_or(crate::pic::Usage::Display);
        // Las CONSTANTES FIGURATIVAS son parte del idioma, no azucar: `VALUE
        // ZERO` es lo que escribe todo el mundo y `VALUE 0` casi nadie. Se
        // traducen aqui, una sola vez, en vez de que cada consumidor del AST
        // tenga que acordarse de las tres formas de escribir el cero.
        let value = value.map(|v| Self::normalizar_figurativa(&v));
        let mut item = DataItem::new_with_usage(level, name, pic, value, usage);

        // ── Donde un COMP-3 no tiene sentido ──
        //
        // Empaquetar son DIGITOS: dos por byte y un nibble de signo. Sin PIC no
        // se sabe cuantos, y sobre una PIC X no hay digitos que empaquetar. Las
        // dos cosas se dicen en vez de reservar un tamano inventado.
        if item.usage == crate::pic::Usage::Comp3 {
            match &item.pic_field {
                None => {
                    return Err(CobolError::new(
                        line_no,
                        format!(
                            "{}: COMP-3 sin PIC. Empaquetar es meter DIGITOS de dos en dos, \
                             y sin PICTURE no se sabe cuantos hay",
                            item.name
                        ),
                    ))
                }
                Some(campo) if !campo.numeric => {
                    return Err(CobolError::new(
                        line_no,
                        format!(
                            "{} es COMP-3 con PIC {}: solo se empaqueta lo numerico (9/S/V). \
                             Un campo de texto se guarda tal cual",
                            item.name,
                            item.pic.as_deref().unwrap_or("?")
                        ),
                    ))
                }
                Some(_) => {}
            }
            if item.edicion.is_some() {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "{}: una PIC de EDICION ({}) es para ENSENAR, y COMP-3 es como se GUARDA. \
                         Guarda en un COMP-3 y muevelo a un campo editado para el informe",
                        item.name,
                        item.pic.as_deref().unwrap_or("?")
                    ),
                ));
            }
        }

        // ── Nivel 88: un NOMBRE DE CONDICIÓN, que no es un dato ──
        //
        // `88 FIN-DE-FICHERO VALUE 1.` no reserva memoria: le pone nombre a la
        // comparación `<el dato de arriba> = 1`. Es lo que convierte
        // `PERFORM UNTIL FIN = 1` en `PERFORM UNTIL FIN-DE-FICHERO`, y es COBOL
        // bancario idiomático puro: la condición se lee, no se descifra.
        if item.level == 88 {
            if item.pic.is_some() {
                return Err(CobolError::new(
                    line_no,
                    format!("{} es un nivel 88: es un nombre de condicion, no un dato, y no lleva PIC", item.name),
                ));
            }
            if item.value.is_none() {
                return Err(CobolError::new(
                    line_no,
                    format!("{} necesita su VALUE: un nombre de condicion sin valor no compara nada", item.name),
                ));
            }
            // ★ `VALUE 1 THRU 5` y `VALUE 6, 7` — las dos formas que escribe
            // todo el mundo, y que estaban rechazadas porque expandirlas pide un
            // OR. Ya hay OR.
            item.valores = Self::parse_valores_88(rest, &item.name, line_no)?;
            return Ok(Some(item));
        }

        // ── `VALUE` en un dato: el valor con el que ARRANCA ──
        //
        // Se parseaba desde siempre y **no se emitia nunca**: `codegen.rs` solo
        // miraba `item.value` para los 88, asi que `01 SALDO PIC 9(5)V99 VALUE
        // 100.00.` compilaba y SALDO arrancaba con lo que hubiera en la pila.
        // Ningun ejemplo lo destapaba porque todos inicializan con `MOVE`.
        //
        // Aqui se comprueba lo que el codegen no puede decir con numero de
        // linea; el que emite es `emit_valores_iniciales`.
        if let Some(v) = item.value.clone() {
            let Some(campo) = item.pic_field.clone() else {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "{}: VALUE sin PIC. Un valor inicial necesita saber en que cabe: \
                         cuantos digitos y donde cae la coma",
                        item.name
                    ),
                ));
            };
            // ★ Un campo alfanumérico guarda su VALUE como CARACTERES, y por eso
            // no pasa por la comprobación de "esto es un número". `VALUE "00"`
            // en un `PIC XX` es exactamente lo que escribe un `FILE STATUS`.
            if !campo.numeric {
                return Ok(Some(item));
            }
            if !Self::es_numero_cobol(&v) {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "{} VALUE {v}: eso no es un numero. En un campo numerico valen \
                         un literal (`100.00`, `-5`) y las figurativas ZERO / ZEROS / ZEROES",
                        item.name
                    ),
                ));
            }
        }

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
            let (target, arit) = self.leer_aritmetica(&rest[to_pos + 4..], "ADD", line_no)?;
            Ok(CobolStatement::Add(val, target, arit))
        } else if upper.starts_with("SUBTRACT ") {
            let rest = line[9..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(from_pos) = up.find(" FROM ") else {
                return Err(CobolError::new(line_no, "SUBTRACT requires `FROM`"));
            };
            let val = Self::parse_operand(&rest[..from_pos]);
            let (target, arit) = self.leer_aritmetica(&rest[from_pos + 6..], "SUBTRACT", line_no)?;
            Ok(CobolStatement::Subtract(val, target, arit))
        } else if upper.starts_with("MULTIPLY ") {
            let rest = line[9..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(by_pos) = up.find(" BY ") else {
                return Err(CobolError::new(line_no, "MULTIPLY requires `BY`"));
            };
            let val = Self::parse_operand(&rest[..by_pos]);
            let (target, arit) = self.leer_aritmetica(&rest[by_pos + 4..], "MULTIPLY", line_no)?;
            Ok(CobolStatement::Multiply(val, target, arit))
        } else if upper.starts_with("DIVIDE ") {
            let rest = line[7..].trim();
            let up = rest.to_ascii_uppercase();
            let Some(by_pos) = up.find(" BY ") else {
                return Err(CobolError::new(line_no, "DIVIDE requires `BY`"));
            };
            let val = Self::parse_operand(&rest[..by_pos]);
            let (target, arit) = self.leer_aritmetica(&rest[by_pos + 4..], "DIVIDE", line_no)?;
            Ok(CobolStatement::Divide(val, target, arit))
        } else if upper.starts_with("COMPUTE ") {
            let rest = line[8..].trim();
            let eq_pos = rest.find('=').unwrap_or(0);
            if eq_pos == 0 { return Err(CobolError::new(line_no, "COMPUTE requires `=`")); }
            let (target, arit) = Self::partir_rounded(&rest[..eq_pos], line_no)?;
            let (expr, arit) = {
                let cola = rest[eq_pos + 1..].trim().to_string();
                let (e, mut a2) = self.leer_aritmetica(&cola, "COMPUTE", line_no)?;
                a2.redondeo = arit.redondeo;
                (e, a2)
            };
            Ok(CobolStatement::Compute(target, expr, arit))
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
        } else if upper.starts_with("INSPECT ") {
            Self::parse_inspect(line, line_no)
        } else if upper.starts_with("STRING ") {
            self.parse_string(line, line_no)
        } else if upper.starts_with("EVALUATE ") {
            self.parse_evaluate(line, line_no)
        } else if upper.starts_with("IF ") {
            self.parse_if(line, line_no)
        } else if upper.starts_with("PERFORM ") {
            self.parse_perform(line, line_no)
        } else if upper == "STOP RUN" || upper == "STOP RUN." {
            Ok(CobolStatement::StopRun)
        } else if upper == "EXIT" || upper == "EXIT." {
            // `EXIT` no hace nada, y ese es su trabajo: es el destino de un
            // `PERFORM … THRU X-SALIR`. Emitir nada es lo correcto.
            Ok(CobolStatement::Exit)
        } else if upper == "CONTINUE" || upper == "CONTINUE." {
            // Igual que EXIT pero en medio de una sentencia: el hueco explícito
            // de una rama del `IF` que no hace nada.
            Ok(CobolStatement::Exit)
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

        // ★ `FILE STATUS IS <campo>` — el campo donde el sistema deja el
        // resultado de cada operación. Se recorta ANTES de leer la ruta, porque
        // viene detrás de ella y si no acabaría dentro del nombre del fichero.
        let arriba_cola = cola.to_ascii_uppercase();
        let (cola, estado) = match Self::pos_palabra(&arriba_cola, "FILE") {
            Some(i) if arriba_cola[i..].starts_with("FILE STATUS") => {
                let tras = cola[i + 11..].trim();
                let tras = Self::strip_leading_word(tras, "IS");
                let campo = tras.trim().trim_end_matches('.').trim().to_ascii_uppercase();
                if campo.is_empty() || campo.split_whitespace().count() != 1 {
                    return Err(CobolError::new(
                        line_no,
                        format!("SELECT {name}: `FILE STATUS IS` necesita UN campo"),
                    ));
                }
                (cola[..i].trim(), Some(campo))
            }
            _ => (cola, None),
        };

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
        Ok(crate::ast::CobolFile { name, path, record: String::new(), estado })
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
        let conditions = Self::parse_condicion(head, line_no)?;

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

    /// Separa el destino de una aritmética de su cláusula `ROUNDED`.
    ///
    /// ```text
    ///   SALDO                                → truncar (el default de COBOL)
    ///   SALDO ROUNDED                        → NEAREST-AWAY-FROM-ZERO
    ///   SALDO ROUNDED MODE IS NEAREST-EVEN   → el redondeo del banquero
    /// ```
    ///
    /// ★ **El default sin `ROUNDED` es TRUNCAR, y eso no es un descuido del
    /// estándar**: en un desglose de asiento hay que truncar para que la suma
    /// de las partes cuadre con el total, y en un interés hay que redondear.
    /// Por eso la cláusula va en la OPERACIÓN y no en el dato: el mismo campo
    /// hace las dos cosas en dos líneas seguidas.
    ///
    /// Y `ROUNDED` a secas es `NEAREST-AWAY-FROM-ZERO` porque es lo que dice el
    /// estándar y lo que espera cualquiera que lo escriba sin pensar. El del
    /// banquero **hay que pedirlo**: cambia el resultado y no puede colarse.
    fn partir_rounded(
        texto: &str,
        line_no: usize,
    ) -> Result<(String, crate::ast::Aritmetica), CobolError> {
        let (t, redondeo) = Self::partir_modo(texto, line_no)?;
        Ok((t, crate::ast::Aritmetica::con(redondeo)))
    }

    /// Igual, pero leyendo además las cláusulas `ON SIZE ERROR`.
    ///
    /// ```text
    ///   ADD A TO B ON SIZE ERROR
    ///       DISPLAY "no cabe"
    ///   NOT ON SIZE ERROR
    ///       ADD 1 TO CUANTOS
    ///   END-ADD
    /// ```
    ///
    /// ★ La cláusula tiene que **empezar en la línea del verbo**. Sin eso, un
    /// `ADD A TO B` a secas y un `ADD A TO B` que sigue abajo se leen igual, y
    /// habría que adivinar mirando adelante. Adivinar aquí significa tragarse
    /// las sentencias de después como si fueran del `ADD`.
    fn leer_aritmetica(
        &mut self,
        cola: &str,
        verbo: &str,
        line_no: usize,
    ) -> Result<(String, crate::ast::Aritmetica), CobolError> {
        let arriba = cola.to_ascii_uppercase();
        let corte = Self::pos_palabra(&arriba, "ON")
            .filter(|i| arriba[*i..].starts_with("ON SIZE ERROR"))
            .or_else(|| {
                Self::pos_palabra(&arriba, "NOT")
                    .filter(|i| arriba[*i..].starts_with("NOT ON SIZE ERROR"))
            });
        let Some(i) = corte else {
            return Self::partir_rounded(cola, line_no);
        };

        let (destino, mut arit) = Self::partir_rounded(&cola[..i], line_no)?;
        let fin = format!("END-{verbo}");

        // La primera rama empieza en esta misma línea, detrás de la cláusula.
        let mut resto = cola[i..].trim().to_string();
        let mut en_desborda = resto.to_ascii_uppercase().starts_with("ON SIZE ERROR");
        resto = if en_desborda {
            resto[13..].trim().to_string()
        } else {
            resto[17..].trim().to_string()
        };

        let mut desborda: Vec<CobolStatement> = Vec::new();
        let mut cabe: Vec<CobolStatement> = Vec::new();
        let mut pendiente = resto;

        loop {
            let linea = if !pendiente.is_empty() {
                std::mem::take(&mut pendiente)
            } else {
                let Some((_, raw)) = self.current().cloned() else {
                    return Err(CobolError::new(
                        line_no,
                        format!("{verbo} … ON SIZE ERROR sin {fin}"),
                    ));
                };
                self.advance();
                Self::strip_comment(&raw).trim().to_string()
            };
            if linea.is_empty() {
                continue;
            }
            let limpio = linea.trim_end_matches('.').trim().to_string();
            let arriba = limpio.to_ascii_uppercase();
            if arriba == fin {
                break;
            }
            if arriba.starts_with("NOT ON SIZE ERROR") {
                en_desborda = false;
                pendiente = limpio[17..].trim().to_string();
                continue;
            }
            if arriba.starts_with("ON SIZE ERROR") {
                en_desborda = true;
                pendiente = limpio[13..].trim().to_string();
                continue;
            }
            let stmt = self.parse_statement(&limpio, line_no)?;
            if en_desborda {
                desborda.push(stmt);
            } else {
                cabe.push(stmt);
            }
        }

        arit.si_desborda = Some(desborda);
        if !cabe.is_empty() {
            arit.si_cabe = Some(cabe);
        }
        Ok((destino, arit))
    }

    fn partir_modo(texto: &str, line_no: usize) -> Result<(String, Redondeo), CobolError> {
        let t = texto.trim().trim_end_matches('.').trim();
        let arriba = t.to_ascii_uppercase();
        let Some(i) = Self::pos_palabra(&arriba, "ROUNDED") else {
            return Ok((t.to_string(), Redondeo::Truncar));
        };
        let destino = t[..i].trim().to_string();
        if destino.is_empty() {
            return Err(CobolError::new(line_no, "ROUNDED sin campo al que aplicarse"));
        }
        let cola = t[i + 7..].trim();
        let cola = Self::strip_leading_word(cola, "MODE");
        let cola = Self::strip_leading_word(cola.trim(), "IS");
        let cola = cola.trim();
        if cola.is_empty() {
            return Ok((destino, Redondeo::MasCercanoLejosDeCero));
        }
        let modo = match cola.to_ascii_uppercase().as_str() {
            "NEAREST-AWAY-FROM-ZERO" => Redondeo::MasCercanoLejosDeCero,
            "NEAREST-EVEN" => Redondeo::MasCercanoPar,
            "NEAREST-TOWARD-ZERO" => Redondeo::MasCercanoHaciaCero,
            "TOWARD-GREATER" => Redondeo::HaciaArriba,
            "TOWARD-LESSER" => Redondeo::HaciaAbajo,
            "TRUNCATION" => Redondeo::Truncar,
            // `PROHIBITED` obliga a que la operación sea exacta y a fallar si
            // no lo es. Eso no es un modo de redondeo: es un `ON SIZE ERROR`
            // sobre la parte decimal, y necesita a dónde saltar cuando pasa.
            "PROHIBITED" => {
                return Err(CobolError::new(
                    line_no,
                    "ROUNDED MODE IS PROHIBITED no se compila: exige fallar cuando la \
                     operacion no es exacta, y eso necesita el ON SIZE ERROR que \
                     todavia no existe",
                ))
            }
            otro => {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "ROUNDED MODE IS {otro}: no es un modo del estandar. Son \
                         NEAREST-AWAY-FROM-ZERO (el de `ROUNDED` a secas), NEAREST-EVEN \
                         (el del banquero), NEAREST-TOWARD-ZERO, TOWARD-GREATER, \
                         TOWARD-LESSER y TRUNCATION"
                    ),
                ))
            }
        };
        Ok((destino, modo))
    }

    /// El literal entre comillas de una línea, **con sus espacios intactos**.
    ///
    /// Devuelve `None` si no hay comillas — entonces el `VALUE` es un número o
    /// una figurativa y el troceado por espacios vale.
    fn literal_entrecomillado(linea: &str) -> Option<String> {
        let bytes = linea.as_bytes();
        let abre = bytes.iter().position(|b| *b == b'"' || *b == b'\'')?;
        let q = bytes[abre];
        let cierra = bytes[abre + 1..].iter().position(|b| *b == q)? + abre + 1;
        Some(linea[abre + 1..cierra].to_string())
    }

    /// Un literal de UN carácter entre comillas, o la figurativa `SPACE`.
    ///
    /// Sólo un carácter: buscar o sustituir una CADENA es búsqueda de
    /// subcadena, que es otra cosa y bastante más grande. Se dice en vez de
    /// mirar sólo la primera letra y callar el resto — eso contaría de más y
    /// sustituiría donde no toca.
    fn un_caracter(texto: &str, que: &str, line_no: usize) -> Result<char, CobolError> {
        let t = texto.trim();
        let arriba = t.to_ascii_uppercase();
        if arriba == "SPACE" || arriba == "SPACES" {
            return Ok(' ');
        }
        if arriba == "ZERO" || arriba == "ZEROS" || arriba == "ZEROES" {
            return Ok('0');
        }
        let sin = t.trim_matches('"').trim_matches('\'');
        let mut cs = sin.chars();
        match (cs.next(), cs.next()) {
            (Some(c), None) => Ok(c),
            _ => Err(CobolError::new(
                line_no,
                format!(
                    "{que} '{t}': aqui solo vale UN caracter (o SPACE / ZERO). Buscar una \
                     CADENA es busqueda de subcadena, y eso todavia no se compila — \
                     aceptarlo mirando solo la primera letra contaria de mas"
                ),
            )),
        }
    }

    /// `INSPECT <campo> TALLYING <n> FOR ALL "<c>"`
    /// `INSPECT <campo> REPLACING {ALL|LEADING} "<a>" BY "<b>"`
    fn parse_inspect(line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let resto = line[8..].trim().trim_end_matches('.').trim();
        let arriba = resto.to_ascii_uppercase();

        if let Some(i) = Self::pos_palabra(&arriba, "TALLYING") {
            let campo = resto[..i].trim().to_ascii_uppercase();
            let cola = resto[i + 8..].trim();
            let arriba_cola = cola.to_ascii_uppercase();
            let Some(j) = Self::pos_palabra(&arriba_cola, "FOR") else {
                return Err(CobolError::new(line_no, "INSPECT … TALLYING sin FOR"));
            };
            let contador = cola[..j].trim().to_ascii_uppercase();
            let tras = cola[j + 3..].trim();
            let tras = Self::strip_leading_word(tras, "ALL");
            let buscado = Self::un_caracter(tras, "INSPECT … FOR ALL", line_no)?;
            if campo.is_empty() || contador.is_empty() {
                return Err(CobolError::new(line_no, "INSPECT … TALLYING: falta el campo o el contador"));
            }
            return Ok(CobolStatement::InspectContar { campo, contador, buscado });
        }

        if let Some(i) = Self::pos_palabra(&arriba, "REPLACING") {
            let campo = resto[..i].trim().to_ascii_uppercase();
            let cola = resto[i + 9..].trim();
            let arriba_cola = cola.to_ascii_uppercase();
            let (cola, solo_delante) = match Self::pos_palabra(&arriba_cola, "LEADING") {
                Some(0) => (cola[7..].trim(), true),
                _ => (Self::strip_leading_word(cola, "ALL").trim(), false),
            };
            let arriba_cola = cola.to_ascii_uppercase();
            let Some(j) = Self::pos_palabra(&arriba_cola, "BY") else {
                return Err(CobolError::new(line_no, "INSPECT … REPLACING sin BY"));
            };
            let viejo = Self::un_caracter(&cola[..j], "INSPECT … REPLACING", line_no)?;
            let nuevo = Self::un_caracter(&cola[j + 2..], "INSPECT … BY", line_no)?;
            if campo.is_empty() {
                return Err(CobolError::new(line_no, "INSPECT sin campo"));
            }
            return Ok(CobolStatement::InspectReemplazar { campo, viejo, nuevo, solo_delante });
        }

        Err(CobolError::new(
            line_no,
            "INSPECT: hoy se compilan `TALLYING <n> FOR ALL \"<c>\"` y \
             `REPLACING {ALL|LEADING} \"<a>\" BY \"<b>\"`. Las formas con \
             CHARACTERS, BEFORE y AFTER todavia no",
        ))
    }

    /// `STRING <fuentes> DELIMITED BY SIZE INTO <destino>`.
    ///
    /// Sólo `DELIMITED BY SIZE`: cada fuente entra con TODO su ancho. Las otras
    /// formas —`DELIMITED BY SPACE` o por un carácter— cortan por un largo que
    /// sólo se sabe en ejecución, y eso es otro emisor. Se dice.
    ///
    /// ★ Se lee en VARIAS LÍNEAS, porque así se escribe siempre: una fuente por
    /// renglón. Se sigue leyendo hasta el `INTO`, y ahí acaba — con `END-STRING`
    /// detrás o sin él, que las dos formas son legales.
    fn parse_string(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let mut junto = line[7..].trim().trim_end_matches('.').trim().to_string();
        while Self::pos_palabra(&junto.to_ascii_uppercase(), "INTO").is_none() {
            let Some((_, raw)) = self.current().cloned() else {
                return Err(CobolError::new(line_no, "STRING sin INTO: falta decir donde va"));
            };
            self.advance();
            let siguiente = Self::strip_comment(&raw).trim().to_string();
            if siguiente.is_empty() {
                continue;
            }
            junto.push(' ');
            junto.push_str(siguiente.trim_end_matches('.').trim());
        }
        // Un `END-STRING` detrás del destino es legal y aquí sobra.
        let junto = junto.trim().to_string();
        let junto = match Self::pos_palabra(&junto.to_ascii_uppercase(), "END-STRING") {
            Some(i) => junto[..i].trim().to_string(),
            None => junto,
        };
        let resto = junto.as_str();
        let arriba = resto.to_ascii_uppercase();
        let Some(i) = Self::pos_palabra(&arriba, "INTO") else {
            return Err(CobolError::new(line_no, "STRING sin INTO: falta decir donde va"));
        };
        let destino = resto[i + 4..].trim().to_ascii_uppercase();
        if destino.is_empty() || destino.split_whitespace().count() != 1 {
            return Err(CobolError::new(line_no, "STRING … INTO necesita UN campo"));
        }

        // Las fuentes van separadas por sus `DELIMITED BY SIZE`.
        let cabeza = &resto[..i];
        let mut fuentes = Vec::new();
        // La forma es `<f1> DELIMITED BY SIZE <f2> DELIMITED BY SIZE …`, así que
        // partir por `DELIMITED` deja el primer trozo limpio y los demás con su
        // `BY SIZE` delante.
        //
        // ★ Se quitan por TOKENS y no recortando texto: `strip_leading_word` no
        // corta una palabra que esté al final de la cadena, y un `SIZE` que
        // sobrevivía a eso se colaba como si fuera un campo. Sus dos primeras
        // letras acababan escritas dentro del destino — un fallo que compila,
        // no avisa, y mete basura en un registro.
        for trozo in Self::split_on_word(cabeza, "DELIMITED") {
            let mut tokens: Vec<&str> = trozo.split_whitespace().collect();
            while matches!(
                tokens.first().map(|t| t.to_ascii_uppercase()).as_deref(),
                Some("BY")
            ) {
                tokens.remove(0);
            }
            if let Some(primero) = tokens.first().map(|t| t.to_ascii_uppercase()) {
                match primero.as_str() {
                    "SIZE" => {
                        tokens.remove(0);
                    }
                    "SPACE" | "SPACES" => {
                        return Err(CobolError::new(
                            line_no,
                            "STRING: hoy solo `DELIMITED BY SIZE`. Cortar por un espacio o \
                             por un caracter necesita un largo que solo se sabe en ejecucion",
                        ))
                    }
                    _ => {}
                }
            }
            if tokens.is_empty() {
                continue;
            }
            if tokens.len() > 1 {
                return Err(CobolError::new(
                    line_no,
                    format!(
                        "STRING: '{}' no es UNA fuente. Cada una lleva su \
                         `DELIMITED BY SIZE` detras",
                        tokens.join(" ")
                    ),
                ));
            }
            fuentes.push(tokens[0].to_string());
        }
        if fuentes.is_empty() {
            return Err(CobolError::new(line_no, "STRING sin nada que pegar"));
        }
        Ok(CobolStatement::StringInto { fuentes, destino })
    }

    /// ★ `EVALUATE … WHEN … END-EVALUATE` — el `switch` de COBOL.
    ///
    /// Dos formas, las dos corrientes en banca y las dos aquí:
    ///
    /// ```text
    ///   EVALUATE TIPO-MOV              EVALUATE TRUE
    ///       WHEN 1                         WHEN SALDO > 1000.00
    ///           ...                            ...
    ///       WHEN 2 THRU 5                  WHEN SALDO > 100.00
    ///           ...                            ...
    ///       WHEN 6, 7                      WHEN OTHER
    ///           ...                            ...
    ///       WHEN OTHER                 END-EVALUATE
    ///           ...
    ///   END-EVALUATE
    /// ```
    ///
    /// La de la derecha es **la tabla de decisión**, y es como un banco escribe
    /// un escalado de comisiones: cada `WHEN` es una condición entera. La de la
    /// izquierda es el `switch` clásico, y sus `WHEN` se traducen **aquí** a
    /// comparaciones contra el sujeto, con la misma expansión que usa un nivel
    /// 88 — `THRU` es un rango cerrado y la coma es un `OR`.
    ///
    /// Las sentencias van en las líneas de DEBAJO de su `WHEN`. La forma de
    /// meterlas en la misma línea existe en el estándar y se rechaza con su
    /// motivo: separar "el valor" de "la sentencia" sin tokens es adivinar, y
    /// adivinar mal manda el programa a otra rama.
    fn parse_evaluate(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let sujeto = line[9..].trim().trim_end_matches('.').trim().to_string();
        if sujeto.is_empty() {
            return Err(CobolError::new(line_no, "EVALUATE sin sujeto"));
        }
        // `EVALUATE TRUE` no compara con nada: cada WHEN trae su condición.
        let por_condicion = sujeto.eq_ignore_ascii_case("TRUE");
        if sujeto.eq_ignore_ascii_case("FALSE") {
            return Err(CobolError::new(
                line_no,
                "EVALUATE FALSE no se compila: invierte las condiciones y usa EVALUATE TRUE",
            ));
        }
        if !por_condicion && sujeto.split_whitespace().count() != 1 {
            return Err(CobolError::new(
                line_no,
                format!(
                    "EVALUATE {sujeto}: se esperaba UN campo o `TRUE`. Varios sujetos \
                     (`EVALUATE A ALSO B`) todavia no se compilan"
                ),
            ));
        }

        let mut ramas: Vec<(Option<Condicion>, Vec<CobolStatement>)> = Vec::new();
        let mut hubo_other = false;
        loop {
            let (inner_no, raw) = match self.current() {
                Some(v) => (v.0, v.1.clone()),
                None => {
                    return Err(CobolError::new(
                        line_no,
                        "EVALUATE sin END-EVALUATE: esta implementacion exige el cierre explicito",
                    ))
                }
            };
            let inner = Self::strip_comment(&raw).trim().to_string();
            self.advance();
            if inner.is_empty() {
                continue;
            }
            let limpio = inner.trim_end_matches('.').trim().to_string();
            let arriba = limpio.to_ascii_uppercase();

            if arriba == "END-EVALUATE" {
                break;
            }

            if arriba == "WHEN OTHER" {
                if hubo_other {
                    return Err(CobolError::new(inner_no, "dos WHEN OTHER en el mismo EVALUATE"));
                }
                hubo_other = true;
                ramas.push((None, Vec::new()));
                continue;
            }

            if let Some(resto) = arriba.strip_prefix("WHEN ") {
                if hubo_other {
                    return Err(CobolError::new(
                        inner_no,
                        "un WHEN despues del WHEN OTHER no se alcanza nunca: \
                         el OTHER va el ultimo",
                    ));
                }
                let texto = limpio[5..].trim();
                if resto.trim().is_empty() {
                    return Err(CobolError::new(inner_no, "WHEN sin valor"));
                }
                let cond = if por_condicion {
                    Self::parse_condicion(texto, inner_no)?
                } else {
                    // La MISMA expansión que un nivel 88, y por eso `THRU` y la
                    // coma funcionan aquí sin escribir nada nuevo.
                    let valores = Self::partir_valores(texto, inner_no)?;
                    Condicion::de_valores(&sujeto, &valores).ok_or_else(|| {
                        CobolError::new(inner_no, "WHEN sin ningun valor con el que comparar")
                    })?
                };
                ramas.push((Some(cond), Vec::new()));
                continue;
            }

            // Una sentencia: es del último WHEN abierto.
            let Some((_, cuerpo)) = ramas.last_mut() else {
                return Err(CobolError::new(
                    inner_no,
                    format!(
                        "'{limpio}' esta entre el EVALUATE y el primer WHEN, y ahi no se \
                         ejecuta nada. Ponlo antes del EVALUATE o debajo de un WHEN"
                    ),
                ));
            };
            let _ = cuerpo;
            let stmt = self.parse_statement(&limpio, inner_no)?;
            ramas.last_mut().unwrap().1.push(stmt);
        }

        if ramas.is_empty() {
            return Err(CobolError::new(line_no, "EVALUATE sin ningun WHEN"));
        }
        Ok(CobolStatement::Evaluate(ramas))
    }

    /// Todas las formas del `PERFORM`.
    ///
    /// ```text
    ///   EN LINEA (el cuerpo va entre PERFORM y END-PERFORM)
    ///     PERFORM <n> TIMES … END-PERFORM
    ///     PERFORM UNTIL <cond> … END-PERFORM
    ///
    ///   FUERA DE LINEA (el cuerpo es un parrafo con nombre)
    ///     PERFORM <parrafo>
    ///     PERFORM <parrafo> THRU <parrafo>
    ///     PERFORM <parrafo> <n> TIMES
    ///     PERFORM <parrafo> UNTIL <cond>
    ///     PERFORM <parrafo> THRU <parrafo> UNTIL <cond>
    /// ```
    ///
    /// Se distinguen por lo PRIMERO que viene detrás de `PERFORM`: si es
    /// `UNTIL` o un número, el cuerpo viene abajo; si es un nombre, el cuerpo
    /// es ese párrafo. No hace falta mirar más lejos, y eso es lo que hace que
    /// la regla se pueda explicar en una frase.
    fn parse_perform(&mut self, line: &str, line_no: usize) -> Result<CobolStatement, CobolError> {
        let rest = line[8..].trim().trim_end_matches('.').trim();
        let upper = rest.to_ascii_uppercase();

        // ── El PERFORM fuera de línea ──
        let primera = upper.split_whitespace().next().unwrap_or("");
        let es_fuera_de_linea = !primera.is_empty()
            && primera != "UNTIL"
            && primera.parse::<u32>().is_err()
            && !crate::generated::words::is_reserved(primera);
        if es_fuera_de_linea {
            return Self::parse_perform_fuera(rest, line_no);
        }

        let header = if let Some(pos) = upper.find("UNTIL ") {
            if pos == 0 {
                PerformHeader::Until(Self::parse_condicion(rest[6..].trim(), line_no)?)
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
    /// Se combinan con `AND` y con `OR`, y **`AND` liga más fuerte**: por eso
    /// hay dos niveles de análisis y no una lista plana. `A OR B AND C` es
    /// `A OR (B AND C)`, como manda el estándar — leerlo al revés manda el
    /// programa a la otra rama sin que nada avise.
    ///
    /// La normalización de las formas en palabras corre PRIMERO, y eso no es
    /// casualidad: `IS GREATER THAN OR EQUAL TO` lleva un `OR` dentro que no es
    /// un `OR` lógico. Partir antes de normalizar lo cortaría por la mitad.
    fn parse_condicion(text: &str, line_no: usize) -> Result<Condicion, CobolError> {
        let normalized = Self::normalize_condition_words(text);
        Self::parse_condicion_o(&normalized, line_no)
    }

    /// El nivel de menos fuerza: `OR`.
    fn parse_condicion_o(text: &str, line_no: usize) -> Result<Condicion, CobolError> {
        let partes = Self::split_on_word(text, "OR");
        let mut acc: Option<Condicion> = None;
        for parte in partes {
            let c = Self::parse_condicion_y(parte, line_no)?;
            acc = Some(match acc {
                None => c,
                Some(izq) => Condicion::o(izq, c),
            });
        }
        acc.ok_or_else(|| CobolError::new(line_no, "condicion vacia"))
    }

    /// El nivel de más fuerza: `AND`.
    fn parse_condicion_y(text: &str, line_no: usize) -> Result<Condicion, CobolError> {
        let partes = Self::split_on_word(text, "AND");
        let mut acc: Option<Condicion> = None;
        for parte in partes {
            let c = Condicion::Simple(Self::parse_one_condition(parte.trim(), line_no)?);
            acc = Some(match acc {
                None => c,
                Some(izq) => Condicion::y(izq, c),
            });
        }
        acc.ok_or_else(|| CobolError::new(line_no, "condicion vacia"))
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
        // Sin operador: puede ser un NOMBRE DE CONDICIÓN (`IF FIN-DE-FICHERO`).
        // Aquí no se puede saber —el parser no conoce los datos—, así que se
        // pasa por nombre y lo resuelve el codegen, que sí sabe de quién cuelga
        // y puede decir "eso no es ningún 88" cuando no existe.
        let limpio = text.trim();
        if !limpio.is_empty() && !limpio.contains(char::is_whitespace) {
            return Ok(CobolCondition::Nombre(limpio.to_ascii_uppercase()));
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

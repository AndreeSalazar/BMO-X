//! C Preprocessor — handles #define, #include, #ifdef, #if, #endif.
//!
//! Runs before the tokenizer. Expands macros, resolves includes,
//! evaluates conditional compilation, and outputs a clean source string.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use crate::CError;
use crate::StandardFeatures;

#[derive(Debug, Clone)]
struct MacroDef {
    params: Vec<String>,
    /// ¿Se definió con paréntesis **pegados** al nombre?
    ///
    /// No se puede deducir de `params.is_empty()`, y confundirlo es un bug con
    /// dos caras:
    ///
    /// ```text
    ///   #define F() 1      función SIN parámetros  → params vacío, función
    ///   #define F (1)      OBJETO cuyo cuerpo empieza por paréntesis
    /// ```
    ///
    /// El lector viejo hacía `rest.find('(')` sin mirar si había un espacio
    /// delante, así que `#define CAJA_ANCHO (760)` se registraba como una
    /// macro-función con un parámetro llamado `760` y cuerpo **vacío**. La
    /// constante desaparecía en silencio. En C el espacio es significativo
    /// aquí, y es el único sitio del lenguaje donde lo es.
    funcion: bool,
    /// Termina en `...`: el resto de argumentos entra por `__VA_ARGS__`.
    variadica: bool,
    body: String,
}

/// Cuántas veces se re-recorre una línea buscando más macros que expandir.
///
/// Hace falta un tope porque una macro puede producir otra: `#define A B` y
/// `#define B 1` necesitan dos pasadas. Y hace falta que sea un TOPE porque
/// `#define A A` es legal y no termina nunca — en C de verdad se evita con la
/// regla de "una macro no se re-expande dentro de sí misma", que pide llevar un
/// conjunto de macros en curso; aquí se corta por profundidad, que es más pobre
/// pero no cuelga el compilador.
const MAX_PASADAS: usize = 16;

pub struct Preprocessor {
    defines: HashMap<String, MacroDef>,
    include_paths: Vec<PathBuf>,
    features: StandardFeatures,
    line: usize,
    /// Lo que salió mal expandiendo. No puede devolverse en el acto porque la
    /// expansión ocurre a mitad de recorrer el fichero; se acumula y
    /// `preprocess` se niega a entregar el texto si hay algo aquí.
    errores: Vec<CError>,
}

impl Preprocessor {
    pub fn new(features: &StandardFeatures, include_paths: Vec<PathBuf>) -> Self {
        let mut pp = Self {
            defines: HashMap::new(),
            include_paths,
            features: features.clone(),
            line: 0,
            errores: Vec::new(),
        };
        pp.definir_objeto("__BMO__", "1");
        pp.definir_objeto("__STDC__", "1");
        pp
    }

    fn definir_objeto(&mut self, nombre: &str, cuerpo: &str) {
        self.defines.insert(
            nombre.into(),
            MacroDef {
                params: vec![],
                funcion: false,
                variadica: false,
                body: cuerpo.into(),
            },
        );
    }

    /// Helper: get the current skip status (should we emit this line?)
    fn is_active(&self, skip_active: &[bool]) -> bool {
        skip_active.last().copied().unwrap_or(true)
    }

    pub fn preprocess(&mut self, source: &str, file_path: &Path) -> Result<String, CError> {
        if let Some(parent) = file_path.parent() {
            if !self.include_paths.contains(&parent.to_path_buf()) {
                self.include_paths.insert(0, parent.to_path_buf());
            }
        }

        let lines: Vec<&str> = source.lines().collect();
        let mut output = String::with_capacity(source.len());
        let mut skip_active: Vec<bool> = vec![true];
        let mut i = 0usize;

        while i < lines.len() {
            self.line = i + 1;
            let raw = lines[i].trim();

            if raw.starts_with('#') {
                let directive = raw[1..].trim_start();
                let (cmd, rest) = split_first_word(directive);

                match cmd {
                    "define" => {
                        if self.is_active(&skip_active) {
                            self.handle_define(rest)?;
                        }
                    }
                    "undef" => {
                        if self.is_active(&skip_active) {
                            self.defines.remove(rest.trim());
                        }
                    }
                    "include" => {
                        if self.is_active(&skip_active) {
                            let included = self.handle_include(rest, file_path)?;
                            output.push_str(&included);
                            output.push('\n');
                        }
                    }
                    "ifdef" => {
                        let name = rest.trim();
                        let is_def = self.defines.contains_key(name);
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && is_def);
                    }
                    "ifndef" => {
                        let name = rest.trim();
                        let is_def = self.defines.contains_key(name);
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && !is_def);
                    }
                    "if" => {
                        let val = self.eval_if_expr(rest)?;
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && val != 0);
                    }
                    "else" => {
                        if let Some(prev) = skip_active.pop() {
                            let parent = self.is_active(&skip_active);
                            skip_active.push(parent && !prev);
                        }
                    }
                    "elif" => {
                        skip_active.pop();
                        let val = self.eval_if_expr(rest)?;
                        let parent = self.is_active(&skip_active);
                        skip_active.push(parent && val != 0);
                    }
                    "endif" => {
                        skip_active.pop();
                    }
                    "error" => {
                        if self.is_active(&skip_active) {
                            return Err(CError::new(self.line, format!("#error: {}", rest)));
                        }
                    }
                    "pragma" => {
                        // skip (handled by include guard)
                    }
                    _ => {}
                }
            } else {
                if self.is_active(&skip_active) {
                    let expanded = self.expand_line(raw, true);
                    output.push_str(&expanded);
                    output.push('\n');
                }
            }
            i += 1;
        }

        if skip_active.len() != 1 {
            return Err(CError::new(self.line, "unterminated #if/#ifdef (missing #endif)"));
        }
        // Una macro mal invocada NO puede pasar de largo. Antes ni siquiera se
        // podía detectar —las macros con parámetros no se expandían— y la
        // llamada sobrevivía hasta el codegen, que emitía un `call` a una
        // función inexistente.
        if let Some(e) = self.errores.first() {
            return Err(e.clone());
        }

        Ok(output)
    }

    fn handle_define(&mut self, rest: &str) -> Result<(), CError> {
        let rest = rest.trim();
        // El nombre llega hasta el primer carácter que no es de identificador.
        let fin_nombre = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let name = rest[..fin_nombre].to_string();
        if name.is_empty() {
            return Err(CError::new(self.line, "#define: nombre vacio"));
        }
        let resto = &rest[fin_nombre..];

        // ★ Es función SÓLO si el paréntesis va PEGADO al nombre. El espacio
        // manda, y es el único sitio de C donde manda. Ver `MacroDef::funcion`.
        if resto.starts_with('(') {
            let cierre = resto
                .find(')')
                .ok_or_else(|| CError::new(self.line, "#define: falta el ) de la lista de parametros"))?;
            let dentro = &resto[1..cierre];
            let mut params: Vec<String> = Vec::new();
            let mut variadica = false;
            for p in dentro.split(',') {
                let p = p.trim();
                if p.is_empty() {
                    continue;
                }
                if p == "..." {
                    variadica = true;
                    continue;
                }
                params.push(p.to_string());
            }
            if variadica && !self.features.variadic_macros {
                return Err(CError::new(
                    self.line,
                    format!("macro variadica '{name}(...)': este estandar de C no las tiene"),
                ));
            }
            let body = resto[cierre + 1..].trim().to_string();
            self.defines.insert(name, MacroDef { params, funcion: true, variadica, body });
        } else {
            self.defines.insert(
                name,
                MacroDef {
                    params: vec![],
                    funcion: false,
                    variadica: false,
                    body: resto.trim().to_string(),
                },
            );
        }
        Ok(())
    }

    fn handle_include(&mut self, rest: &str, current_file: &Path) -> Result<String, CError> {
        let path_str = rest.trim().trim_matches('"').trim_matches('<').trim_matches('>').trim_matches('"');
        let mut found: Option<PathBuf> = None;

        // Search relative to current file for "..." includes
        if rest.trim().starts_with('"') {
            if let Some(parent) = current_file.parent() {
                let p = parent.join(path_str);
                if p.exists() { found = Some(p); }
            }
        }

        // Search include paths
        if found.is_none() {
            for base in &self.include_paths {
                let p = base.join(path_str);
                if p.exists() { found = Some(p); break; }
                if let Some(fname) = Path::new(path_str).file_name() {
                    let p2 = base.join(fname);
                    if p2.exists() { found = Some(p2); break; }
                }
            }
        }

        match found {
            Some(p) => {
                let content = fs::read_to_string(&p)
                    .map_err(|e| CError::new(self.line,
                        format!("#include cannot read {}: {}", p.display(), e)))?;
                let mut sub = Preprocessor {
                    defines: self.defines.clone(),
                    include_paths: self.include_paths.clone(),
                    features: self.features.clone(),
                    line: 0,
                    errores: Vec::new(),
                };
                let texto = sub.preprocess(&content, &p)?;
                // ★ Lo que la cabecera DEFINIÓ se queda.
                //
                // Antes no: el sub-preprocesador nacía con una copia de las
                // macros, expandía el fichero incluido y **se moría con sus
                // `#define` dentro**. O sea, una cabecera podía traer funciones
                // pero no constantes — que es justo para lo que sirve una
                // cabecera.
                //
                // Y no fallaba: la directiva se consumía, así que
                // `BMO_TECLA_REPAG` seguía en el texto como un identificador
                // suelto y el parser lo tomaba por una variable. Dos constantes
                // distintas se volvían la MISMA variable inventada, así que
                // `if (t == REPAG)` era cierto también para AvPag. Comparaba
                // basura contra la misma basura y parecía que funcionaba.
                //
                // Se conserva lo que ya había en caso de choque: un `#define`
                // del fichero que incluye manda sobre el de la cabecera, que es
                // lo que espera quien escribe el `#define` antes del `#include`
                // para configurarla.
                for (nombre, cuerpo) in sub.defines {
                    self.defines.entry(nombre).or_insert(cuerpo);
                }
                Ok(texto)
            }
            None => Err(CError::new(self.line,
                format!("#include: file not found: {}", path_str))),
        }
    }

    fn eval_if_expr(&mut self, expr: &str) -> Result<i64, CError> {
        let expr = expr.trim();
        if expr.is_empty() { return Ok(0); }

        let mut expanded = expr.to_string();
        while let Some(pos) = expanded.find("defined(") {
            let start = pos + 8;
            if let Some(end) = expanded[start..].find(')') {
                let name = expanded[start..start+end].trim();
                let is_def = if self.defines.contains_key(name) { "1" } else { "0" };
                expanded.replace_range(pos..start+end+1, is_def);
            } else { break; }
        }
        for (name, _) in self.defines.clone().iter() {
            let pattern = format!("defined {}", name);
            if expanded.contains(&pattern) { expanded = expanded.replace(&pattern, "1"); }
        }
        expanded = self.expand_line(&expanded, false);
        Self::eval_simple(&expanded).ok_or_else(||
            CError::new(self.line, format!("#if: cannot evaluate '{}'", expanded)))
    }

    fn eval_simple(expr: &str) -> Option<i64> {
        let expr = expr.trim();
        if expr.is_empty() { return Some(0); }
        if let Ok(n) = expr.parse::<i64>() { return Some(n); }
        if let Some(inner) = expr.strip_prefix('!') {
            let v = Self::eval_simple(inner)?;
            return Some(if v == 0 { 1 } else { 0 });
        }
        let ops = ["==", "!=", "<=", ">=", "&&", "||", "<<", ">>", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^"];
        for op in &ops {
            if let Some(pos) = expr.find(op) {
                let a = Self::eval_simple(expr[..pos].trim())?;
                let b = Self::eval_simple(expr[pos+op.len()..].trim())?;
                return match *op {
                    "==" => Some(if a == b { 1 } else { 0 }),
                    "!=" => Some(if a != b { 1 } else { 0 }),
                    "<=" | ">=" => Some(if (op.as_bytes()[0] == b'<' && a <= b) || (op.as_bytes()[0] == b'>' && a >= b) { 1 } else { 0 }),
                    "<"  => Some(if a < b  { 1 } else { 0 }),
                    ">"  => Some(if a > b  { 1 } else { 0 }),
                    "&&" => Some(if a != 0 && b != 0 { 1 } else { 0 }),
                    "||" => Some(if a != 0 || b != 0 { 1 } else { 0 }),
                    "+" => Some(a.wrapping_add(b)),
                    "-" => Some(a.wrapping_sub(b)),
                    "*" => Some(a.wrapping_mul(b)),
                    "/" => if b != 0 { Some(a / b) } else { None },
                    "%" => if b != 0 { Some(a % b) } else { None },
                    "&" => Some(a & b), "|" => Some(a | b), "^" => Some(a ^ b),
                    "<<" => Some(a.wrapping_shl(b as u32)),
                    ">>" => Some(a.wrapping_shr(b as u32)),
                    _ => None,
                };
            }
        }
        None
    }

    /// Expande macros en una línea hasta que deje de cambiar.
    ///
    /// ## Lo que había antes
    ///
    /// Un bucle por CADA macro definida, ordenadas por longitud descendente,
    /// buscando su nombre por toda la línea. Tres cosas mal:
    ///
    /// 1. **Las macros con parámetros no se expandían.** El `if` pedía
    ///    `m.params.is_empty()`, así que `MAX(a,b)` se quedaba en el texto tal
    ///    cual y el parser lo tomaba por una llamada a una función `MAX` que no
    ///    existe. Era el agujero grande de "lo típico de C".
    /// 2. El orden por longitud era un apaño para que `AB` no se comiera a `A`;
    ///    recorrer el texto una vez y mirar identificadores COMPLETOS lo hace
    ///    innecesario.
    /// 3. Sustituía **dentro de las cadenas**: `printf("BMO_TECLA_REPAG")`
    ///    imprimía `135`.
    ///
    /// Ahora se recorre el texto una sola vez por pasada, saltando literales, y
    /// se repite mientras algo cambie (con tope, ver [`MAX_PASADAS`]).
    fn expand_line(&mut self, line: &str, _report_errors: bool) -> String {
        let mut texto = line.to_string();
        for _ in 0..MAX_PASADAS {
            let siguiente = self.expandir_una_pasada(&texto);
            if siguiente == texto {
                return texto;
            }
            texto = siguiente;
        }
        texto
    }

    fn expandir_una_pasada(&mut self, texto: &str) -> String {
        let b = texto.as_bytes();
        let mut out = String::with_capacity(texto.len());
        let mut i = 0usize;
        while i < b.len() {
            // Un literal no es código, es dato: lo de dentro se copia entero.
            if b[i] == b'"' || b[i] == b'\'' {
                i = copiar_literal(texto, i, &mut out);
                continue;
            }
            if !is_ident_start(b[i]) {
                out.push(b[i] as char);
                i += 1;
                continue;
            }
            let ini = i;
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            let nombre = &texto[ini..i];
            let Some(m) = self.defines.get(nombre).cloned() else {
                out.push_str(nombre);
                continue;
            };
            if !m.funcion {
                out.push_str(&m.body);
                continue;
            }
            // Macro-función: sin paréntesis detrás, el nombre a secas NO es una
            // invocación. En C eso es legal y se deja tal cual — es como se
            // pasa el nombre de una macro a otra.
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            if j >= b.len() || b[j] != b'(' {
                out.push_str(nombre);
                continue;
            }
            let Some((args, fin)) = recoger_args(texto, j) else {
                // Paréntesis sin cerrar en esta línea. Puede ser una invocación
                // partida en varias líneas, que este preprocesador no junta:
                // se dice en vez de expandir a medias.
                let linea = self.line;
                self.errores.push(CError::new(
                    linea,
                    format!("la macro '{nombre}(' no cierra su parentesis en esta linea"),
                ));
                out.push_str(nombre);
                continue;
            };
            let esperados = m.params.len();
            let cuadra = if m.variadica { args.len() >= esperados } else { args.len() == esperados };
            if !cuadra {
                let linea = self.line;
                self.errores.push(CError::new(
                    linea,
                    format!(
                        "la macro '{nombre}' espera {esperados} argumento(s){}, recibio {}",
                        if m.variadica { " o mas" } else { "" },
                        args.len()
                    ),
                ));
                out.push_str(nombre);
                continue;
            }
            out.push_str(&sustituir(&m, &args));
            i = fin;
        }
        out
    }
}

/// Copia un literal de cadena o de carácter tal cual. Devuelve dónde sigue.
fn copiar_literal(texto: &str, desde: usize, out: &mut String) -> usize {
    let b = texto.as_bytes();
    let cierre = b[desde];
    out.push(cierre as char);
    let mut i = desde + 1;
    while i < b.len() {
        // Una comilla escapada no cierra nada.
        if b[i] == b'\\' && i + 1 < b.len() {
            out.push(b[i] as char);
            out.push(b[i + 1] as char);
            i += 2;
            continue;
        }
        out.push(b[i] as char);
        i += 1;
        if b[i - 1] == cierre {
            break;
        }
    }
    i
}

/// Los argumentos de una invocación, empezando en el `(`.
///
/// Devuelve `(argumentos, posición justo detrás del `)`)`. Las comas que hay
/// **dentro** de paréntesis, corchetes o literales no separan argumentos: sin
/// eso, `MAX(f(a,b), c)` se leería como tres argumentos.
fn recoger_args(texto: &str, abre: usize) -> Option<(Vec<String>, usize)> {
    let b = texto.as_bytes();
    let mut nivel = 0i32;
    let mut args: Vec<String> = Vec::new();
    let mut actual = String::new();
    let mut i = abre;
    while i < b.len() {
        let c = b[i];
        if c == b'"' || c == b'\'' {
            let mut tmp = String::new();
            i = copiar_literal(texto, i, &mut tmp);
            actual.push_str(&tmp);
            continue;
        }
        match c {
            b'(' | b'[' => {
                nivel += 1;
                if nivel > 1 {
                    actual.push(c as char);
                }
                i += 1;
            }
            b')' | b']' => {
                nivel -= 1;
                if nivel == 0 {
                    let a = actual.trim();
                    // `F()` con la lista vacía es CERO argumentos, no uno vacío.
                    if !(args.is_empty() && a.is_empty()) {
                        args.push(a.to_string());
                    }
                    return Some((args, i + 1));
                }
                actual.push(c as char);
                i += 1;
            }
            b',' if nivel == 1 => {
                args.push(actual.trim().to_string());
                actual.clear();
                i += 1;
            }
            _ => {
                actual.push(c as char);
                i += 1;
            }
        }
    }
    None
}

/// El cuerpo de la macro con los parámetros ya sustituidos.
///
/// Hace las tres cosas que un cuerpo puede pedir: `#p` convierte el argumento
/// en cadena, `a ## b` pega dos piezas en un solo símbolo, y `__VA_ARGS__` es
/// todo lo que sobró en una variádica.
fn sustituir(m: &MacroDef, args: &[String]) -> String {
    let cuerpo = m.body.as_bytes();
    let mut out = String::with_capacity(m.body.len());
    let mut i = 0usize;
    while i < cuerpo.len() {
        if cuerpo[i] == b'"' || cuerpo[i] == b'\'' {
            i = copiar_literal(&m.body, i, &mut out);
            continue;
        }
        // `##` — pegar. Se come el espacio de los dos lados: lo que queda tiene
        // que ser UN símbolo, no dos separados.
        if cuerpo[i] == b'#' && i + 1 < cuerpo.len() && cuerpo[i + 1] == b'#' {
            while out.ends_with(' ') || out.ends_with('\t') {
                out.pop();
            }
            i += 2;
            while i < cuerpo.len() && (cuerpo[i] == b' ' || cuerpo[i] == b'\t') {
                i += 1;
            }
            continue;
        }
        // `#p` — convertir el argumento en cadena.
        if cuerpo[i] == b'#' && i + 1 < cuerpo.len() && is_ident_start(cuerpo[i + 1]) {
            let ini = i + 1;
            let mut j = ini;
            while j < cuerpo.len() && is_ident_char(cuerpo[j]) {
                j += 1;
            }
            let nombre = &m.body[ini..j];
            if let Some(k) = m.params.iter().position(|p| p == nombre) {
                out.push('"');
                for ch in args[k].chars() {
                    if ch == '"' || ch == '\\' {
                        out.push('\\');
                    }
                    out.push(ch);
                }
                out.push('"');
                i = j;
                continue;
            }
        }
        if !is_ident_start(cuerpo[i]) {
            out.push(cuerpo[i] as char);
            i += 1;
            continue;
        }
        let ini = i;
        while i < cuerpo.len() && is_ident_char(cuerpo[i]) {
            i += 1;
        }
        let nombre = &m.body[ini..i];
        if nombre == "__VA_ARGS__" && m.variadica {
            let sobrantes = &args[m.params.len().min(args.len())..];
            out.push_str(&sobrantes.join(", "));
        } else if let Some(k) = m.params.iter().position(|p| p == nombre) {
            out.push_str(&args[k]);
        } else {
            out.push_str(nombre);
        }
    }
    out
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

fn split_first_word(s: &str) -> (&str, &str) {
    let s = s.trim_start();
    if let Some(pos) = s.find(|c: char| c.is_whitespace()) {
        (&s[..pos], s[pos..].trim_start())
    } else {
        (s, "")
    }
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

//! C Preprocessor -- handles #define, #include, #ifdef, #if, #endif.
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
    /// Se definio con parentesis **pegados** al nombre?
    ///
    /// No se puede deducir de `params.is_empty()`, y confundirlo es un bug con
    /// dos caras:
    ///
    /// ```text
    ///   #define F() 1      funcion SIN parametros  -> params vacio, funcion
    ///   #define F (1)      OBJETO cuyo cuerpo empieza por parentesis
    /// ```
    ///
    /// El lector viejo hacia `rest.find('(')` sin mirar si habia un espacio
    /// delante, asi que `#define CAJA_ANCHO (760)` se registraba como una
    /// macro-funcion con un parametro llamado `760` y cuerpo **vacio**. La
    /// constante desaparecia en silencio. En C el espacio es significativo
    /// aqui, y es el unico sitio del lenguaje donde lo es.
    funcion: bool,
    /// Termina en `...`: el resto de argumentos entra por `__VA_ARGS__`.
    variadica: bool,
    body: String,
}

/// Cuantas veces se re-recorre una linea buscando mas macros que expandir.
///
/// Hace falta un tope porque una macro puede producir otra: `#define A B` y
/// `#define B 1` necesitan dos pasadas. Y hace falta que sea un TOPE porque
/// `#define A A` es legal y no termina nunca -- en C de verdad se evita con la
/// regla de "una macro no se re-expande dentro de si misma", que pide llevar un
/// conjunto de macros en curso; aqui se corta por profundidad, que es mas pobre
/// pero no cuelga el compilador.
const MAX_PASADAS: usize = 16;

pub struct Preprocessor {
    defines: HashMap<String, MacroDef>,
    include_paths: Vec<PathBuf>,
    features: StandardFeatures,
    line: usize,
    /// Lo que salio mal expandiendo. No puede devolverse en el acto porque la
    /// expansion ocurre a mitad de recorrer el fichero; se acumula y
    /// `preprocess` se niega a entregar el texto si hay algo aqui.
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

    fn definir_objeto(&mut self, name: &str, cuerpo: &str) {
        self.defines.insert(
            name.into(),
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
        // podia detectar --las macros con parametros no se expandian-- y la
        // llamada sobrevivia hasta el codegen, que emitia un `call` a una
        // funcion inexistente.
        if let Some(e) = self.errores.first() {
            return Err(e.clone());
        }

        Ok(output)
    }

    fn handle_define(&mut self, rest: &str) -> Result<(), CError> {
        let rest = rest.trim();
        // El nombre llega hasta el primer caracter que no es de identificador.
        let fin_nombre = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let name = rest[..fin_nombre].to_string();
        if name.is_empty() {
            return Err(CError::new(self.line, "#define: nombre vacio"));
        }
        let resto = &rest[fin_nombre..];

        // * Es funcion SOLO si el parentesis va PEGADO al nombre. El espacio
        // manda, y es el unico sitio de C donde manda. Ver `MacroDef::funcion`.
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
            let body = sin_comentarios(&resto[cierre + 1..]);
            self.defines.insert(name, MacroDef { params, funcion: true, variadica, body });
        } else {
            self.defines.insert(
                name,
                MacroDef {
                    params: vec![],
                    funcion: false,
                    variadica: false,
                    body: sin_comentarios(resto),
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
                // * Lo que la cabecera DEFINIO se queda.
                //
                // Antes no: el sub-preprocesador nacia con una copia de las
                // macros, expandia el fichero incluido y **se moria con sus
                // `#define` dentro**. O sea, una cabecera podia traer funciones
                // pero no constantes -- que es justo para lo que sirve una
                // cabecera.
                //
                // Y no fallaba: la directiva se consumia, asi que
                // `BMO_TECLA_REPAG` seguia en el texto como un identificador
                // suelto y el parser lo tomaba por una variable. Dos constantes
                // distintas se volvian la MISMA variable inventada, asi que
                // `if (t == REPAG)` era cierto tambien para AvPag. Comparaba
                // basura contra la misma basura y parecia que funcionaba.
                //
                // Se conserva lo que ya habia en caso de choque: un `#define`
                // del fichero que incluye manda sobre el de la cabecera, que es
                // lo que espera quien escribe el `#define` antes del `#include`
                // para configurarla.
                for (name, cuerpo) in sub.defines {
                    self.defines.entry(name).or_insert(cuerpo);
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

    /// Expande macros en una linea hasta que deje de cambiar.
    ///
    /// ## Lo que habia antes
    ///
    /// Un bucle por CADA macro definida, ordenadas por longitud descendente,
    /// buscando su nombre por toda la linea. Tres cosas mal:
    ///
    /// 1. **Las macros con parametros no se expandian.** El `if` pedia
    ///    `m.params.is_empty()`, asi que `MAX(a,b)` se quedaba en el texto tal
    ///    cual y el parser lo tomaba por una llamada a una funcion `MAX` que no
    ///    existe. Era el agujero grande de "lo tipico de C".
    /// 2. El orden por longitud era un apano para que `AB` no se comiera a `A`;
    ///    recorrer el texto una vez y mirar identificadores COMPLETOS lo hace
    ///    innecesario.
    /// 3. Sustituia **dentro de las cadenas**: `printf("BMO_TECLA_REPAG")`
    ///    imprimia `135`.
    ///
    /// Ahora se recorre el texto una sola vez por pasada, saltando literales, y
    /// se repite mientras algo cambie (con tope, ver [`MAX_PASADAS`]).
    fn expand_line(&mut self, line: &str, _report_errors: bool) -> String {
        let mut texto = line.to_string();
        for _ in 0..MAX_PASADAS {
            let next = self.expandir_una_pasada(&texto);
            if next == texto {
                return texto;
            }
            texto = next;
        }
        texto
    }

    fn expandir_una_pasada(&mut self, texto: &str) -> String {
        let b = texto.as_bytes();
        let mut out = String::with_capacity(texto.len());
        let mut i = 0usize;
        while i < b.len() {
            // Un literal no es codigo, es dato: lo de dentro se copia entero.
            if b[i] == b'"' || b[i] == b'\'' {
                i = copiar_literal(texto, i, &mut out);
                continue;
            }
            if !is_ident_start(b[i]) {
                // ** UN BYTE NO ES UN CARACTER, y esto costaba medio megabyte.
                //
                // `b[i] as char` interpreta el byte como Latin-1: el `n` de
                // `"ano"` son DOS bytes en UTF-8 (C3 B1) y salian como dos
                // caracteres U+00C3 U+00B1, que al volver a codificarse son
                // **cuatro** bytes. Cada byte no-ASCII se duplicaba por pasada.
                //
                // Y las pasadas son 16, porque el bucle repite "mientras algo
                // cambie" y esto cambiaba siempre: 2^16. Medido, un `hola
                // mundo` con una sola `n` daba un `.bex` de **492.032 bytes**
                // --con `MAX_BEX` en 1 MiB, dos palabras con tilde y el programa
                // ya no carga-- y donde iba la `n` habia 65.536 bytes de basura.
                //
                // El acento no "no funcionaba": se convertia en un problema de
                // tamano de binario, que es el ultimo sitio donde uno lo busca.
                if b[i] < 0x80 {
                    out.push(b[i] as char);
                    i += 1;
                } else {
                    let c = texto[i..].chars().next().unwrap();
                    i += c.len_utf8();
                    out.push(c);
                }
                continue;
            }
            let ini = i;
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            let name = &texto[ini..i];
            let Some(m) = self.defines.get(name).cloned() else {
                out.push_str(name);
                continue;
            };
            if !m.funcion {
                out.push_str(&m.body);
                continue;
            }
            // Macro-funcion: sin parentesis detras, el nombre a secas NO es una
            // invocacion. En C eso es legal y se deja tal cual -- es como se
            // pasa el nombre de una macro a otra.
            let mut j = i;
            while j < b.len() && (b[j] == b' ' || b[j] == b'\t') {
                j += 1;
            }
            if j >= b.len() || b[j] != b'(' {
                out.push_str(name);
                continue;
            }
            let Some((args, fin)) = recoger_args(texto, j) else {
                // Parentesis sin cerrar en esta linea. Puede ser una invocacion
                // partida en varias lineas, que este preprocesador no junta:
                // se dice en vez de expandir a medias.
                let linea = self.line;
                self.errores.push(CError::new(
                    linea,
                    format!("la macro '{name}(' no cierra su parentesis en esta linea"),
                ));
                out.push_str(name);
                continue;
            };
            let esperados = m.params.len();
            let cuadra = if m.variadica { args.len() >= esperados } else { args.len() == esperados };
            if !cuadra {
                let linea = self.line;
                self.errores.push(CError::new(
                    linea,
                    format!(
                        "la macro '{name}' espera {esperados} argumento(s){}, recibio {}",
                        if m.variadica { " o mas" } else { "" },
                        args.len()
                    ),
                ));
                out.push_str(name);
                continue;
            }
            out.push_str(&sustituir(&m, &args));
            i = fin;
        }
        out
    }
}

/// Copia un literal de cadena o de caracter tal cual. Devuelve donde sigue.
fn copiar_literal(texto: &str, desde: usize, out: &mut String) -> usize {
    let b = texto.as_bytes();
    let cierre = b[desde];
    out.push(cierre as char);
    let mut i = desde + 1;
    while i < b.len() {
        // Una comilla escapada no cierra nada. El `\` y lo que le sigue son
        // ASCII los dos, asi que aqui el byte si es el caracter.
        if b[i] == b'\\' && i + 1 < b.len() && b[i + 1] < 0x80 {
            out.push(b[i] as char);
            out.push(b[i + 1] as char);
            i += 2;
            continue;
        }
        // * Y AQUI ESTABA LA `n`. Copiar el literal byte a byte con
        // `b[i] as char` es lo mismo que hace `expandir_una_pasada`, con el
        // mismo resultado: el texto del programa --lo que el usuario LEE-- salia
        // multiplicado por 2^16. Ver la nota larga alli.
        if b[i] < 0x80 {
            out.push(b[i] as char);
            i += 1;
        } else {
            let c = texto[i..].chars().next().unwrap();
            i += c.len_utf8();
            out.push(c);
        }
        if b[i - 1] == cierre {
            break;
        }
    }
    i
}

/// Los argumentos de una invocacion, empezando en el `(`.
///
/// Devuelve `(argumentos, posicion justo detras del `)`)`. Las comas que hay
/// **dentro** de parentesis, corchetes o literales no separan argumentos: sin
/// eso, `MAX(f(a,b), c)` se leeria como tres argumentos.
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
                    // `F()` con la lista vacia es CERO argumentos, no uno vacio.
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

/// El cuerpo de la macro con los parametros ya sustituidos.
///
/// Hace las tres cosas que un cuerpo puede pedir: `#p` convierte el argumento
/// en cadena, `a ## b` pega dos piezas en un solo simbolo, y `__VA_ARGS__` es
/// todo lo que sobro en una variadica.
fn sustituir(m: &MacroDef, args: &[String]) -> String {
    let cuerpo = m.body.as_bytes();
    let mut out = String::with_capacity(m.body.len());
    let mut i = 0usize;
    while i < cuerpo.len() {
        if cuerpo[i] == b'"' || cuerpo[i] == b'\'' {
            i = copiar_literal(&m.body, i, &mut out);
            continue;
        }
        // `##` -- pegar. Se come el espacio de los dos lados: lo que queda tiene
        // que ser UN simbolo, no dos separados.
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
        // `#p` -- convertir el argumento en cadena.
        if cuerpo[i] == b'#' && i + 1 < cuerpo.len() && is_ident_start(cuerpo[i + 1]) {
            let ini = i + 1;
            let mut j = ini;
            while j < cuerpo.len() && is_ident_char(cuerpo[j]) {
                j += 1;
            }
            let name = &m.body[ini..j];
            if let Some(k) = m.params.iter().position(|p| p == name) {
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
        let name = &m.body[ini..i];
        if name == "__VA_ARGS__" && m.variadica {
            let sobrantes = &args[m.params.len().min(args.len())..];
            out.push_str(&sobrantes.join(", "));
        } else if let Some(k) = m.params.iter().position(|p| p == name) {
            out.push_str(&args[k]);
        } else {
            out.push_str(name);
        }
    }
    out
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_'
}

/// * El cuerpo de un `#define` NO incluye el comentario que lleva detras.
///
/// # El bug que esto arregla, que parecia imposible
///
/// El estandar de C borra los comentarios en la **fase 3**, antes de que el
/// preprocesador mire una sola directiva. Aqui no se hacia, asi que
///
/// ```c
/// #define UNO 65536   /* 1.0 en 16.16 */
/// ```
///
/// definia `UNO` como `65536   /* 1.0 en 16.16 */`, **comentario incluido**. En
/// codigo no se notaba: un comentario de mas donde ya iba a haber uno.
///
/// Pero la expansion tambien se aplica **dentro de los comentarios**, y ahi ese
/// cuerpo mete un `*/` que **cierra el comentario antes de tiempo**. O sea que
/// escribir en un comentario el nombre de una macro que lleve comentario
/// convierte el resto del parrafo en codigo, y el compilador se queja de una
/// linea que no tiene nada malo, varias mas abajo.
///
/// Se cazo escribiendo `t = 20 * UNO` dentro de un comentario del raycaster.
fn sin_comentarios(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'*' {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            // Un comentario vale por UN espacio: `A/**/B` son dos piezas, no
            // `AB`. Es lo que dice la fase 3 y aqui importa igual.
            out.push(' ');
            continue;
        }
        if b[i] == b'/' && i + 1 < b.len() && b[i + 1] == b'/' {
            break;
        }
        if b[i] < 0x80 {
            out.push(b[i] as char);
            i += 1;
        } else {
            // Ver `expandir_una_pasada`: un byte no es un caracter.
            let c = s[i..].chars().next().unwrap();
            i += c.len_utf8();
            out.push(c);
        }
    }
    out.trim().to_string()
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

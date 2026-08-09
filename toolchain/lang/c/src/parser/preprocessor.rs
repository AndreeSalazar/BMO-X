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

/// Una rama de un grupo `#if / #elif / #else / #endif`.
///
/// ** POR QUE HACEN FALTA DOS BITS Y NO UNO
///
/// Con solo "esta rama esta activa", `#elif` no puede saber si **alguna
/// anterior ya entro**, y lo que hacia era mirar la de justo antes. Resultado:
///
/// ```c
/// #if (__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__)
/// #define SYS_LITTLE_ENDIAN
/// #elif (__BYTE_ORDER__ == __ORDER_BIG_ENDIAN__)
/// #define SYS_BIG_ENDIAN
/// #endif
/// ```
///
/// **Las DOS se definian.** Los dos identificadores son desconocidos y valen 0
/// --que es lo que manda C11 6.10.1p4 y este preprocesador cumple a proposito--
/// asi que las dos condiciones son ciertas, y sin memoria de grupo la segunda
/// entra igual. Ese es exactamente `i_swap.h` de DOOM: quedaba definido
/// `SYS_BIG_ENDIAN` en una maquina little-endian, y mas abajo se llamaba a una
/// funcion que solo existe en el otro mundo.
///
/// No falla ruidosamente: **compila las DOS ramas de todo grupo cuya primera
/// condicion sea cierta**. Un `#define` repetido se pisa y gana el ultimo, o
/// sea que la configuracion que queda puesta es la que el programa descarto.
///
/// `ya_tomada` es esa memoria: una vez que una rama del grupo entro, ninguna
/// otra puede.
#[derive(Clone, Copy)]
struct Rama {
    /// Se emite lo que hay dentro de esta rama?
    activa: bool,
    /// Alguna rama de este grupo --esta o una anterior-- ya entro?
    ya_tomada: bool,
}

impl Rama {
    fn abierta(activa: bool) -> Self {
        Rama { activa, ya_tomada: activa }
    }
}

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
    fn is_active(&self, skip_active: &[Rama]) -> bool {
        skip_active.last().map(|r| r.activa).unwrap_or(true)
    }

    pub fn preprocess(&mut self, source: &str, file_path: &Path) -> Result<String, CError> {
        if let Some(parent) = file_path.parent() {
            if !self.include_paths.contains(&parent.to_path_buf()) {
                self.include_paths.insert(0, parent.to_path_buf());
            }
        }

        let lines: Vec<&str> = source.lines().collect();
        let mut output = String::with_capacity(source.len());
        let mut skip_active: Vec<Rama> = vec![Rama::abierta(true)];
        let mut i = 0usize;

        while i < lines.len() {
            self.line = i + 1;
            let mut raw = lines[i].trim().to_string();

            // * A LINE ENDING IN `\` IS NOT A LINE (C11 phase 2).
            //
            // The backslash and the newline are deleted and the two lines
            // become one, BEFORE anything else looks at them. Without it, a
            // macro written across several lines -- which is how every
            // non-trivial macro is written --
            //
            //   #define Z_ChangeTag(p,t) \
            //   { ... Z_ChangeTag2((p),(t)); }
            //
            // defines a body of `\` and then drops the second line into the
            // file as if it were code. The error lands on that line and says
            // "expected type, got Ident(Z_ChangeTag2)", which is a true
            // statement about a line that should not exist.
            //
            // It joins in the OUTPUT too, not only in directives: the same
            // rule holds for a long string or a table split across lines.
            while raw.ends_with('\\') && i + 1 < lines.len() {
                raw.pop();
                i += 1;
                raw.push_str(lines[i].trim());
            }
            let raw = raw.as_str();

            if raw.starts_with('#') {
                // * Comments die BEFORE the directive is read.
                //
                // In C a comment is removed in translation phase 3, which runs
                // before directives are executed in phase 4. Doing it later --
                // or not at all -- makes the comment part of the payload, and
                // then it fails as something else entirely:
                //
                //   #if 0 // UNUSED          -> "cannot evaluate '0 // UNUSED'"
                //   #include "m_argv.h" // x -> "file not found: m_argv.h" // x"
                //
                // Both messages point at the wrong thing: the first blames the
                // expression, the second blames the path. DOOM hit each of them
                // in dozens of files.
                //
                // It is string-aware because it has to be: `#define URL
                // "http://x"` carries a `//` that is NOT a comment, and a
                // stripper that does not know what a string is would cut the
                // macro in half and leave something that still compiles.
                let raw = sin_comentarios(raw);
                let raw = raw.trim();
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
                        skip_active.push(Rama::abierta(parent && is_def));
                    }
                    "ifndef" => {
                        let name = rest.trim();
                        let is_def = self.defines.contains_key(name);
                        let parent = self.is_active(&skip_active);
                        skip_active.push(Rama::abierta(parent && !is_def));
                    }
                    "if" => {
                        let val = self.eval_if_expr(rest)?;
                        let parent = self.is_active(&skip_active);
                        skip_active.push(Rama::abierta(parent && val != 0));
                    }
                    "else" => {
                        if let Some(prev) = skip_active.pop() {
                            let parent = self.is_active(&skip_active);
                            // El `else` entra si NINGUNA rama anterior del grupo
                            // entro -- no solo si no entro la de justo antes.
                            skip_active.push(Rama {
                                activa: parent && !prev.ya_tomada,
                                ya_tomada: true,
                            });
                        }
                    }
                    "elif" => {
                        if let Some(prev) = skip_active.pop() {
                            let parent = self.is_active(&skip_active);
                            // ** Se evalua IGUAL aunque el grupo ya este
                            // resuelto, porque un `#elif` puede llevar una
                            // division por cero o un `defined` de algo que no
                            // existe y en C eso no se mira si no toca. Aqui se
                            // mira y da 0, que es inofensivo: lo que decide es
                            // `ya_tomada`.
                            let val = self.eval_if_expr(rest)?;
                            let entra = parent && !prev.ya_tomada && val != 0;
                            skip_active.push(Rama {
                                activa: entra,
                                ya_tomada: prev.ya_tomada || entra,
                            });
                        }
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
                    // * UNA INVOCACION DE MACRO PARTIDA EN VARIAS LINEAS.
                    //
                    // Este preprocesador expande LINEA A LINEA, y una
                    // invocacion cuyo parentesis se cierra tres lineas mas
                    // abajo no cabe en esa idea:
                    //
                    //   V_DrawPatchDirect(0, 0, W_CacheLumpName(DEH_String(
                    //       "HELP2"), PU_CACHE));
                    //
                    // Se juntan las lineas que hagan falta hasta que los
                    // parentesis cierren. La cuenta ignora lo que va dentro de
                    // una cadena --un `(` entre comillas es texto, no
                    // gramatica-- y tiene tope: un parentesis que no cierra
                    // nunca es un fallo del programa, y colgarse buscandolo
                    // seria peor que decirlo.
                    //
                    // Solo se juntan lineas donde hay una macro de funcion
                    // esperando: fuera de ahi, un parentesis abierto entre
                    // lineas es normal en C y no hay nada que expandir.
                    let mut fuente = raw.to_string();
                    if self.hay_macro_funcion(&fuente) {
                        let mut juntadas = 0;
                        while balance_parentesis(&fuente) > 0
                            && i + 1 < lines.len()
                            && juntadas < 32
                        {
                            i += 1;
                            juntadas += 1;
                            fuente.push(' ');
                            fuente.push_str(lines[i].trim());
                        }
                    }
                    let expanded = self.expand_line(&fuente, true);
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

        // * What is left over is ZERO, and that is the C rule, not a shortcut.
        //
        // C11 6.10.1p4: after macro expansion, every identifier still standing
        // in a `#if` is replaced by `0`. Refusing to evaluate it instead looks
        // stricter and is simply wrong -- `#if ORIGCODE` and `#if _WIN64` are
        // how a portable program says "not this platform", and DOOM says it in
        // 45 of its 81 files.
        //
        // It also settles `#if (__BYTE_ORDER__ == __ORDER_LITTLE_ENDIAN__)`:
        // both sides become 0, the test is true, and the little-endian branch
        // is taken -- which is the correct branch on x86-64. Getting the right
        // answer here is luck; getting a defined answer is the rule.
        let expanded = zero_out_identifiers(&expanded);

        Self::eval_simple(&expanded).ok_or_else(||
            CError::new(self.line, format!("#if: cannot evaluate '{}'", expanded)))
    }

    /// Evaluate a `#if` expression.
    ///
    /// * WHY THIS IS A REAL PARSER AND NOT A `find(op)` LOOP
    ///
    /// The version this replaces scanned for the first operator of a fixed
    /// list, anywhere in the string, and split there. Two things followed, and
    /// only one of them was visible:
    ///
    ///   1. `#if (0 == 0)` could not be evaluated at all -- parentheses were
    ///      not a thing it knew about, and DOOM writes them in every
    ///      endianness test.
    ///   2. **`a == b && c` split at `==` first**, so it computed
    ///      `a == (b && c)`. That one does not fail: it answers, and the
    ///      answer is wrong, and what it decides is which half of a file
    ///      exists. A preprocessor that picks the wrong branch produces a
    ///      program that compiles cleanly and is not the program that was
    ///      written.
    ///
    /// So it is precedence climbing over a real token list. Same size, and it
    /// cannot get (2) wrong by construction.
    fn eval_simple(expr: &str) -> Option<i64> {
        let tokens = lex_if_expr(expr)?;
        let mut p = IfExprParser { t: &tokens, i: 0 };
        let v = p.expr(0)?;
        // Trailing junk means it was not understood -- saying so beats
        // answering with the part that happened to parse.
        if p.i != p.t.len() {
            return None;
        }
        Some(v)
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
    /// Hay en esta linea el nombre de una macro DE FUNCION seguido de `(`?
    ///
    /// Es la condicion para traerse la linea siguiente. Sin ella se juntarian
    /// tambien las lineas de cualquier expresion partida --normalisimas en C--
    /// y el texto de salida dejaria de parecerse al que se escribio.
    fn hay_macro_funcion(&self, linea: &str) -> bool {
        let b = linea.as_bytes();
        let mut i = 0usize;
        while i < b.len() {
            if !is_ident_char(b[i]) || (i > 0 && is_ident_char(b[i - 1])) {
                i += 1;
                continue;
            }
            let inicio = i;
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            if b.get(i) == Some(&b'(') {
                if let Some(m) = self.defines.get(&linea[inicio..i]) {
                    if m.funcion {
                        return true;
                    }
                }
            }
        }
        false
    }

    fn expand_line(&mut self, line: &str, _report_errors: bool) -> String {
        // * `__LINE__` y `__FILE__`: las dos macros que NO son una entrada de
        // la tabla, porque su valor cambia en cada linea.
        //
        // Se sustituyen aqui, antes de la expansion normal, con el numero que
        // el preprocesador lleva puesto. Sin ellas, `Z_Malloc(size, tag, user)`
        // --que en `z_zone.h` se expande a `Z_MallocDebug(..., __FILE__,
        // __LINE__)`-- dejaba dos identificadores sueltos, y el error hablaba
        // de una variable `__LINE__` que nadie escribio.
        //
        // `__FILE__` sale como una cadena vacia y no como el nombre del
        // fichero, y se dice por que: el preprocesador expande linea a linea y
        // aqui no tiene la ruta a mano. Lo que se usa de verdad es el numero.
        let mut texto = if line.contains("__LINE__") || line.contains("__FILE__") {
            line.replace("__LINE__", &self.line.to_string())
                .replace("__FILE__", "\"\"")
        } else {
            line.to_string()
        };
        for _ in 0..MAX_PASADAS {
            let next = self.expandir_una_pasada(&texto);
            if next == texto {
                break;
            }
            texto = next;
        }
        // * Y OTRA VEZ AL SALIR, que es la vez que hacia falta.
        //
        // La sustitucion de arriba solo ve lo que ya estaba escrito en la
        // linea. Pero `__LINE__` casi nunca esta escrito ahi: llega DENTRO del
        // cuerpo de una macro --`Z_ChangeTag(p,t)` se expande a
        // `Z_ChangeTag2((p),(t),__FILE__,__LINE__)`-- o sea que aparece
        // despues de expandir, cuando la pasada de antes ya no puede verlo.
        //
        // Con las dos, da igual por donde entre.
        if texto.contains("__LINE__") || texto.contains("__FILE__") {
            texto = texto
                .replace("__LINE__", &self.line.to_string())
                .replace("__FILE__", "\"\"");
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
/// * AND a `//` INSIDE A STRING IS NOT A COMMENT.
///
/// This function used to cut at the first `//` it saw, wherever it was. So
///
/// ```c
/// #define PATH "http://x/y"
/// ```
///
/// stored `PATH` as `"http:` -- an unterminated string. What the user then
/// gets is *"'PATH' is not declared ... if it came from a #define, the header
/// did not expand"*, which sends them to look at the include chain. The macro
/// expanded perfectly; it was cut in half when it was stored.
///
/// Found by writing the test for the OTHER half of this rule, which is the
/// only reason it is not still in here.
fn sin_comentarios(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    // Which literal we are inside, if any: `"` or `'`.
    let mut comilla: Option<u8> = None;
    while i < b.len() {
        if let Some(q) = comilla {
            // A backslash escapes the next byte, the closing quote included:
            // without this, `"\""` ends one byte early and the rest of the
            // line is read as code.
            if b[i] == b'\\' && i + 1 < b.len() {
                out.push(b[i] as char);
                out.push(b[i + 1] as char);
                i += 2;
                continue;
            }
            if b[i] == q {
                comilla = None;
            }
            if b[i] < 0x80 {
                out.push(b[i] as char);
                i += 1;
            } else {
                let c = s[i..].chars().next().unwrap();
                i += c.len_utf8();
                out.push(c);
            }
            continue;
        }
        if b[i] == b'"' || b[i] == b'\'' {
            comilla = Some(b[i]);
            out.push(b[i] as char);
            i += 1;
            continue;
        }
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

/// One token of a `#if` expression: a number, an operator, or a parenthesis.
#[derive(Debug, PartialEq)]
enum IfTok {
    Num(i64),
    Op(&'static str),
    Open,
    Close,
}

/// Split a `#if` expression into tokens. `None` if a byte makes no sense here
/// -- by this point identifiers are already gone (see `zero_out_identifiers`),
/// so anything left that is not an operator is a genuine surprise.
fn lex_if_expr(expr: &str) -> Option<Vec<IfTok>> {
    // Two bytes before one, always: otherwise `<<` lexes as `<` `<` and
    // `a << 2` quietly becomes a comparison.
    const OPS2: [&str; 8] = ["==", "!=", "<=", ">=", "&&", "||", "<<", ">>"];
    const OPS1: [&str; 12] = ["!", "~", "<", ">", "+", "-", "*", "/", "%", "&", "|", "^"];

    let b = expr.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'(' {
            out.push(IfTok::Open);
            i += 1;
        } else if c == b')' {
            out.push(IfTok::Close);
            i += 1;
        } else if c.is_ascii_digit() {
            let start = i;
            while i < b.len() && (is_ident_char(b[i]) || b[i] == b'x' || b[i] == b'X') {
                i += 1;
            }
            out.push(IfTok::Num(parse_int_literal(&expr[start..i])?));
        } else {
            let dos = if i + 1 < b.len() { &expr[i..i + 2] } else { "" };
            if let Some(op) = OPS2.iter().find(|o| **o == dos) {
                out.push(IfTok::Op(op));
                i += 2;
            } else if let Some(op) = OPS1.iter().find(|o| o.as_bytes()[0] == c) {
                out.push(IfTok::Op(op));
                i += 1;
            } else {
                return None;
            }
        }
    }
    Some(out)
}

struct IfExprParser<'a> {
    t: &'a [IfTok],
    i: usize,
}

impl IfExprParser<'_> {
    /// Binding power of a binary operator, lowest first. The order is C's.
    fn bp(op: &str) -> Option<u8> {
        Some(match op {
            "||" => 1,
            "&&" => 2,
            "|" => 3,
            "^" => 4,
            "&" => 5,
            "==" | "!=" => 6,
            "<" | ">" | "<=" | ">=" => 7,
            "<<" | ">>" => 8,
            "+" | "-" => 9,
            "*" | "/" | "%" => 10,
            _ => return None,
        })
    }

    fn expr(&mut self, min_bp: u8) -> Option<i64> {
        let mut left = self.unary()?;
        while let Some(IfTok::Op(op)) = self.t.get(self.i) {
            let bp = match Self::bp(op) {
                Some(bp) if bp >= min_bp => bp,
                _ => break,
            };
            let op = *op;
            self.i += 1;
            // Left-associative: the right side stops at the same power.
            let right = self.expr(bp + 1)?;
            left = match op {
                "||" => (left != 0 || right != 0) as i64,
                "&&" => (left != 0 && right != 0) as i64,
                "|" => left | right,
                "^" => left ^ right,
                "&" => left & right,
                "==" => (left == right) as i64,
                "!=" => (left != right) as i64,
                "<" => (left < right) as i64,
                ">" => (left > right) as i64,
                "<=" => (left <= right) as i64,
                ">=" => (left >= right) as i64,
                "<<" => left.wrapping_shl(right as u32),
                ">>" => left.wrapping_shr(right as u32),
                "+" => left.wrapping_add(right),
                "-" => left.wrapping_sub(right),
                "*" => left.wrapping_mul(right),
                // A division by zero in a `#if` is not a branch: it is a
                // broken expression, and it says so instead of picking one.
                "/" => { if right == 0 { return None; } left / right }
                "%" => { if right == 0 { return None; } left % right }
                _ => return None,
            };
        }
        Some(left)
    }

    fn unary(&mut self) -> Option<i64> {
        match self.t.get(self.i)? {
            IfTok::Num(n) => {
                let n = *n;
                self.i += 1;
                Some(n)
            }
            IfTok::Open => {
                self.i += 1;
                let v = self.expr(0)?;
                if self.t.get(self.i) != Some(&IfTok::Close) {
                    return None;
                }
                self.i += 1;
                Some(v)
            }
            IfTok::Op("!") => { self.i += 1; Some((self.unary()? == 0) as i64) }
            IfTok::Op("~") => { self.i += 1; Some(!self.unary()?) }
            IfTok::Op("-") => { self.i += 1; Some(self.unary()?.wrapping_neg()) }
            IfTok::Op("+") => { self.i += 1; self.unary() }
            _ => None,
        }
    }
}

/// An integer constant the way `#if` writes one: decimal or `0x`, with the
/// `U`/`L` suffixes C allows.
///
/// `str::parse` handles none of those, and the failure is silent in the worst
/// way: `#if (FLAGS & 0x10)` reports "cannot evaluate" and points at the whole
/// expression, so the suffix -- the actual cause -- is the one thing the
/// message does not name.
fn parse_int_literal(s: &str) -> Option<i64> {
    let s = s.trim();
    let cuerpo = s.trim_end_matches(['u', 'U', 'l', 'L']);
    if cuerpo.is_empty() {
        return None;
    }
    if let Some(hex) = cuerpo.strip_prefix("0x").or_else(|| cuerpo.strip_prefix("0X")) {
        return i64::from_str_radix(hex, 16).ok();
    }
    cuerpo.parse::<i64>().ok()
}

/// Replace every surviving identifier in a `#if` expression with `0` (C11
/// 6.10.1p4).
///
/// A number that merely starts with a letter is not an identifier: `0x10` is a
/// hex constant and `1UL` is a suffixed one, so a run is only zeroed when its
/// FIRST byte cannot start a number.
fn zero_out_identifiers(expr: &str) -> String {
    let b = expr.as_bytes();
    let mut out = String::with_capacity(expr.len());
    let mut i = 0usize;
    while i < b.len() {
        if is_ident_char(b[i]) {
            let start = i;
            while i < b.len() && is_ident_char(b[i]) {
                i += 1;
            }
            if b[start].is_ascii_digit() {
                out.push_str(&expr[start..i]);
            } else {
                out.push('0');
            }
        } else {
            out.push(b[i] as char);
            i += 1;
        }
    }
    out
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

/// Cuenta los parentesis de una linea, sin mirar dentro de las cadenas.
///
/// Positivo = quedan abiertos. Es lo que decide si hay que traerse la linea
/// siguiente para completar una invocacion de macro. Un `(` entre comillas es
/// texto y no gramatica, y contarlo partiria justo las lineas que llevan un
/// parentesis en un mensaje.
fn balance_parentesis(linea: &str) -> i32 {
    let b = linea.as_bytes();
    let mut n = 0i32;
    let mut i = 0usize;
    let mut comilla: Option<u8> = None;
    while i < b.len() {
        let c = b[i];
        match comilla {
            Some(q) => {
                if c == b'\\' && i + 1 < b.len() {
                    i += 2;
                    continue;
                }
                if c == q {
                    comilla = None;
                }
                i += 1;
            }
            None => {
                match c {
                    b'"' | b'\'' => comilla = Some(c),
                    b'(' => n += 1,
                    b')' => n -= 1,
                    _ => {}
                }
                i += 1;
            }
        }
    }
    n
}

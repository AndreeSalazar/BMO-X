//! Lexer de COBOL — Source → Tokens. La base del pipeline.
//!
//! Convierte el texto COBOL en un flujo de tokens con número de línea. Usa
//! las tablas GENERADAS por la fábrica Python (`generated::words`) para
//! clasificar palabras reservadas/verbos/intrínsecas — así el vocabulario
//! crece solo al ampliar `definition.py`.
//!
//! Sutileza clave de COBOL: el punto `.` es un TERMINADOR de sentencia, pero
//! también el punto decimal de `10.05`. Regla: un `.` entre dígitos es
//! decimal; un `.` seguido de espacio/fin es terminador.

use crate::generated::words;

/// Clase léxica de un token.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    /// Palabra reservada de COBOL (MAYÚSCULAS canónicas).
    Keyword(String),
    /// Nombre de dato / párrafo / identificador de usuario.
    Ident(String),
    /// Literal numérico, PRESERVADO como texto para no perder la escala
    /// decimal ("10.05", "007", "3.20").
    Number(String),
    /// Literal de cadena (sin las comillas).
    Str(String),
    /// `.` terminador de sentencia.
    Period,
    Comma,
    LParen,
    RParen,
    /// `=` (COMPUTE, condiciones).
    Equal,
    /// Cualquier otro carácter suelto (`+ - * / < >` …).
    Punct(char),
}

/// Token con su posición.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

impl Token {
    /// ¿Es la palabra reservada `kw`?
    pub fn is_keyword(&self, kw: &str) -> bool {
        matches!(&self.tok, Tok::Keyword(w) if w == kw)
    }
}

fn is_word_start(c: char) -> bool {
    c.is_ascii_alphabetic()
}
fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

/// Tokeniza el fuente COBOL completo.
pub fn lex(source: &str) -> Vec<Token> {
    let mut out = Vec::new();
    for (i, raw_line) in source.lines().enumerate() {
        let line_no = i + 1;
        lex_line(raw_line, line_no, &mut out);
    }
    out
}

fn lex_line(line: &str, line_no: usize, out: &mut Vec<Token>) {
    let chars: Vec<char> = line.chars().collect();
    let n = chars.len();
    let mut i = 0;

    // Comentario de línea entera (formato fijo: '*' o '/' en col 7 → aquí
    // simplificado a línea que empieza por '*').
    if let Some(first) = line.trim_start().chars().next() {
        if first == '*' {
            return;
        }
    }

    while i < n {
        let c = chars[i];

        // Espacios.
        if c.is_whitespace() {
            i += 1;
            continue;
        }

        // Comentario libre '*>' hasta fin de línea.
        if c == '*' && i + 1 < n && chars[i + 1] == '>' {
            return;
        }

        // Palabra (reservada / ident).
        if is_word_start(c) {
            let start = i;
            while i < n && is_word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let upper = word.to_ascii_uppercase();
            if words::is_reserved(&upper) {
                out.push(Token { tok: Tok::Keyword(upper), line: line_no });
            } else {
                out.push(Token { tok: Tok::Ident(word), line: line_no });
            }
            continue;
        }

        // Número (con posible punto DECIMAL, no terminador).
        if c.is_ascii_digit() {
            let start = i;
            while i < n && chars[i].is_ascii_digit() {
                i += 1;
            }
            // Punto decimal solo si le sigue un dígito.
            if i + 1 < n && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
                i += 1; // el '.'
                while i < n && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            let num: String = chars[start..i].iter().collect();
            out.push(Token { tok: Tok::Number(num), line: line_no });
            continue;
        }

        // Literal de cadena.
        if c == '"' || c == '\'' {
            let quote = c;
            i += 1;
            let start = i;
            while i < n && chars[i] != quote {
                i += 1;
            }
            let s: String = chars[start..i].iter().collect();
            if i < n {
                i += 1; // comilla de cierre
            }
            out.push(Token { tok: Tok::Str(s), line: line_no });
            continue;
        }

        // Puntuación.
        let t = match c {
            '.' => Tok::Period,
            ',' => Tok::Comma,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '=' => Tok::Equal,
            other => Tok::Punct(other),
        };
        out.push(Token { tok: t, line: line_no });
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_keywords_idents_and_decimal() {
        let toks = lex("MOVE 10.05 TO PRECIO.");
        let kinds: Vec<&Tok> = toks.iter().map(|t| &t.tok).collect();
        assert_eq!(kinds[0], &Tok::Keyword("MOVE".into()));
        assert_eq!(kinds[1], &Tok::Number("10.05".into())); // decimal, no terminador
        assert_eq!(kinds[2], &Tok::Keyword("TO".into()));
        assert_eq!(kinds[3], &Tok::Ident("PRECIO".into()));
        assert_eq!(kinds[4], &Tok::Period); // ESTE punto sí es terminador
    }

    #[test]
    fn string_and_pic_like() {
        let toks = lex("DISPLAY \"hola mundo\".");
        assert_eq!(toks[0].tok, Tok::Keyword("DISPLAY".into()));
        assert_eq!(toks[1].tok, Tok::Str("hola mundo".into()));
        assert_eq!(toks[2].tok, Tok::Period);
    }

    #[test]
    fn skips_comments_and_tracks_lines() {
        let toks = lex("* comentario\nADD 1 TO X.");
        // La línea de comentario no produce tokens; ADD está en la línea 2.
        assert_eq!(toks[0].tok, Tok::Keyword("ADD".into()));
        assert_eq!(toks[0].line, 2);
    }

    #[test]
    fn level_number_is_a_number() {
        let toks = lex("01 SALDO PIC 9(5)V99.");
        assert_eq!(toks[0].tok, Tok::Number("01".into()));
        assert_eq!(toks[1].tok, Tok::Ident("SALDO".into()));
        assert_eq!(toks[2].tok, Tok::Keyword("PIC".into()));
    }
}

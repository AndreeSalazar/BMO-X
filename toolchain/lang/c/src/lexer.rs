//! Lexer de BMO C -- Source a Tokens (con linea real por token).

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Ident(String), IntLit(i64), FloatLit(f64), StringLit(String), CharLit(u8),
    Int, Void, Char, Short, Long, Unsigned, Signed,
    If, Else, While, Do, For, Switch, Case, Default, Break, Continue,
    Float, Double,
    Return, Sizeof, Struct, Union, Typedef, Enum, Goto, Use,
    Const, Volatile, Extern,
    /// `static`. Ver `parser::mod` -- significa DOS cosas distintas segun donde
    /// este, y esa es la mitad del trabajo de implementarla.
    Static,
    OpenParen, CloseParen, OpenBrace, CloseBrace, OpenBracket, CloseBracket,
    Semicolon, Comma, Colon, Question,
    /// `#` -- una directiva del preprocesador.
    ///
    /// Tiene token PROPIO aunque no haya preprocesador, y ahi esta el motivo:
    /// el catch-all del lexer se tragaba cualquier caracter desconocido, asi
    /// que un `#define X 5` dentro de una funcion **compilaba y se ignoraba en
    /// silencio**. Al principio del fichero daba un "expected type, got
    /// Ident(define)", que manda a mirar donde no es. Con token propio, el
    /// analisis puede decir la verdad: aqui no hay preprocesador todavia.
    Hash,
    Plus, Minus, Star, Slash, Percent,
    PlusPlus, MinusMinus,
    EqEq, Neq, Lt, Gt, Le, Ge,
    And, Or, Xor, Not, Tilde,
    LAnd, LOr,
    Shl, Shr,
    Arrow, Dot,
    /// `...` -- el resto de los argumentos.
    Puntos,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    ShlAssign, ShrAssign, AndAssign, XorAssign, OrAssign,
    Eof,
}

/// Vec de tokens que registra la LINEA de cada uno (para errores con linea real).
struct TokStream {
    toks: Vec<Token>,
    lines: Vec<usize>,
    cur_line: usize,
    /// Lo que salio mal AQUI, con su linea. El lexer no puede cortar --tiene
    /// que seguir hasta el final para que el parser reciba un vector-- asi que
    /// los guarda y el parser se niega a seguir al verlos. Sin esto, la unica
    /// salida era un valor inventado.
    errores: Vec<crate::CError>,
}

impl TokStream {
    fn push(&mut self, tk: Token) {
        self.toks.push(tk);
        self.lines.push(self.cur_line);
    }
}

pub(crate) fn tokenize(source: &str) -> (Vec<Token>, Vec<usize>, Vec<crate::CError>) {
    let mut t = TokStream { toks: Vec::new(), lines: Vec::new(), cur_line: 1, errores: Vec::new() };
    let c: Vec<char> = source.chars().collect();
    let mut i = 0;
    while i < c.len() {
        if c[i].is_whitespace() { if c[i] == '\n' { t.cur_line += 1; } i += 1; continue; }
        if c[i] == '/' && i + 1 < c.len() {
            if c[i+1] == '/' { while i < c.len() && c[i] != '\n' { i += 1; } continue; }
            if c[i+1] == '*' { i += 2; while i + 1 < c.len() && !(c[i] == '*' && c[i+1] == '/') { if c[i] == '\n' { t.cur_line += 1; } i += 1; } i += 2; continue; }
        }
        match c[i] {
            '(' => { t.push(Token::OpenParen); i += 1; }
            ')' => { t.push(Token::CloseParen); i += 1; }
            '{' => { t.push(Token::OpenBrace); i += 1; }
            '}' => { t.push(Token::CloseBrace); i += 1; }
            '[' => { t.push(Token::OpenBracket); i += 1; }
            ']' => { t.push(Token::CloseBracket); i += 1; }
            ';' => { t.push(Token::Semicolon); i += 1; }
            ',' => { t.push(Token::Comma); i += 1; }
            '?' => { t.push(Token::Question); i += 1; }
            ':' => { t.push(Token::Colon); i += 1; }
            '~' => { t.push(Token::Tilde); i += 1; }
            '+' => {
                if i + 1 < c.len() && c[i+1] == '+' { t.push(Token::PlusPlus); i += 2; }
                else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::AddAssign); i += 2; }
                else { t.push(Token::Plus); i += 1; }
            }
            '-' => {
                if i + 1 < c.len() && c[i+1] == '-' { t.push(Token::MinusMinus); i += 2; }
                else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::SubAssign); i += 2; }
                else if i + 1 < c.len() && c[i+1] == '>' { t.push(Token::Arrow); i += 2; }
                else { t.push(Token::Minus); i += 1; }
            }
            '*' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::MulAssign); i += 2; } else { t.push(Token::Star); i += 1; }
            }
            '/' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::DivAssign); i += 2; } else { t.push(Token::Slash); i += 1; }
            }
            '%' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::ModAssign); i += 2; } else { t.push(Token::Percent); i += 1; }
            }
            '&' => {
                if i + 1 < c.len() && c[i+1] == '&' { t.push(Token::LAnd); i += 2; }
                else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::AndAssign); i += 2; }
                else { t.push(Token::And); i += 1; }
            }
            '|' => {
                if i + 1 < c.len() && c[i+1] == '|' { t.push(Token::LOr); i += 2; }
                else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::OrAssign); i += 2; }
                else { t.push(Token::Or); i += 1; }
            }
            '^' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::XorAssign); i += 2; } else { t.push(Token::Xor); i += 1; }
            }
            '!' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::Neq); i += 2; } else { t.push(Token::Not); i += 1; }
            }
            '<' => {
                if i + 1 < c.len() && c[i+1] == '<' {
                    if i + 2 < c.len() && c[i+2] == '=' { t.push(Token::ShlAssign); i += 3; }
                    else { t.push(Token::Shl); i += 2; }
                } else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::Le); i += 2; }
                else { t.push(Token::Lt); i += 1; }
            }
            '>' => {
                if i + 1 < c.len() && c[i+1] == '>' {
                    if i + 2 < c.len() && c[i+2] == '=' { t.push(Token::ShrAssign); i += 3; }
                    else { t.push(Token::Shr); i += 2; }
                } else if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::Ge); i += 2; }
                else { t.push(Token::Gt); i += 1; }
            }
            // `...` antes que `.`: el mas largo primero, o `...` saldria como
            // tres accesos a campo y el error hablaria de un campo sin nombre.
            '.' => {
                if i + 2 < c.len() && c[i+1] == '.' && c[i+2] == '.' {
                    t.push(Token::Puntos); i += 3;
                } else { t.push(Token::Dot); i += 1; }
            }
            '=' => {
                if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::EqEq); i += 2; } else { t.push(Token::Assign); i += 1; }
            }
            '"' => {
                i += 1; let mut s = String::new();
                while i < c.len() && c[i] != '"' {
                    if c[i] == '\\' && i + 1 < c.len() { i += 1;
                        match c[i] {
                            'n' => s.push('\n'), 't' => s.push('\t'), 'r' => s.push('\r'), '0' => s.push('\0'),
                            '\\' => s.push('\\'), '"' => s.push('"'), '\'' => s.push('\''),
                            'x' | 'X' => {
                                let mut hex = String::new();
                                while i + 1 < c.len() && c[i+1].is_ascii_hexdigit() { i += 1; hex.push(c[i]); }
                                if let Ok(v) = u8::from_str_radix(&hex, 16) { s.push(v as char); }
                            }
                            d if d.is_ascii_digit() && d != '8' && d != '9' => {
                                let mut oct = String::new(); oct.push(d);
                                for _ in 0..2 {
                                    if i + 1 < c.len() && c[i+1] >= '0' && c[i+1] <= '7' { i += 1; oct.push(c[i]); } else { break; }
                                }
                                if let Ok(v) = u8::from_str_radix(&oct, 8) { s.push(v as char); }
                            }
                            x => { s.push('\\'); s.push(x); }
                        }
                    } else { s.push(c[i]); } i += 1;
                } i += 1;
                // string literal concatenation: "foo" "bar" -> "foobar"
                let mut combined = s;
                while i < c.len() && (c[i] == ' ' || c[i] == '\t' || c[i] == '\n' || c[i] == '\r') { i += 1; }
                while i < c.len() && c[i] == '"' {
                    i += 1;
                    while i < c.len() && c[i] != '"' {
                        if c[i] == '\\' && i + 1 < c.len() { i += 1;
                            match c[i] {
                                'n' => combined.push('\n'), 't' => combined.push('\t'), 'r' => combined.push('\r'), '0' => combined.push('\0'),
                                '\\' => combined.push('\\'), '"' => combined.push('"'), '\'' => combined.push('\''),
                                'x' | 'X' => {
                                    let mut hex = String::new();
                                    while i + 1 < c.len() && c[i+1].is_ascii_hexdigit() { i += 1; hex.push(c[i]); }
                                    if let Ok(v) = u8::from_str_radix(&hex, 16) { combined.push(v as char); }
                                }
                                d if d.is_ascii_digit() && d != '8' && d != '9' => {
                                    let mut oct = String::new(); oct.push(d);
                                    for _ in 0..2 {
                                        if i + 1 < c.len() && c[i+1] >= '0' && c[i+1] <= '7' { i += 1; oct.push(c[i]); } else { break; }
                                    }
                                    if let Ok(v) = u8::from_str_radix(&oct, 8) { combined.push(v as char); }
                                }
                                x => { combined.push('\\'); combined.push(x); }
                            }
                        } else { combined.push(c[i]); } i += 1;
                    } i += 1;
                    // skip whitespace between strings
                    while i < c.len() && (c[i] == ' ' || c[i] == '\t' || c[i] == '\n' || c[i] == '\r') { i += 1; }
                }
                t.push(Token::StringLit(combined));
            }
            '\'' => {
                i += 1; let val = if c[i] == '\\' { i += 1;
                    match c[i] {
                        'n' => 10, 't' => 9, 'r' => 13, '0' => 0, '\\' => 92, '\'' => 39,
                        'x' | 'X' => {
                            let mut hex = String::new();
                            while i + 1 < c.len() && c[i+1].is_ascii_hexdigit() { i += 1; hex.push(c[i]); }
                            u8::from_str_radix(&hex, 16).unwrap_or(0)
                        }
                        d if d.is_ascii_digit() && d != '8' && d != '9' => {
                            let mut oct = String::new(); oct.push(d);
                            for _ in 0..2 {
                                if i + 1 < c.len() && c[i+1] >= '0' && c[i+1] <= '7' { i += 1; oct.push(c[i]); } else { break; }
                            }
                            u8::from_str_radix(&oct, 8).unwrap_or(0)
                        }
                        x => x as u8,
                    }
                } else { c[i] as u8 };
                i += 1; if i < c.len() && c[i] == '\'' { i += 1; } t.push(Token::CharLit(val));
            }
            d if d.is_ascii_digit() => {
                let mut n = String::new();
                if d == '0' && i + 1 < c.len() && (c[i+1] == 'x' || c[i+1] == 'X') {
                    n.push_str("0x"); i += 2;
                    while i < c.len() && c[i].is_ascii_hexdigit() { n.push(c[i]); i += 1; }
                    // * Por `u64` y no por `i64`. Un hexadecimal es un PATRON DE
                    // BITS, no un numero con signo: `0xFFFFFFFFFFFFFFFE` no cabe
                    // en un `i64` y `i64::from_str_radix` fallaba, asi que el
                    // `unwrap_or(0)` lo convertia en **cero, en silencio**.
                    //
                    // Eso dejaba fuera del lenguaje toda la mitad alta de 64
                    // bits -- empezando por `CURRENT_TASK` (0xFF..FE), que es el
                    // pseudo-handle con el que un programa se nombra a si mismo.
                    // Escribir la constante correcta compilaba y llamaba a la
                    // capability 0.
                    let bits = u64::from_str_radix(&n[2..], 16)
                        .or_else(|_| i64::from_str_radix(&n[2..], 16).map(|v| v as u64));
                    match bits {
                        Ok(v) => t.push(Token::IntLit(v as i64)),
                        // Mas de 16 digitos no es un entero de esta maquina.
                        // Callarlo seria repetir el mismo error con otro valor.
                        Err(_) => {
                            let linea = t.cur_line;
                            t.errores.push(crate::CError::new(
                                linea,
                                format!("literal hexadecimal fuera de 64 bits: {n}"),
                            ));
                            t.push(Token::IntLit(0));
                        }
                    }
                } else {
                    while i < c.len() && c[i].is_ascii_digit() { n.push(c[i]); i += 1; }
                    // literal float: 1.5, 3.14f -- antes "1.5" se partia en 1 . 5
                    if i + 1 < c.len() && c[i] == '.' && c[i+1].is_ascii_digit() {
                        n.push('.'); i += 1;
                        while i < c.len() && c[i].is_ascii_digit() { n.push(c[i]); i += 1; }
                        if i < c.len() && (c[i] == 'f' || c[i] == 'F') { i += 1; }
                        t.push(Token::FloatLit(n.parse().unwrap_or(0.0)));
                        continue;
                    }
                    t.push(Token::IntLit(n.parse().unwrap_or(0)));
                }
                // sufijos de literal entero: U, L, UL, LL, ULL (cualquier orden/caso)
                while i < c.len() && matches!(c[i], 'u' | 'U' | 'l' | 'L') { i += 1; }
            }
            l if l.is_ascii_alphabetic() || l == '_' => {
                let mut id = String::new();
                while i < c.len() && (c[i].is_ascii_alphanumeric() || c[i] == '_') { id.push(c[i]); i += 1; }
                match id.as_str() {
                    "int" => t.push(Token::Int), "void" => t.push(Token::Void),
                    "char" => t.push(Token::Char), "short" => t.push(Token::Short),
                    "long" => t.push(Token::Long), "unsigned" => t.push(Token::Unsigned),
                    "signed" => t.push(Token::Signed),
                    "if" => t.push(Token::If), "else" => t.push(Token::Else),
                    "while" => t.push(Token::While), "do" => t.push(Token::Do),
                    "for" => t.push(Token::For), "switch" => t.push(Token::Switch),
                    "case" => t.push(Token::Case), "default" => t.push(Token::Default),
                    "break" => t.push(Token::Break), "continue" => t.push(Token::Continue),
                    "return" => t.push(Token::Return), "sizeof" => t.push(Token::Sizeof),
                    "goto" => t.push(Token::Goto),
                    "use" => t.push(Token::Use),
                    "const" => t.push(Token::Const),
                    "volatile" => t.push(Token::Volatile),
                    "extern" => t.push(Token::Extern),
                    "static" => t.push(Token::Static),
                    // `auto` y `register` se ACEPTAN Y SE TIRAN. No es pereza:
                    // `register` es una sugerencia que todos los compiladores
                    // del mundo ignoran desde hace treinta anos, y `auto` es
                    // redundante desde 1978 (una local ya es automatica). No
                    // cambian lo que el programa HACE, asi que emitir algo por
                    // ellas seria emitir ruido. Se comen aqui para que el
                    // codigo ajeno que las trae compile sin tocarlo.
                    "auto" | "register" => {}
                    "float" => t.push(Token::Float),
                    "double" => t.push(Token::Double),
                    "struct" => t.push(Token::Struct), "union" => t.push(Token::Union), "typedef" => t.push(Token::Typedef),
                    "enum" => t.push(Token::Enum),
                    _ => t.push(Token::Ident(id)),
                }
            }
            '#' => { t.push(Token::Hash); i += 1; }
            _ => { i += 1; }
        }
    }
    t.push(Token::Eof);
    (t.toks, t.lines, t.errores)
}

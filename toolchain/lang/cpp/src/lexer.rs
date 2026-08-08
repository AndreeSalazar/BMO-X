//! **Lexer de BMO C++** -- fuente a tokens, con la linea real de cada uno.
//!
//! === Por que esto existe (y que reemplaza) ===
//!
//! El frontend anterior no tenia lexer: el parser miraba la fuente caracter a
//! caracter con `peek_str("return")`. Dos consecuencias que costaron caro:
//!
//! 1. **El identificador `x` se leia como un numero.** El bucle de digitos
//!    aceptaba `x`, `X` y las letras `a`-`f` para poder leer hexadecimales,
//!    asi que `x * 2` entraba por la rama numerica antes de ser un nombre.
//! 2. **`returnx` empezaba por `return`.** Sin tokens no hay frontera de
//!    palabra, y `peek_str` no la puede inventar.
//!
//! Aqui un numero es un numero y una palabra clave es una palabra **entera**.
//!
//! === El lexer no corta ===
//!
//! Igual que el de C: cuando algo no se puede leer, se **anota el error y se
//! sigue**, porque el parser necesita recibir un vector completo. Cortar en el
//! primer caracter raro obligaria a devolver un token inventado, y un token
//! inventado se propaga hasta un mensaje que manda a mirar donde no es.

use crate::CppError;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // -- Literales y nombres --
    Ident(String), IntLit(i64), FloatLit(f64), StringLit(String), CharLit(u8),

    // -- Tipos --
    Void, Bool, Char, Short, Int, Long, Float, Double, Unsigned, Signed, Auto,

    // -- Control --
    If, Else, While, Do, For, Switch, Case, Default, Break, Continue, Return,

    // -- C++ --
    Class, Struct, Namespace, Template, Typename,
    Public, Private, Protected, Virtual, Override,
    New, Delete, This, Nullptr, True, False, Operator, Friend, Using, Enum,
    Const, Static, Sizeof,

    // -- Signos --
    OpenParen, CloseParen, OpenBrace, CloseBrace, OpenBracket, CloseBracket,
    Semicolon, Comma, Colon, Question, Tilde,
    /// `::` -- el que hace que un nombre de C++ no sea un nombre de C.
    ColonColon,
    Dot, Arrow,
    Plus, Minus, Star, Slash, Percent,
    PlusPlus, MinusMinus,
    EqEq, Neq, Lt, Gt, Le, Ge,
    And, Or, Xor, Not,
    LAnd, LOr,
    Shl, Shr,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    ShlAssign, ShrAssign, AndAssign, XorAssign, OrAssign,
    /// `...`
    Puntos,
    /// `#` -- una directiva del preprocesador.
    ///
    /// Token propio **aunque no haya preprocesador todavia**, por la misma
    /// razon que en C: sin el, el catch-all se traga el `#` y un `#define`
    /// dentro de una funcion compila y se ignora en silencio. Con token
    /// propio, el parser puede decir la verdad -- *aqui todavia no hay
    /// preprocesador, llega en la segunda mitad del paso 1*.
    Hash,
    Eof,
}

impl Token {
    /// La palabra clave que corresponde a un identificador, si lo es.
    ///
    /// Va en una tabla y no en una cadena de `if`s **a proposito**: anadir una
    /// palabra al lenguaje es anadir una fila. Es la misma regla que hace que
    /// un intrinseco de C sea una fila de `intrinsics.toml`.
    fn palabra_clave(s: &str) -> Option<Token> {
        Some(match s {
            "void" => Token::Void, "bool" => Token::Bool, "char" => Token::Char,
            "short" => Token::Short, "int" => Token::Int, "long" => Token::Long,
            "float" => Token::Float, "double" => Token::Double,
            "unsigned" => Token::Unsigned, "signed" => Token::Signed,
            "auto" => Token::Auto,
            "if" => Token::If, "else" => Token::Else, "while" => Token::While,
            "do" => Token::Do, "for" => Token::For, "switch" => Token::Switch,
            "case" => Token::Case, "default" => Token::Default,
            "break" => Token::Break, "continue" => Token::Continue,
            "return" => Token::Return,
            "class" => Token::Class, "struct" => Token::Struct,
            "namespace" => Token::Namespace, "template" => Token::Template,
            "typename" => Token::Typename,
            "public" => Token::Public, "private" => Token::Private,
            "protected" => Token::Protected, "virtual" => Token::Virtual,
            "override" => Token::Override,
            "new" => Token::New, "delete" => Token::Delete, "this" => Token::This,
            "nullptr" => Token::Nullptr, "true" => Token::True, "false" => Token::False,
            "operator" => Token::Operator, "friend" => Token::Friend,
            "using" => Token::Using, "enum" => Token::Enum,
            "const" => Token::Const, "static" => Token::Static, "sizeof" => Token::Sizeof,
            _ => return None,
        })
    }
}

pub struct Lexemas {
    pub toks: Vec<Token>,
    pub lineas: Vec<usize>,
    pub errores: Vec<CppError>,
}

pub fn tokenizar(fuente: &str) -> Lexemas {
    let c: Vec<char> = fuente.chars().collect();
    let mut out = Lexemas { toks: Vec::new(), lineas: Vec::new(), errores: Vec::new() };
    let mut i = 0usize;
    let mut linea = 1usize;

    macro_rules! empujar {
        ($t:expr, $n:expr) => {{ out.toks.push($t); out.lineas.push(linea); i += $n; }};
    }

    while i < c.len() {
        let ch = c[i];

        // -- Espacios y comentarios --
        if ch == '\n' { linea += 1; i += 1; continue; }
        if ch.is_whitespace() { i += 1; continue; }
        if ch == '/' && i + 1 < c.len() && c[i + 1] == '/' {
            while i < c.len() && c[i] != '\n' { i += 1; }
            continue;
        }
        if ch == '/' && i + 1 < c.len() && c[i + 1] == '*' {
            let abre = linea;
            i += 2;
            loop {
                if i + 1 >= c.len() {
                    out.errores.push(CppError::new(abre, "comentario /* sin cerrar"));
                    i = c.len();
                    break;
                }
                if c[i] == '*' && c[i + 1] == '/' { i += 2; break; }
                if c[i] == '\n' { linea += 1; }
                i += 1;
            }
            continue;
        }

        // -- Nombres y palabras clave --
        if ch.is_alphabetic() || ch == '_' {
            let ini = i;
            while i < c.len() && (c[i].is_alphanumeric() || c[i] == '_') { i += 1; }
            let s: String = c[ini..i].iter().collect();
            let tk = Token::palabra_clave(&s).unwrap_or(Token::Ident(s));
            out.toks.push(tk);
            out.lineas.push(linea);
            continue;
        }

        // -- Numeros --
        //
        // * La frontera de palabra es lo que arregla el bug de origen: un
        // numero empieza por un DIGITO. `x` ya se fue por la rama de arriba.
        if ch.is_ascii_digit() {
            let ini = i;
            let hex = ch == '0' && i + 1 < c.len() && (c[i + 1] == 'x' || c[i + 1] == 'X');
            if hex {
                i += 2;
                while i < c.len() && c[i].is_ascii_hexdigit() { i += 1; }
                let s: String = c[ini + 2..i].iter().collect();
                match i64::from_str_radix(&s, 16) {
                    Ok(v) => { out.toks.push(Token::IntLit(v)); out.lineas.push(linea); }
                    Err(_) => out.errores.push(CppError::new(linea, format!("hexadecimal ilegible: 0x{s}"))),
                }
                continue;
            }
            while i < c.len() && c[i].is_ascii_digit() { i += 1; }
            // Punto decimal: solo si detras viene un digito, para que `1..2`
            // o un `x.f` mal escrito no se coman el punto.
            let es_float = i < c.len() && c[i] == '.'
                && i + 1 < c.len() && c[i + 1].is_ascii_digit();
            if es_float {
                i += 1;
                while i < c.len() && c[i].is_ascii_digit() { i += 1; }
                let s: String = c[ini..i].iter().collect();
                match s.parse::<f64>() {
                    Ok(v) => { out.toks.push(Token::FloatLit(v)); out.lineas.push(linea); }
                    Err(_) => out.errores.push(CppError::new(linea, format!("numero ilegible: {s}"))),
                }
            } else {
                let s: String = c[ini..i].iter().collect();
                // Sufijos `u`, `l`, `ul`, `ll`... se leen y se tiran: no cambian
                // lo que el programa hace en un entero que ya cabe.
                while i < c.len() && matches!(c[i], 'u' | 'U' | 'l' | 'L') { i += 1; }
                match s.parse::<i64>() {
                    Ok(v) => { out.toks.push(Token::IntLit(v)); out.lineas.push(linea); }
                    Err(_) => out.errores.push(CppError::new(linea, format!("entero fuera de rango: {s}"))),
                }
            }
            continue;
        }

        // -- Cadenas --
        if ch == '"' {
            i += 1;
            let mut s = String::new();
            loop {
                if i >= c.len() {
                    out.errores.push(CppError::new(linea, "cadena sin cerrar"));
                    break;
                }
                match c[i] {
                    '"' => { i += 1; break; }
                    '\\' if i + 1 < c.len() => { s.push(escape(c[i + 1])); i += 2; }
                    '\n' => {
                        out.errores.push(CppError::new(linea, "salto de linea dentro de una cadena"));
                        linea += 1; i += 1;
                    }
                    otro => { s.push(otro); i += 1; }
                }
            }
            out.toks.push(Token::StringLit(s));
            out.lineas.push(linea);
            continue;
        }

        // -- Caracteres --
        if ch == '\'' {
            i += 1;
            let v = if i < c.len() && c[i] == '\\' && i + 1 < c.len() {
                let e = escape(c[i + 1]); i += 2; e as u8
            } else if i < c.len() {
                let e = c[i]; i += 1; e as u8
            } else {
                out.errores.push(CppError::new(linea, "literal de caracter sin cerrar"));
                0
            };
            if i < c.len() && c[i] == '\'' { i += 1; }
            else { out.errores.push(CppError::new(linea, "literal de caracter sin cerrar")); }
            out.toks.push(Token::CharLit(v));
            out.lineas.push(linea);
            continue;
        }

        // -- Signos: SIEMPRE de mas largo a mas corto --
        //
        // Si `<` se probara antes que `<<=`, un `x <<= 1` saldria como tres
        // tokens y el parser veria una comparacion. El orden de estas ramas
        // ES el desempate.
        let tres: String = c[i..(i + 3).min(c.len())].iter().collect();
        match tres.as_str() {
            "<<=" => { empujar!(Token::ShlAssign, 3); continue; }
            ">>=" => { empujar!(Token::ShrAssign, 3); continue; }
            "..." => { empujar!(Token::Puntos, 3); continue; }
            _ => {}
        }
        let dos: String = c[i..(i + 2).min(c.len())].iter().collect();
        match dos.as_str() {
            "::" => { empujar!(Token::ColonColon, 2); continue; }
            "->" => { empujar!(Token::Arrow, 2); continue; }
            "++" => { empujar!(Token::PlusPlus, 2); continue; }
            "--" => { empujar!(Token::MinusMinus, 2); continue; }
            "==" => { empujar!(Token::EqEq, 2); continue; }
            "!=" => { empujar!(Token::Neq, 2); continue; }
            "<=" => { empujar!(Token::Le, 2); continue; }
            ">=" => { empujar!(Token::Ge, 2); continue; }
            "&&" => { empujar!(Token::LAnd, 2); continue; }
            "||" => { empujar!(Token::LOr, 2); continue; }
            "<<" => { empujar!(Token::Shl, 2); continue; }
            ">>" => { empujar!(Token::Shr, 2); continue; }
            "+=" => { empujar!(Token::AddAssign, 2); continue; }
            "-=" => { empujar!(Token::SubAssign, 2); continue; }
            "*=" => { empujar!(Token::MulAssign, 2); continue; }
            "/=" => { empujar!(Token::DivAssign, 2); continue; }
            "%=" => { empujar!(Token::ModAssign, 2); continue; }
            "&=" => { empujar!(Token::AndAssign, 2); continue; }
            "^=" => { empujar!(Token::XorAssign, 2); continue; }
            "|=" => { empujar!(Token::OrAssign, 2); continue; }
            _ => {}
        }
        let uno = match ch {
            '(' => Token::OpenParen, ')' => Token::CloseParen,
            '{' => Token::OpenBrace, '}' => Token::CloseBrace,
            '[' => Token::OpenBracket, ']' => Token::CloseBracket,
            ';' => Token::Semicolon, ',' => Token::Comma,
            ':' => Token::Colon, '?' => Token::Question, '~' => Token::Tilde,
            '.' => Token::Dot,
            '+' => Token::Plus, '-' => Token::Minus, '*' => Token::Star,
            '/' => Token::Slash, '%' => Token::Percent,
            '<' => Token::Lt, '>' => Token::Gt,
            '&' => Token::And, '|' => Token::Or, '^' => Token::Xor,
            '!' => Token::Not, '=' => Token::Assign,
            '#' => Token::Hash,
            otro => {
                out.errores.push(CppError::new(linea, format!("caracter inesperado: {otro:?}")));
                i += 1;
                continue;
            }
        };
        empujar!(uno, 1);
    }

    out.toks.push(Token::Eof);
    out.lineas.push(linea);
    out
}

fn escape(c: char) -> char {
    match c {
        'n' => '\n', 't' => '\t', 'r' => '\r', '0' => '\0',
        '\\' => '\\', '\'' => '\'', '"' => '"',
        otro => otro,
    }
}

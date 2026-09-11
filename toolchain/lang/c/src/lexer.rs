//! Lexer de BMO C -- Source a Tokens (con linea real por token).
//!
//! [fase]     LEXICO
//!
//! [aparece]  AQUI -- un byte que no es un token es un error de compilacion,
//!            en la linea que lo trae
//!
//! [carril]   VERDE    -- si se rompe, ALGUIEN TE LO DICE antes de que salga de aqui
//!            * y sale de su `[aparece]`, no de una opinion: ver toolchain/tools/fases/
//!

/// **EL SUFIJO DE UN LITERAL ENTERO, que ES UN TIPO.**
///
/// # Por que existe, y desde cuando faltaba
///
/// Hasta el 2026-09-10 el lexer leia el sufijo y **lo tiraba**: una linea que
/// avanzaba el cursor y no guardaba nada. O sea que `1UL` y `1` eran el mismo
/// token, y por tanto el mismo TIPO: `int`.
///
/// ** Y de ahi salia un cero. `tipos.rs` decia que `1UL << 63` era una cuenta
/// de 32 bits, `recortar_a_32` metia su `mov eax,eax` detras, y la mascara se
/// perdia entera:
///
/// ```text
///    1UL << 31   ->  0xFFFFFFFF80000000   (recortado y extendido con signo)
///    1UL << 32   ->  0
///    1UL << 63   ->  0
///    a   << 63   ->  BIEN, porque en ejecucion nadie recorta
/// ```
///
/// *** `1 << n` es EL modismo de las mascaras. Toda mascara de 64 bits escrita
/// como literal salia CERO, sin un solo error: banderas, bits de pagina,
/// capabilities. Y el sintoma es una comprobacion que no comprueba.
///
/// # Por que un sufijo se desazucara a un CAST y no a un tipo en el nodo
///
/// Porque eso es lo que un sufijo ES: el estandar dice que `1UL` tiene tipo
/// `unsigned long`, ni mas ni menos. `Expr::Cast` ya existe y **los tres
/// jueces del compilador ya saben leerlo** --`tipo_de`, `expr_is_float`,
/// `expr_is_unsigned`--, asi que el arreglo no anade un caso a ninguno.
///
///   > Cuando la forma que sobra ya existe en el arbol, el arreglo no es
///   > escribir codigo: es dejar de tirar un dato.
#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum Suf {
    /// Sin sufijo: `int`.
    Ninguno,
    /// `U` -- `unsigned int`.
    U,
    /// `L` o `LL` -- `long`. Aqui los dos miden 64 bits.
    L,
    /// `UL`, `LU`, `ULL`... -- `unsigned long`.
    UL,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Token {
    Ident(String), IntLit(i64, Suf), FloatLit(f64), StringLit(String), CharLit(u8),
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

/// **Lee el sufijo de un literal entero y avanza el cursor.**
///
/// `U`, `L`, `UL`, `LU`, `LL`, `ULL`... en cualquier orden y cualquier caja,
/// que es lo que C permite. Se cuentan las letras y de ahi sale el tipo: si
/// hay alguna `u` es sin signo, si hay alguna `l` es de 64 bits.
///
/// * Va aparte y no repetida en las dos ramas del literal --hexadecimal y
/// decimal-- porque esas dos ramas ya divergieron una vez: el `unwrap_or(0)`
/// estaba arreglado en una y vivo en la otra. Dos copias de una regla son dos
/// sitios donde solo se arregla uno.
fn sufijo(c: &[char], i: &mut usize) -> Suf {
    let (mut u, mut l) = (false, false);
    while *i < c.len() && matches!(c[*i], 'u' | 'U' | 'l' | 'L') {
        if c[*i] == 'u' || c[*i] == 'U' { u = true; } else { l = true; }
        *i += 1;
    }
    match (u, l) {
        (false, false) => Suf::Ninguno,
        (true, false) => Suf::U,
        (false, true) => Suf::L,
        (true, true) => Suf::UL,
    }
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
                } else if i + 1 < c.len() && c[i+1].is_ascii_digit() {
                    // * `.867` -- un flotante SIN el cero de delante.
                    //
                    // C lo permite y `am_map.c` lo usa para las flechas del
                    // mapa: `(fixed_t)(-.867*(1<<16))`. Salia como un `Dot`
                    // suelto seguido de un entero, y el error --"unexpected
                    // token: Dot"-- acusaba a un punto que ahi es parte del
                    // NUMERO, no un acceso a campo.
                    //
                    // Va en la rama del punto y no en la del digito porque
                    // aqui es donde empieza: el numero no tiene primera cifra.
                    let mut n = String::from("0.");
                    i += 1;
                    while i < c.len() && c[i].is_ascii_digit() { n.push(c[i]); i += 1; }
                    if i < c.len() && (c[i] == 'f' || c[i] == 'F') { i += 1; }
                    t.push(Token::FloatLit(n.parse().unwrap_or(0.0)));
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
                    let suf = sufijo(&c, &mut i);
                    match bits {
                        Ok(v) => t.push(Token::IntLit(v as i64, suf)),
                        // Mas de 16 digitos no es un entero de esta maquina.
                        // Callarlo seria repetir el mismo error con otro valor.
                        Err(_) => {
                            let linea = t.cur_line;
                            t.errores.push(crate::CError::new(
                                linea,
                                format!("literal hexadecimal fuera de 64 bits: {n}"),
                            ));
                            t.push(Token::IntLit(0, suf));
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
                    let suf = sufijo(&c, &mut i);
                    // ** POR `i64` Y LUEGO POR `u64`, Y SI NO CABE SE DICE.
                    //
                    // *** Aqui ponia `n.parse().unwrap_or(0)`, y es EXACTAMENTE
                    // el fallo que la rama hexadecimal de doce lineas mas
                    // arriba ya tiene arreglado y comentado. La misma linea,
                    // el mismo fichero, una rama si y la otra no:
                    //
                    // ```text
                    //    18446744073709551615UL   ->  0, en silencio
                    //    0xFFFFFFFFFFFFFFFF       ->  bien desde hace semanas
                    // ```
                    //
                    // ** Un numero que el compilador no sabe representar y
                    // convierte en CERO es la peor respuesta posible: cero es
                    // un numero valido, asi que el programa sigue y la
                    // comprobacion que dependia de esa constante deja de
                    // comprobar. El dueno lo pidio con estas palabras: *si
                    // adivina, no lo convierte en BEX*.
                    let v = n.parse::<i64>().ok().or_else(|| n.parse::<u64>().ok().map(|x| x as i64));
                    match v {
                        Some(x) => t.push(Token::IntLit(x, suf)),
                        None => {
                            let linea = t.cur_line;
                            t.errores.push(crate::CError::new(
                                linea,
                                format!("literal entero fuera de 64 bits: {n}"),
                            ));
                            t.push(Token::IntLit(0, suf));
                        }
                    }
                }
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
                    // del mundo ignoran desde hace treinta anios, y `auto` es
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

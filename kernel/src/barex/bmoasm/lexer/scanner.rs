//! Scanner DFA real — recorre source byte-por-byte y emite `Token`s.
//!
//! Reconoce: keywords (90+), identifiers, decimal/hex/binary literals,
//! comentarios `// ...`, delimitadores estructurales. Single-pass, sin
//! lookahead más allá de 2 caracteres (`->`, `0x`).

use super::token::{Token, TokenKind};

/// Tabla `(bytes_lexema, TokenKind)` ordenada por longitud descendente
/// (greedy match). Cualquier keyword futuro se agrega aquí sin tocar
/// el lexer.
pub const KEYWORDS: &[(&[u8], TokenKind)] = &[
    // ── 10 letras ────────────────────────────────────────────────────
    (b"intrinseco",  TokenKind::KwIntrinseco),
    // ── 8 letras ─────────────────────────────────────────────────────
    (b"mientras",    TokenKind::KwMientras),
    (b"continua",    TokenKind::KwContinua),
    (b"etiqueta",    TokenKind::KwEtiqueta),
    (b"paralelo",    TokenKind::KwParalelo),
    // ── 7 letras ─────────────────────────────────────────────────────
    (b"retorna",     TokenKind::KwRetorna),
    (b"ventana",     TokenKind::KwVentana),
    (b"defecto",     TokenKind::KwDefecto),
    (b"acquire",     TokenKind::KwAcquire),
    (b"release",     TokenKind::KwRelease),
    (b"atomico",     TokenKind::KwAtomico),
    (b"volatil",     TokenKind::KwVolatil),
    (b"repetir",     TokenKind::KwRepetir),
    (b"incluye",     TokenKind::KwIncluye),
    (b"syscall",     TokenKind::KwSyscall),
    (b"lfence",      TokenKind::KwLfence),
    (b"mfence",      TokenKind::KwMfence),
    (b"sfence",      TokenKind::KwSfence),
    // ── 6 letras ─────────────────────────────────────────────────────
    (b"dibuja",      TokenKind::KwDibuja),
    (b"evento",      TokenKind::KwEvento),
    (b"cuando",      TokenKind::KwCuando),
    (b"sincro",      TokenKind::KwSincro),
    (b"seccion",     TokenKind::KwSeccion),
    // ── 5 letras ─────────────────────────────────────────────────────
    (b"libre",       TokenKind::KwLibre),
    (b"mayor",       TokenKind::OpMayor),
    (b"menor",       TokenKind::OpMenor),
    (b"igual",       TokenKind::OpIgual),
    (b"rompe",       TokenKind::KwRompe),
    (b"nuevo",       TokenKind::KwNuevo),
    (b"match",       TokenKind::KwMatch),
    (b"resta",       TokenKind::OpResta),
    (b"pausa",       TokenKind::KwPausa),
    (b"rdtsc",       TokenKind::KwRdtsc),
    (b"cpuid",       TokenKind::KwCpuid),
    (b"movnt",       TokenKind::KwMovnt),
    (b"relax",       TokenKind::KwRelax),
    (b"const",       TokenKind::KwConst),
    (b"bucle",       TokenKind::KwBucle),
    (b"hasta",       TokenKind::KwHasta),
    (b"cerca",       TokenKind::KwCerca),
    (b"prest",       TokenKind::KwPrest),
    (b"tabla",       TokenKind::KwTabla),
    (b"salto",       TokenKind::KwSalto),
    (b"caso",        TokenKind::KwCaso),
    // ── 4 letras ─────────────────────────────────────────────────────
    (b"sino",        TokenKind::KwSino),
    (b"emit",        TokenKind::KwEmit),
    (b"aloc",        TokenKind::KwAloc),
    (b"byte",        TokenKind::TyByte),
    (b"impl",        TokenKind::KwImpl),
    (b"tipo",        TokenKind::KwTipo),
    (b"suma",        TokenKind::OpSuma),
    (b"mult",        TokenKind::OpMult),
    (b"nulo",        TokenKind::LitNulo),
    (b"barr",        TokenKind::KwBarr),
    (b"para",        TokenKind::KwPara),
    (b"paso",        TokenKind::KwPaso),
    (b"puro",        TokenKind::KwPuro),
    (b"comen",       TokenKind::KwComen),
    (b"fin",         TokenKind::KwFin),
    (b"int3",        TokenKind::KwInt3),
    (b"desde",       TokenKind::KwDesde),
    (b"align",       TokenKind::KwAlign),
    // ── 3 letras ─────────────────────────────────────────────────────
    (b"def",         TokenKind::KwDef),
    (b"let",         TokenKind::KwLet),
    (b"reg",         TokenKind::KwReg),
    (b"num",         TokenKind::TyNum),
    (b"ptr",         TokenKind::TyPtr),
    (b"arr",         TokenKind::TyArr),
    (b"ref",         TokenKind::TyRef),
    (b"div",         TokenKind::OpDiv),
    (b"mod",         TokenKind::OpMod),
    (b"xor",         TokenKind::OpXor),
    (b"shl",         TokenKind::OpShl),
    (b"shr",         TokenKind::OpShr),
    (b"rol",         TokenKind::OpRol),
    (b"ror",         TokenKind::OpRor),
    (b"nop",         TokenKind::KwNop),
    (b"hlt",         TokenKind::KwHlt),
    (b"cli",         TokenKind::KwCli),
    (b"sti",         TokenKind::KwSti),
    (b"mio",         TokenKind::KwMio),
    (b"mut",         TokenKind::KwMut),
    // ── 2 letras ─────────────────────────────────────────────────────
    (b"si",          TokenKind::KwSi),
    (b"no",          TokenKind::OpNo),
    (b"cf",          TokenKind::FlagCf),
    (b"zf",          TokenKind::FlagZf),
    (b"sf",          TokenKind::FlagSf),
    (b"of",          TokenKind::FlagOf),
    (b"pf",          TokenKind::FlagPf),
    (b"df",          TokenKind::FlagDf),
    // ── 1 letra ──────────────────────────────────────────────────────
    (b"y",           TokenKind::OpY),
    (b"o",           TokenKind::OpO),
];

#[derive(Debug, Clone, Copy)]
pub struct Scanner<'a> {
    pub src: &'a [u8],
    pub pos: usize,
}

impl<'a> Scanner<'a> {
    pub const fn new(src: &'a [u8]) -> Self {
        Self { src, pos: 0 }
    }

    /// Avanza un token. DFA real — no es stub.
    pub fn next_token(&mut self) -> Token {
        self.skip_ws_and_comments();
        if self.pos >= self.src.len() {
            return Token::EOF;
        }
        let start = self.pos as u32;
        let b = self.src[self.pos];

        // Literal de cadena: "hola"
        if b == b'"' {
            return self.scan_string(start);
        }
        // Identificador / keyword.
        if is_ident_start(b) {
            return self.scan_ident_or_kw(start);
        }
        // Literal numérico (decimal / hex / binario).
        if b.is_ascii_digit() {
            return self.scan_number(start);
        }
        // Operadores/delimitadores 1-char + lookahead para `->`.
        self.scan_punct(start)
    }

    fn scan_string(&mut self, start: u32) -> Token {
        let begin = self.pos;
        self.pos += 1; // saltar el primer '"'
        let mut closed = false;
        while self.pos < self.src.len() {
            let c = self.src[self.pos];
            self.pos += 1;
            if c == b'"' {
                closed = true;
                break;
            }
        }
        let kind = if closed { TokenKind::LitStr } else { TokenKind::Unknown };
        Token {
            kind, _pad: [0; 3],
            start, len: (self.pos - begin) as u32,
            value: 0,
        }
    }

    // ── helpers ──────────────────────────────────────────────────────

    fn skip_ws_and_comments(&mut self) {
        loop {
            while self.pos < self.src.len() && is_ws(self.src[self.pos]) {
                self.pos += 1;
            }
            // `// comentario hasta \n`
            if self.pos + 1 < self.src.len()
                && self.src[self.pos] == b'/' && self.src[self.pos + 1] == b'/'
            {
                while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
    }

    fn scan_ident_or_kw(&mut self, start: u32) -> Token {
        let begin = self.pos;
        while self.pos < self.src.len() && is_ident_cont(self.src[self.pos]) {
            self.pos += 1;
        }
        let lex = &self.src[begin..self.pos];
        // Greedy match contra tabla.
        let kind = lookup_keyword(lex).unwrap_or(TokenKind::Ident);
        Token {
            kind, _pad: [0; 3],
            start, len: (self.pos - begin) as u32,
            value: 0,
        }
    }

    fn scan_number(&mut self, start: u32) -> Token {
        let begin = self.pos;
        let (kind, value) = if begin + 1 < self.src.len() && self.src[begin] == b'0'
            && (self.src[begin + 1] == b'x' || self.src[begin + 1] == b'X')
        {
            // Hex literal.
            self.pos += 2;
            let mut v: u64 = 0;
            while self.pos < self.src.len() {
                let c = self.src[self.pos];
                let d = match c {
                    b'0'..=b'9' => (c - b'0') as u64,
                    b'a'..=b'f' => (c - b'a' + 10) as u64,
                    b'A'..=b'F' => (c - b'A' + 10) as u64,
                    b'_' => { self.pos += 1; continue; }
                    _ => break,
                };
                v = v.wrapping_shl(4) | d;
                self.pos += 1;
            }
            (TokenKind::LitHex, v)
        } else if begin + 1 < self.src.len() && self.src[begin] == b'0'
            && (self.src[begin + 1] == b'b' || self.src[begin + 1] == b'B')
        {
            // Binary literal.
            self.pos += 2;
            let mut v: u64 = 0;
            while self.pos < self.src.len() {
                let c = self.src[self.pos];
                let d = match c {
                    b'0' => 0, b'1' => 1,
                    b'_' => { self.pos += 1; continue; }
                    _ => break,
                };
                v = (v << 1) | d;
                self.pos += 1;
            }
            (TokenKind::LitBin, v)
        } else {
            // Decimal.
            let mut v: u64 = 0;
            while self.pos < self.src.len() {
                let c = self.src[self.pos];
                if c == b'_' { self.pos += 1; continue; }
                if !c.is_ascii_digit() { break; }
                v = v.wrapping_mul(10) + (c - b'0') as u64;
                self.pos += 1;
            }
            (TokenKind::LitInt, v)
        };
        Token {
            kind, _pad: [0; 3],
            start, len: (self.pos - begin) as u32, value,
        }
    }

    fn scan_punct(&mut self, start: u32) -> Token {
        let b = self.src[self.pos];
        let kind = match b {
            b'{' => TokenKind::LBrace,
            b'}' => TokenKind::RBrace,
            b'(' => TokenKind::LParen,
            b')' => TokenKind::RParen,
            b'[' => TokenKind::LBracket,
            b']' => TokenKind::RBracket,
            b',' => TokenKind::Comma,
            b':' => TokenKind::Colon,
            b';' => TokenKind::Semicolon,
            b'.' => TokenKind::Dot,
            b'=' => TokenKind::Assign,
            b'-' if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'>' => {
                self.pos += 2;
                return Token {
                    kind: TokenKind::Arrow, _pad: [0; 3],
                    start, len: 2, value: 0,
                };
            }
            _ => TokenKind::Unknown,
        };
        self.pos += 1;
        Token { kind, _pad: [0; 3], start, len: 1, value: 0 }
    }
}

// ── predicados de bytes ──────────────────────────────────────────────

#[inline(always)]
const fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

#[inline(always)]
const fn is_ident_start(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'_')
}

#[inline(always)]
const fn is_ident_cont(b: u8) -> bool {
    matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_')
}

#[inline]
fn lookup_keyword(lex: &[u8]) -> Option<TokenKind> {
    for (k, t) in KEYWORDS.iter() {
        if *k == lex { return Some(*t); }
    }
    None
}

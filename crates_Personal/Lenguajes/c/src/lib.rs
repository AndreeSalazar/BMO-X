pub mod codegen;
pub mod ast;

use ast::*;
use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile::C
}

pub fn parse(source: &str) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.parse_program()
}

pub fn compile_source_to_bef(source: &str) -> Result<Vec<u8>, CError> {
    let program = parse(source)?;
    codegen::compile_to_bef_bytes(&program)
}

#[derive(Debug, Clone)]
pub struct CError {
    pub line: usize,
    pub message: String,
}

impl CError {
    pub fn new(line: usize, message: impl Into<String>) -> Self {
        Self { line, message: message.into() }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Ident(String), IntLit(i64), StringLit(String), CharLit(u8),
    Int, Void, Char, Short, Long, Unsigned, Signed,
    If, Else, While, Do, For, Switch, Case, Default, Break, Continue,
    Return, Sizeof, Struct, Typedef, Enum,
    OpenParen, CloseParen, OpenBrace, CloseBrace, OpenBracket, CloseBracket,
    Semicolon, Comma, Colon, Question,
    Plus, Minus, Star, Slash, Percent,
    PlusPlus, MinusMinus,
    EqEq, Neq, Lt, Gt, Le, Ge,
    And, Or, Xor, Not, Tilde,
    LAnd, LOr,
    Shl, Shr,
    Arrow, Dot,
    Assign, AddAssign, SubAssign, MulAssign, DivAssign, ModAssign,
    ShlAssign, ShrAssign, AndAssign, XorAssign, OrAssign,
    Eof,
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self { tokens: Self::tokenize(source), pos: 0 }
    }

    fn tokenize(source: &str) -> Vec<Token> {
        let mut t = Vec::new();
        let c: Vec<char> = source.chars().collect();
        let mut i = 0;
        while i < c.len() {
            if c[i].is_whitespace() { i += 1; continue; }
            if c[i] == '/' && i + 1 < c.len() {
                if c[i+1] == '/' { while i < c.len() && c[i] != '\n' { i += 1; } continue; }
                if c[i+1] == '*' { i += 2; while i + 1 < c.len() && !(c[i] == '*' && c[i+1] == '/') { i += 1; } i += 2; continue; }
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
                '.' => { t.push(Token::Dot); i += 1; }
                '=' => {
                    if i + 1 < c.len() && c[i+1] == '=' { t.push(Token::EqEq); i += 2; } else { t.push(Token::Assign); i += 1; }
                }
                '"' => {
                    i += 1; let mut s = String::new();
                    while i < c.len() && c[i] != '"' {
                        if c[i] == '\\' && i + 1 < c.len() { i += 1;
                            match c[i] { 'n' => s.push('\n'), 't' => s.push('\t'), 'r' => s.push('\r'), '0' => s.push('\0'), '\\' => s.push('\\'), '"' => s.push('"'), '\'' => s.push('\''), x => { s.push('\\'); s.push(x); } }
                        } else { s.push(c[i]); } i += 1;
                    } i += 1; t.push(Token::StringLit(s));
                }
                '\'' => {
                    i += 1; let val = if c[i] == '\\' { i += 1; match c[i] { 'n' => 10, 't' => 9, 'r' => 13, '0' => 0, '\\' => 92, '\'' => 39, x => x as u8 } } else { c[i] as u8 };
                    i += 1; if i < c.len() && c[i] == '\'' { i += 1; } t.push(Token::CharLit(val));
                }
                d if d.is_ascii_digit() => {
                    let mut n = String::new();
                    if d == '0' && i + 1 < c.len() && (c[i+1] == 'x' || c[i+1] == 'X') {
                        n.push_str("0x"); i += 2;
                        while i < c.len() && c[i].is_ascii_hexdigit() { n.push(c[i]); i += 1; }
                        t.push(Token::IntLit(i64::from_str_radix(&n[2..], 16).unwrap_or(0)));
                    } else {
                        while i < c.len() && c[i].is_ascii_digit() { n.push(c[i]); i += 1; }
                        t.push(Token::IntLit(n.parse().unwrap_or(0)));
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
                        "struct" => t.push(Token::Struct), "typedef" => t.push(Token::Typedef),
                        "enum" => t.push(Token::Enum),
                        _ => t.push(Token::Ident(id)),
                    }
                }
                _ => { i += 1; }
            }
        }
        t.push(Token::Eof); t
    }

    fn peek(&self) -> &Token { &self.tokens[self.pos] }
    fn advance(&mut self) -> Token { let t = self.tokens[self.pos].clone(); self.pos += 1; t }

    fn expect(&mut self, expected: &Token) -> Result<Token, CError> {
        let tok = self.advance();
        if std::mem::discriminant(&tok) != std::mem::discriminant(expected) {
            return Err(CError::new(1, format!("expected {:?}, got {:?}", expected, tok)));
        }
        Ok(tok)
    }

    fn skip_semicolon(&mut self) {
        while *self.peek() == Token::Semicolon { self.advance(); }
    }

    // ---- Program ----
    fn parse_program(&mut self) -> Result<Program, CError> {
        let mut globals = Vec::new();
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
            if let Some(f) = self.try_parse_function()? {
                functions.push(f);
            } else {
                let (typ, name) = self.parse_type_and_name()?;
                self.skip_semicolon();
                globals.push(GlobalDecl::Var(typ, name, None));
            }
        }
        Ok(Program { globals, functions })
    }

    fn try_parse_function(&mut self) -> Result<Option<Function>, CError> {
        let save = self.pos;
        let ret_type = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(None); }
        };
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        self.advance();
        if *self.peek() != Token::OpenParen { self.pos = save; return Ok(None); }
        self.advance();
        let mut params = Vec::new();
        while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
            if *self.peek() == Token::Void && (self.pos + 1 >= self.tokens.len() || self.tokens[self.pos + 1] == Token::CloseParen) {
                self.advance(); break;
            }
            let ptype = self.parse_type_spec()?;
            let pname = match self.advance() {
                Token::Ident(n) => n,
                t => return Err(CError::new(1, format!("expected param name, got {:?}", t))),
            };
            params.push(Param { typ: ptype, name: pname });
            if *self.peek() == Token::Comma { self.advance(); }
        }
        self.expect(&Token::CloseParen)?;
        if *self.peek() != Token::OpenBrace { self.pos = save; return Ok(None); }
        self.advance();
        let mut var_count = 0u32;
        let mut body = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(1, "unexpected eof in function body")),
                _ => {
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        var_count += 1;
                        body.push(Stmt::DeclAssign(typ, name, None));
                        self.skip_semicolon();
                    } else {
                        body.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Some(Function { ret_type, name, params, var_count, body }))
    }

    fn try_parse_decl(&mut self) -> Result<Option<(TypeSpec, String)>, CError> {
        let save = self.pos;
        let typ = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(None); }
        };
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::OpenParen {
            self.pos = save; return Ok(None);
        }
        self.advance();
        if *self.peek() != Token::Semicolon && *self.peek() != Token::Assign {
            self.pos = save; return Ok(None);
        }
        Ok(Some((typ, name)))
    }

    fn parse_type_and_name(&mut self) -> Result<(TypeSpec, String), CError> {
        let typ = self.parse_type_spec()?;
        let name = match self.advance() {
            Token::Ident(n) => n,
            t => return Err(CError::new(1, format!("expected identifier, got {:?}", t))),
        };
        Ok((typ, name))
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, CError> {
        let base = match self.advance() {
            Token::Void => TypeSpec::Void,
            Token::Char => TypeSpec::Char,
            Token::Short => TypeSpec::Short,
            Token::Int => TypeSpec::Int,
            Token::Long => {
                if *self.peek() == Token::Long { self.advance(); TypeSpec::LongLong } else { TypeSpec::Long }
            }
            Token::Unsigned => {
                match self.peek() {
                    Token::Char => { self.advance(); TypeSpec::UnsignedChar }
                    Token::Short => { self.advance(); TypeSpec::UnsignedShort }
                    Token::Int => { self.advance(); TypeSpec::UnsignedInt }
                    Token::Long => {
                        self.advance();
                        if *self.peek() == Token::Long { self.advance(); TypeSpec::UnsignedLongLong }
                        else { TypeSpec::UnsignedLong }
                    }
                    _ => TypeSpec::UnsignedInt,
                }
            }
            Token::Signed => { self.advance(); TypeSpec::Int }
            t => return Err(CError::new(1, format!("expected type, got {:?}", t))),
        };
        if *self.peek() == Token::Star {
            self.advance();
            Ok(TypeSpec::Ptr(Box::new(base)))
        } else {
            Ok(base)
        }
    }

    // ---- Statements ----
    fn parse_stmt(&mut self) -> Result<Stmt, CError> {
        match self.peek() {
            Token::If => self.parse_if(),
            Token::While => self.parse_while(),
            Token::Do => self.parse_do(),
            Token::For => self.parse_for(),
            Token::Switch => self.parse_switch(),
            Token::Break => { self.advance(); self.skip_semicolon(); Ok(Stmt::Break) }
            Token::Continue => { self.advance(); self.skip_semicolon(); Ok(Stmt::Continue) }
            Token::Return => self.parse_return(),
            Token::OpenBrace => self.parse_block(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_if(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let then = Box::new(self.parse_stmt()?);
        let else_ = if *self.peek() == Token::Else { self.advance(); Some(Box::new(self.parse_stmt()?)) } else { None };
        Ok(Stmt::If(cond, then, else_))
    }

    fn parse_while(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::While(cond, body))
    }

    fn parse_do(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let body = Box::new(self.parse_stmt()?);
        self.expect(&Token::While)?;
        self.expect(&Token::OpenParen)?;
        let cond = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.skip_semicolon();
        Ok(Stmt::DoWhile(body, cond))
    }

    fn parse_for(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let init = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
        self.skip_semicolon();
        let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
        self.expect(&Token::CloseParen)?;
        let body = Box::new(self.parse_stmt()?);
        Ok(Stmt::For(init, cond, inc, body))
    }

    fn parse_switch(&mut self) -> Result<Stmt, CError> {
        self.advance();
        self.expect(&Token::OpenParen)?;
        let expr = self.parse_expr()?;
        self.expect(&Token::CloseParen)?;
        self.expect(&Token::OpenBrace)?;
        let mut cases = Vec::new();
        let mut current = Vec::new();
        let mut current_val = None;
        loop {
            match self.peek() {
                Token::Case => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    let val = match self.advance() {
                        Token::IntLit(n) => n,
                        t => return Err(CError::new(1, format!("expected int in case, got {:?}", t))),
                    };
                    current_val = Some(val);
                    self.expect(&Token::Colon)?;
                }
                Token::Default => {
                    if !current.is_empty() { cases.push(Case { value: current_val, stmts: std::mem::take(&mut current) }); }
                    self.advance();
                    current_val = None;
                    self.expect(&Token::Colon)?;
                }
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(1, "unexpected eof in switch")),
                _ => { current.push(self.parse_stmt()?); }
            }
        }
        if !current.is_empty() { cases.push(Case { value: current_val, stmts: current }); }
        Ok(Stmt::Switch(expr, cases))
    }

    fn parse_return(&mut self) -> Result<Stmt, CError> {
        self.advance();
        if *self.peek() == Token::Semicolon { self.advance(); Ok(Stmt::Return(None)) }
        else { let e = self.parse_expr()?; self.skip_semicolon(); Ok(Stmt::Return(Some(e))) }
    }

    fn parse_block(&mut self) -> Result<Stmt, CError> {
        self.advance();
        let mut stmts = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(1, "unexpected eof in block")),
                _ => {
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
                        self.skip_semicolon();
                        stmts.push(Stmt::DeclAssign(typ, name, init));
                        continue;
                    } else {
                        stmts.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Stmt::Block(stmts))
    }

    fn parse_expr_stmt(&mut self) -> Result<Stmt, CError> {
        let expr = self.parse_expr()?;
        self.skip_semicolon();
        match &expr {
            Expr::Call(name, args) if name == "printf" => {
                if let Some(Expr::StringLit(s)) = args.first() {
                    return Ok(if s.ends_with('\n') { let mut t = s.clone(); t.pop(); Stmt::PrintfLn(t) } else { Stmt::Printf(s.clone()) });
                }
            }
            _ => {}
        }
        Ok(Stmt::Expr(expr))
    }

    // ---- Expressions (precedence climbing) ----
    fn parse_expr(&mut self) -> Result<Expr, CError> {
        self.parse_comma()
    }

    fn parse_comma(&mut self) -> Result<Expr, CError> {
        let mut exprs = vec![self.parse_assign()?];
        while *self.peek() == Token::Comma { self.advance(); exprs.push(self.parse_assign()?); }
        if exprs.len() == 1 { Ok(exprs.into_iter().next().unwrap()) } else { Ok(Expr::Comma(exprs)) }
    }

    fn parse_assign(&mut self) -> Result<Expr, CError> {
        let expr = self.parse_conditional()?;
        match self.peek() {
            Token::Assign => { self.advance(); let val = self.parse_assign()?; match expr { Expr::Var(n) => Ok(Expr::Assign(n, Box::new(val))), _ => Ok(val) } }
            Token::AddAssign => { self.advance(); let val = self.parse_assign()?; match expr { Expr::Var(n) => { let n2 = n.clone(); Ok(Expr::Assign(n, Box::new(Expr::Add(Box::new(Expr::Var(n2)), Box::new(val))))) } _ => Ok(val) } }
            Token::SubAssign => { self.advance(); let val = self.parse_assign()?; match expr { Expr::Var(n) => { let n2 = n.clone(); Ok(Expr::Assign(n, Box::new(Expr::Sub(Box::new(Expr::Var(n2)), Box::new(val))))) } _ => Ok(val) } }
            _ => Ok(expr),
        }
    }

    fn parse_conditional(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_lor()?;
        if *self.peek() == Token::Question {
            self.advance();
            let t = self.parse_expr()?;
            self.expect(&Token::Colon)?;
            let f = self.parse_conditional()?;
            expr = Expr::Conditional(Box::new(expr), Box::new(t), Box::new(f));
        }
        Ok(expr)
    }

    fn parse_lor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_land()?; while *self.peek() == Token::LOr { self.advance(); let r = self.parse_land()?; l = Expr::LOr(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_land(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitor()?; while *self.peek() == Token::LAnd { self.advance(); let r = self.parse_bitor()?; l = Expr::LAnd(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitxor()?; while *self.peek() == Token::Or { self.advance(); let r = self.parse_bitxor()?; l = Expr::BitOr(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitxor(&mut self) -> Result<Expr, CError> { let mut l = self.parse_bitand()?; while *self.peek() == Token::Xor { self.advance(); let r = self.parse_bitand()?; l = Expr::BitXor(Box::new(l), Box::new(r)); } Ok(l) }
    fn parse_bitand(&mut self) -> Result<Expr, CError> { let mut l = self.parse_equality()?; while *self.peek() == Token::And { self.advance(); let r = self.parse_equality()?; l = Expr::BitAnd(Box::new(l), Box::new(r)); } Ok(l) }

    fn parse_equality(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_relational()?;
        loop {
            match self.peek() {
                Token::EqEq => { self.advance(); let r = self.parse_relational()?; l = Expr::Eq(Box::new(l), Box::new(r)); }
                Token::Neq => { self.advance(); let r = self.parse_relational()?; l = Expr::Neq(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_relational(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_shift()?;
        loop {
            match self.peek() {
                Token::Lt => { self.advance(); let r = self.parse_shift()?; l = Expr::Lt(Box::new(l), Box::new(r)); }
                Token::Gt => { self.advance(); let r = self.parse_shift()?; l = Expr::Gt(Box::new(l), Box::new(r)); }
                Token::Le => { self.advance(); let r = self.parse_shift()?; l = Expr::Le(Box::new(l), Box::new(r)); }
                Token::Ge => { self.advance(); let r = self.parse_shift()?; l = Expr::Ge(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_shift(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_add()?;
        loop {
            match self.peek() {
                Token::Shl => { self.advance(); let r = self.parse_add()?; l = Expr::Shl(Box::new(l), Box::new(r)); }
                Token::Shr => { self.advance(); let r = self.parse_add()?; l = Expr::Shr(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_add(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_mul()?;
        loop {
            match self.peek() {
                Token::Plus => { self.advance(); let r = self.parse_mul()?; l = Expr::Add(Box::new(l), Box::new(r)); }
                Token::Minus => { self.advance(); let r = self.parse_mul()?; l = Expr::Sub(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_mul(&mut self) -> Result<Expr, CError> {
        let mut l = self.parse_unary()?;
        loop {
            match self.peek() {
                Token::Star => { self.advance(); let r = self.parse_unary()?; l = Expr::Mul(Box::new(l), Box::new(r)); }
                Token::Slash => { self.advance(); let r = self.parse_unary()?; l = Expr::Div(Box::new(l), Box::new(r)); }
                Token::Percent => { self.advance(); let r = self.parse_unary()?; l = Expr::Mod(Box::new(l), Box::new(r)); }
                _ => break,
            }
        }
        Ok(l)
    }

    fn parse_unary(&mut self) -> Result<Expr, CError> {
        match self.peek() {
            Token::Minus => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Neg(Box::new(e))) }
            Token::Not => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Not(Box::new(e))) }
            Token::Tilde => { self.advance(); let e = self.parse_unary()?; Ok(Expr::BitNot(Box::new(e))) }
            Token::PlusPlus => { self.advance(); match &self.peek() { Token::Ident(n) => { let name = n.clone(); self.advance(); Ok(Expr::PreInc(name)) } _ => Err(CError::new(1, "expected variable after ++")) } }
            Token::MinusMinus => { self.advance(); match &self.peek() { Token::Ident(n) => { let name = n.clone(); self.advance(); Ok(Expr::PreDec(name)) } _ => Err(CError::new(1, "expected variable after --")) } }
            Token::And => { self.advance(); match &self.peek() { Token::Ident(n) => { let name = n.clone(); self.advance(); Ok(Expr::AddrOf(name)) } _ => Err(CError::new(1, "expected variable after &")) } }
            Token::Star => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Deref(Box::new(e))) }
            Token::Sizeof => { self.advance(); self.expect(&Token::OpenParen)?; let _ = self.parse_type_spec()?; self.expect(&Token::CloseParen)?; Ok(Expr::Int(8)) }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Token::PlusPlus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostInc(n.clone()), _ => {} } }
                Token::MinusMinus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostDec(n.clone()), _ => {} } }
                Token::OpenBracket => { self.advance(); let index = self.parse_expr()?; self.expect(&Token::CloseBracket)?; match expr { Expr::Var(ref n) => expr = Expr::Subscript(n.clone(), Box::new(index)), _ => {} } }
                Token::Dot => { self.advance(); let _ = self.advance(); }
                Token::Arrow => { self.advance(); let _ = self.advance(); }
                _ => break,
            }
        }
        Ok(expr)
    }

    fn parse_primary(&mut self) -> Result<Expr, CError> {
        let tok = self.advance();
        match tok {
            Token::IntLit(n) => Ok(Expr::Int(n)),
            Token::StringLit(s) => Ok(Expr::StringLit(s)),
            Token::CharLit(c) => Ok(Expr::CharLit(c)),
            Token::Ident(name) => {
                if *self.peek() == Token::OpenParen {
                    self.advance();
                    let mut args = Vec::new();
                    while *self.peek() != Token::CloseParen && *self.peek() != Token::Eof {
                        args.push(self.parse_expr()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Var(name))
                }
            }
            Token::OpenParen => {
                let expr = self.parse_expr()?;
                self.expect(&Token::CloseParen)?;
                Ok(expr)
            }
            t => Err(CError::new(1, format!("unexpected token: {:?}", t))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_hello_world() {
        let src = "int main() { printf(\"HOLA C\"); return 0; }";
        let p = parse(src).unwrap();
        assert_eq!(p.functions.len(), 1);
        assert_eq!(p.functions[0].name, "main");
    }

    #[test]
    fn emits_bef() {
        let bef = compile_source_to_bef("int main() { printf(\"HOLA C\"); return 0; }").unwrap();
        assert!(bef.len() > 48);
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
    }

    #[test]
    fn emits_bef_with_correct_string_offset() {
        let bef = compile_source_to_bef("int main() { printf(\"HOLA C\"); return 0; }").unwrap();
        let sec_off = u64::from_le_bytes(bef[32..40].try_into().unwrap()) as usize;
        let file_off = u64::from_le_bytes(bef[sec_off+8..sec_off+16].try_into().unwrap()) as usize;
        let file_sz = u64::from_le_bytes(bef[sec_off+16..sec_off+24].try_into().unwrap()) as usize;
        let code = &bef[file_off..file_off+file_sz];
        let disp = i32::from_le_bytes(code[7..11].try_into().unwrap());
        let s_off = (11 + disp as i64) as usize;
        let end = code[s_off..].iter().position(|&b| b == 0).unwrap();
        let s = core::str::from_utf8(&code[s_off..s_off+end]).unwrap();
        assert_eq!(s, "HOLA C");
    }

    #[test]
    fn parses_for_if_while_switch() {
        let src = r#"
int main() {
    int x;
    for (x = 0; x < 10; x = x + 1) {
        if (x == 5) { printf("half"); }
    }
    while (x > 0) { x = x - 1; }
    do { x = x + 1; } while (x < 5);
    switch (x) {
        case 0: printf("zero"); break;
        case 1: printf("one"); break;
        default: printf("many");
    }
    return 0;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_multiple_types() {
        let src = r#"
int main() {
    char c;
    short s;
    long l;
    unsigned int u;
    unsigned long ul;
    long long ll;
    return 0;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn profile_is_c() {
        assert_eq!(profile().name, "C");
    }

    #[test]
    fn parses_multi_param_call() {
        let src = r#"
int add(int a, int b) {
    return a + b;
}
int main() {
    int r;
    r = add(3, 4);
    return r;
}
"#;
        let p = parse(src).unwrap();
        assert_eq!(p.functions.len(), 2);
    }

    #[test]
    fn handles_void_param() {
        let src = "int main(void) { return 0; }";
        parse(src).unwrap();
    }
}

pub mod codegen;
pub mod ast;
pub mod module;
pub mod ir_emit;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
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

/// Compile C source to a unified IrModule (language-agnostic IR).
pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
}

pub fn compile_source_to_bef_with_modules(source: &str, base_paths: Vec<PathBuf>) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths);
    let program = Parser::new(source).parse_program_with_modules(&mut resolver, None)?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
}

pub fn compile_source_to_bef_with_all(
    source: &str,
    base_paths: Vec<PathBuf>,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths);
    let program = Parser::new(source).parse_program_with_modules(&mut resolver, Some(asm_paths))?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
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
    Float, Double,
    Return, Sizeof, Struct, Union, Typedef, Enum, Goto, Use,
    Const, Volatile, Extern,
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
    var_types: HashMap<String, TypeSpec>,
    struct_fields: HashMap<String, Vec<(String, u32, u32)>>, // name â†’ [(field, offset, size)]
    struct_sizes: HashMap<String, u32>,
    usings: Vec<String>, // module paths collected from `use "path"` directives
    typedefs: HashMap<String, TypeSpec>,
    syscalls: HashMap<String, SyscallDef>, // known semantic syscall definitions
}

impl Parser {
    fn new(source: &str) -> Self {
        Self {
            tokens: Self::tokenize(source), pos: 0,
            var_types: HashMap::new(),
            struct_fields: HashMap::new(),
            struct_sizes: HashMap::new(),
            usings: Vec::new(),
            typedefs: HashMap::new(),
            syscalls: HashMap::new(),
        }
    }

    #[cfg(test)]
    fn tokenize_for_test(source: &str) -> Vec<Token> { Self::tokenize(source) }

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
                    // string literal concatenation: "foo" "bar" â†’ "foobar"
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
                        "goto" => t.push(Token::Goto),
                        "use" => t.push(Token::Use),
                        "const" => t.push(Token::Const),
                        "volatile" => t.push(Token::Volatile),
                        "extern" => t.push(Token::Extern),
                        "float" => t.push(Token::Float),
                        "double" => t.push(Token::Double),
                        "struct" => t.push(Token::Struct), "union" => t.push(Token::Union), "typedef" => t.push(Token::Typedef),
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

    fn get_field_offset(&self, struct_name: &str, field: &str) -> Option<u32> {
        self.struct_fields.get(struct_name).and_then(|fields| {
            fields.iter().find(|(n, _, _)| n == field).map(|(_, off, _)| *off)
        })
    }

    fn resolve_struct_type(&self, expr: &Expr) -> Option<String> {
        match expr {
            Expr::Var(n) => {
                self.var_types.get(n).and_then(|t| match t {
                    TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
                    _ => None,
                })
            }
            Expr::Subscript(n, _, _) => {
                self.var_types.get(n).and_then(|t| match t {
                    TypeSpec::Ptr(base) => match base.as_ref() {
                        TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
                        _ => None,
                    },
                    TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
                    _ => None,
                })
            }
            Expr::Deref(inner) => {
                match inner.as_ref() {
                    Expr::Var(n) => {
                        self.var_types.get(n).and_then(|t| match t {
                            TypeSpec::Ptr(base) => match base.as_ref() {
                                TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => Some(s.clone()),
                                _ => None,
                            },
                            _ => None,
                        })
                    }
                    _ => None,
                }
            }
            Expr::Field(base, _, _) => {
                self.resolve_struct_type(base)
            }
            _ => None,
        }
    }

    fn resolve_field_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        self.resolve_struct_type(expr)
            .and_then(|s| self.get_field_offset(&s, field))
            .unwrap_or(0)
    }

    fn resolve_arrow_expr_offset(&self, expr: &Expr, field: &str) -> u32 {
        // Arrow: expr->field, expr is a pointer to struct
        match expr {
            Expr::Var(n) => {
                self.var_types.get(n).and_then(|t| match t {
                    TypeSpec::Ptr(base) => match base.as_ref() {
                        TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => self.get_field_offset(s, field),
                        _ => None,
                    },
                    _ => None,
                }).unwrap_or(0)
            }
            _ => 0,
        }
    }

    fn element_size(&self, name: &str) -> u8 {
        if let Some(typ) = self.var_types.get(name) {
            match typ {
                TypeSpec::Char => 1, TypeSpec::UnsignedChar => 1,
                TypeSpec::Short => 2, TypeSpec::UnsignedShort => 2,
                TypeSpec::Int => 4, TypeSpec::UnsignedInt => 4,
                TypeSpec::Long | TypeSpec::UnsignedLong => 8,
                TypeSpec::Ptr(ref base) => {
                    match base.as_ref() {
                        TypeSpec::Char | TypeSpec::UnsignedChar => 1,
                        TypeSpec::Short | TypeSpec::UnsignedShort => 2,
                        TypeSpec::Int | TypeSpec::UnsignedInt => 4,
                        TypeSpec::Float => 4, TypeSpec::Double => 8,
                        TypeSpec::Void => 1,
                        TypeSpec::StructRef(s) | TypeSpec::UnionRef(s) => *self.struct_sizes.get(s.as_str()).unwrap_or(&8) as u8,
                        _ => 8,
                    }
                }
                _ => 8,
            }
        } else { 8 }
    }

    fn compute_struct_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut offset = 0u32;
        for m in members {
            let sz = m.typ.stack_size();
            let align = sz.min(8).max(1);
            offset = (offset + align - 1) / align * align;
            layout.push((m.name.clone(), offset, sz));
            offset += sz;
        }
        let max_align = members.iter().map(|m| m.typ.stack_size().min(8).max(1)).max().unwrap_or(1);
        let total = (offset + max_align - 1) / max_align * max_align;
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), total);
    }

    fn compute_union_layout(&mut self, name: &str, members: &[StructMember]) {
        let mut layout = Vec::new();
        let mut max_sz = 0u32;
        for m in members {
            let sz = m.typ.stack_size();
            layout.push((m.name.clone(), 0u32, sz));
            if sz > max_sz { max_sz = sz; }
        }
        self.struct_fields.insert(name.to_string(), layout);
        self.struct_sizes.insert(name.to_string(), max_sz);
    }

    fn expect(&mut self, expected: &Token) -> Result<Token, CError> {
        if *self.peek() != *expected {
            return Err(CError::new(1, format!("expected {:?}, got {:?}", expected, self.peek())));
        }
        Ok(self.advance())
    }

    fn skip_semicolon(&mut self) {
        if *self.peek() == Token::Semicolon { self.advance(); }
    }

    // ---- Program ----
    fn parse_program(&mut self) -> Result<Program, CError> {
        let mut globals = Vec::new();
        let mut functions = Vec::new();
        while *self.peek() != Token::Eof {
            if *self.peek() == Token::Struct || *self.peek() == Token::Union {
                let is_union = *self.peek() == Token::Union;
                self.advance();
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(1, format!("expected struct name, got {:?}", t))),
                };
                if *self.peek() == Token::OpenBrace {
                    self.advance();
                    let mut members = Vec::new();
                    while *self.peek() != Token::CloseBrace && *self.peek() != Token::Eof {
                        let mtype = self.parse_type_spec()?;
                        let mname = match self.advance() {
                            Token::Ident(n) => n,
                            t => return Err(CError::new(1, format!("expected member name, got {:?}", t))),
                        };
                        self.skip_semicolon();
                        members.push(StructMember { typ: mtype, name: mname });
                    }
                    self.expect(&Token::CloseBrace)?;
                    self.skip_semicolon();
                    if is_union {
                        self.compute_union_layout(&name, &members);
                        globals.push(GlobalDecl::Union(name, members));
                    } else {
                        self.compute_struct_layout(&name, &members);
                        globals.push(GlobalDecl::Struct(name, members));
                    }
                } else {
                    // struct name var; â€” handled as type+name below
                    if let Token::Ident(vname) = self.advance() {
                        let typ = if is_union { TypeSpec::UnionRef(name) } else { TypeSpec::StructRef(name) };
                        self.skip_semicolon();
                        globals.push(GlobalDecl::Var(typ.clone(), vname.clone(), None));
                        self.var_types.insert(vname, typ);
                    }
                }
                continue;
            }
            if *self.peek() == Token::Enum {
                self.advance();
                let _name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(1, format!("expected enum name, got {:?}", t))),
                };
                self.expect(&Token::OpenBrace)?;
                let mut val = 0i64;
                loop {
                    match self.advance() {
                        Token::Ident(en) => {
                            if *self.peek() == Token::Assign {
                                self.advance();
                                let assigned = match self.advance() {
                                    Token::IntLit(n) => n,
                                    t => return Err(CError::new(1, format!("expected int in enum, got {:?}", t))),
                                };
                                val = assigned;
                            }
                            // Store enum constant as if it were a variable with int type + constant value
                            self.var_types.insert(en.clone(), TypeSpec::Int);
                        }
                        Token::CloseBrace => { break; }
                        t => return Err(CError::new(1, format!("expected enum constant, got {:?}", t))),
                    }
                    val += 1;
                    if *self.peek() == Token::Comma { self.advance(); }
                }
                self.skip_semicolon();
                continue;
            }
            if *self.peek() == Token::Use {
                self.advance();
                let path = match self.advance() {
                    Token::StringLit(s) => s,
                    t => return Err(CError::new(1, format!("expected module path string, got {:?}", t))),
                };
                self.skip_semicolon();
                self.usings.push(path);
                continue;
            }
            if *self.peek() == Token::Extern {
                self.advance();
                let (typ, name) = self.parse_type_and_name()?;
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, None));
                continue;
            }
            if *self.peek() == Token::Typedef {
                self.advance();
                let typ = self.parse_type_spec()?;
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(1, format!("expected typedef name, got {:?}", t))),
                };
                self.skip_semicolon();
                self.typedefs.insert(name, typ);
                continue;
            }
            if let Some(f) = self.try_parse_function()? {
                functions.push(f);
            } else {
                let (typ, name) = self.parse_type_and_name()?;
                let init = if *self.peek() == Token::Assign {
                    self.advance();
                    Some(self.parse_assign()?)
                } else {
                    None
                };
                self.skip_semicolon();
                self.var_types.insert(name.clone(), typ.clone());
                globals.push(GlobalDecl::Var(typ, name, init));
            }
        }
        Ok(Program { globals, functions, exported: Vec::new() })
    }

    /// Parse with module resolution. Returns merged Program with all dependency sources.
    /// If `asm_paths` is provided, also loads Semantic_ASM .toml files for each `use` directive.
    fn parse_program_with_modules(
        &mut self,
        resolver: &mut module::ModuleResolver,
        asm_paths: Option<Vec<PathBuf>>,
    ) -> Result<Program, CError> {
        let mut program = self.parse_program()?;
        // Syscall defs and module manifests are loaded AFTER parse_program().
        // We must post-process the AST to convert Expr::Call â†’ Expr::Syscall
        // for any function names that match a loaded syscall definition.
        let usings = std::mem::take(&mut self.usings);
        for path in &usings {
            // Load module sources (optional â€” module may not exist for syscall-only paths)
            if let Ok(manifest) = resolver.find_manifest(path) {
                let mod_dir = resolver.find_base_dir(path);
                for src_file in &manifest.source_files {
                    let full_path = mod_dir.join(src_file);
                    let source = std::fs::read_to_string(&full_path)
                        .map_err(|e| CError::new(0, format!("cannot read module source {}: {e}", full_path.display())))?;
                    let mut sub = Parser::new(&source);
                    let sub_prog = sub.parse_program()?;
                    for f in sub_prog.functions {
                        if !program.functions.iter().any(|pf| pf.name == f.name) {
                            program.functions.push(f);
                        }
                    }
                    for g in sub_prog.globals {
                        if !program.globals.iter().any(|pg| std::mem::discriminant(pg) == std::mem::discriminant(&g)) {
                            program.globals.push(g);
                        }
                    }
                    for (k, v) in sub.struct_fields {
                        self.struct_fields.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.struct_sizes {
                        self.struct_sizes.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.var_types {
                        self.var_types.entry(k).or_insert(v);
                    }
                    for (k, v) in sub.typedefs {
                        self.typedefs.entry(k).or_insert(v);
                    }
                }
                program.exported.extend(manifest.exports);
            }

            // Load syscall definitions from embedded registry
            if self.syscalls.is_empty() {
                for d in bmo_abi::asm::defs::syscalls() {
                    self.syscalls.entry(d.name.clone()).or_insert(SyscallDef { name: d.name, nr: d.nr, arg_count: d.arg_count });
                }
            }
        }
        // Post-process: convert Expr::Call(name,args) â†’ Expr::Syscall(def,args)
        // for any function calls whose name matches a loaded syscall definition.
        self.resolve_syscalls_in_program(&mut program);
        // Validate syscall argument counts
        self.validate_syscall_args(&program)?;
        Ok(program)
    }

    /// Validate that all Expr::Syscall nodes have the correct argument count.
    fn validate_syscall_args(&self, program: &Program) -> Result<(), CError> {
        for func in &program.functions {
            Self::check_syscall_args_in_stmt_slice(&func.body, func.line)?;
        }
        Ok(())
    }

    fn check_syscall_args_in_stmt_slice(stmts: &[Stmt], line: usize) -> Result<(), CError> {
        for stmt in stmts {
            Self::check_syscall_args_in_stmt(stmt, line)?;
        }
        Ok(())
    }

    fn check_syscall_args_in_stmt(stmt: &Stmt, line: usize) -> Result<(), CError> {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(t, line)?;
                if let Some(el) = e { Self::check_syscall_args_in_stmt(el, line)?; }
            }
            Stmt::While(cond, body) => {
                Self::check_syscall_args_in_expr(cond, line)?;
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::DoWhile(body, cond) => {
                Self::check_syscall_args_in_stmt(body, line)?;
                Self::check_syscall_args_in_expr(cond, line)?;
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = cond { Self::check_syscall_args_in_expr(e, line)?; }
                if let Some(e) = inc { Self::check_syscall_args_in_expr(e, line)?; }
                Self::check_syscall_args_in_stmt(body, line)?;
            }
            Stmt::Switch(expr, cases) => {
                Self::check_syscall_args_in_expr(expr, line)?;
                for c in cases { Self::check_syscall_args_in_stmt_slice(&c.stmts, line)?; }
            }
            Stmt::Block(stmts) => Self::check_syscall_args_in_stmt_slice(stmts, line)?,
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            Stmt::DeclAssign(_, _, Some(e)) => Self::check_syscall_args_in_expr(e, line)?,
            _ => {}
        }
        Ok(())
    }

    fn check_syscall_args_in_expr(expr: &Expr, line: usize) -> Result<(), CError> {
        match expr {
            Expr::Syscall(def, args) => {
                if args.len() != def.arg_count as usize {
                    return Err(CError::new(line, format!(
                        "syscall {}() expects {} arguments, got {}",
                        def.name, def.arg_count, args.len()
                    )));
                }
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a)
                => Self::check_syscall_args_in_expr(a, line)?,
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::check_syscall_args_in_expr(a, line)?;
                Self::check_syscall_args_in_expr(b, line)?;
            }
            Expr::Conditional(c,t,f) => {
                Self::check_syscall_args_in_expr(c, line)?;
                Self::check_syscall_args_in_expr(t, line)?;
                Self::check_syscall_args_in_expr(f, line)?;
            }
            Expr::Call(_, args) | Expr::Comma(args) => {
                for a in args { Self::check_syscall_args_in_expr(a, line)?; }
            }
            Expr::Arrow(p,_,_) | Expr::AssignArrow(p,_,_,_) => Self::check_syscall_args_in_expr(p, line)?,
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,v) => Self::check_syscall_args_in_expr(v, line)?,
            Expr::AssignDeref(a, v) => { Self::check_syscall_args_in_expr(a, line)?; Self::check_syscall_args_in_expr(v, line)?; }
            Expr::Field(b,_,_) => Self::check_syscall_args_in_expr(b, line)?,
            _ => {}
        }
        Ok(())
    }

    /// Walk all function bodies and convert Expr::Call â†’ Expr::Syscall for
    /// any function calls whose name matches a loaded syscall definition.
    fn resolve_syscalls_in_program(&self, program: &mut Program) {
        for func in &mut program.functions {
            Self::resolve_syscalls_in_stmt_slice(&self.syscalls, &mut func.body);
        }
    }

    fn resolve_syscalls_in_stmt_slice(syscalls: &HashMap<String, SyscallDef>, stmts: &mut Vec<Stmt>) {
        for stmt in stmts.iter_mut() {
            Self::resolve_syscalls_in_stmt(syscalls, stmt);
        }
    }

    fn resolve_syscalls_in_stmt(syscalls: &HashMap<String, SyscallDef>, stmt: &mut Stmt) {
        match stmt {
            Stmt::If(cond, t, e) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, t);
                if let Some(el) = e { Self::resolve_syscalls_in_stmt(syscalls, el); }
            }
            Stmt::While(cond, body) => {
                Self::resolve_syscalls_in_expr(syscalls, cond);
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::DoWhile(body, cond) => {
                Self::resolve_syscalls_in_stmt(syscalls, body);
                Self::resolve_syscalls_in_expr(syscalls, cond);
            }
            Stmt::For(init, cond, inc, body) => {
                if let Some(e) = init { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = cond { Self::resolve_syscalls_in_expr(syscalls, e); }
                if let Some(e) = inc { Self::resolve_syscalls_in_expr(syscalls, e); }
                Self::resolve_syscalls_in_stmt(syscalls, body);
            }
            Stmt::Switch(expr, cases) => {
                Self::resolve_syscalls_in_expr(syscalls, expr);
                for c in cases { Self::resolve_syscalls_in_stmt_slice(syscalls, &mut c.stmts); }
            }
            Stmt::Block(stmts) => Self::resolve_syscalls_in_stmt_slice(syscalls, stmts),
            Stmt::Expr(e) | Stmt::Return(Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            Stmt::DeclAssign(_, _, Some(e)) => Self::resolve_syscalls_in_expr(syscalls, e),
            _ => {}
        }
    }

    fn resolve_syscalls_in_expr(syscalls: &HashMap<String, SyscallDef>, expr: &mut Expr) {
        match expr {
            Expr::Call(name, args) => {
                let mut new_args = std::mem::take(args);
                // Resolve syscalls in args first (before we potentially move them)
                for a in new_args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
                if let Some(def) = syscalls.get(name).cloned() {
                    *expr = Expr::Syscall(def, new_args);
                } else {
                    *expr = Expr::Call(std::mem::take(name), new_args);
                }
            }
            Expr::Syscall(_, args) => {
                for a in args.iter_mut() {
                    Self::resolve_syscalls_in_expr(syscalls, a);
                }
            }
            Expr::Neg(a) | Expr::Not(a) | Expr::BitNot(a) | Expr::Deref(a) | Expr::AddrOf(a) => Self::resolve_syscalls_in_expr(syscalls, a),
            Expr::Add(a,b) | Expr::Sub(a,b) | Expr::Mul(a,b) | Expr::Div(a,b) | Expr::Mod(a,b)
                | Expr::Eq(a,b) | Expr::Neq(a,b) | Expr::Lt(a,b) | Expr::Gt(a,b) | Expr::Le(a,b) | Expr::Ge(a,b)
                | Expr::BitAnd(a,b) | Expr::BitXor(a,b) | Expr::BitOr(a,b) | Expr::LAnd(a,b) | Expr::LOr(a,b)
                | Expr::Shl(a,b) | Expr::Shr(a,b) => {
                Self::resolve_syscalls_in_expr(syscalls, a);
                Self::resolve_syscalls_in_expr(syscalls, b);
            }
            Expr::Conditional(c,t,f) => {
                Self::resolve_syscalls_in_expr(syscalls, c);
                Self::resolve_syscalls_in_expr(syscalls, t);
                Self::resolve_syscalls_in_expr(syscalls, f);
            }
            Expr::Arrow(p,_,_) | Expr::AssignArrow(p,_,_,_) => Self::resolve_syscalls_in_expr(syscalls, p),
            Expr::Assign(_, v) | Expr::AssignField(_,_,_,v) => Self::resolve_syscalls_in_expr(syscalls, v),
            Expr::AssignDeref(a, v) => { Self::resolve_syscalls_in_expr(syscalls, a); Self::resolve_syscalls_in_expr(syscalls, v); }
            Expr::Field(b,_,_) => Self::resolve_syscalls_in_expr(syscalls, b),
            Expr::Comma(v) => { for e in v { Self::resolve_syscalls_in_expr(syscalls, e); } }
            _ => {}
        }
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
        // After expect advances past ), pos should be at {
        if self.pos >= self.tokens.len() || *self.peek() != Token::OpenBrace { self.pos = save; return Ok(None); }
        self.advance();
        let mut var_count = 0u32;
        let mut var_names: Vec<String> = params.iter().map(|p| p.name.clone()).collect();
        let mut body = Vec::new();
        loop {
            match self.peek() {
                Token::CloseBrace => { self.advance(); break; }
                Token::Eof => return Err(CError::new(1, "unexpected eof in function body")),
                _ => {
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance();
                            self.advance();
                            body.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        var_count += 1;
                        var_names.push(name.clone());
                        let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
                        self.skip_semicolon();
                        self.var_types.insert(name.clone(), typ.clone());
                        body.push(Stmt::DeclAssign(typ, name, init));
                    } else {
                        body.push(self.parse_stmt()?);
                    }
                }
            }
        }
        Ok(Some(Function { ret_type, name, params, var_count, var_names, body, line: 0 }))
    }

    fn try_parse_decl(&mut self) -> Result<Option<(TypeSpec, String)>, CError> {
        let save = self.pos;
        if !self.peek_is_type_start() {
            return Ok(None);
        }
        let typ = match self.parse_type_spec() {
            Ok(t) => t,
            Err(_) => { self.pos = save; return Ok(None); }
        };
        let Token::Ident(name) = self.peek().clone() else { self.pos = save; return Ok(None); };
        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::OpenParen {
            self.pos = save; return Ok(None);
        }
        self.advance();
        // handle array declarator: name[size]
        if *self.peek() == Token::OpenBracket {
            self.advance();
            // consume size expression
            self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
        }
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
        // skip optional array declarator [size]
        if *self.peek() == Token::OpenBracket {
            self.advance();
            self.parse_expr()?;
            self.expect(&Token::CloseBracket)?;
        }
        Ok((typ, name))
    }

    fn peek_is_type_start(&self) -> bool {
        match self.peek() {
            Token::Int | Token::Void | Token::Char | Token::Short | Token::Long |
            Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Enum | Token::Const | Token::Volatile => true,
            Token::Ident(name) => self.typedefs.contains_key(name),
            _ => false,
        }
    }

    fn strip_qualifiers(&mut self) {
        loop {
            match self.peek() {
                Token::Const | Token::Volatile => { self.advance(); }
                _ => break,
            }
        }
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, CError> {
        self.strip_qualifiers();
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
            Token::Float => TypeSpec::Float,
            Token::Double => TypeSpec::Double,
            Token::Struct => { 
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(1, format!("expected struct name, got {:?}", t))),
                };
                TypeSpec::StructRef(name)
            }
            Token::Union => {
                let name = match self.advance() {
                    Token::Ident(n) => n,
                    t => return Err(CError::new(1, format!("expected union name, got {:?}", t))),
                };
                TypeSpec::UnionRef(name)
            }
            Token::Ident(name) => {
                if let Some(typ) = self.typedefs.get(&name).cloned() {
                    typ
                } else {
                    return Err(CError::new(1, format!("expected type, got {:?}", Token::Ident(name))));
                }
            }
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
            Token::Goto => {
                self.advance();
                let label = match self.advance() {
                    Token::Ident(s) => s,
                    t => return Err(CError::new(1, format!("expected label name, got {:?}", t))),
                };
                self.skip_semicolon();
                Ok(Stmt::Goto(label))
            }
            Token::Semicolon => { self.advance(); Ok(Stmt::Block(vec![])) }
            _ => {
                // Try to parse as declaration if it starts with a type keyword
                if self.peek_is_type_start() {
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
                        self.skip_semicolon();
                        return Ok(Stmt::DeclAssign(typ, name, init));
                    }
                }
                self.parse_expr_stmt()
            }
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
        // check for declaration: for(int i = 0; ...)
        let has_decl = match self.peek() {
            Token::Int | Token::Char | Token::Short | Token::Long |
            Token::Void | Token::Unsigned | Token::Signed | Token::Float | Token::Double |
            Token::Struct | Token::Union | Token::Const | Token::Volatile => true,
            _ => false,
        };
        if has_decl {
            let save = self.pos;
            self.strip_qualifiers();
            let _typ = match self.parse_type_spec() {
                Ok(t) => t,
                Err(_) => { self.pos = save; return self.parse_for_expr(); }
            };
            let name = match self.advance() {
                Token::Ident(n) => n,
                _ => { self.pos = save; return self.parse_for_expr(); }
            };
            let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
            self.skip_semicolon();
            self.var_types.insert(name.clone(), _typ.clone());
            // wrap in Block: { type name = init; for(; cond; inc) body }
            let mut stmts = Vec::new();
            stmts.push(Stmt::DeclAssign(_typ, name, init));
            let cond = if *self.peek() == Token::Semicolon { None } else { Some(self.parse_expr()?) };
            self.skip_semicolon();
            let inc = if *self.peek() == Token::CloseParen { None } else { Some(self.parse_expr()?) };
            self.expect(&Token::CloseParen)?;
            let body = self.parse_stmt()?;
            stmts.push(Stmt::For(None, cond, inc, Box::new(body)));
            return Ok(Stmt::Block(stmts));
        }
        self.parse_for_expr()
    }

    fn parse_for_expr(&mut self) -> Result<Stmt, CError> {
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
                    // check for label: ident followed by colon
                    if let Token::Ident(name) = self.peek().clone() {
                        if self.pos + 1 < self.tokens.len() && self.tokens[self.pos + 1] == Token::Colon {
                            self.advance(); // consume ident
                            self.advance(); // consume colon
                            stmts.push(Stmt::Label(name));
                            continue;
                        }
                    }
                    if let Some((typ, name)) = self.try_parse_decl()? {
                        let init = if *self.peek() == Token::Assign { self.advance(); Some(self.parse_expr()?) } else { None };
                        self.skip_semicolon();
                        self.var_types.insert(name.clone(), typ.clone());
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
        let assign_op = |n: String, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let n2 = n.clone(); Expr::Assign(n, Box::new(op(Box::new(Expr::Var(n2)), Box::new(val))))
        };
        let field_assign_op = |e: Expr, f: String, off: u32, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            let lhs = Expr::Field(Box::new(e.clone()), f.clone(), off);
            Expr::AssignField(Box::new(e), f, off, Box::new(op(Box::new(lhs), Box::new(val))))
        };
        let arrow_assign_op = |e: Box<Expr>, f: String, off: u32, val: Expr, op: fn(Box<Expr>, Box<Expr>) -> Expr| {
            Expr::AssignArrow(e.clone(), f.clone(), off, Box::new(op(Box::new(Expr::Arrow(e, f, off)), Box::new(val))))
        };
        match self.peek() {
            Token::Assign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(Expr::Assign(n, Box::new(val))),
                Expr::Deref(a) => Ok(Expr::AssignDeref(a, Box::new(val))),
                Expr::Field(e, f, off) => Ok(Expr::AssignField(e, f, off, Box::new(val))),
                Expr::Arrow(e, f, off) => Ok(Expr::AssignArrow(e, f, off, Box::new(val))),
                _ => Ok(val),
            }}
            Token::AddAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Add)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Add)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Add)),
                _ => Ok(val),
            }}
            Token::SubAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Sub)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Sub)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Sub)),
                _ => Ok(val),
            }}
            Token::MulAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mul)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Mul)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Mul)),
                _ => Ok(val),
            }}
            Token::DivAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Div)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Div)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Div)),
                _ => Ok(val),
            }}
            Token::ModAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Mod)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Mod)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Mod)),
                _ => Ok(val),
            }}
            Token::ShlAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shl)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Shl)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Shl)),
                _ => Ok(val),
            }}
            Token::ShrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::Shr)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::Shr)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::Shr)),
                _ => Ok(val),
            }}
            Token::AndAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitAnd)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::BitAnd)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::BitAnd)),
                _ => Ok(val),
            }}
            Token::XorAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitXor)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::BitXor)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::BitXor)),
                _ => Ok(val),
            }}
            Token::OrAssign => { self.advance(); let val = self.parse_assign()?; match expr {
                Expr::Var(n) => Ok(assign_op(n, val, Expr::BitOr)),
                Expr::Field(e, f, off) => Ok(field_assign_op(*e, f, off, val, Expr::BitOr)),
                Expr::Arrow(e, f, off) => Ok(arrow_assign_op(e, f, off, val, Expr::BitOr)),
                _ => Ok(val),
            }}
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
            Token::And => { self.advance(); let expr = self.parse_unary()?; Ok(Expr::AddrOf(Box::new(expr))) }
            Token::Star => { self.advance(); let e = self.parse_unary()?; Ok(Expr::Deref(Box::new(e))) }
            Token::Sizeof => { self.advance(); self.expect(&Token::OpenParen)?; let t = self.parse_type_spec()?; self.expect(&Token::CloseParen)?; Ok(Expr::Int(t.stack_size() as i64)) }
            Token::OpenParen => {
                let save = self.pos;
                self.advance();
                // Try to parse as cast: (type)expr
                let is_cast = self.peek_is_type_start();
                if is_cast {
                    if let Ok(_typ) = self.parse_type_spec() {
                        if *self.peek() == Token::CloseParen {
                            self.advance();
                            let expr = self.parse_unary()?;
                            // cast is a no-op â€” just return the inner expression
                            return Ok(expr);
                        }
                    }
                }
                self.pos = save;
                self.parse_postfix()
            }
            _ => self.parse_postfix(),
        }
    }

    fn parse_postfix(&mut self) -> Result<Expr, CError> {
        let mut expr = self.parse_primary()?;
        loop {
            match self.peek() {
                Token::PlusPlus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostInc(n.clone()), _ => {} } }
                Token::MinusMinus => { self.advance(); match expr { Expr::Var(ref n) => expr = Expr::PostDec(n.clone()), _ => {} } }
                Token::OpenBracket => { self.advance(); let index = self.parse_expr()?; self.expect(&Token::CloseBracket)?; match &expr { Expr::Var(n) => { let scale = self.element_size(n); let n2 = n.clone(); expr = Expr::Subscript(n2, Box::new(index), scale); } _ => {} } }
                Token::Dot => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(1, format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_field_expr_offset(&expr, &field);
                    expr = Expr::Field(Box::new(expr), field, offset);
                }
                Token::Arrow => {
                    self.advance();
                    let field = match self.advance() {
                        Token::Ident(s) => s,
                        t => return Err(CError::new(1, format!("expected field name, got {:?}", t))),
                    };
                    let offset = self.resolve_arrow_expr_offset(&expr, &field);
                    expr = Expr::Arrow(Box::new(expr), field, offset);
                }
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
                        // Use parse_assign (not parse_expr) to avoid the comma operator
                        // consuming argument separators â€” C grammar requires
                        // argument_expression_list: assignment_expression (',' assignment_expression)*
                        args.push(self.parse_assign()?);
                        if *self.peek() == Token::Comma { self.advance(); }
                    }
                    self.expect(&Token::CloseParen)?;
                    // Check if this function name matches a known syscall definition
                    if let Some(def) = self.syscalls.get(&name).cloned() {
                        if args.len() != def.arg_count as usize {
                            return Err(CError::new(1, format!(
                                "syscall {}() expects {} arguments, got {}",
                                def.name, def.arg_count, args.len()
                            )));
                        }
                        Ok(Expr::Syscall(def, args))
                    } else {
                        Ok(Expr::Call(name, args))
                    }
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
        use bmo_abi::bef::sections::{SectionEntry, SectionKind};
        let bef = compile_source_to_bef("int main() { printf(\"HOLA C\"); return 0; }").unwrap();
        let sec_off = u64::from_le_bytes(bef[32..40].try_into().unwrap()) as usize;
        let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
        let count = hdr.section_count as usize;
        // Find rodata section
        let mut rodata_off = 0usize;
        let mut rodata_sz = 0usize;
        for i in 0..count {
            let entry_off = sec_off + i * SectionEntry::SIZE;
            let kind = bef[entry_off];
            if kind == SectionKind::RoData as u8 {
                rodata_off = u64::from_le_bytes(bef[entry_off+8..entry_off+16].try_into().unwrap()) as usize;
                rodata_sz = u64::from_le_bytes(bef[entry_off+16..entry_off+24].try_into().unwrap()) as usize;
                break;
            }
        }
        assert!(rodata_sz > 0, "rodata section not found");
        let rodata = &bef[rodata_off..rodata_off+rodata_sz];
        let end = rodata.iter().position(|&b| b == 0).unwrap();
        let s = core::str::from_utf8(&rodata[..end]).unwrap();
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

    #[test]
    fn handles_variable_assign_and_use() {
        let src = r#"
int main() {
    int x;
    x = 42;
    int y;
    y = x;
    return y;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_inc_dec() {
        let src = r#"
int main() {
    int x;
    x = 10;
    x = x + 1;
    x = x - 1;
    return x;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_pre_post_inc_dec() {
        let src = r#"
int main() {
    int x;
    x = 5;
    int a;
    a = ++x;
    a = --x;
    a = x++;
    a = x--;
    return x;
}
"#;
        let _p = parse(src).unwrap();
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_sizeof_types() {
        let src = r#"
int main() {
    int a;
    char b;
    long c;
    long long d;
    int* p;
    a = sizeof(int);
    a = sizeof(char);
    a = sizeof(long);
    a = sizeof(long long);
    a = sizeof(int*);
    return 0;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_function_call_codegen() {
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
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn handles_compound_assign() {
        let src = r#"
int main() {
    int x;
    x = 10;
    x += 5;
    x -= 3;
    x *= 2;
    x /= 4;
    x %= 3;
    return x;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_goto_and_label() {
        let src = "int main() { int x; x = 0; goto end; x = 1; end: return x; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_const_volatile() {
        let src = "int main() { const volatile int x; const int y; volatile int z; return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_float_double() {
        let src = "int main() { float f; double d; return 0; }";
        let p = parse(src).unwrap();
        assert!(p.functions.len() > 0);
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_for_decl() {
        let src = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 10; i = i + 1) {
        sum = sum + i;
    }
    return sum;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_escape_sequences() {
        let src = r#"int main() { char c; c = '\x41'; c = '\101'; printf("hello\x0aworld"); return 0; }"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_string_concat() {
        let src = r#"int main() { printf("hello " "world"); return 0; }"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_extern() {
        let src = "extern int global_var; int main() { return 0; }";
        let p = parse(src).unwrap();
        assert_eq!(p.globals.len(), 1);
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_struct_declaration() {
        let src = r#"
struct Point { int x; long y; };
int main() { return 0; }
"#;
        let p = parse(src).unwrap();
        assert_eq!(p.globals.len(), 1);
        match &p.globals[0] {
            GlobalDecl::Struct(name, members) => {
                assert_eq!(name, "Point");
                assert_eq!(members.len(), 2);
            }
            _ => panic!("expected struct decl"),
        }
    }

    #[test]
    fn parses_struct_field_access() {
        let src = r#"
struct Point { int x; long y; };
int main() {
    struct Point pt;
    pt.x = 10;
    pt.y = 20;
    int a;
    a = pt.y;
    return a;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_enum() {
        let src = r#"
enum Color { RED, GREEN, BLUE };
int main() { return 0; }
"#;
        let p = parse(src).unwrap();
        assert!(p.functions.len() > 0);
    }

    #[test]
    fn parses_use_directive() {
        let src = r#"use "bmo/core"; int main() { return 0; }"#;
        // tokenize and check
        let tokens = crate::Parser::tokenize_for_test(src);
        assert!(tokens.contains(&Token::Use), "should contain Use token");
    }

    #[test]
    fn handles_var_names_in_function() {
        let src = r#"
int sum(int a, int b, int c) {
    int t;
    t = a + b + c;
    return t;
}
"#;
        let p = parse(src).unwrap();
        assert_eq!(p.functions[0].var_names.len(), 4); // 3 params + 1 local
        assert_eq!(p.functions[0].var_names[0], "a");
        assert_eq!(p.functions[0].var_names[1], "b");
        assert_eq!(p.functions[0].var_names[2], "c");
        assert_eq!(p.functions[0].var_names[3], "t");
    }

    #[test]
    fn parses_cast_expression() {
        let src = "int main() { int x; x = (int)42; return x; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_typedef() {
        let src = "typedef unsigned int u32; u32 x; int main() { x = 42; return (int)x; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_array_decl() {
        let src = "int main() { int arr[4]; arr[0] = 1; return arr[0]; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_for_loop() {
        assert!(parse("void f() { }").is_ok());
        assert!(parse("int f() { }").is_ok());
        assert!(parse("void f() { int x; for(x = 0; x < 10; x = x + 1) { } }").is_ok());
        assert!(parse("void f() { for(;;); }").is_ok());
        assert!(parse("void f() { for(;;) { x = 0; } }").is_ok());
        assert!(parse("void f(char* fmt) { for(;;) { } }").is_ok());
        assert!(parse("void f(char* fmt) { int x; for(;;) { } }").is_ok());
        assert!(parse("void f(char* fmt) { int x; for (;;) { x = 0; } }").is_ok());
    }

    #[test]
    fn parses_syscall_direct() {
        // Test that a syscall (bmo_exit) is recognized when definitions are loaded
        let src = r#"use "bmo/proc"; int main() { bmo_exit(0); }"#;
        let p = parse(src).unwrap();
        // Without asm_path, bmo_exit is treated as a normal function call
        assert_eq!(p.functions.len(), 1);
    }

    #[test]
    fn parses_syscall_with_asm_defs() {
        use std::path::PathBuf;
        let src = r#"use "bmo/proc"; int main() { bmo_exit(42); }"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn syscall_arg_count_validation() {
        use std::path::PathBuf;
        // bmo_exit expects 1 arg â†’ passing 0 should fail
        let src = r#"use "bmo/proc"; int main() { bmo_exit(); }"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let result = compile_source_to_bef_with_all(src, vec![base], vec![asm]);
        assert!(result.is_err(), "should reject wrong arg count");
        if let Err(e) = result {
            assert!(e.message.contains("expects 1"), "error should mention expected arg count: {e:?}");
        }
    }

    #[test]
    fn syscall_multiple_categories() {
        use std::path::PathBuf;
        let src = r#"use "bmo/proc"; use "bmo/diag"; int main() { bmo_exit(0); bmo_debug_print("test", 4); }"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn syscall_all_toml_files_loadable() {
        use std::path::PathBuf;
        // Use every category to verify all .toml files load without error
        let src = r#"
use "bmo/proc";
use "bmo/fs";
use "bmo/mem";
use "bmo/input";
use "bmo/time";
use "bmo/diag";
use "bmo/wm";
use "bmo/draw";
use "bmo/winpaint";
use "bmo/compositor";
use "bmo/audio";
use "bmo/ipc";
use "bmo/surface";
int main() { bmo_exit(0); }
"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn syscall_emits_correct_code() {
        use std::path::PathBuf;
        let src = r#"use "bmo/proc"; int main() { bmo_exit(42); }"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        // BEF validation: magic, correct header, code section present
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
        // The emitted code should contain: mov eax, 0x181 (bmo_exit nr)
        let _code_start = 48; // BEF header is 48 bytes
        // Find b5 81 01 00 00 = mov eax, 0x181 (in little-endian)
        let mov_eax = &[0xB8u8, 0x81, 0x01, 0x00, 0x00]; // mov eax, 0x181
        let found = bef.windows(5).any(|w| w == mov_eax);
        assert!(found, "BEF output should contain mov eax, 0x181 for bmo_exit syscall");
        // Should contain syscall instruction (0F 05)
        let syscall = &[0x0F, 0x05];
        let found_syscall = bef.windows(2).any(|w| w == syscall);
        assert!(found_syscall, "BEF output should contain syscall instruction");
    }

    #[test]
    fn compiles_heap_module() {
        use std::path::PathBuf;
        // Load the heap stdlib module and the bmo/mem syscalls
        let src = r#"
use "bmo/mem";
use "stdlib/heap";
int main() {
    void *p = malloc(64);
    if (p == 0) return 1;
    free(p);
    return 0;
}
"#;
        let base = PathBuf::from("X:\\FastOS\\crates_Personal\\Lenguajes\\base");
        let asm = PathBuf::from("X:\\FastOS\\crates_Personal\\Semantic_ASM");
        // Need both base and Semantic_ASM as module search paths so stdlib/heap can be found
        let bef = compile_source_to_bef_with_all(src, vec![base, asm.clone()], vec![asm]).unwrap();
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
        // Should contain bmo_mem_alloc syscall mov eax, 0x190
        let mov_alloc = &[0xB8u8, 0x90, 0x01, 0x00, 0x00];
        assert!(bef.windows(5).any(|w| w == mov_alloc), "BEF should contain bmo_mem_alloc syscall");
        // Should contain bmo_mem_free syscall mov eax, 0x191
        let mov_free = &[0xB8u8, 0x91, 0x01, 0x00, 0x00];
        assert!(bef.windows(5).any(|w| w == mov_free), "BEF should contain bmo_mem_free syscall");
    }

    #[test]
    fn parses_assign_deref() {
        // Test that *ptr = val parsing and codegen works
        let src = r#"int main() {
    unsigned long x;
    unsigned long *p;
    p = &x;
    *p = 42;
    return x;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert_eq!(u32::from_le_bytes(bef[..4].try_into().unwrap()), bmo_abi::bef::BEF_MAGIC);
        // Verify that the codegen doesn't crash and returns valid BEF
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_ptr_string_init() {
        let src = r#"int main() { char *p = "hello"; return 0; }"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_field_on_subscript() {
        let src = r#"
struct Point { int x; int y; };
int main() {
    struct Point pts[2];
    pts[0].x = 10;
    return pts[0].x;
}
"#;
        let p = parse(src).unwrap();
        assert!(p.functions.len() > 0);
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn parses_compound_field_assign() {
        let src = r#"
struct Point { int x; int y; };
int main() {
    struct Point pt;
    pt.x = 5;
    pt.x = pt.x + 1;
    return pt.x;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn global_var_load_store() {
        let src = r#"
int g = 42;
int main() {
    int x;
    x = g;
    g = 100;
    return x;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn global_var_zero_init() {
        let src = r#"
int z;
int main() {
    z = 7;
    return z;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn global_var_addr_of() {
        let src = r#"
int g;
int main() {
    int *p = &g;
    *p = 99;
    return g;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn loads_via_bef_loader() {
        use bmo_abi::bef::loader::{load, no_imports};
        use bmo_abi::bef::sections::SectionKind;
        let bef = compile_source_to_bef("int main() { return 42; }").unwrap();
        let loaded = load(&bef, 0, no_imports).unwrap();
        assert!(loaded.entry_point > 0, "entry_point should be non-zero");
        let has_code = loaded.sections.iter().any(|s| s.kind == SectionKind::Code);
        assert!(has_code, "should have Code section");
        // Code section should contain a RET instruction at minimum
        let code = loaded.sections.iter().find(|s| s.kind == SectionKind::Code).unwrap();
        assert!(code.size >= 16, "code section should be at least 16 bytes");
        // Should have non-zero base address
        assert!(loaded.base_addr > 0, "base_addr should be non-zero");
    }

    #[test]
    fn loaded_bef_has_rodata() {
        use bmo_abi::bef::loader::{load, no_imports};
        use bmo_abi::bef::sections::SectionKind;
        let bef = compile_source_to_bef("int main() { printf(\"hello\"); return 0; }").unwrap();
        let loaded = load(&bef, 0, no_imports).unwrap();
        let has_rodata = loaded.sections.iter().any(|s| s.kind == SectionKind::RoData);
        assert!(has_rodata, "printf should create RoData section with the string");
    }

    #[test]
    fn loaded_bef_has_global_data() {
        use bmo_abi::bef::loader::{load, no_imports};
        use bmo_abi::bef::sections::SectionKind;
        let bef = compile_source_to_bef("int g = 42; int main() { return g; }").unwrap();
        let loaded = load(&bef, 0, no_imports).unwrap();
        let has_data = loaded.sections.iter().any(|s| s.kind == SectionKind::Data);
        assert!(has_data, "global vars should create Data section");
    }
}

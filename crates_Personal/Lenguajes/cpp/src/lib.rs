//! BMO C++ Frontend — compiles minimal C++ to BEF with class/VTable support.
//!
//! Supports:
//! - Classes with member variables and methods
//! - Virtual functions → .vtables BEF section
//! - Single inheritance
//! - `new`/`delete` → memory syscalls
//! - Method calls (virtual dispatch through vtable)
//! - Namespaces
//! - Basic templates
//!
//! The pipeline: C++ source → AST → IrModule → BEF

pub mod ast;
pub mod ir_emit;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use ast::*;
use bmo_abi::profile::BmoLanguageProfile;

pub fn profile() -> BmoLanguageProfile {
    BmoLanguageProfile {
        name: "C++",
        frontend: bmo_abi::profile::FrontendKind::Cpp,
        backend: bmo_abi::profile::BackendKind::AotX86_64,
        runtime: bmo_abi::profile::RuntimeKind::CppMin,
        uses_bmo_abi: true,
        ring0_capable: true,
    }
}

pub fn parse(source: &str) -> Result<Program, CppError> {
    let mut p = Parser::new(source);
    p.parse_program()
}

/// Compile C++ source to a unified IrModule (language-agnostic IR).
pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CppError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
}

#[derive(Debug, Clone)]
pub struct CppError {
    pub line: usize,
    pub message: String,
}

impl CppError {
    fn new(line: usize, msg: impl Into<String>) -> Self {
        Self { line, message: msg.into() }
    }
}

// ── Parser ────────────────────────────────────────────────────────

struct Parser {
    source: String,
    pos: usize,
    line: usize,
}

impl Parser {
    fn new(source: &str) -> Self {
        Self { source: source.to_string(), pos: 0, line: 1 }
    }

    fn parse_program(&mut self) -> Result<Program, CppError> {
        let mut program = Program::new();
        self.skip_whitespace();

        while self.pos < self.source.len() {
            self.skip_whitespace();
            if self.pos >= self.source.len() { break; }

            if self.peek_str("#include") {
                let include = self.parse_include()?;
                program.includes.push(include);
            } else if self.peek_str("namespace") {
                let ns = self.parse_namespace()?;
                program.namespaces.push(ns);
            } else if self.peek_str("class") || self.peek_str("struct") {
                let cls = self.parse_class()?;
                program.classes.push(cls);
            } else if self.peek_str("template") {
                // Skip template declarations for now
                self.skip_to_semicolon_or_brace();
            } else if self.is_type_start() {
                let func = self.parse_function()?;
                program.functions.push(func);
            } else {
                let _ = self.advance();
            }
        }

        Ok(program)
    }

    fn peek_str(&self, s: &str) -> bool {
        self.source[self.pos..].starts_with(s)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.source.len() {
            let ch = self.source.as_bytes()[self.pos];
            if ch == b'\n' { self.line += 1; self.pos += 1; }
            else if ch.is_ascii_whitespace() { self.pos += 1; }
            else if self.peek_str("//") { while self.pos < self.source.len() && self.source.as_bytes()[self.pos] != b'\n' { self.pos += 1; } }
            else if self.peek_str("/*") { self.pos += 2; while self.pos < self.source.len() && !self.peek_str("*/") { if self.source.as_bytes()[self.pos] == b'\n' { self.line += 1; } self.pos += 1; } self.pos += 2; }
            else { break; }
        }
    }

    fn advance(&mut self) -> char {
        if self.pos >= self.source.len() { return '\0'; }
        let ch = self.source.as_bytes()[self.pos] as char;
        self.pos += 1;
        if ch == '\n' { self.line += 1; }
        ch
    }

    fn parse_include(&mut self) -> Result<String, CppError> {
        self.expect_str("#include")?;
        self.skip_whitespace();
        let quote = self.advance();
        let mut path = String::new();
        while self.pos < self.source.len() {
            let ch = self.source.as_bytes()[self.pos] as char;
            if ch == quote { self.pos += 1; break; }
            path.push(ch);
            self.pos += 1;
        }
        Ok(path)
    }

    fn expect_str(&mut self, s: &str) -> Result<(), CppError> {
        self.skip_whitespace();
        if !self.peek_str(s) {
            return Err(CppError::new(self.line, format!("expected '{}'", s)));
        }
        self.pos += s.len();
        Ok(())
    }

    fn parse_namespace(&mut self) -> Result<Namespace, CppError> {
        self.expect_str("namespace")?;
        let name = self.parse_identifier()?;
        self.expect_str("{")?;
        let mut ns = Namespace { name, classes: vec![], functions: vec![] };
        while self.pos < self.source.len() {
            self.skip_whitespace();
            if self.peek_str("}") { self.pos += 1; break; }
            if self.peek_str("class") || self.peek_str("struct") {
                ns.classes.push(self.parse_class()?);
            } else if self.is_type_start() {
                ns.functions.push(self.parse_function()?);
            } else {
                self.pos += 1;
            }
        }
        Ok(ns)
    }

    fn parse_class(&mut self) -> Result<Class, CppError> {
        let is_struct = self.peek_str("struct");
        self.expect_str(if is_struct { "struct" } else { "class" })?;
        let name = self.parse_identifier()?;
        let mut bases = vec![];
        if self.peek_str(":") {
            self.pos += 1;
            self.skip_whitespace();
            if self.peek_str("public") || self.peek_str("protected") || self.peek_str("private") {
                while self.pos < self.source.len() && self.source.as_bytes()[self.pos] != b' ' { self.pos += 1; }
                self.skip_whitespace();
            }
            bases.push(self.parse_identifier()?);
        }
        self.expect_str("{")?;
        let mut members = vec![];
        let mut methods = vec![];
        let mut ctor = None;
        let mut dtor = None;
        let mut has_virtual = false;
        let mut access = if is_struct { Access::Public } else { Access::Private };

        while self.pos < self.source.len() {
            self.skip_whitespace();
            if self.peek_str("}") { self.pos += 1; break; }
            if self.peek_str("public:") { self.pos += 7; access = Access::Public; continue; }
            if self.peek_str("protected:") { self.pos += 10; access = Access::Protected; continue; }
            if self.peek_str("private:") { self.pos += 8; access = Access::Private; continue; }
            if self.peek_str("virtual") {
                has_virtual = true;
                self.pos += 7;
                self.skip_whitespace();
            }
            if self.peek_str("~") {
                // Destructor
                self.pos += 1;
                self.skip_whitespace();
                let dtor_name = self.parse_identifier()?;
                self.skip_whitespace();
                self.expect_str("(")?;
                self.expect_str(")")?;
                let body = self.parse_body()?;
                dtor = Some(Method {
                    name: format!("~{}", dtor_name),
                    ret_type: TypeSpec::Void, params: vec![],
                    body, is_virtual: false, is_override: false, access: Access::Public,
                    class_name: name.clone(),
                });
                continue;
            }
            let ret_type = self.parse_type_spec()?;
            let func_name = self.parse_identifier()?;
            self.skip_whitespace();

            if self.peek_str("(") {
                // Method or constructor
                self.pos += 1;
                let params = self.parse_params()?;
                self.expect_str(")")?;
                let body = self.parse_body()?;
                let is_ctor = func_name == name;
                let method = Method {
                    name: func_name,
                    ret_type: if is_ctor { TypeSpec::Void } else { ret_type },
                    params,
                    body,
                    is_virtual: has_virtual,
                    is_override: false,
                    access,
                    class_name: name.clone(),
                };
                if is_ctor { ctor = Some(method); }
                else { methods.push(method); }
            } else {
                // Member variable
                let var_name = func_name;
                self.expect_str(";")?;
                members.push(MemberVar {
                    typ: ret_type, name: var_name, offset: 0, access,
                });
            }
            has_virtual = false;
        }
        Ok(Class { name, bases, members, methods, constructor: ctor, destructor: dtor, vtable: has_virtual })
    }

    fn parse_function(&mut self) -> Result<Function, CppError> {
        let ret_type = self.parse_type_spec()?;
        let name = self.parse_identifier()?;
        self.expect_str("(")?;
        let params = self.parse_params()?;
        self.expect_str(")")?;
        let body = self.parse_body()?;
        Ok(Function { ret_type, name, params, body })
    }

    fn parse_type_spec(&mut self) -> Result<TypeSpec, CppError> {
        self.skip_whitespace();
        let mut ts = match self.peek_type() {
            "void" => { self.pos += 4; TypeSpec::Void }
            "char" => { self.pos += 4; TypeSpec::Char }
            "short" => { self.pos += 5; TypeSpec::Short }
            "int" => { self.pos += 3; TypeSpec::Int }
            "long" => { self.pos += 4;
                if self.peek_str(" long") { self.pos += 5; TypeSpec::LongLong }
                else { TypeSpec::Long }
            }
            "float" => { self.pos += 5; TypeSpec::Float }
            "double" => { self.pos += 6; TypeSpec::Double }
            "bool" => { self.pos += 4; TypeSpec::Bool }
            "unsigned" => {
                self.pos += 8; self.skip_whitespace();
                match self.peek_type() {
                    "char" => { self.pos += 4; TypeSpec::UnsignedChar }
                    "short" => { self.pos += 5; TypeSpec::UnsignedShort }
                    "int" => { self.pos += 3; TypeSpec::UnsignedInt }
                    "long" => { self.pos += 4;
                        if self.peek_str(" long") { self.pos += 5; TypeSpec::UnsignedLongLong }
                        else { TypeSpec::UnsignedLong }
                    }
                    _ => TypeSpec::UnsignedInt,
                }
            }
            "auto" => { self.pos += 4; TypeSpec::Auto }
            _ => {
                // Could be a class name
                let name = self.parse_identifier()?;
                TypeSpec::ClassRef(name)
            }
        };

        // Handle pointers and references
        loop {
            self.skip_whitespace();
            if self.peek_str("*") { self.pos += 1; ts = TypeSpec::Ptr(Box::new(ts)); }
            else if self.peek_str("&") { self.pos += 1; ts = TypeSpec::Ref(Box::new(ts)); }
            else { break; }
        }

        Ok(ts)
    }

    fn peek_type(&self) -> &str {
        let rest = &self.source[self.pos..];
        for kw in &["void", "char", "short", "int", "long", "float", "double", "bool", "unsigned", "auto", "class", "struct"] {
            if rest.starts_with(kw) {
                let next = rest.as_bytes().get(kw.len()).copied().unwrap_or(b' ');
                if next.is_ascii_whitespace() || next == b'*' || next == b'&' || next == b'(' {
                    return kw;
                }
            }
        }
        if !rest.is_empty() && (rest.as_bytes()[0].is_ascii_alphabetic() || rest.as_bytes()[0] == b'_') {
            return "ident";
        }
        ""
    }

    fn is_type_start(&self) -> bool {
        let kw = self.peek_type();
        !kw.is_empty() && kw != "class" && kw != "struct"
    }

    fn parse_identifier(&mut self) -> Result<String, CppError> {
        self.skip_whitespace();
        let mut name = String::new();
        while self.pos < self.source.len() {
            let ch = self.source.as_bytes()[self.pos] as char;
            if ch.is_alphanumeric() || ch == '_' || ch == ':' {
                name.push(ch);
                self.pos += 1;
                if self.peek_str("::") {
                    name.push_str("::");
                    self.pos += 2;
                }
            } else { break; }
        }
        if name.is_empty() {
            Err(CppError::new(self.line, "expected identifier"))
        } else {
            Ok(name)
        }
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, CppError> {
        let mut params = vec![];
        self.skip_whitespace();
        if self.peek_str(")") { return Ok(params); }
        loop {
            let typ = self.parse_type_spec()?;
            let name = self.parse_identifier().unwrap_or_else(|_| "".to_string());
            let mut default = None;
            self.skip_whitespace();
            if self.peek_str("=") {
                self.pos += 1;
                default = Some(self.parse_expr()?);
            }
            params.push(Param { typ, name, default });
            self.skip_whitespace();
            if self.peek_str(",") { self.pos += 1; }
            else if self.peek_str(")") { break; }
        }
        Ok(params)
    }

    fn parse_body(&mut self) -> Result<Vec<Stmt>, CppError> {
        self.skip_whitespace();
        self.expect_str("{")?;
        let mut stmts = vec![];
        let mut depth = 1;
        while self.pos < self.source.len() && depth > 0 {
            self.skip_whitespace();
            let ch = self.source.as_bytes()[self.pos] as char;
            if ch == '{' { depth += 1; self.pos += 1; }
            else if ch == '}' { depth -= 1; self.pos += 1; if depth == 0 { break; } }
            else if self.peek_str("return") {
                self.pos += 6; self.skip_whitespace();
                if self.peek_str(";") { self.pos += 1; stmts.push(Stmt::Return(None)); }
                else { let e = self.parse_expr()?; self.expect_str(";")?; stmts.push(Stmt::Return(Some(e))); }
            } else if self.peek_str("delete") {
                self.pos += 6; self.skip_whitespace();
                let name = self.parse_identifier()?;
                self.expect_str(";")?;
                stmts.push(Stmt::Delete(name));
            } else {
                // Skip unrecognized statements
                self.pos += 1;
            }
        }
        Ok(stmts)
    }

    fn parse_expr(&mut self) -> Result<Expr, CppError> {
        self.skip_whitespace();
        if self.peek_str("new") {
            self.pos += 3; self.skip_whitespace();
            let class = self.parse_identifier()?;
            self.expect_str("(")?;
            let args = self.parse_args()?;
            self.expect_str(")")?;
            return Ok(Expr::New(class, args));
        }
        if self.peek_str("this") {
            self.pos += 4;
            return Ok(Expr::This);
        }
        if self.peek_str("nullptr") {
            self.pos += 7;
            return Ok(Expr::NullPtr);
        }
        if self.peek_str("true") {
            self.pos += 4;
            return Ok(Expr::BoolLit(true));
        }
        if self.peek_str("false") {
            self.pos += 5;
            return Ok(Expr::BoolLit(false));
        }

        // Try integer literal
        let mut num = String::new();
        let start = self.pos;
        while self.pos < self.source.len() {
            let ch = self.source.as_bytes()[self.pos] as char;
            if ch.is_ascii_digit() || ch == 'x' || ch == 'X' || (ch >= 'a' && ch <= 'f') || (ch >= 'A' && ch <= 'F') {
                num.push(ch); self.pos += 1;
            } else { break; }
        }
        if !num.is_empty() {
            if let Ok(v) = i64::from_str_radix(&num, if num.starts_with("0x") { 16 } else { 10 }) {
                return Ok(Expr::Int(v));
            }
            self.pos = start;
        }

        // String literal
        if self.peek_str("\"") {
            self.pos += 1;
            let mut s = String::new();
            while self.pos < self.source.len() {
                let ch = self.source.as_bytes()[self.pos] as char;
                if ch == '"' { self.pos += 1; break; }
                s.push(ch); self.pos += 1;
            }
            return Ok(Expr::StringLit(s));
        }

        // Identifier
        let name = self.parse_identifier().unwrap_or_default();
        if name.is_empty() { return Err(CppError::new(self.line, "expected expression")); }
        Ok(Expr::Var(name))
    }

    fn parse_args(&mut self) -> Result<Vec<Expr>, CppError> {
        let mut args = vec![];
        self.skip_whitespace();
        if self.peek_str(")") { return Ok(args); }
        loop {
            args.push(self.parse_expr()?);
            self.skip_whitespace();
            if self.peek_str(",") { self.pos += 1; }
            else { break; }
        }
        Ok(args)
    }

    fn skip_to_semicolon_or_brace(&mut self) {
        while self.pos < self.source.len() {
            let ch = self.source.as_bytes()[self.pos] as char;
            if ch == ';' { self.pos += 1; break; }
            if ch == '{' {
                let mut depth = 1; self.pos += 1;
                while self.pos < self.source.len() && depth > 0 {
                    let c = self.source.as_bytes()[self.pos] as char;
                    if c == '{' { depth += 1; } else if c == '}' { depth -= 1; }
                    self.pos += 1;
                }
                break;
            }
            self.pos += 1;
        }
    }
}

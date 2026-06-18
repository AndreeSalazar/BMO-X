//! lang::nexo — Lenguaje de programación ÑEXO.
//!
//! Inspirado en CMD + Rust + Ada, ÑEXO es el lenguaje nativo de FastOS/BMO.
//! Compila a BMOasm como IR intermedio, que luego emite código nativo
//! vía el emitter de BareX.
//!
//! ## Pipeline
//!
//! ```text
//!   Fuente ÑEXO → Lexer → Parser → AST → Sema → BMOasm AST → Traductor → Native
//! ```
//!
//! ## Estado
//!
//! Fase 1: Lexer completo (32 keywords, hex/bin/oct, strings, escapes)
//! Fase 2: Parser completo (fn, let, if, while, for, struct, enum, impl, match)
//! Fase 3: Sema completo (scopes, tipos, funciones, structs)
//! Fase 4: Codegen → BmoAst (produces BMOasm IR)
//! Fase 5: Pipeline end-to-end

#![allow(dead_code)]

pub mod lexer;
pub mod parser;
pub mod sema;
pub mod codegen;
pub mod modules;
pub mod runtime;
pub mod c;
pub mod stdlib;
pub mod pm;
pub mod plugins;

#[cfg(test)]
pub mod tests;

use crate::barex::BxResult;

/// Versión del lenguaje ÑEXO.
pub const NEXO_VERSION: (u8, u8, u8) = (0, 1, 0);

/// Magic bytes del bytecode ÑEXO.
pub const NEXO_MAGIC: u32 = u32::from_le_bytes(*b"NEXO");

/// Compile ÑEXO source to native code via BMOasm.
///
/// Pipeline: source → lexer → parser → sema → codegen → BMOasm → traductor → native
pub fn compile(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    // 1. Lexing
    let mut lex = lexer::Lexer::new(source);
    let tokens = lex.tokenize()?;

    // 2. Parsing
    let mut parser = parser::Parser::new(&tokens);
    let ast = parser.parse()?;

    // 3. Semantic analysis
    let sema = sema::Sema::new();
    sema.check(&ast)?;

    // 4. Codegen → BMOasm AST
    let mut codegen = codegen::Codegen::new();
    let bmo_ast = codegen.emit(&ast)?;

    // 5. BMOasm → native code
    let mut traductor = crate::lang::bmoasm::traductor::Traductor::new();
    // Serialize BMOasm AST back to BMOasm source for the traductor
    let bmo_source = serialize_bmoasm(&bmo_ast);
    traductor.traducir(bmo_source.as_bytes())
}

/// Compile C source code to native code via ÑEXO.
///
/// Pipeline: C source → C lexer → C parser → C AST → ÑEXO AST → BMOasm → native
pub fn compile_c(source: &[u8]) -> BxResult<alloc::vec::Vec<u8>> {
    // 1. C Frontend: C source → ÑEXO AST
    let ast = c::compile_c(source)?;

    // 2. Semantic analysis
    let sema = sema::Sema::new();
    sema.check(&ast)?;

    // 3. Codegen → BMOasm AST
    let mut codegen = codegen::Codegen::new();
    let bmo_ast = codegen.emit(&ast)?;

    // 4. BMOasm → native code
    let mut traductor = crate::lang::bmoasm::traductor::Traductor::new();
    let bmo_source = serialize_bmoasm(&bmo_ast);
    traductor.traducir(bmo_source.as_bytes())
}

/// Serialize BMOasm AST back to BMOasm source text (for tests).
#[cfg(test)]
pub fn serialize_bmoasm_for_test(ast: &crate::lang::bmoasm::parser::ast::Ast) -> alloc::string::String {
    serialize_bmoasm(ast)
}

/// Serialize BMOasm AST back to BMOasm source text.
fn serialize_bmoasm(ast: &crate::lang::bmoasm::parser::ast::Ast) -> alloc::string::String {
    let mut out = alloc::string::String::new();
    for item in &ast.items {
        serialize_stmt(item, &mut out);
    }
    out
}

fn serialize_stmt(stmt: &crate::lang::bmoasm::parser::ast::Stmt, out: &mut alloc::string::String) {
    use crate::lang::bmoasm::parser::ast::{Stmt as S, Type as T};
    match stmt {
        S::Def { name, params, ret, body } => {
            out.push_str("def ");
            out.push_str(name);
            out.push('(');
            for (i, (pname, pty)) in params.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                out.push_str(pname);
                out.push_str(": ");
                out.push_str(&serialize_type(*pty));
            }
            out.push(')');
            if *ret != T::Void {
                out.push_str(" -> ");
                out.push_str(&serialize_type(*ret));
            }
            out.push_str(" {\n");
            for s in body {
                serialize_stmt(s, out);
                out.push('\n');
            }
            out.push_str("}\n");
        }
        S::Let { name, ty: _, value } => {
            out.push_str("    let ");
            out.push_str(name);
            out.push_str(" = ");
            serialize_expr(value, out);
            out.push('\n');
        }
        S::Retorna(Some(expr)) => {
            out.push_str("    retorna ");
            serialize_expr(expr, out);
            out.push('\n');
        }
        S::Retorna(None) => {
            out.push_str("    retorna\n");
        }
        S::Si { cond, then_body, else_body } => {
            out.push_str("    si ");
            serialize_expr(cond, out);
            out.push_str(" {\n");
            for s in then_body { serialize_stmt(s, out); out.push('\n'); }
            out.push_str("    }");
            if let Some(eb) = else_body {
                out.push_str(" sino {\n");
                for s in eb { serialize_stmt(s, out); out.push('\n'); }
                out.push_str("    }");
            }
            out.push('\n');
        }
        S::Mientras { cond, body } => {
            out.push_str("    mientras ");
            serialize_expr(cond, out);
            out.push_str(" {\n");
            for s in body { serialize_stmt(s, out); out.push('\n'); }
            out.push_str("    }\n");
        }
        S::Rompe => { out.push_str("    rompe\n"); }
        S::Continua => { out.push_str("    continua\n"); }
        S::Emit(bytes) => {
            out.push_str("    emit");
            for b in bytes {
                out.push_str(&alloc::format!(" 0x{:02X}", b));
            }
            out.push('\n');
        }
        S::RegAssign { reg, value } => {
            out.push_str("    reg ");
            out.push_str(reg);
            out.push_str(" = ");
            serialize_expr(value, out);
            out.push('\n');
        }
        S::ExprStmt(expr) => {
            out.push_str("    ");
            serialize_expr(expr, out);
            out.push('\n');
        }
        _ => {}
    }
}

fn serialize_expr(expr: &crate::lang::bmoasm::parser::ast::Expr, out: &mut alloc::string::String) {
    use crate::lang::bmoasm::parser::ast::{Expr as E, BinOp as B};
    match expr {
        E::LitInt(n) => { out.push_str(&alloc::format!("{}", n)); }
        E::LitByte(b) => { out.push_str(&alloc::format!("0x{:02X}", b)); }
        E::LitStr(s) => { out.push('"'); out.push_str(s); out.push('"'); }
        E::LitNulo => { out.push_str("nulo"); }
        E::Ident(name) => { out.push_str(name); }
        E::Bin(op, left, right) => {
            serialize_expr(left, out);
            out.push(' ');
            out.push_str(match op {
                B::Suma => "suma",
                B::Resta => "resta",
                B::Mult => "mult",
                B::Div => "div",
                B::Mod => "mod",
                B::Y => "y",
                B::O => "o",
                B::Xor => "xor",
                B::Shl => "shl",
                B::Shr => "shr",
                B::Igual => "igual",
                B::Mayor => "mayor",
                B::Menor => "menor",
                B::MayIg => "mayor_igual",
                B::MenIg => "menor_igual",
                B::Difer => "diferente",
            });
            out.push(' ');
            serialize_expr(right, out);
        }
        E::No(inner) => {
            out.push_str("no ");
            serialize_expr(inner, out);
        }
        E::Reg(name) => {
            out.push_str("reg ");
            out.push_str(name);
        }
        E::Aloc(size) => {
            out.push_str("aloc ");
            serialize_expr(size, out);
        }
        E::Call { name, args } => {
            out.push_str(name);
            out.push('(');
            for (i, arg) in args.iter().enumerate() {
                if i > 0 { out.push_str(", "); }
                serialize_expr(arg, out);
            }
            out.push(')');
        }
        E::Flag(flag) => {
            out.push_str(&alloc::format!("{:?}", flag));
        }
        E::MemOrder(mo, inner) => {
            out.push_str(&alloc::format!("{:?} ", mo));
            serialize_expr(inner, out);
        }
    }
}

fn serialize_type(ty: crate::lang::bmoasm::parser::ast::Type) -> &'static str {
    match ty {
        crate::lang::bmoasm::parser::ast::Type::Num => "num",
        crate::lang::bmoasm::parser::ast::Type::Byte => "byte",
        crate::lang::bmoasm::parser::ast::Type::Ptr => "ptr",
        crate::lang::bmoasm::parser::ast::Type::Arr => "arr",
        crate::lang::bmoasm::parser::ast::Type::Ref => "ref",
        crate::lang::bmoasm::parser::ast::Type::Void => "void",
    }
}


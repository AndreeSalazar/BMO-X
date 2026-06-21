//! BMO test suite — callable test functions for hardware/QEMU validation.
//!
//! Tests the complete pipeline: BMO source → lexer → parser → sema → codegen.

#![allow(dead_code)]

extern crate alloc;
use alloc::vec::Vec;
use alloc::format;
use alloc::string::String;

use super::compile;

fn nexo_compile(src: &[u8]) -> Result<Vec<u8>, String> {
    compile(src).map_err(|e| format!("{:?}", e))
}

pub fn test_nexo_lexer_basic() -> Result<(), String> {
    let mut lex = super::lexer::Lexer::new(b"fn main() { retorna 42 }");
    let tokens = lex.tokenize().map_err(|e| format!("{:?}", e))?;
    assert!(!tokens.is_empty(), "expected tokens");
    Ok(())
}

pub fn test_nexo_parser_fn() -> Result<(), String> {
    let mut lex = super::lexer::Lexer::new(b"fn main() -> num { retorna 42 }");
    let tokens = lex.tokenize().map_err(|e| format!("{:?}", e))?;
    let mut parser = super::parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|e| format!("{:?}", e))?;
    assert_eq!(ast.items.len(), 1, "expected 1 fn decl");
    Ok(())
}

pub fn test_nexo_parser_let() -> Result<(), String> {
    let mut lex = super::lexer::Lexer::new(b"fn main() { let x = 10 }");
    let tokens = lex.tokenize().map_err(|e| format!("{:?}", e))?;
    let mut parser = super::parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|e| format!("{:?}", e))?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_nexo_compile_return_42() -> Result<(), String> {
    let bytes = nexo_compile(b"fn main() -> num { retorna 42 }")?;
    let _ = bytes;
    Ok(())
}

pub fn test_nexo_compile_function_call() -> Result<(), String> {
    let src = b"fn add(a: num, b: num) -> num { retorna a } fn main() -> num { retorna add(1, 2) }";
    let bytes = nexo_compile(src)?;
    let _ = bytes;
    Ok(())
}

pub fn test_nexo_compile_arithmetic() -> Result<(), String> {
    let src = b"fn main() -> num { reg rax = 10 reg rcx = 20 reg rax = rax + rcx retorna rax }";
    let bytes = nexo_compile(src)?;
    let _ = bytes;
    Ok(())
}

pub fn test_nexo_compile_if_else() -> Result<(), String> {
    let src = b"fn main() -> num { si 1 { retorna 1 } sino { retorna 0 } }";
    let bytes = nexo_compile(src)?;
    let _ = bytes;
    Ok(())
}

pub fn test_nexo_compile_while_loop() -> Result<(), String> {
    let src = b"fn main() -> num { mientras 0 { } retorna 0 }";
    let bytes = nexo_compile(src)?;
    let _ = bytes;
    Ok(())
}

pub fn run_all_tests() -> Result<u32, String> {
    let tests: &[(&str, fn() -> Result<(), String>)] = &[
        ("nexo_lexer_basic", test_nexo_lexer_basic),
        ("nexo_parser_fn", test_nexo_parser_fn),
        ("nexo_parser_let", test_nexo_parser_let),
        ("nexo_compile_return_42", test_nexo_compile_return_42),
        ("nexo_compile_function_call", test_nexo_compile_function_call),
        ("nexo_compile_arithmetic", test_nexo_compile_arithmetic),
        ("nexo_compile_if_else", test_nexo_compile_if_else),
        ("nexo_compile_while_loop", test_nexo_compile_while_loop),
    ];

    let mut passed = 0u32;
    let mut failed = 0u32;
    for (_name, test_fn) in tests {
        match test_fn() {
            Ok(()) => { passed += 1; }
            Err(_msg) => { failed += 1; }
        }
    }
    if failed > 0 {
        Err(format!("{} tests failed", failed))
    } else {
        Ok(passed)
    }
}

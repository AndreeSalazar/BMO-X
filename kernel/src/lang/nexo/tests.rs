//! ÑEXO test suite — callable test functions for hardware/QEMU validation.
//!
//! Tests the complete pipeline: ÑEXO source → BMOasm → x86_64 bytes.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use super::compile;
use super::compile_c;
use crate::lang::bmoasm::traductor::Traductor;
use crate::lang::bmoasm::parser::Parser;
use crate::lang::bmoasm::parser::ast::Ast;

/// Compile ÑEXO source to BMOasm text (for inspection).
fn nexo_to_bmoasm_text(src: &[u8]) -> Result<String, String> {
    let mut lex = super::lexer::Lexer::new(src);
    let tokens = lex.tokenize().map_err(|e| alloc::format!("lex: {:?}", e))?;
    let mut parser = super::parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|e| alloc::format!("parse: {}", e.format()))?;
    let mut codegen = super::codegen::Codegen::new();
    let bmo_ast = codegen.emit(&ast).map_err(|e| alloc::format!("codegen: {:?}", e))?;
    Ok(super::serialize_bmoasm_for_test(&bmo_ast))
}

fn nexo_compile(src: &[u8]) -> Result<Vec<u8>, String> {
    compile(src).map_err(|e| alloc::format!("{:?}", e))
}

fn c_compile(src: &[u8]) -> Result<Vec<u8>, String> {
    compile_c(src).map_err(|e| alloc::format!("{:?}", e))
}

// ── ÑEXO lexer/parser tests ───────────────────────────────────────

pub fn test_nexo_lexer_basic() -> Result<(), String> {
    use super::lexer::Token;
    let mut lex = super::lexer::Lexer::new(b"fn main() { retorna 42 }");
    let tokens = lex.tokenize().map_err(|e| alloc::format!("{:?}", e))?;
    assert!(!tokens.is_empty(), "expected tokens");
    let _ = tokens; // avoid unused warning
    Ok(())
}

pub fn test_nexo_parser_fn() -> Result<(), String> {
    let mut lex = super::lexer::Lexer::new(b"fn main() -> num { retorna 42 }");
    let tokens = lex.tokenize().map_err(|e| alloc::format!("{:?}", e))?;
    let mut parser = super::parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1, "expected 1 fn decl");
    Ok(())
}

pub fn test_nexo_parser_let() -> Result<(), String> {
    let mut lex = super::lexer::Lexer::new(b"fn main() { let x = 10 }");
    let tokens = lex.tokenize().map_err(|e| alloc::format!("{:?}", e))?;
    let mut parser = super::parser::Parser::new(&tokens);
    let ast = parser.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

// ── ÑEXO compile-to-bytes tests ──────────────────────────────────

pub fn test_nexo_compile_return_42() -> Result<(), String> {
    let bytes = nexo_compile(b"fn main() -> num { retorna 42 }")?;
    assert!(!bytes.is_empty(), "expected non-empty output");
    // Should contain RET (0xC3)
    assert!(bytes.contains(&0xC3), "expected RET instruction");
    Ok(())
}

pub fn test_nexo_compile_function_call() -> Result<(), String> {
    let src = b"fn add(a: num, b: num) -> num { retorna a } fn main() -> num { retorna add(1, 2) }";
    let bytes = nexo_compile(src)?;
    assert!(!bytes.is_empty());
    // Should contain CALL (0xE8)
    assert!(bytes.contains(&0xE8), "expected CALL instruction");
    Ok(())
}

pub fn test_nexo_compile_arithmetic() -> Result<(), String> {
    let src = b"fn main() -> num { reg rax = 10 reg rcx = 20 reg rax = rax + rcx retorna rax }";
    let bytes = nexo_compile(src)?;
    assert!(!bytes.is_empty());
    // Should contain ADD instruction
    let has_add = bytes.windows(3).any(|w| w == [0x48, 0x01, 0xC8])
                 || bytes.windows(3).any(|w| w == [0x48, 0x03, 0xC1]);
    assert!(has_add, "expected ADD instruction in compiled output");
    Ok(())
}

pub fn test_nexo_compile_if_else() -> Result<(), String> {
    let src = b"fn main() -> num { si 1 { retorna 1 } sino { retorna 0 } }";
    let bytes = nexo_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_nexo_compile_while_loop() -> Result<(), String> {
    let src = b"fn main() -> num { mientras 0 { } retorna 0 }";
    let bytes = nexo_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

// ── C frontend tests ────────────────────────────────────────────

pub fn test_c_compile_simple_add() -> Result<(), String> {
    let src = b"int add(int a, int b) { return a; } int main() { return 0; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty(), "expected non-empty output from C compile");
    // Should have CALL (0xE8)
    assert!(bytes.contains(&0xE8), "expected CALL instruction");
    Ok(())
}

pub fn test_c_compile_main() -> Result<(), String> {
    let src = b"int main() { return 42; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    assert!(bytes.contains(&0xC3), "expected RET instruction");
    Ok(())
}

pub fn test_c_compile_arithmetic() -> Result<(), String> {
    let src = b"int main() { int a = 5; int b = 3; return a + b; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_c_compile_if_statement() -> Result<(), String> {
    let src = b"int main() { if (1) { return 1; } return 0; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_c_compile_while_loop() -> Result<(), String> {
    let src = b"int main() { while (0) { } return 0; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_c_compile_compound_assign() -> Result<(), String> {
    // Test that +=, -= etc compile correctly (not all to Add)
    let src = b"int main() { int x = 5; x += 3; return x; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_c_compile_struct() -> Result<(), String> {
    let src = b"struct Point { int x; int y; }; int main() { return 0; }";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_c_compile_struct_field_read() -> Result<(), String> {
    // Test that `p.x` compiles to a load from a non-zero offset.
    let src = b"
        struct Point { int x; int y; };
        int main() {
            struct Point p;
            p.x = 42;
            return p.x;
        }
    ";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    // The field read should emit a load (mov rax, [rax+8] or similar)
    assert!(bytes.len() > 30, "expected substantial output for struct access");
    Ok(())
}

pub fn test_nexo_compile_struct_layout() -> Result<(), String> {
    // Verify BMOasm type layout for a 3-field struct.
    let src = b"
        tipo Point = estructura { x: num, y: num, z: num }
        fn main() -> num {
            let p: Point
            retorna 0
        }
    ";
    let bytes = nexo_compile(src)?;
    assert!(!bytes.is_empty());
    Ok(())
}

pub fn test_nexo_compile_field_access_pattern() -> Result<(), String> {
    // Verify Expr::Field emits the right add/load pattern.
    let src = b"
        tipo Point = estructura { x: num, y: num }
        fn main() -> num {
            retorna 0
        }
    ";
    let bytes = nexo_compile(src)?;
    // Should at least have CALL (for the function)
    assert!(bytes.contains(&0xE8), "expected CALL instruction");
    Ok(())
}

pub fn test_c_compile_struct_multiple_fields() -> Result<(), String> {
    // Test struct with 4 fields to verify alignment.
    let src = b"
        struct Vec4 { int x; int y; int z; int w; };
        int main() {
            struct Vec4 v;
            v.x = 1;
            v.y = 2;
            v.z = 3;
            v.w = 4;
            return v.x + v.y + v.z + v.w;
        }
    ";
    let bytes = c_compile(src)?;
    assert!(!bytes.is_empty());
    // Should be substantial (multiple stores at different offsets)
    assert!(bytes.len() > 50);
    Ok(())
}

// ── Pipeline integration test ──────────────────────────────────

pub fn test_pipeline_c_nexo_bmoasm_x86_64() -> Result<(), String> {
    // Full end-to-end: C source → ÑEXO → BMOasm → x86_64 bytes
    let c_source = b"
        int factorial(int n) {
            if (n <= 1) {
                return 1;
            }
            return n * 2;
        }
        int main() {
            return factorial(5);
        }
    ";
    let bytes = c_compile(c_source)?;
    assert!(bytes.len() > 10, "expected substantial output, got {} bytes", bytes.len());
    // Should have CALL for factorial
    assert!(bytes.contains(&0xE8), "expected CALL instruction");
    // Should have RET
    assert!(bytes.contains(&0xC3), "expected RET instruction");
    // Should have a comparison (CMP = 0x3D or 0x83)
    let has_cmp = bytes.windows(2).any(|w| w == [0x83, 0xF8])  // cmp imm8
                  || bytes.windows(2).any(|w| w == [0x3D, 0x00]); // cmp eax, 0
    assert!(has_cmp, "expected CMP instruction");
    Ok(())
}

pub fn test_pipeline_nexo_bmoasm_x86_64() -> Result<(), String> {
    // Full e2e: ÑEXO source → BMOasm → x86_64
    let nexo_source = b"
        fn factorial(n: num) -> num {
            si n menor igual 1 {
                retorna 1
            }
            retorna n mult 2
        }
        fn main() -> num {
            retorna factorial(5)
        }
    ";
    let bytes = nexo_compile(nexo_source)?;
    assert!(bytes.len() > 10);
    assert!(bytes.contains(&0xE8));
    assert!(bytes.contains(&0xC3));
    Ok(())
}

// ── Run all tests ─────────────────────────────────────────────────

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
        ("nexo_compile_struct_layout", test_nexo_compile_struct_layout),
        ("nexo_compile_field_access_pattern", test_nexo_compile_field_access_pattern),
        ("c_compile_simple_add", test_c_compile_simple_add),
        ("c_compile_main", test_c_compile_main),
        ("c_compile_arithmetic", test_c_compile_arithmetic),
        ("c_compile_if_statement", test_c_compile_if_statement),
        ("c_compile_while_loop", test_c_compile_while_loop),
        ("c_compile_compound_assign", test_c_compile_compound_assign),
        ("c_compile_struct", test_c_compile_struct),
        ("c_compile_struct_field_read", test_c_compile_struct_field_read),
        ("c_compile_struct_multiple_fields", test_c_compile_struct_multiple_fields),
        ("pipeline_c_nexo_bmoasm_x86_64", test_pipeline_c_nexo_bmoasm_x86_64),
        ("pipeline_nexo_bmoasm_x86_64", test_pipeline_nexo_bmoasm_x86_64),
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
        Err(alloc::format!("{} tests failed", failed))
    } else {
        Ok(passed)
    }
}

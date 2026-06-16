//! BMOasm test suite — callable test functions for hardware/QEMU validation.
//!
//! Each function returns Ok(count) on success, Err(msg) on failure.
//! Call from kernel diagnostic infrastructure.

extern crate alloc;
use alloc::string::String;
use alloc::vec::Vec;

use super::traductor::Traductor;
use super::emit::TargetArch;
use super::parser::Parser;

fn run_bmoasm_test(name: &str, src: &[u8]) -> Result<Vec<u8>, String> {
    let mut trad = Traductor::new();
    trad.traducir(src).map_err(|e| {
        alloc::format!("{}: BxError {:?}", name, e)
    })
}

// ── Lexer tests ───────────────────────────────────────────────────

pub fn test_lexer_basic_tokens() -> Result<(), String> {
    use super::lexer::{Scanner, TokenKind};
    let mut sc = Scanner::new(b"def main() { let x = 42 }");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::KwDef, "expected KwDef");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::Ident, "expected Ident");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::LParen, "expected LParen");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::RParen, "expected RParen");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::LBrace, "expected LBrace");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::KwLet, "expected KwLet");
    Ok(())
}

pub fn test_lexer_line_tracking() -> Result<(), String> {
    use super::lexer::Scanner;
    let mut sc = Scanner::new(b"def\nmain");
    sc.next_token(); // skip 'def', advances past \n
    let (line, _col) = sc.current_loc();
    assert!(line >= 2, "expected line >= 2 after newline, got {}", line);
    Ok(())
}

pub fn test_lexer_string_literal() -> Result<(), String> {
    use super::lexer::{Scanner, TokenKind};
    let mut sc = Scanner::new(b"\"hola mundo\"");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::LitStr, "expected LitStr");
    assert_eq!(tok.len, 12, "expected len 12 (including quotes)");
    Ok(())
}

pub fn test_lexer_hex_literal() -> Result<(), String> {
    use super::lexer::{Scanner, TokenKind};
    let mut sc = Scanner::new(b"0xFF");
    let tok = sc.next_token();
    assert_eq!(tok.kind, TokenKind::LitHex, "expected LitHex");
    assert_eq!(tok.value, 0xFF, "expected value 0xFF");
    Ok(())
}

pub fn test_lexer_keywords() -> Result<(), String> {
    use super::lexer::{Scanner, TokenKind};
    let mut sc = Scanner::new(b"match caso defecto para desde hasta paso bucle etiqueta salto");
    assert_eq!(sc.next_token().kind, TokenKind::KwMatch);
    assert_eq!(sc.next_token().kind, TokenKind::KwCaso);
    assert_eq!(sc.next_token().kind, TokenKind::KwDefecto);
    assert_eq!(sc.next_token().kind, TokenKind::KwPara);
    assert_eq!(sc.next_token().kind, TokenKind::KwDesde);
    assert_eq!(sc.next_token().kind, TokenKind::KwHasta);
    assert_eq!(sc.next_token().kind, TokenKind::KwPaso);
    assert_eq!(sc.next_token().kind, TokenKind::KwBucle);
    assert_eq!(sc.next_token().kind, TokenKind::KwEtiqueta);
    assert_eq!(sc.next_token().kind, TokenKind::KwSalto);
    Ok(())
}

// ── Parser tests ──────────────────────────────────────────────────

pub fn test_parser_def_function() -> Result<(), String> {
    let mut p = Parser::new(b"def main() -> num { retorna 42 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1, "expected 1 item");
    Ok(())
}

pub fn test_parser_params() -> Result<(), String> {
    let mut p = Parser::new(b"def add(a: num, b: num) -> num { retorna a }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_let_bindings() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { let x = 10 let y = 20 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_si_sino() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { si 1 { let x = 1 } sino { let x = 2 } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_mientras() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { mientras 1 { } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_match_caso() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { match 1 { caso 1 => { } defecto => { } } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_para_loop() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { para i desde 0 hasta 10 { } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_para_with_step() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { para i desde 0 hasta 10 paso 2 { } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_bucle() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { bucle { rompe } }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_etiqueta_salto() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { etiqueta inicio salto inicio }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_function_call() -> Result<(), String> {
    let mut p = Parser::new(b"def foo() -> num { retorna 1 } def main() { foo() }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 2);
    Ok(())
}

pub fn test_parser_binary_ops() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { let x = 1 suma 2 mult 3 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_bitwise_ops() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { let x = 1 xor 2 shl 3 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_emit() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { emit 0x90 0x90 0xCC }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_aloc() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { let p = aloc 1024 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_reg_assign() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { reg rax = 42 }");
    let ast = p.parse().map_err(|e| e.format())?;
    assert_eq!(ast.items.len(), 1);
    Ok(())
}

pub fn test_parser_error_line_col() -> Result<(), String> {
    let mut p = Parser::new(b"def main() { let = }");
    let err = p.parse().unwrap_err();
    let msg = err.format();
    assert!(msg.contains("line"), "error should contain line info: {}", msg);
    assert!(msg.contains("col"), "error should contain col info: {}", msg);
    Ok(())
}

// ── Traductor (codegen) tests ─────────────────────────────────────

pub fn test_codegen_def_simple() -> Result<(), String> {
    run_bmoasm_test("def_simple", b"def main() { retorna 0 }")?;
    Ok(())
}

pub fn test_codegen_let_binding() -> Result<(), String> {
    run_bmoasm_test("let_binding", b"def main() { let x = 42 }")?;
    Ok(())
}

pub fn test_codegen_reg_assign() -> Result<(), String> {
    run_bmoasm_test("reg_assign", b"def main() { reg rax = 42 }")?;
    Ok(())
}

pub fn test_codegen_emit() -> Result<(), String> {
    let bytes = run_bmoasm_test("emit", b"def main() { emit 0x90 0x90 }")?;
    assert!(bytes.windows(2).any(|w| w == [0x90, 0x90]), "expected NOP NOP");
    Ok(())
}

pub fn test_codegen_ret() -> Result<(), String> {
    let bytes = run_bmoasm_test("ret", b"def main() { retorna 0 }")?;
    assert!(!bytes.is_empty(), "expected non-empty output");
    Ok(())
}

pub fn test_codegen_if_else() -> Result<(), String> {
    run_bmoasm_test("if_else", b"def main() { si 1 { let x = 1 } sino { let x = 2 } }")?;
    Ok(())
}

pub fn test_codegen_while_loop() -> Result<(), String> {
    run_bmoasm_test("while_loop", b"def main() { mientras 1 { rompe } }")?;
    Ok(())
}

pub fn test_codegen_match() -> Result<(), String> {
    run_bmoasm_test("match", b"def main() { match 1 { caso 1 => { } defecto => { } } }")?;
    Ok(())
}

pub fn test_codegen_para_loop() -> Result<(), String> {
    run_bmoasm_test("para_loop", b"def main() { para i desde 0 hasta 10 { } }")?;
    Ok(())
}

pub fn test_codegen_bucle() -> Result<(), String> {
    run_bmoasm_test("bucle", b"def main() { bucle { rompe } }")?;
    Ok(())
}

pub fn test_codegen_function_call() -> Result<(), String> {
    run_bmoasm_test("fn_call", b"def foo() -> num { retorna 1 } def main() { foo() }")?;
    Ok(())
}

pub fn test_codegen_label_goto() -> Result<(), String> {
    run_bmoasm_test("label_goto", b"def main() { etiqueta inicio salto inicio }")?;
    Ok(())
}

pub fn test_codegen_binary_ops() -> Result<(), String> {
    run_bmoasm_test("bin_ops", b"def main() { let x = 1 suma 2 }")?;
    Ok(())
}

pub fn test_codegen_comparison() -> Result<(), String> {
    run_bmoasm_test("cmp", b"def main() { let x = 1 igual 2 }")?;
    Ok(())
}

pub fn test_codegen_string_literal() -> Result<(), String> {
    let bytes = run_bmoasm_test("string", b"def main() { reg rax = \"hola\" }")?;
    assert!(bytes.windows(5).any(|w| w == b"hola\0"), "expected 'hola\\0' in output");
    Ok(())
}

pub fn test_codegen_syscall() -> Result<(), String> {
    let bytes = run_bmoasm_test("syscall", b"def main() { syscall }")?;
    assert!(bytes.windows(2).any(|w| w == [0x0F, 0x05]), "expected syscall opcode");
    Ok(())
}

pub fn test_codegen_nop() -> Result<(), String> {
    let bytes = run_bmoasm_test("nop", b"def main() { nop }")?;
    assert!(bytes.contains(&0x90), "expected NOP 0x90");
    Ok(())
}

pub fn test_codegen_multi_arch() -> Result<(), String> {
    for arch in &[TargetArch::X86_64, TargetArch::Aarch64, TargetArch::Riscv64] {
        let mut trad = Traductor::with_target(*arch);
        trad.traducir(b"def main() { reg rax = 42 }")
            .map_err(|e| alloc::format!("arch {:?} failed: {:?}", arch, e))?;
    }
    Ok(())
}

// ── Run all tests ─────────────────────────────────────────────────

pub fn run_all_tests() -> Result<u32, String> {
    let tests: &[(&str, fn() -> Result<(), String>)] = &[
        ("lexer_basic_tokens", test_lexer_basic_tokens),
        ("lexer_line_tracking", test_lexer_line_tracking),
        ("lexer_string_literal", test_lexer_string_literal),
        ("lexer_hex_literal", test_lexer_hex_literal),
        ("lexer_keywords", test_lexer_keywords),
        ("parser_def_function", test_parser_def_function),
        ("parser_params", test_parser_params),
        ("parser_let_bindings", test_parser_let_bindings),
        ("parser_si_sino", test_parser_si_sino),
        ("parser_mientras", test_parser_mientras),
        ("parser_match_caso", test_parser_match_caso),
        ("parser_para_loop", test_parser_para_loop),
        ("parser_para_with_step", test_parser_para_with_step),
        ("parser_bucle", test_parser_bucle),
        ("parser_etiqueta_salto", test_parser_etiqueta_salto),
        ("parser_function_call", test_parser_function_call),
        ("parser_binary_ops", test_parser_binary_ops),
        ("parser_bitwise_ops", test_parser_bitwise_ops),
        ("parser_emit", test_parser_emit),
        ("parser_aloc", test_parser_aloc),
        ("parser_reg_assign", test_parser_reg_assign),
        ("parser_error_line_col", test_parser_error_line_col),
        ("codegen_def_simple", test_codegen_def_simple),
        ("codegen_let_binding", test_codegen_let_binding),
        ("codegen_reg_assign", test_codegen_reg_assign),
        ("codegen_emit", test_codegen_emit),
        ("codegen_ret", test_codegen_ret),
        ("codegen_if_else", test_codegen_if_else),
        ("codegen_while_loop", test_codegen_while_loop),
        ("codegen_match", test_codegen_match),
        ("codegen_para_loop", test_codegen_para_loop),
        ("codegen_bucle", test_codegen_bucle),
        ("codegen_function_call", test_codegen_function_call),
        ("codegen_label_goto", test_codegen_label_goto),
        ("codegen_binary_ops", test_codegen_binary_ops),
        ("codegen_comparison", test_codegen_comparison),
        ("codegen_string_literal", test_codegen_string_literal),
        ("codegen_syscall", test_codegen_syscall),
        ("codegen_nop", test_codegen_nop),
        ("codegen_multi_arch", test_codegen_multi_arch),
    ];

    let mut passed = 0u32;
    let mut failed = 0u32;
    for (_name, test_fn) in tests {
        match test_fn() {
            Ok(()) => { passed += 1; }
            Err(_msg) => {
                failed += 1;
            }
        }
    }
    if failed > 0 {
        Err(alloc::format!("{} tests failed", failed))
    } else {
        Ok(passed)
    }
}

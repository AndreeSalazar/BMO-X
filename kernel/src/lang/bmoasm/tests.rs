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
use super::parser::ast::{Ast, Stmt, Expr};

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

// ── Optimizer tests (v0.4.0) ──────────────────────────────────────

fn parse_to_ast(src: &[u8]) -> Result<Ast, String> {
    let mut parser = Parser::new(src);
    parser.parse().map_err(|e| e.format())
}

pub fn test_opt_constant_folding() -> Result<(), String> {
    use crate::lang::bmoasm::sema::fold::Folder;
    let mut ast = parse_to_ast(b"def main() { let x = 2 suma 3 }")?;
    Folder::fold(&mut ast);
    // The let binding should now have LitInt(5) instead of Bin(...)
    if let Stmt::Def { body, .. } = &ast.items[0] {
        if let Stmt::Let { value, .. } = &body[0] {
            if let Expr::LitInt(v) = value {
                assert_eq!(*v, 5, "expected 5 after folding 2+3");
                return Ok(());
            }
        }
    }
    Err("constant folding did not produce LitInt".into())
}

pub fn test_opt_unused_let() -> Result<(), String> {
    use crate::lang::bmoasm::sema::opt::Optimizer;
    let mut ast = parse_to_ast(b"def main() { let x = 42 let y = 10 retorna 0 }")?;
    Optimizer::optimize(&mut ast);
    if let Stmt::Def { body, .. } = &ast.items[0] {
        // Both x and y are unused, so only the return should remain
        let count = body.iter().filter(|s| matches!(s, Stmt::Let { .. })).count();
        assert_eq!(count, 0, "expected 0 lets, got {}", count);
    }
    Ok(())
}

pub fn test_opt_algebraic() -> Result<(), String> {
    use crate::lang::bmoasm::sema::opt::Optimizer;
    let mut ast = parse_to_ast(b"def main() { let x = 5 mult 1 }")?;
    Optimizer::optimize(&mut ast);
    if let Stmt::Def { body, .. } = &ast.items[0] {
        if let Stmt::Let { value, .. } = &body[0] {
            if let Expr::Ident(name) = value {
                assert_eq!(name, "x", "expected x * 1 → x simplification");
                return Ok(());
            }
        }
    }
    Err("x * 1 should simplify to x".into())
}

pub fn test_opt_dead_branch() -> Result<(), String> {
    use crate::lang::bmoasm::sema::opt::Optimizer;
    let mut ast = parse_to_ast(b"def main() { si 0 { let a = 1 } sino { let b = 2 } }")?;
    Optimizer::optimize(&mut ast);
    if let Stmt::Def { body, .. } = &ast.items[0] {
        if let Stmt::Si { then_body, else_body, .. } = &body[0] {
            // si 0 { a } sino { b } → sino { b } with cond=1
            // then_body should now contain the else content
            let has_let_b = then_body.iter().any(|s| {
                if let Stmt::Let { name, .. } = s { name == "b" } else { false }
            });
            assert!(has_let_b, "expected `let b = 2` after si 0 .. sino optimization");
            assert!(else_body.is_none(), "expected no else_body after optimization");
            return Ok(());
        }
    }
    Err("dead branch optimization failed".into())
}

pub fn test_opt_strength_reduction() -> Result<(), String> {
    use crate::lang::bmoasm::sema::opt::Optimizer;
    let mut ast = parse_to_ast(b"def main() { let x = 5 mult 4 }")?;
    Optimizer::optimize(&mut ast);
    if let Stmt::Def { body, .. } = &ast.items[0] {
        if let Stmt::Let { value, .. } = &body[0] {
            if let Expr::Bin(op, _, right) = value {
                use crate::lang::bmoasm::parser::ast::BinOp;
                if matches!(op, BinOp::Shl) {
                    if let Expr::LitInt(shift) = right.as_ref() {
                        assert_eq!(*shift, 2, "expected 5 * 4 → 5 << 2 (shift=2)");
                        return Ok(());
                    }
                }
            }
        }
    }
    Err("strength reduction did not convert 5*4 to 5<<2".into())
}

// ── DCE tests ─────────────────────────────────────────────────────

pub fn test_dce_unused_function() -> Result<(), String> {
    use crate::lang::bmoasm::sema::dce::Dce;
    let mut ast = parse_to_ast(b"def unused() -> num { retorna 1 } def main() { retorna 0 }")?;
    let before = ast.items.len();
    Dce::eliminate(&mut ast);
    let after = ast.items.len();
    assert!(after < before, "DCE should remove unused function (before={}, after={})", before, after);
    Ok(())
}

pub fn test_dce_unreachable_code() -> Result<(), String> {
    use crate::lang::bmoasm::sema::dce::Dce;
    let mut ast = parse_to_ast(b"def main() { retorna 0 let x = 42 let y = 100 }")?;
    Dce::eliminate(&mut ast);
    if let Stmt::Def { body, .. } = &ast.items[0] {
        // After DCE, only the return should remain
        assert_eq!(body.len(), 1, "expected 1 stmt after DCE, got {}", body.len());
    }
    Ok(())
}

// ── e2e pipeline test ────────────────────────────────────────────

pub fn test_e2e_hello_function() -> Result<(), String> {
    // Simulates compiling a complete BMOasm program
    let src = b"
        def add(a: num, b: num) -> num {
            retorna a suma b
        }
        def main() -> num {
            reg rax = 0
            reg rdi = 3
            reg rsi = 4
            call add
            retorna rax
        }
    ";
    let mut trad = Traductor::new();
    let bytes = trad.traducir(src).map_err(|e| alloc::format!("compile failed: {:?}", e))?;
    // The output should be non-empty
    assert!(!bytes.is_empty(), "expected non-empty output");
    // Should contain a CALL instruction (0xE8)
    assert!(bytes.contains(&0xE8), "expected CALL instruction in output");
    // Should contain a RET instruction (0xC3)
    assert!(bytes.contains(&0xC3), "expected RET instruction in output");
    Ok(())
}

pub fn test_e2e_arithmetic() -> Result<(), String> {
    let src = b"def main() -> num {
        reg rax = 10
        reg rcx = 20
        reg rax = rax suma rcx
        retorna rax
    }";
    let mut trad = Traductor::new();
    let bytes = trad.traducir(src).map_err(|e| alloc::format!("{:?}", e))?;
    // Should contain ADD (0x01 with REX.W prefix 0x48)
    let has_add = bytes.windows(3).any(|w| w == [0x48, 0x01, 0xC8] || w == [0x48, 0x03, 0xC1]);
    assert!(has_add || bytes.windows(2).any(|w| w == [0x01, 0xC8]), "expected ADD instruction");
    Ok(())
}

pub fn test_e2e_function_codegen_all_archs() -> Result<(), String> {
    for arch in &[TargetArch::X86_64, TargetArch::Aarch64, TargetArch::Riscv64] {
        let mut trad = Traductor::with_target(*arch);
        let bytes = trad.traducir(b"def add(a: num, b: num) -> num { retorna a suma b } def main() -> num { retorna add(1, 2) }")
            .map_err(|e| alloc::format!("arch {:?}: {:?}", arch, e))?;
        assert!(bytes.len() > 4, "arch {:?}: expected non-trivial output, got {} bytes", arch, bytes.len());
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
        // v0.4.0 optimizer tests
        ("opt_constant_folding", test_opt_constant_folding),
        ("opt_unused_let", test_opt_unused_let),
        ("opt_algebraic", test_opt_algebraic),
        ("opt_dead_branch", test_opt_dead_branch),
        ("opt_strength_reduction", test_opt_strength_reduction),
        // DCE tests
        ("dce_unused_function", test_dce_unused_function),
        ("dce_unreachable_code", test_dce_unreachable_code),
        // e2e pipeline tests
        ("e2e_hello_function", test_e2e_hello_function),
        ("e2e_arithmetic", test_e2e_arithmetic),
        ("e2e_function_codegen_all_archs", test_e2e_function_codegen_all_archs),
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

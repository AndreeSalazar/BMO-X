//! `lang::tests` — Tests integrados del compilador.
//!
//! Como el kernel es no_std, no podemos usar `#[cfg(test)]`. En su
//! lugar, estos tests se ejecutan manualmente desde el boot
//! (ver `boot::smoke::run_lang_tests`).
//!
//! ## Tests disponibles
//!
//! - `hello_world_bmo()` — compila un "Hello, World!" en BMO
//!   y verifica que produce > 0 bytes de x86-64.

#![allow(dead_code)]

use crate::bmo_gpu::BxResult;
use crate::lang::common::ast::Module;
use crate::lang::pipeline::{compile, CompiledProgram, SourceLang};

/// Resultado de un test: nombre + pass/fail + info.
pub struct TestResult {
    pub name: &'static str,
    pub passed: bool,
    pub message: alloc::string::String,
}

/// Ejecuta todos los tests del compilador. Retorna los resultados.
pub fn run_all() -> alloc::vec::Vec<TestResult> {
    let mut results = alloc::vec::Vec::new();

    results.push(hello_world_bmo());
    results.push(arithmetic_bmo());
    results.push(if_else_bmo());
    results.push(call_bmo_abi());

    results
}

/// Test 1: Hello World en BMO.
fn hello_world_bmo() -> TestResult {
    let src = b"\
extern {
    fn diag_print(ptr: *const u8, len: u64) -> u64;
    fn proc_exit(code: i32) -> ();
}

fn main() {
    diag_print(\"Hello, World!\\n\" as *const u8, 14);
    proc_exit(0);
}
";
    run_compile_test("hello_world_bmo", src, SourceLang::Bmo, |code| {
        if code.is_empty() {
            alloc::string::String::from("code buffer is empty")
        } else if code.len() < 20 {
            alloc::format!("code too short: {} bytes", code.len())
        } else {
            // Verificar que el primer byte sea 0x55 (push rbp) o 0x48 (REX.W)
            if code[0] == 0x55 || (code[0] & 0xF0) == 0x40 {
                alloc::string::String::from("ok")
            } else {
                alloc::format!("unexpected first byte: 0x{:02X}", code[0])
            }
        }
    })
}

/// Test 2: Aritmética simple.
fn arithmetic_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let a: i64 = 1 + 2;
    let b: i64 = 3 * 4;
    a + b
}
";
    run_compile_test("arithmetic_bmo", src, SourceLang::Bmo, |code| {
        if code.is_empty() {
            alloc::string::String::from("code buffer is empty")
        } else {
            alloc::string::String::from("ok")
        }
    })
}

/// Test 3: if/else.
fn if_else_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let x: i64 = 10;
    if x > 5 {
        1
    } else {
        0
    }
}
";
    run_compile_test("if_else_bmo", src, SourceLang::Bmo, |code| {
        if code.is_empty() {
            alloc::string::String::from("code buffer is empty")
        } else {
            alloc::string::String::from("ok")
        }
    })
}

/// Test 4: llamada a BMO ABI (diag_print).
fn call_bmo_abi() -> TestResult {
    let src = b"\
fn main() {
    diag_print(\"test\" as *const u8, 4);
}
";
    run_compile_test("call_bmo_abi", src, SourceLang::Bmo, |code| {
        if code.is_empty() {
            alloc::string::String::from("code buffer is empty")
        } else {
            // Buscar la secuencia 0F 05 (syscall).
            let mut has_syscall = false;
            for w in code.windows(2) {
                if w[0] == 0x0F && w[1] == 0x05 { has_syscall = true; break; }
            }
            if has_syscall {
                alloc::string::String::from("ok (found syscall)")
            } else {
                alloc::string::String::from("no syscall instruction emitted")
            }
        }
    })
}

fn run_compile_test<F>(name: &'static str, src: &[u8], lang: SourceLang, check: F) -> TestResult
where F: FnOnce(&[u8]) -> alloc::string::String
{
    match compile(src, lang, name) {
        Ok(prog) => {
            let msg = check(&prog.code);
            let passed = msg == "ok" || msg.starts_with("ok ");
            TestResult { name, passed, message: msg }
        }
        Err(e) => TestResult {
            name,
            passed: false,
            message: alloc::format!("compile error: {:?}", e),
        }
    }
}

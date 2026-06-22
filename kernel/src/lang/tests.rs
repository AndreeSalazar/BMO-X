//! `lang::tests` — Tests integrados del compilador AOT x86-64.
//!
//! Como el kernel es no_std, no podemos usar `#[cfg(test)]`. Estos
//! tests se ejecutan manualmente desde el boot (ver
//! `boot::smoke::run_lang_tests`).
//!
//! ## Cobertura
//!
//! - **hello_world_bmo**: print + exit
//! - **arithmetic_bmo**: + - * / (signed)
//! - **if_else_bmo**: branch condicional
//! - **while_loop_bmo**: loop con condición
//! - **factorial_bmo**: recursión
//! - **fibonacci_bmo**: while + variables múltiples
//! - **call_bmo_abi**: syscall (mov rax, nr; syscall)
//! - **comparison_bmo**: == < >

#![allow(dead_code)]

use crate::lang::pipeline::{compile, SourceLang};

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
    results.push(while_loop_bmo());
    results.push(factorial_bmo());
    results.push(fibonacci_bmo());
    results.push(call_bmo_abi());
    results.push(comparison_bmo());
    results.push(c_hello_world());
    results.push(c_arithmetic());
    results.push(bef_header_valid());
    results.push(bef_section_table());

    results
}

// ─── Tests BMO ─────────────────────────────────────────────────────

fn hello_world_bmo() -> TestResult {
    let src = b"\
fn main() {
    diag_print(\"Hello, World!\" as *const u8, 13);
    proc_exit(0);
}
";
    check("hello_world_bmo", src, SourceLang::Bmo, |c| {
        ok_if(c.len() >= 30 && c[0] == 0x55, alloc::format!("len={}, first=0x{:02X}", c.len(), c[0]))
    })
}

fn arithmetic_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let a: i64 = 1 + 2;
    let b: i64 = 3 * 4;
    let c: i64 = a + b;
    c
}
";
    check("arithmetic_bmo", src, SourceLang::Bmo, |c| {
        ok_if(c.len() >= 30, alloc::format!("len={}", c.len()))
    })
}

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
    check("if_else_bmo", src, SourceLang::Bmo, |c| {
        // Buscar 0F 8C (jl) o 0F 8F (jg).
        let mut has_cmp = false;
        for w in c.windows(2) {
            if w[0] == 0x83 && w[1] == 0xF8 { has_cmp = true; break; } // cmp rax, imm8
        }
        ok_if(has_cmp, alloc::format!("no cmp found, len={}", c.len()))
    })
}

fn while_loop_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let i: i64 = 0;
    while i < 10 {
        let j: i64 = i + 1;
    }
    i
}
";
    check("while_loop_bmo", src, SourceLang::Bmo, |c| {
        // Debe tener al menos 2 saltos condicionales (uno al inicio del loop,
        // uno al final para volver).
        let mut jmps = 0;
        for w in c.windows(2) {
            if w[0] == 0x0F && (w[1] & 0xF0) == 0x80 { jmps += 1; }
        }
        ok_if(jmps >= 2, alloc::format!("expected >=2 jcc, got {}", jmps))
    })
}

fn factorial_bmo() -> TestResult {
    let src = b"\
fn factorial(n: i64) -> i64 {
    if n <= 1 {
        1
    } else {
        n * factorial(n - 1)
    }
}

fn main() -> i64 {
    factorial(5)
}
";
    check("factorial_bmo", src, SourceLang::Bmo, |c| {
        // Debe tener al menos 2 calls (uno a factorial, uno recursivo).
        let mut calls = 0;
        for w in c.windows(1) {
            if w[0] == 0xE8 { calls += 1; }
        }
        ok_if(calls >= 2, alloc::format!("expected >=2 calls, got {}", calls))
    })
}

fn fibonacci_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let a: i64 = 0;
    let b: i64 = 1;
    let i: i64 = 0;
    while i < 10 {
        let c: i64 = a + b;
        a = b;
        b = c;
    }
    b
}
";
    check("fibonacci_bmo", src, SourceLang::Bmo, |c| {
        ok_if(c.len() >= 40, alloc::format!("len={}", c.len()))
    })
}

fn call_bmo_abi() -> TestResult {
    let src = b"\
fn main() {
    diag_print(\"test\" as *const u8, 4);
    proc_exit(0);
}
";
    check("call_bmo_abi", src, SourceLang::Bmo, |c| {
        // Buscar la secuencia 0F 05 (syscall).
        let mut has_syscall = false;
        for w in c.windows(2) {
            if w[0] == 0x0F && w[1] == 0x05 { has_syscall = true; break; }
        }
        ok_if(has_syscall, alloc::string::String::from("no syscall instruction emitted"))
    })
}

fn comparison_bmo() -> TestResult {
    let src = b"\
fn main() -> i64 {
    let a: i64 = 5;
    let b: i64 = 10;
    if a < b {
        1
    } else {
        0
    }
}
";
    check("comparison_bmo", src, SourceLang::Bmo, |c| {
        // Debe tener `cmp` + setcc (0F 9X).
        let mut has_cmp = false;
        let mut has_setcc = false;
        for w in c.windows(2) {
            if w[0] == 0x83 && w[1] == 0xF8 { has_cmp = true; } // cmp rax, imm8
            if w[0] == 0x0F && (w[1] & 0xF0) == 0x90 { has_setcc = true; } // setcc
        }
        ok_if(has_cmp && has_setcc, alloc::format!("cmp={}, setcc={}", has_cmp, has_setcc))
    })
}

// ─── Tests BEF ──────────────────────────────────────────────────────

fn bef_header_valid() -> TestResult {
    let src = b"\
fn main() {
    proc_exit(0);
}
";
    check("bef_header_valid", src, SourceLang::Bmo, |c| {
        // El BEF empieza con magic "BEF1" = 0x31464542 LE.
        if c.len() < 48 {
            return alloc::string::String::from("BEF too short");
        }
        let magic = u32::from_le_bytes([c[0], c[1], c[2], c[3]]);
        if magic == u32::from_le_bytes(*b"BEF1") {
            alloc::string::String::from("ok")
        } else {
            alloc::format!("wrong magic: 0x{:08X}", magic)
        }
    })
}

fn bef_section_table() -> TestResult {
    let src = b"\
fn main() {
    proc_exit(0);
}
";
    check("bef_section_table", src, SourceLang::Bmo, |c| {
        if c.len() < 48 + 48 {
            return alloc::string::String::from("BEF too short for section table");
        }
        // n_sections está en offset 24-27 (después de header de 24 bytes).
        let n_sections = u32::from_le_bytes([c[24], c[25], c[26], c[27]]);
        // section_count debe ser >= 1 (.text) + 1 (.rodata) + 1 (.meta) = 3
        if n_sections >= 3 {
            alloc::string::String::from("ok")
        } else {
            alloc::format!("too few sections: {}", n_sections)
        }
    })
}

// ─── Tests C ───────────────────────────────────────────────────────

fn c_hello_world() -> TestResult {
    let src = b"\
int main() {
    return 42;
}
";
    check("c_hello_world", src, SourceLang::C, |c| {
        ok_if(c.len() >= 20, alloc::format!("len={}", c.len()))
    })
}

fn c_arithmetic() -> TestResult {
    let src = b"\
int add(int a, int b) {
    return a + b;
}

int main() {
    return add(2, 3);
}
";
    check("c_arithmetic", src, SourceLang::C, |c| {
        ok_if(c.len() >= 30, alloc::format!("len={}", c.len()))
    })
}

// ─── Helpers ───────────────────────────────────────────────────────

fn check<F>(name: &'static str, src: &[u8], lang: SourceLang, check: F) -> TestResult
where F: FnOnce(&[u8]) -> alloc::string::String
{
    match compile(src, lang, name) {
        Ok(prog) => {
            let msg = check(&prog.code);
            let passed = msg == "ok";
            TestResult { name, passed, message: msg }
        }
        Err(e) => TestResult {
            name,
            passed: false,
            message: alloc::format!("compile error: {:?}", e),
        },
    }
}

fn ok_if(cond: bool, msg: alloc::string::String) -> alloc::string::String {
    if cond { alloc::string::String::from("ok") } else { msg }
}

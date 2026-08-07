//! PARSEAR: que la forma se reconozca y quede bien en el AST
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

#[test]
fn parses_hello_world() {
    let src = "int main() { printf(\"HOLA C\"); return 0; }";
    let p = parse(src).unwrap();
    assert_eq!(p.functions.len(), 1);
    assert_eq!(p.functions[0].name, "main");
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
fn parses_multi_level_pointers() {
    let src = r#"
int main() {
int x;
int* p;
int** pp;
int*** ppp;
x = 1;
return x;
}
"#;
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn parses_int_literal_suffixes() {
    let src = r#"
int main() {
long a;
unsigned long b;
a = 10L;
b = 10UL;
a = 100ll;
b = 0xFFul;
b = 42u;
return 0;
}
"#;
    let p = parse(src).unwrap();
    assert!(p.functions.len() > 0);
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
    let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
    let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
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


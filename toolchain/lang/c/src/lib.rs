pub mod codegen;
pub mod ast;
pub mod module;
pub mod ir_emit;
pub mod parser;
pub mod standard;
mod lexer;

use parser::Parser;

pub use standard::{CStandard, StandardFeatures};
use lexer::Token;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

/// Compile with a specific C standard (C89/C99/C11/C17/C23).
/// Loads the standard TOML manifest and applies feature gating during parsing.
pub fn compile_with_standard(source: &str, std: CStandard) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);
    let program = parse_with_features(source, &features)?;
    codegen::compile_to_bef_bytes(&program)
}

/// Compile with full preprocessor pass (macros, includes, conditionals).
/// This is the recommended entry point for real C files.
pub fn compile_with_preprocessor(
    source: &str,
    file_path: &Path,
    std: CStandard,
) -> Result<Vec<u8>, CError> {
    let features = StandardFeatures::load_standard(std);

    // Run preprocessor: expand #include, #define, #ifdef, etc.
    let include_paths = module::discover_include_paths();
    let mut pp = parser::preprocessor::Preprocessor::new(&features, include_paths);
    let expanded = pp.preprocess(source, file_path)?;

    // Parse + compile the expanded source
    let program = parse_with_features(&expanded, &features)?;
    codegen::compile_to_bef_bytes(&program)
}

/// Parse with standard feature gating.
pub fn parse_with_features(source: &str, features: &StandardFeatures) -> Result<Program, CError> {
    let mut p = Parser::new(source);
    p.features = features.clone();
    p.parse_program()
}

/// Compile C source to a unified IrModule (language-agnostic IR).
pub fn compile_to_ir(source: &str) -> Result<bmo_abi::ir::IrModule, CError> {
    let program = parse(source)?;
    Ok(ir_emit::compile_to_ir(&program))
}

pub fn compile_source_to_bef_with_modules(source: &str, base_paths: Vec<PathBuf>) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths).with_semantic_asm();
    let program = Parser::new(source).parse_program_with_modules(&mut resolver, None)?;
    let used = module::find_used_functions(&program, &program.exported);
    codegen::compile_to_bef_bytes_filtered(&program, &used)
}

pub fn compile_source_to_bef_with_all(
    source: &str,
    base_paths: Vec<PathBuf>,
    asm_paths: Vec<PathBuf>,
) -> Result<Vec<u8>, CError> {
    let mut resolver = module::ModuleResolver::new(base_paths).with_semantic_asm();
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
    fn resolves_nested_arrow_offsets() {
        // a->b->c ANIDADO: antes devolvía offset 0 silencioso. Ahora el parser
        // sigue los tipos de campo y calcula el offset REAL de cada salto.
        let src = r#"
struct Inner { int x; long y; };
struct Outer { int pad; struct Inner* in; };
int main() {
    struct Outer* o;
    int a;
    a = o->in->y;
    return a;
}
"#;
        let p = parse(src).unwrap();
        let main_fn = p.functions.iter().find(|f| f.name == "main").unwrap();
        // buscar el Assign("a", Arrow(Arrow(o,"in",8),"y",8))
        let mut found = false;
        for stmt in &main_fn.body {
            if let Stmt::Expr(Expr::Assign(name, val)) = stmt {
                if name == "a" {
                    if let Expr::Arrow(base, f2, off2, ft2) = val.as_ref() {
                        assert_eq!(f2, "y");
                        assert_eq!(*off2, 8, "offset de y en Inner debe ser 8 (x:4 + padding)");
                        assert_eq!(*ft2, TypeSpec::Long, "el tipo del campo y debe viajar en el AST");
                        if let Expr::Arrow(_, f1, off1, _) = base.as_ref() {
                            assert_eq!(f1, "in");
                            assert_eq!(*off1, 8, "offset de in en Outer debe ser 8 (pad:4 + align 8)");
                            found = true;
                        }
                    }
                }
            }
        }
        assert!(found, "no se encontro el acceso anidado a->b->c en el AST");
    }

    #[test]
    fn resolves_nested_dot_offsets() {
        // a.b.c con structs por valor: el offset del campo interior debe resolverse.
        let src = r#"
struct Inner { int x; long y; };
struct Outer { long pad; struct Inner in; };
int main() {
    struct Outer o;
    int a;
    a = o.in.y;
    return a;
}
"#;
        let p = parse(src).unwrap();
        let main_fn = p.functions.iter().find(|f| f.name == "main").unwrap();
        let mut found = false;
        for stmt in &main_fn.body {
            if let Stmt::Expr(Expr::Assign(name, val)) = stmt {
                if name == "a" {
                    if let Expr::Field(base, f2, off2, ft2) = val.as_ref() {
                        assert_eq!(f2, "y");
                        assert_eq!(*off2, 8, "offset de y dentro de Inner debe ser 8");
                        assert_eq!(*ft2, TypeSpec::Long, "el tipo del campo y debe viajar en el AST");
                        if let Expr::Field(_, f1, off1, _) = base.as_ref() {
                            assert_eq!(f1, "in");
                            assert_eq!(*off1, 8, "offset de in dentro de Outer debe ser 8");
                            found = true;
                        }
                    }
                }
            }
        }
        assert!(found, "no se encontro el acceso anidado a.b.c en el AST");
    }

    #[test]
    fn field_assign_carries_exact_type() {
        // pt.x = 10 con x:int — el AssignField lleva TypeSpec::Int para que
        // codegen escriba 4 bytes, NO 8 (antes pisaba a pt.y).
        let src = r#"
struct Point { int x; long y; };
int main() { struct Point pt; pt.x = 10; return 0; }
"#;
        let p = parse(src).unwrap();
        let mut found = false;
        for stmt in &p.functions[0].body {
            if let Stmt::Expr(Expr::AssignField(_, f, off, ft, _)) = stmt {
                assert_eq!(f, "x");
                assert_eq!(*off, 0);
                assert_eq!(*ft, TypeSpec::Int, "tipo del campo x debe ser Int (store de 4 bytes)");
                found = true;
            }
        }
        assert!(found, "pt.x = 10 debe producir AssignField con tipo");
        compile_source_to_bef(src).unwrap();
    }

    #[test]
    fn cast_is_real_node() {
        // (char)x ya NO es no-op: el AST lleva Cast(Char, x) y codegen trunca.
        let src = "int main() { int x; x = 300; x = (char)x; return x; }";
        let p = parse(src).unwrap();
        let mut found = false;
        for stmt in &p.functions[0].body {
            if let Stmt::Expr(Expr::Assign(_, val)) = stmt {
                if let Expr::Cast(t, _) = val.as_ref() {
                    assert_eq!(*t, TypeSpec::Char);
                    found = true;
                }
            }
        }
        assert!(found, "(char)x debe producir Expr::Cast(Char, ...)");
        compile_source_to_bef(src).unwrap();
    }

    #[test]
    fn float_literal_rejected_honestly() {
        // Sin aritmética SSE, un float compilado como int es basura silenciosa.
        // Preferimos el error claro.
        let err = parse("int main() { int x; x = 1.5; return x; }").unwrap_err();
        assert!(err.message.contains("float"), "el error debe explicar que floats están pendientes: {}", err.message);
    }

    #[test]
    fn errors_report_real_line() {
        // Antes TODO error decía "línea 1".
        let src = "int main() {\n    int x;\n    x = ;\n    return 0;\n}";
        let err = parse(src).unwrap_err();
        assert_eq!(err.line, 3, "el error de 'x = ;' está en la línea 3, no la 1");
    }

    #[test]
    fn array_decl_records_size() {
        // int arr[4] debe ser Array(Int, 4) — antes el tamaño se TIRABA.
        let src = "int main() { int arr[4]; return 0; }";
        let p = parse(src).unwrap();
        let main_fn = &p.functions[0];
        let mut found = false;
        for stmt in &main_fn.body {
            if let Stmt::DeclAssign(TypeSpec::Array(elem, n), name, _) = stmt {
                assert_eq!(name, "arr");
                assert_eq!(**elem, TypeSpec::Int);
                assert_eq!(*n, 4);
                found = true;
            }
        }
        assert!(found, "int arr[4] debe declarar TypeSpec::Array(Int, 4)");
    }

    #[test]
    fn subscript_assign_not_discarded() {
        // arr[i] = x ANTES SE DESCARTABA EN SILENCIO (parse_assign no tenia caso).
        let src = "int main() { int arr[4]; arr[2] = 7; return arr[2]; }";
        let p = parse(src).unwrap();
        let main_fn = &p.functions[0];
        let mut found = false;
        for stmt in &main_fn.body {
            if let Stmt::Expr(Expr::AssignSubscript(name, _, scale, val)) = stmt {
                assert_eq!(name, "arr");
                assert_eq!(*scale, 4, "escala de int = 4 bytes");
                assert_eq!(**val, Expr::Int(7));
                found = true;
            }
        }
        assert!(found, "arr[2] = 7 debe producir AssignSubscript, no descartarse");
        // y debe compilar a BEF
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn subscript_compound_assign() {
        let src = "int main() { int arr[4]; arr[1] = 1; arr[1] += 5; arr[1] <<= 2; return arr[1]; }";
        let p = parse(src).unwrap();
        let n_assigns = p.functions[0].body.iter().filter(|s| {
            matches!(s, Stmt::Expr(Expr::AssignSubscript(_, _, _, _)))
        }).count();
        assert_eq!(n_assigns, 3, "las 3 asignaciones a arr[1] deben sobrevivir");
        compile_source_to_bef(src).unwrap();
    }

    #[test]
    fn subscript_on_compound_base_errors_honestly() {
        // p->arr[i]: antes el [i] se IGNORABA en silencio; ahora error claro.
        let src = r#"
struct S { int* arr; };
int main() { struct S s; int x; x = s.arr[0]; return x; }
"#;
        // s.arr[0]: base compuesta (Field) — debe RECHAZARSE, no compilar mal
        assert!(parse(src).is_err(), "subscript sobre base compuesta debe dar error honesto");
    }

    #[test]
    fn nested_decl_compiles() {
        // int i dentro del for: antes NO recibia slot de stack (loads = 0,
        // loop infinito en runtime). Ahora build_var_map recorre anidado.
        let src = r#"
int main() {
    int sum = 0;
    for (int i = 0; i < 10; i = i + 1) {
        if (i > 5) { int extra = 2; sum = sum + extra; }
        sum = sum + i;
    }
    return sum;
}
"#;
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
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn syscall_arg_count_validation() {
        use std::path::PathBuf;
        // bmo_exit expects 1 arg â†’ passing 0 should fail
        let src = r#"use "bmo/proc"; int main() { bmo_exit(); }"#;
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
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
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
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
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
        let bef = compile_source_to_bef_with_all(src, vec![base], vec![asm]).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn syscall_emits_correct_code() {
        use std::path::PathBuf;
        let src = r#"use "bmo/proc"; int main() { bmo_exit(42); }"#;
        let base = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../base");
        let asm = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../forge/sem-asm/tables");
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

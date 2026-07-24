pub mod codegen;
pub mod ast;
pub mod module;
pub mod ir_emit;
pub mod parser;
pub mod standard;
mod lexer;

use parser::Parser;

pub use standard::{CStandard, StandardFeatures};
#[cfg(test)]
use lexer::Token;

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

    // ── Banco de pruebas: EJECUTAR el programa, no mirarlo ──────────────
    //
    // Mismo criterio que en COBOL: un formateo que produce dígitos erróneos
    // se ve perfectamente sano en un volcado de bytes.

    /// Compila y ejecuta un programa C, devolviendo lo que el kernel habría
    /// pintado.
    fn run_c(source: &str) -> String {
        use bmo_abi::bef::sections::{SectionEntry, SectionKind};
        use bmo_lower::emu::{run, Machine};

        let bef = compile_source_to_bef(source).expect("el programa debe compilar");
        let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
        let entry = hdr.entry_offset as usize;
        let sec_off = hdr.section_table_offset as usize;

        // La imagen se rearma en el MISMO orden en que el codegen la
        // dispuso: código, luego rodata, luego data. El `lea rax,[rip+disp]`
        // con el que se alcanzan las cadenas se calculó asumiendo que van
        // pegadas detrás del código; cargar solo la sección CODE dejaba esos
        // punteros apuntando al vacío y un `%s` imprimía cadena vacía.
        let mut code = Vec::new();
        for kind in [SectionKind::Code, SectionKind::RoData, SectionKind::Data] {
            for i in 0..hdr.section_count as usize {
                let e = sec_off + i * SectionEntry::SIZE;
                if bef[e] == kind as u8 {
                    let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap()) as usize;
                    let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize;
                    code.extend_from_slice(&bef[off..off + size]);
                }
            }
        }
        assert!(!code.is_empty(), "el BEF no tiene seccion CODE");

        let mut machine = Machine::new(code);
        machine.rip = entry; // `main` no tiene por que estar al principio
        let machine = run(machine, 500_000);
        assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
        machine.console
    }





    /// El ejemplo del repositorio, ejecutado. Si alguien vuelve a invertir
    /// un operador, este test lo dice antes de que haga falta flashear nada.
    #[test]
    fn hola_example_produces_its_documented_output() {
        let out = run_c(include_str!("../examples/hola.c"));
        assert_eq!(
            out,
            "BMO-X: hola mundo desde C\n\
             cuenta=3 total=42 resto=2\n\
             42 - 100 = -58\n\
             estado LISTO = 1 de 2\n\
             hex=beef char=B texto=cadena\n\
             C -> puerta L1 -> INVOKE -> Ring 0\n"
        );
    }

    /// Los operadores NO conmutativos estaban invertidos: se emitían sobre
    /// `b - a` en vez de `a - b`. Con `+` y `*` no se notaba; con `-`, `/`,
    /// `%` y los desplazamientos, sí. Nadie lo vio en 1.600 líneas de
    /// codegen porque ningún test los ejecutaba.
    #[test]
    fn non_commutative_operators_respect_operand_order() {
        for (expr, expected) in [
            ("10 - 3", "7"),
            ("3 - 10", "-7"),
            ("10 / 3", "3"),
            ("10 % 3", "1"),
            ("1 << 3", "8"),
            ("16 >> 2", "4"),
            ("10 + 3", "13"),
            ("10 * 3", "30"),
        ] {
            let out = run_c(&format!("int main() {{ printf(\"%d\\n\", {expr}); return 0; }}"));
            assert_eq!(out.trim(), expected, "expresion: {expr}");
        }
    }

    /// La división entera es CON SIGNO. Antes dividía sin signo, así que un
    /// negativo daba un número astronómico.
    #[test]
    fn integer_division_is_signed() {
        let out = run_c("int main() { printf(\"%d %d\\n\", 0 - 10, (0 - 10) / 3); return 0; }");
        assert_eq!(out, "-10 -3\n");
    }

    /// Todas las comparaciones, en ambos sentidos. `<`, `>` y `>=` daban el
    /// resultado contrario.
    #[test]
    fn comparisons_answer_in_the_right_direction() {
        for (expr, expected) in [
            ("1 < 2", "1"), ("2 < 1", "0"),
            ("2 > 1", "1"), ("1 > 2", "0"),
            ("1 <= 1", "1"), ("2 <= 1", "0"),
            ("1 >= 1", "1"), ("1 >= 2", "0"),
            ("1 == 1", "1"), ("1 == 2", "0"),
            ("1 != 2", "1"), ("1 != 1", "0"),
        ] {
            let out = run_c(&format!("int main() {{ printf(\"%d\\n\", {expr}); return 0; }}"));
            assert_eq!(out.trim(), expected, "comparacion: {expr}");
        }
    }

    /// `setcc` solo escribe `al`. Sin extender a cero el resto de `rax`, el
    /// resultado de una comparación arrastraba los bits altos del operando
    /// derecho: parecía correcto con valores chicos y fallaba con grandes.
    #[test]
    fn comparison_result_is_clean_with_large_operands() {
        let out = run_c(
            "int main() { long a = 4294967296; long b = 4294967296; printf(\"%d\\n\", a == b); return 0; }",
        );
        assert_eq!(out, "1\n");
    }

    /// Un `int` con signo debe releerse con signo. Antes `mov eax,[..]`
    /// rellenaba de ceros y `-7` volvía como 4294967289.
    #[test]
    fn negative_int_survives_a_round_trip_through_memory() {
        let out = run_c("int main() { int y = 0 - 7; printf(\"%d\\n\", y); return 0; }");
        assert_eq!(out, "-7\n");
    }

    #[test]
    fn printf_prints_signed_integers() {
        let out = run_c(
            "int main() { int x = 42; int y = 0 - 7; printf(\"x=%d y=%d\\n\", x, y); return 0; }",
        );
        assert_eq!(out, "x=42 y=-7\n");
    }

    /// El caso que motivó todo: antes `printf(\"%d\", x)` descartaba `x` en
    /// el parser e imprimía el literal `%d`.
    #[test]
    fn printf_no_longer_prints_the_format_specifier() {
        let out = run_c("int main() { printf(\"%d\\n\", 5); return 0; }");
        assert_eq!(out, "5\n");
        assert!(!out.contains('%'), "no debe salir el especificador crudo");
    }

    #[test]
    fn printf_supports_the_common_conversions() {
        let out = run_c(
            "int main() { printf(\"[%d][%u][%x][%c][%s][%%]\\n\", 0 - 3, 3, 255, 65, \"hola\"); return 0; }",
        );
        assert_eq!(out, "[-3][3][ff][A][hola][%]\n");
    }

    /// Los modificadores de longitud se aceptan y no cambian nada: en BMO
    /// todo entero viaja en 64 bits.
    #[test]
    fn printf_accepts_length_modifiers() {
        let out = run_c("int main() { printf(\"%ld\\n\", 123456789); return 0; }");
        assert_eq!(out, "123456789\n");
    }

    #[test]
    fn printf_computes_its_arguments() {
        let out = run_c("int main() { int a = 6; int b = 7; printf(\"%d\\n\", a * b); return 0; }");
        assert_eq!(out, "42\n");
    }

    /// Un formato que aún no se compila debe FALLAR, no imprimir basura.
    #[test]
    fn printf_rejects_unsupported_conversions() {
        let err = compile_source_to_bef("int main() { printf(\"%f\\n\", 1); return 0; }").unwrap_err();
        assert!(err.message.contains("%f"), "mensaje: {}", err.message);
    }

    #[test]
    fn printf_rejects_missing_arguments() {
        let err = compile_source_to_bef("int main() { printf(\"%d %d\\n\", 1); return 0; }").unwrap_err();
        assert!(err.message.contains("argumento"), "mensaje: {}", err.message);
    }

    /// Las constantes de `enum` valen lo que dicen. Antes el parser
    /// calculaba el valor y lo descartaba.
    #[test]
    fn enum_constants_carry_their_value() {
        let out = run_c(
            "enum Color { ROJO, VERDE, AZUL }; \
             int main() { printf(\"%d %d %d\\n\", ROJO, VERDE, AZUL); return 0; }",
        );
        assert_eq!(out, "0 1 2\n");
    }

    #[test]
    fn enum_explicit_values_continue_from_there() {
        let out = run_c(
            "enum E { A = 10, B, C = 100, D }; \
             int main() { printf(\"%d %d %d %d\\n\", A, B, C, D); return 0; }",
        );
        assert_eq!(out, "10 11 100 101\n");
    }

    #[test]
    fn enum_constants_work_in_expressions_and_conditions() {
        let out = run_c(
            "enum E { UNO = 1, DOS = 2 }; \
             int main() { if (DOS > UNO) { printf(\"mayor %d\\n\", DOS + UNO); } return 0; }",
        );
        assert_eq!(out, "mayor 3\n");
    }

    /// Busca una subsecuencia de bytes dentro del BEF ya escrito.
    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
    }

    /// El puente L2→L1: `printf("literal")` debe bajar a la puerta de
    /// consola del ABI, byte por byte igual que lo que emite `bmo-lower`.
    ///
    /// Antes de esto, C emitía `syscall 0x1F0` con un puntero — número que
    /// el kernel no despacha y forma que la superficie congelada rechaza.
    /// Compilaba, validaba, y en hardware no imprimía nada.
    #[test]
    fn printf_literal_lowers_to_the_console_door() {
        let bef = compile_source_to_bef("int main() { printf(\"hola\\n\"); return 0; }").unwrap();
        let mut door = Vec::new();
        bmo_lower::console::write_const(&mut door, b"hola\n");
        assert!(
            contains_bytes(&bef, &door),
            "el BEF debe contener la secuencia INVOKE/CONSOLE_WRITE de la puerta"
        );
    }

    /// Volver de `main` debe terminar el proceso por la puerta. Si no, la
    /// ejecución sigue de largo hacia lo que haya después del código.
    #[test]
    fn returning_from_main_exits_through_the_door() {
        let bef = compile_source_to_bef("int main() { return 0; }").unwrap();
        let mut net = Vec::new();
        bmo_lower::task::exit(&mut net);
        assert!(
            contains_bytes(&bef, &net),
            "el epílogo de main debe ser INVOKE(EXIT) + red de pause/jmp"
        );
    }

    /// `printf` con argumentos NO puede tomar el atajo del literal: hacerlo
    /// descartaba los argumentos en silencio e imprimía "%d" tal cual.
    #[test]
    fn printf_with_arguments_keeps_them() {
        let program = parse("int main() { int x = 7; printf(\"%d\\n\", x); return 0; }").unwrap();
        let body = &program.functions[0].body;
        let has_literal_shortcut = body.iter().any(|s| {
            matches!(s, Stmt::Printf(_) | Stmt::PrintfLn(_))
        });
        assert!(
            !has_literal_shortcut,
            "printf variádico no debe degradarse a impresión de literal"
        );
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

    // ---- Punteros a función (Fase 2) ----

    #[test]
    fn parses_function_pointer_declarator() {
        // int (*op)(int, int); — variable de tipo puntero.
        let src = r#"
int add(int a, int b) { return a + b; }
int main() {
    int (*op)(int, int);
    op = add;
    int r;
    r = op(3, 4);
    return r;
}
"#;
        let p = parse(src).unwrap();
        let main_fn = p.functions.iter().find(|f| f.name == "main").unwrap();
        // op debe estar declarada como puntero
        let has_op = main_fn.body.iter().any(|s| matches!(s,
            Stmt::DeclAssign(TypeSpec::Ptr(_), name, _) if name == "op"));
        assert!(has_op, "int (*op)(int,int) debe declarar un puntero llamado op");
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn function_decays_to_address() {
        // op = add; — 'add' como valor = lea rax,[rip+add] (48 8D 05).
        let src = r#"
int add(int a, int b) { return a + b; }
int main() { int (*op)(int, int); op = add; return 0; }
"#;
        let bef = compile_source_to_bef(src).unwrap();
        let lea = [0x48, 0x8D, 0x05];
        assert!(bef.windows(lea.len()).any(|w| w == lea),
            "la decadencia función→dirección debe emitir lea rax,[rip+func]");
    }

    #[test]
    fn indirect_call_through_pointer() {
        // op(3,4) donde op es variable → call rax (FF D0), no call rel32.
        let src = r#"
int add(int a, int b) { return a + b; }
int main() {
    int (*op)(int, int);
    op = add;
    return op(3, 4);
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(2).any(|w| w == [0xFF, 0xD0]),
            "la llamada indirecta debe emitir call rax (FF D0)");
    }

    #[test]
    fn addr_of_function_works() {
        // &myfunc también da la dirección (equivalente a la decadencia).
        let src = r#"
int foo(void) { return 7; }
int main() { int (*fp)(void); fp = &foo; return fp(); }
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(3).any(|w| w == [0x48, 0x8D, 0x05]), "falta lea del &foo");
        assert!(bef.windows(2).any(|w| w == [0xFF, 0xD0]), "falta call rax indirecto");
    }

    #[test]
    fn subscript_on_compound_base_now_works() {
        // p->arr[i] con arr: int* — antes ERROR honesto, ahora compila.
        let src = r#"
struct S { int pad; int* arr; };
int main() {
    struct S* s;
    int x;
    x = s->arr[2];
    return x;
}
"#;
        let p = parse(src).unwrap();
        let main_fn = p.functions.iter().find(|f| f.name == "main").unwrap();
        // x = IndexPtr(Arrow(s,"arr"), 2, Int)
        let ok = main_fn.body.iter().any(|st| matches!(st,
            Stmt::Expr(Expr::Assign(n, v)) if n == "x" && matches!(v.as_ref(), Expr::IndexPtr(_, _, TypeSpec::Int))));
        assert!(ok, "s->arr[2] debe ser IndexPtr con elemento Int");
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.len() > 48);
    }

    #[test]
    fn subscript_compound_base_assign_and_compound() {
        // p->arr[i] = v  y  p->arr[i] += v — no se descartan.
        let src = r#"
struct S { int* arr; };
int main() {
    struct S* s;
    s->arr[0] = 5;
    s->arr[0] += 3;
    return s->arr[0];
}
"#;
        let p = parse(src).unwrap();
        let n = p.functions[0].body.iter().filter(|st| matches!(st,
            Stmt::Expr(Expr::AssignIndexPtr(_, _, _, _)))).count();
        assert_eq!(n, 2, "las 2 asignaciones a s->arr[0] deben sobrevivir");
        compile_source_to_bef(src).unwrap();
    }

    #[test]
    fn explicit_deref_call_works() {
        // (*fp)(args) — forma explícita del puntero a función.
        let src = r#"
int add(int a, int b) { return a + b; }
int main() {
    int (*fp)(int, int);
    fp = add;
    return (*fp)(3, 4);
}
"#;
        let p = parse(src).unwrap();
        // return CallPtr(Deref(Var fp), [3,4])
        let ok = p.functions.iter().find(|f| f.name == "main").unwrap().body.iter().any(|st|
            matches!(st, Stmt::Return(Some(Expr::CallPtr(callee, _))) if matches!(callee.as_ref(), Expr::Deref(_))));
        assert!(ok, "(*fp)(3,4) debe ser CallPtr sobre Deref");
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(2).any(|w| w == [0xFF, 0xD0]), "falta call rax indirecto");
    }

    // ---- LA FUSIÓN sem-asm↔C (Fase 1) ----

    #[test]
    fn intrinsic_emits_exact_table_bytes() {
        // __pause() y __hlt() = bytes EXACTOS de intrinsics.toml en el código.
        let src = "int main() { __pause(); __hlt(); return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(2).any(|w| w == [0xF3, 0x90]), "falta pause (F3 90)");
        assert!(bef.contains(&0xF4), "falta hlt (F4)");
    }

    #[test]
    fn intrinsic_rdtsc_returns_combined_value() {
        // __rdtsc() devuelve u64: rdtsc + shl rdx,32 + or rax,rdx.
        let src = "int main() { unsigned long t; t = __rdtsc(); return (int)t; }";
        let bef = compile_source_to_bef(src).unwrap();
        let seq = [0x0F, 0x31, 0x48, 0xC1, 0xE2, 0x20, 0x48, 0x09, 0xD0];
        assert!(bef.windows(seq.len()).any(|w| w == seq),
            "falta la secuencia rdtsc + combine edx:eax → rax");
    }

    #[test]
    fn unknown_intrinsic_fails_honestly() {
        // __zzz() no está en la tabla → error con nombre y ubicación de la tabla.
        let err = compile_source_to_bef("int main() { __zzz(); return 0; }").unwrap_err();
        assert!(err.message.contains("no existe en la tabla"), "mensaje: {}", err.message);
    }

    #[test]
    fn intrinsic_wrong_arity_fails() {
        // __hlt no lleva operandos; __hlt(1) debe fallar en codegen contra la tabla.
        let err = compile_source_to_bef("int main() { __hlt(1); return 0; }").unwrap_err();
        assert!(err.message.contains("espera 0"), "mensaje: {}", err.message);
    }

    #[test]
    fn intrinsic_outb_marshals_args_to_registers() {
        // __outb(0x3F8, 65): puerto→dx, valor→al, luego out dx,al (0xEE).
        let src = "int main() { __outb(1016, 65); return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        // pop rdx (0x5A) para el puerto, pop rax (0x58) para el valor, out (0xEE)
        assert!(bef.contains(&0xEE), "falta out dx,al (0xEE)");
        assert!(bef.windows(2).any(|w| w == [0x5A, 0xEE]) || bef.contains(&0x5A),
            "el puerto debe volcarse a dx (pop rdx 0x5A)");
    }

    #[test]
    fn intrinsic_inb_returns_byte() {
        // __inb(puerto): in al,dx (0xEC) + movzx rax,al (48 0F B6 C0).
        let src = "int main() { int c; c = __inb(96); return c; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.contains(&0xEC), "falta in al,dx (0xEC)");
        let seq = [0x48, 0x0F, 0xB6, 0xC0];
        assert!(bef.windows(seq.len()).any(|w| w == seq), "falta movzx rax,al del retorno");
    }

    #[test]
    fn intrinsic_wrmsr_splits_value_to_edx_eax() {
        // __wrmsr(nr, val): nr→ecx, val(64)→edx:eax, wrmsr (0F 30).
        let src = "int main() { unsigned long v; v = 5; __wrmsr(200, v); return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(2).any(|w| w == [0x0F, 0x30]), "falta wrmsr (0F 30)");
        // shr rdx,32 del split del valor a edx:eax
        let shr = [0x48, 0xC1, 0xEA, 0x20];
        assert!(bef.windows(shr.len()).any(|w| w == shr), "falta el split del valor a edx:eax");
    }

    #[test]
    fn intrinsic_arg_arity_wrong_fails() {
        let err = compile_source_to_bef("int main() { __outb(1); return 0; }").unwrap_err();
        assert!(err.message.contains("espera 2"), "mensaje: {}", err.message);
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
    fn float_in_int_context_truncates() {
        // Evolución del test de Fase 0: 1.5 ya NO se rechaza. En contexto ENTERO
        // (int x = 1.5) se trunca vía cvttsd2si — semántica C correcta.
        let src = "int main() { int x; x = 1.5; return x; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2C, 0xC0]),
            "1.5 en contexto entero debe truncar con cvttsd2si");
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

    // ---- Floats SSE (Fase 2) ----

    #[test]
    fn float_literal_is_number_now() {
        // 1.5 ya NO es error: se acepta y se compila por la ruta SSE.
        let src = "int main() { double d; d = 1.5; return 0; }";
        let p = parse(src).unwrap();
        // d = FloatLit(1.5)
        let ok = p.functions[0].body.iter().any(|s| matches!(s,
            Stmt::Expr(Expr::Assign(n, v)) if n == "d" && matches!(v.as_ref(), Expr::FloatLit(_))));
        assert!(ok, "1.5 debe ser FloatLit, ya no un error");
        let bef = compile_source_to_bef(src).unwrap();
        // movq xmm0, rax (66 48 0F 6E C0) del literal + movsd store (F2 0F 11)
        assert!(bef.windows(5).any(|w| w == [0x66, 0x48, 0x0F, 0x6E, 0xC0]), "falta movq xmm0,rax del literal");
        assert!(bef.windows(3).any(|w| w == [0xF2, 0x0F, 0x11]), "falta movsd store del double");
    }

    #[test]
    fn double_arithmetic_uses_sse() {
        // d = a + b * c → addsd/mulsd, no aritmética entera.
        let src = r#"
int main() {
    double a; double b; double c; double d;
    a = 2.0; b = 3.0; c = 4.0;
    d = a + b * c;
    return 0;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x59, 0xC1]), "falta mulsd xmm0,xmm1");
        assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x58, 0xC1]), "falta addsd xmm0,xmm1");
    }

    #[test]
    fn double_from_int_converts() {
        // double d = 5; → cvtsi2sd (entero a double).
        let src = "int main() { double d; d = 5; return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2A, 0xC0]), "falta cvtsi2sd de 5");
    }

    #[test]
    fn float_to_int_truncates() {
        // int x = (int)2.7; → cvttsd2si (double a entero, trunca).
        let src = "int main() { int x; x = (int)2.7; return x; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(5).any(|w| w == [0xF2, 0x48, 0x0F, 0x2C, 0xC0]), "falta cvttsd2si");
    }

    #[test]
    fn double_comparison_uses_comisd() {
        // if (d > 0.5) → comisd + seta, NO comparación entera de bits.
        let src = r#"
int main() {
    double d; d = 1.0;
    if (d > 0.5) { return 1; }
    return 0;
}
"#;
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(4).any(|w| w == [0x66, 0x0F, 0x2F, 0xC1]), "falta comisd xmm0,xmm1");
        assert!(bef.windows(3).any(|w| w == [0x0F, 0x97, 0xC0]), "falta seta (a > b unsigned)");
    }

    #[test]
    fn float_f32_narrows_on_store() {
        // float f = 1.5; → cvtsd2ss (double a float) + movss store.
        let src = "int main() { float f; f = 1.5; return 0; }";
        let bef = compile_source_to_bef(src).unwrap();
        assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x5A, 0xC0]), "falta cvtsd2ss");
        assert!(bef.windows(3).any(|w| w == [0xF3, 0x0F, 0x11]), "falta movss store");
    }

    #[test]
    fn double_return_value_in_xmm0() {
        // double f() { return d; } — el valor de retorno queda en xmm0.
        let src = r#"
double half(void) { double d; d = 0.5; return d; }
int main() { return 0; }
"#;
        let bef = compile_source_to_bef(src).unwrap();
        // el return de half carga d con movsd xmm0,[rbp+off] (F2 0F 10 45 ..)
        assert!(bef.windows(4).any(|w| w == [0xF2, 0x0F, 0x10, 0x45]), "falta movsd load del return");
    }

    #[test]
    fn subscript_on_compound_base_via_field() {
        // s.arr[0] con arr: int* — evolución del test de Fase 0: antes se
        // rechazaba (honesto pero limitado), en Fase 2 ya COMPILA como IndexPtr.
        let src = r#"
struct S { int* arr; };
int main() { struct S s; int x; x = s.arr[0]; return x; }
"#;
        let p = parse(src).unwrap();
        let ok = p.functions[0].body.iter().any(|st| matches!(st,
            Stmt::Expr(Expr::Assign(n, v)) if n == "x" && matches!(v.as_ref(), Expr::IndexPtr(_, _, _))));
        assert!(ok, "s.arr[0] ahora es IndexPtr, ya no un error");
        compile_source_to_bef(src).unwrap();
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

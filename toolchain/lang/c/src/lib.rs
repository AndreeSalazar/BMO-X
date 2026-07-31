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
        let bef = compile_source_to_bef(source).expect("el programa debe compilar");
        ejecutar_bef(&bef)
    }

    /// Igual, pero pasando ANTES por el preprocesador — que es lo que hace la
    /// linea de ordenes y lo que el camino de biblioteca NO hace.
    fn run_c_con_pp(source: &str) -> String {
        let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
            .expect("con preprocesador debe compilar");
        ejecutar_bef(&bef)
    }

    /// Compila con preprocesador y ejecuta SEMBRANDO la máquina antes.
    ///
    /// Hace falta desde que C puede emitir la puerta: un programa que lee el
    /// ratón necesita que haya un ratón que leer. Sin esto, todo lo que use
    /// `<bmo/entrada.h>` se probaría contra ceros, que es indistinguible de
    /// un driver muerto.
    fn run_c_sembrado(source: &str, sembrar: impl FnOnce(&mut bmo_lower::emu::Machine)) -> String {
        let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
            .expect("con preprocesador debe compilar");
        ejecutar_bef_con(&bef, sembrar)
    }

    fn ejecutar_bef(bef: &[u8]) -> String {
        ejecutar_bef_con(bef, |_| {})
    }

    fn ejecutar_bef_con(
        bef: &[u8],
        sembrar: impl FnOnce(&mut bmo_lower::emu::Machine),
    ) -> String {
        use bmo_abi::bef::sections::{SectionEntry, SectionKind};
        use bmo_lower::emu::{run, Machine};

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
        sembrar(&mut machine);
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


    /// Matriz de conformidad de C: ejecuta TODO lo que el codegen dice
    /// soportar y compara la salida real.
    ///
    /// Cuando se escribió por primera vez, 18 de 36 casos fallaban — entre
    /// ellos que NINGÚN bucle daba más de una vuelta y que `switch` siempre
    /// entraba por el primer caso. Todos compilaban y validaban.
    ///
    /// Si añades una característica al codegen, añádele aquí su fila. Es la
    /// única forma de que "soportado" signifique algo.

    /// `#define` SUSTITUYE de verdad — no se traga la linea y sigue.
    ///
    /// La pregunta no es si COMPILA (tragarse una linea tambien compila) sino
    /// si el valor LLEGA. Este test lo fija ejecutandolo: si algun dia el
    /// preprocesador deja de expandir, aqui sale `0` en vez de `5`.
    ///
    /// Y de paso documenta una asimetria real: el preprocesador SOLO corre en
    /// `compile_with_preprocessor`, que es lo que usa la linea de ordenes. El
    /// camino de biblioteca (`compile_source_to_bef`) no lo llama.
    #[test]
    fn el_define_sustituye_de_verdad() {
        let salida = run_c_con_pp("#define CINCO 5
int main(void){ printf(\"%d\", CINCO); return 0; }");
        assert_eq!(salida, "5", "el #define tiene que SUSTITUIR, no ignorarse");
    }

    /// ★ Y sin preprocesador, una directiva se RECHAZA en vez de ignorarse.
    ///
    /// Esto es lo que estaba mal: el catch-all del lexer se tragaba el `#`, asi
    /// que un `#define` dentro de una funcion **compilaba y no hacia nada** —
    /// el programa corria con la constante sin sustituir y nadie decia una
    /// palabra. Al principio del fichero daba un "expected type, got
    /// Ident(define)", que manda a mirar donde no es.
    #[test]
    fn una_directiva_sin_preprocesador_se_rechaza() {
        // Dentro de una funcion: era el caso silencioso.
        let e = compile_source_to_bef("int main(void){
#define X 5
 return 0; }")
            .unwrap_err();
        assert!(format!("{e:?}").contains("no hay preprocesador"), "{e:?}");
        // Y al principio del fichero, con el mismo mensaje.
        let e = compile_source_to_bef("#define X 5
int main(void){ return 0; }").unwrap_err();
        assert!(format!("{e:?}").contains("no hay preprocesador"), "{e:?}");
    }

    #[test]
    fn c_feature_matrix_runs_correctly() {
        let cases: &[(&str, &str, &str)] = &[
            ("while", "int i=0; int s=0; while(i<5){s=s+i; i=i+1;} printf(\"%d\", s);", "10"),
            ("for", "int s=0; for(int i=0;i<5;i=i+1){s=s+i;} printf(\"%d\", s);", "10"),
            ("do-while", "int i=0; int s=0; do{s=s+1; i=i+1;}while(i<3); printf(\"%d\", s);", "3"),
            ("break", "int s=0; for(int i=0;i<10;i=i+1){ if(i==3) break; s=s+1;} printf(\"%d\", s);", "3"),
            ("continue", "int s=0; for(int i=0;i<5;i=i+1){ if(i==2) continue; s=s+1;} printf(\"%d\", s);", "4"),
            ("switch", "int x=2; switch(x){case 1: printf(\"uno\"); break; case 2: printf(\"dos\"); break; default: printf(\"otro\");}", "dos"),
            ("switch-default", "int x=9; switch(x){case 1: printf(\"uno\"); break; default: printf(\"otro\");}", "otro"),
            ("goto", "int s=0; i: s=s+1; if(s<3) goto i; printf(\"%d\", s);", "3"),
            ("ternary", "int x=5; printf(\"%d\", x>3 ? 10 : 20);", "10"),
            ("logic-and", "printf(\"%d\", 1 && 0);", "0"),
            ("logic-or", "printf(\"%d\", 0 || 3);", "1"),
            ("compound", "int x=10; x+=5; x-=2; x*=2; printf(\"%d\", x);", "26"),
            ("incdec", "int x=5; x++; ++x; x--; printf(\"%d\", x);", "6"),
            ("cast-char", "int x=321; printf(\"%d\", (char)x);", "65"),
            ("sizeof", "printf(\"%d %d\", sizeof(int), sizeof(char));", "4 1"),
            ("charlit", "char c='A'; printf(\"%c\", c);", "A"),
            ("global", "int g = 7; int main(){ printf(\"%d\", g); return 0; }", "@FULL@7"),
            ("array-rw", "int a[3]; a[0]=10; a[1]=20; a[2]=30; printf(\"%d\", a[0]+a[1]+a[2]);", "60"),
            ("array-idx-var", "int a[3]; a[0]=1;a[1]=2;a[2]=3; int s=0; for(int i=0;i<3;i=i+1){s=s+a[i];} printf(\"%d\", s);", "6"),
            ("ptr-deref", "int x=42; int *p=&x; printf(\"%d\", *p);", "42"),
            ("ptr-write", "int x=1; int *p=&x; *p=99; printf(\"%d\", x);", "99"),
            ("ptr-arith", "int a[3]; a[0]=5;a[1]=6;a[2]=7; int *p=a; printf(\"%d\", *(p+1));", "6"),
            ("struct", "struct P{int x; int y;}; int main(){ struct P p; p.x=3; p.y=4; printf(\"%d\", p.x+p.y); return 0; }", "@FULL@7"),
            ("struct-ptr", "struct P{int x; int y;}; int main(){ struct P p; struct P *q=&p; q->x=8; printf(\"%d\", p.x); return 0; }", "@FULL@8"),
            ("union", "union U{int i; char c;}; int main(){ union U u; u.i=65; printf(\"%c\", u.c); return 0; }", "@FULL@A"),
            ("func-call", "int add(int a,int b){return a+b;} int main(){ printf(\"%d\", add(3,4)); return 0; }", "@FULL@7"),
            ("recursion", "int f(int n){ if(n<=1) return 1; return n*f(n-1);} int main(){ printf(\"%d\", f(5)); return 0; }", "@FULL@120"),
            ("func-ptr", "int add(int a,int b){return a+b;} int main(){ int (*f)(int,int)=add; printf(\"%d\", f(2,3)); return 0; }", "@FULL@5"),
            ("nested-loop", "int s=0; for(int i=0;i<3;i=i+1){for(int j=0;j<3;j=j+1){s=s+1;}} printf(\"%d\", s);", "9"),
            ("typedef", "typedef int entero; int main(){ entero x=5; printf(\"%d\", x); return 0; }", "@FULL@5"),
            ("string-index", "char *s=\"ABC\"; printf(\"%c\", s[1]);", "B"),
            ("unsigned", "unsigned int u = 4294967295; printf(\"%u\", u);", "4294967295"),
            ("long", "long l = 9000000000; printf(\"%d\", l);", "9000000000"),
            ("bitops", "printf(\"%d %d %d\", 12 & 10, 12 | 3, 12 ^ 10);", "8 15 6"),
            ("neg-unary", "int x=5; printf(\"%d\", -x);", "-5"),
            ("not", "printf(\"%d %d\", !0, !5);", "1 0"),
        ];
        let mut broken = Vec::new();
        for (name, body, expected) in cases {
            let (src, expected) = if let Some(e) = expected.strip_prefix("@FULL@") {
                (body.to_string(), e.to_string())
            } else {
                (format!("int main() {{ {body} return 0; }}"), expected.to_string())
            };
            let got = std::panic::catch_unwind(|| run_c(&src))
                .unwrap_or_else(|_| "<no ejecuta>".into());
            if got.trim() != expected {
                broken.push(format!("  {name:<16} => {:?}  (esperado {:?})", got.trim(), expected));
            }
        }
        let total = cases.len();
        assert!(broken.is_empty(), "\n{}/{} FUNCIONAN. ROTOS:\n{}", total - broken.len(), total, broken.join("\n"));
    }


    /// El payload `hola_C.bex` que el kernel EMBEBE, ejecutado.
    ///
    /// Si alguien toca el codegen y esta salida cambia, hay que regenerar
    /// el .bex antes de flashear — si no, el kernel llevaria un binario
    /// que ya no corresponde a su fuente.
    ///
    ///   cargo run -p bmo-c-front -- toolchain/lang/c/examples/hola_C.c     ///       -o Ultra_kernel_x86-64/kernel/src/ring0/hola_C.bex
    #[test]
    fn hola_c_payload_output_is_what_the_kernel_will_show() {
        let out = run_c(include_str!("../examples/hola_C.c"));
        let esperado = [
            "hola desde C en el Ryzen",
            "suma 1..10 = 55",
            "42-100=-58  100/7=14  100%7=2",
            "fase: calculo",
            "cadena=viva hex=beef",
            "C termino ok",
        ]
        .map(|l| format!("{l}\n"))
        .concat();
        assert_eq!(out, esperado);
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

    // ═══════════════ BMO C/Control: la puerta como instrucción ═══════════════

    /// El literal que hacía falta antes que ningún otro: `CURRENT_TASK`.
    ///
    /// `i64::from_str_radix` no puede con `0xFFFFFFFFFFFFFFFE` y el
    /// `unwrap_or(0)` lo convertía en **cero, en silencio** — o sea, en la
    /// capability 0. Escribir la constante correcta compilaba y llamaba a otro.
    #[test]
    fn hex_de_64_bits_no_se_convierte_en_cero() {
        let out = run_c("int main() { unsigned long long c; c = 0xFFFFFFFFFFFFFFFE; \
                         printf(\"%x\\n\", c); return 0; }");
        assert_eq!(out.trim(), "fffffffffffffffe");
    }

    /// Y si de verdad no cabe, se dice. Callarlo sería el mismo error con otro
    /// valor.
    #[test]
    fn hex_mas_alla_de_64_bits_es_un_error_no_un_cero() {
        let err = compile_source_to_bef("int main() { return 0x1FFFFFFFFFFFFFFFF; }")
            .expect_err("no cabe en 64 bits: tiene que fallar, no valer 0");
        assert!(err.message.contains("64 bits"), "mensaje: {}", err.message);
    }

    /// `__syscall` es una fila de la tabla sem-asm, no una caja negra: sus
    /// argumentos van a rdi/rsi/rdx/r10/r8, que es la convención de la puerta.
    ///
    /// Se comprueba sobre `CONSOLE_WRITE` porque es la única operación cuyo
    /// efecto se ve desde fuera: si un argumento cayera en otro registro, no
    /// saldría este texto.
    #[test]
    fn syscall_intrinseco_coloca_los_argumentos_donde_dice_la_tabla() {
        // "hola" en little-endian dentro de un solo u64, que es como viaja la
        // consola: 8 bytes por llamada con el cero como final.
        let out = run_c(
            "int main() { __syscall(0, 0xFFFFFFFFFFFFFFFE, 6, 0x616C6F68, 0, 0); return 0; }",
        );
        assert_eq!(out, "hola");
    }

    /// La puerta contesta DOS cosas: el código en rax y el valor en rdx. Las
    /// dos filas de la tabla existen para poder recoger cada una.
    #[test]
    fn syscall_valor_recoge_rdx_y_syscall_recoge_rax() {
        // CONSOLE_READ devuelve `(n << 56) | bytes` en rdx, y 0 (ok) en rax.
        let fuente = "int main() { \
             unsigned long long v; unsigned long long c; \
             v = __syscall_valor(0, 0xFFFFFFFFFFFFFFFE, 0x0F, 0, 0, 0); \
             c = __syscall(0, 0xFFFFFFFFFFFFFFFE, 0x0F, 0, 0, 0); \
             printf(\"valor=%x codigo=%d\\n\", v, (int)c); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("AB"));
        // n=2, bytes = 'A','B' → 0x0200000000004241. La segunda lectura ya no
        // tiene nada, así que el código sigue siendo 0 pero el valor sería 0.
        assert_eq!(out.trim(), "valor=200000000004241 codigo=0");
    }

    // ═══════════════ <bmo/bmo.h>: la superficie en C ═══════════════

    #[test]
    fn la_cabecera_baja_a_la_puerta_sin_runtime_que_enlazar() {
        let out = run_c_con_pp(
            "#include <bmo/bmo.h>\n\
             int main() { printf(\"pid=%d\\n\", (int)bmo_pid()); bmo_ceder(); \
             printf(\"cedi\\n\"); return 0; }",
        );
        assert_eq!(out, "pid=0\ncedi\n");
    }

    // ═══════════════ <bmo/entrada.h>: el ratón y el teclado ═══════════════

    /// Sin ceder la entrada, reclamarla da 0. Es el caso NORMAL —el compositor
    /// la tiene— y un programa que no lo comprueba lee ceros para siempre y
    /// parece un ratón roto.
    #[test]
    fn reclamar_la_entrada_puede_fallar_y_se_nota() {
        let fuente = "#include <bmo/entrada.h>\n\
             int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
             if (e == 0) { printf(\"sin entrada\\n\"); } else { printf(\"handle\\n\"); } \
             return 0; }";
        assert_eq!(run_c_sembrado(fuente, |_| {}).trim(), "sin entrada");
        assert_eq!(run_c_sembrado(fuente, |m| m.ceder_entrada()).trim(), "handle");
    }

    /// Las teclas salen una por llamada, y `-1` significa "no hay", que es el
    /// convenio de `getchar` y no un byte válido.
    #[test]
    fn las_teclas_salen_en_orden_y_el_final_es_menos_uno() {
        let fuente = "#include <bmo/entrada.h>\n\
             int main() { unsigned long long e; int t; \
             e = bmo_entrada_reclamar(); \
             for (;;) { t = bmo_entrada_tecla(e); if (t < 0) break; printf(\"%d \", t); } \
             printf(\"fin\\n\"); return 0; }";
        let out = run_c_sembrado(fuente, |m| {
            m.ceder_entrada();
            m.poner_teclas(&[b'a', b'b', 0x87]);
        });
        assert_eq!(out.trim(), "97 98 135 fin");
    }

    /// ★ La rueda **consume**: dos lecturas seguidas sin girar dan cero la
    /// segunda. Es la propiedad que decide si un scroll se mueve solo, y sólo
    /// se distingue de un acumulado EJECUTÁNDOLA.
    #[test]
    fn la_rueda_se_vacia_al_leerla() {
        let fuente = "#include <bmo/entrada.h>\n\
             int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
             printf(\"%d %d\\n\", bmo_entrada_rueda(e), bmo_entrada_rueda(e)); return 0; }";
        let out = run_c_sembrado(fuente, |m| {
            m.ceder_entrada();
            m.poner_rueda(4);
        });
        assert_eq!(out.trim(), "4 0");
    }

    /// Girar hacia atrás es NEGATIVO. Sin el `(int)` de la cabecera, el valor
    /// viaja como i32 dentro de un u64 y una muesca hacia abajo daría cuatro
    /// mil millones — un scroll que salta al principio del historial.
    #[test]
    fn la_rueda_hacia_atras_es_negativa() {
        let fuente = "#include <bmo/entrada.h>\n\
             int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
             printf(\"%d\\n\", bmo_entrada_rueda(e)); return 0; }";
        let out = run_c_sembrado(fuente, |m| {
            m.ceder_entrada();
            m.poner_rueda(-2);
        });
        assert_eq!(out.trim(), "-2");
    }

    /// Los tres datos del puntero viajan empaquetados en una sola llamada.
    #[test]
    fn el_puntero_se_desempaqueta_bien() {
        let fuente = "#include <bmo/entrada.h>\n\
             int main() { unsigned long long e; e = bmo_entrada_reclamar(); \
             printf(\"%d,%d b=%d ev=%d\\n\", bmo_entrada_x(e), bmo_entrada_y(e), \
             bmo_entrada_botones(e), (int)bmo_entrada_eventos(e)); return 0; }";
        let out = run_c_sembrado(fuente, |m| {
            m.ceder_entrada();
            m.poner_puntero(1024, 600, 1);
        });
        assert_eq!(out.trim(), "1024,600 b=1 ev=1");
    }

    // ═══════════════ <bmo/scroll.h>: la ventana sobre el historial ═══════════

    /// Los dos topes. Pasarse por arriba enseña filas en blanco —parece que se
    /// ha perdido todo—; pasarse por abajo deja la vista en negativo.
    #[test]
    fn el_scroll_se_topa_solo_en_los_dos_extremos() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { \
             printf(\"%d %d %d\\n\", bmo_scroll_mover(0, -50, 200, 16), \
             bmo_scroll_mover(0, 9999, 200, 16), bmo_scroll_mover(0, 10, 200, 16)); \
             return 0; }",
        );
        assert_eq!(out.trim(), "0 184 10");
    }

    /// Un historial que todavía no llena la ventana sólo tiene un sitio válido.
    #[test]
    fn sin_historial_suficiente_la_unica_vista_es_el_fondo() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { printf(\"%d\\n\", bmo_scroll_mover(0, 5, 10, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "0");
    }

    /// Tres filas por muesca, y hacia atrás resta. Es el mismo paso que el
    /// compositor: si divergieran, la rueda haría una cosa en Rust y otra en C.
    #[test]
    fn la_rueda_mueve_tres_filas_por_muesca_en_los_dos_sentidos() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { int v; v = bmo_scroll_rueda(0, 3, 200, 16); \
             printf(\"%d %d\\n\", v, bmo_scroll_rueda(v, -2, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "9 3");
    }

    /// Una página es `visibles - 1`: la fila que se solapa es lo que deja
    /// seguir leyendo sin volver atrás.
    #[test]
    fn repag_y_avpag_dejan_una_fila_de_solape() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { int v; v = bmo_scroll_tecla(0, BMO_TECLA_REPAG, 200, 16); \
             printf(\"%d %d\\n\", v, bmo_scroll_tecla(v, BMO_TECLA_AVPAG, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "15 0");
    }

    /// Una tecla que no es de scroll no mueve la vista. Sin esto, escribir
    /// movería el historial bajo los pies del que escribe.
    #[test]
    fn una_tecla_cualquiera_no_mueve_el_historial() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { printf(\"%d\\n\", bmo_scroll_tecla(7, 97, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "7");
    }

    /// Inicio y Fin van a los extremos de una vez.
    #[test]
    fn inicio_y_fin_saltan_a_los_extremos() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { int v; v = bmo_scroll_tecla(0, BMO_TECLA_INICIO, 200, 16); \
             printf(\"%d %d\\n\", v, bmo_scroll_tecla(v, BMO_TECLA_FIN, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "184 0");
    }

    /// La fila por la que empieza el dibujo. Es la cuenta que se reinventa mal
    /// cuando se escribe a mano en el sitio de pintar.
    #[test]
    fn la_primera_fila_visible_sigue_a_la_vista() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>\n\
             int main() { printf(\"%d %d\\n\", bmo_scroll_primera(0, 200, 16), \
             bmo_scroll_primera(10, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "184 174");
    }


    // ═══════════════ El ejemplo del repositorio, ejecutado ═══════════════

    /// Con el compositor vivo la entrada es SUYA, y esto lo dice en vez de
    /// quedarse leyendo ceros — que se ve igual que un ratón roto y manda a
    /// depurar el USB sin motivo.
    #[test]
    fn scroll_sin_entrada_lo_dice_y_se_va() {
        let out = run_c_con_pp(include_str!("../examples/scroll_C.c"));
        assert_eq!(out, "la entrada es de otro proceso: no hay scroll que hacer.
");
    }

    /// El programa entero: rueda hacia el pasado, RePag, Fin y ESC.
    ///
    /// Es la mitad de la prueba que el Ryzen no puede dar todavía —el ratón
    /// sigue sin verificar en metal—, y RePag/AvPag no dependen del ratón, así
    /// que esa mitad se puede cerrar aquí.
    #[test]
    fn scroll_recorre_el_historial_con_la_rueda_y_con_las_teclas() {
        let out = run_c_sembrado(include_str!("../examples/scroll_C.c"), |m| {
            m.ceder_entrada();
            m.poner_rueda(2);
            m.poner_teclas_por_fotograma(&[&[0x87], &[0x85], &[27]]);
        });
        let cabeceras: Vec<&str> = out.lines().filter(|l| l.starts_with("----")).collect();
        assert_eq!(
            cabeceras,
            vec![
                "---- filas 52..59 [al dia] ----",
                "---- filas 46..53 [historial] ----",
                "---- filas 39..46 [historial] ----",
                "---- filas 52..59 [al dia] ----",
            ],
            "salida completa:
{out}"
        );
        assert!(out.ends_with("hasta luego.
"), "salida completa:
{out}");
        // Y las filas son las que dicen ser: si el índice se calculara mal, la
        // cabecera seguiría cuadrando y el contenido no.
        assert!(out.contains("  fila 052
"), "salida completa:
{out}");
        assert!(out.contains("  fila 039
"), "salida completa:
{out}");
    }

    /// ★ La rueda se drena en la PRIMERA vuelta del bucle. Si el programa la
    /// volviera a sumar en la siguiente, el historial seguiría subiendo solo
    /// después de soltarla — el bug que la semántica de "consumir" evita, y que
    /// sólo se ve dando varias vueltas al bucle.
    #[test]
    fn el_scroll_no_sigue_moviendose_solo_tras_soltar_la_rueda() {
        let out = run_c_sembrado(include_str!("../examples/scroll_C.c"), |m| {
            m.ceder_entrada();
            m.poner_rueda(1);
            // Teclas que no mueven nada, una por fotograma: obligan al bucle a
            // dar vueltas sin que llegue ninguna muesca nueva.
            m.poner_teclas_por_fotograma(&[&[b'a'], &[b'b'], &[b'c'], &[b'd'], &[27]]);
        });
        let cabeceras: Vec<&str> = out.lines().filter(|l| l.starts_with("----")).collect();
        assert_eq!(
            cabeceras,
            vec![
                "---- filas 52..59 [al dia] ----",
                "---- filas 49..56 [historial] ----",
            ],
            "el historial se movió más de una vez con una sola muesca:
{out}"
        );
    }

    // ═══════════════ Los tres silencios que escondían todo esto ═══════════════

    /// ★ `#include` tiraba los `#define` de la cabecera.
    ///
    /// Y no fallaba: la directiva se consumía, el identificador seguía en el
    /// texto y el codegen lo ponía a cero. Dos constantes distintas se volvían
    /// la MISMA variable inventada, así que compararlas era cierto.
    #[test]
    fn una_cabecera_incluida_deja_sus_constantes() {
        let out = run_c_con_pp(
            "#include <bmo/entrada.h>
             int main() { printf(\"%d %d\\n\", BMO_TECLA_REPAG, BMO_TECLA_AVPAG); return 0; }",
        );
        assert_eq!(out.trim(), "135 136", "REPAG y AVPAG no pueden valer lo mismo");
    }

    /// La misma cabecera dos veces no duplica lo que trae. El guardia
    /// `#ifndef` sólo puede funcionar si el `#define` del guardia sobrevive al
    /// `#include` — antes no sobrevivía, así que el guardia no guardaba nada.
    #[test]
    fn incluir_dos_veces_no_duplica_la_cabecera() {
        let out = run_c_con_pp(
            "#include <bmo/scroll.h>
#include <bmo/entrada.h>
#include <bmo/bmo.h>
             int main() { printf(\"%d\\n\", bmo_scroll_mover(0, 4, 200, 16)); return 0; }",
        );
        assert_eq!(out.trim(), "4");
    }

    /// Un nombre que no existe NO VALE CERO. Un cero inventado es la peor
    /// respuesta posible: es legítimo en cualquier expresión, así que el error
    /// viaja hasta donde ya no se puede rastrear.
    #[test]
    fn un_identificador_que_no_existe_es_un_error_no_un_cero() {
        let err = compile_source_to_bef("int main() { return NO_EXISTE; }")
            .expect_err("un nombre sin declarar tiene que fallar");
        assert!(err.message.contains("NO_EXISTE"), "mensaje: {}", err.message);
    }

    /// Y una llamada sin destino tampoco es un hueco: `E8 00000000` es "llama a
    /// la siguiente instrucción", o sea un no-op con dirección de retorno.
    #[test]
    fn llamar_a_una_funcion_que_no_existe_es_un_error() {
        let err = compile_source_to_bef("int main() { fantasma(1); return 0; }")
            .expect_err("llamar a lo que no existe tiene que fallar");
        assert!(err.message.contains("fantasma"), "mensaje: {}", err.message);
    }

    // ═══════════════ Macros CON PARÁMETROS ═══════════════
    //
    // El preprocesador las guardaba y no las expandía nunca: el `if` de
    // `expand_line` pedía `params.is_empty()`. `MAX(a,b)` se quedaba en el
    // texto y el parser lo tomaba por una llamada a una función inexistente.

    #[test]
    fn una_macro_con_parametros_se_expande() {
        let out = run_c_con_pp(
            "#define DOBLE(x) ((x) + (x))\n\
             int main() { printf(\"%d\\n\", DOBLE(21)); return 0; }",
        );
        assert_eq!(out.trim(), "42");
    }

    /// Los paréntesis del cuerpo no son adorno: sin ellos `DOBLE(1+1)` daría
    /// `1+1+1+1`. Se comprueba que el argumento entra ENTERO.
    #[test]
    fn el_argumento_entra_entero_no_troceado() {
        let out = run_c_con_pp(
            "#define TRIPLE(x) ((x) * 3)\n\
             int main() { printf(\"%d\\n\", TRIPLE(2 + 5)); return 0; }",
        );
        assert_eq!(out.trim(), "21");
    }

    /// Una coma DENTRO de paréntesis no separa argumentos. Sin esto,
    /// `MAX(f(a,b), c)` se leería como tres.
    #[test]
    fn las_comas_anidadas_no_separan_argumentos() {
        let out = run_c_con_pp(
            "#define SUMA(a, b) ((a) + (b))\n\
             int main() { printf(\"%d\\n\", SUMA(SUMA(1, 2), 4)); return 0; }",
        );
        assert_eq!(out.trim(), "7");
    }

    /// ★ El espacio manda, y es el único sitio de C donde manda.
    ///
    /// `#define X (760)` es un OBJETO cuyo cuerpo empieza por paréntesis. El
    /// lector viejo lo registraba como macro-función con un parámetro llamado
    /// `760` y cuerpo **vacío**: la constante desaparecía en silencio.
    #[test]
    fn un_parentesis_separado_del_nombre_no_hace_una_funcion() {
        let out = run_c_con_pp(
            "#define ANCHO (760)\n\
             int main() { printf(\"%d\\n\", ANCHO); return 0; }",
        );
        assert_eq!(out.trim(), "760");
    }

    /// Y pegado sí: una función SIN parámetros no es lo mismo que un objeto.
    #[test]
    fn una_macro_funcion_sin_parametros_se_invoca_con_parentesis() {
        let out = run_c_con_pp(
            "#define UNO() 1\n\
             int main() { printf(\"%d\\n\", UNO()); return 0; }",
        );
        assert_eq!(out.trim(), "1");
    }

    /// `#p` convierte el argumento en cadena. Es lo que hace posible un
    /// `assert` que dice QUÉ falló.
    #[test]
    fn el_sostenido_convierte_el_argumento_en_cadena() {
        let out = run_c_con_pp(
            "#define NOMBRE(x) #x\n\
             int main() { printf(\"%s\\n\", NOMBRE(hola)); return 0; }",
        );
        assert_eq!(out.trim(), "hola");
    }

    /// `##` pega dos piezas en UN símbolo, comiéndose el espacio de los lados.
    #[test]
    fn el_doble_sostenido_pega_dos_piezas() {
        let out = run_c_con_pp(
            "#define UNE(a, b) a ## b\n\
             int main() { int xy; xy = 9; printf(\"%d\\n\", UNE(x, y)); return 0; }",
        );
        assert_eq!(out.trim(), "9");
    }

    /// Variádicas: lo que sobra entra por `__VA_ARGS__`.
    #[test]
    fn una_macro_variadica_pasa_el_resto() {
        let out = run_c_con_pp(
            "#define DI(fmt, ...) printf(fmt, __VA_ARGS__)\n\
             int main() { DI(\"%d-%d\\n\", 4, 7); return 0; }",
        );
        assert_eq!(out.trim(), "4-7");
    }

    /// Una macro que produce otra macro: hacen falta varias pasadas.
    #[test]
    fn una_macro_puede_producir_otra() {
        let out = run_c_con_pp(
            "#define A B\n#define B 5\n\
             int main() { printf(\"%d\\n\", A); return 0; }",
        );
        assert_eq!(out.trim(), "5");
    }

    /// ★ Ya NO se sustituye dentro de las cadenas. Antes `printf(\"ANCHO\")`
    /// imprimía el valor: el texto de un literal es dato, no código.
    #[test]
    fn una_macro_no_se_expande_dentro_de_una_cadena() {
        let out = run_c_con_pp(
            "#define ANCHO 760\n\
             int main() { printf(\"ANCHO=%d\\n\", ANCHO); return 0; }",
        );
        assert_eq!(out.trim(), "ANCHO=760");
    }

    /// Invocarla con un número de argumentos que no cuadra es un ERROR. Antes
    /// no podía serlo: la macro no se expandía, así que la llamada sobrevivía
    /// hasta el codegen.
    #[test]
    fn invocar_una_macro_con_argumentos_de_mas_es_un_error() {
        let err = compile_with_preprocessor(
            "#define SUMA(a, b) ((a) + (b))\nint main() { return SUMA(1, 2, 3); }",
            std::path::Path::new("prueba.c"),
            CStandard::C11,
        )
        .expect_err("tres argumentos para dos parametros tiene que fallar");
        assert!(err.message.contains("SUMA"), "mensaje: {}", err.message);
    }

    /// Una macro que se nombra a sí misma no puede colgar el compilador.
    #[test]
    fn una_macro_recursiva_no_cuelga() {
        let out = run_c_con_pp(
            "#define A A\n\
             int main() { printf(\"ok\\n\"); return 0; }",
        );
        assert_eq!(out.trim(), "ok");
    }

    // ═══════════════ Listas de inicialización ═══════════════
    //
    // No existían: ni siquiera `int a[3] = {1,2,3}`. Ver la cabecera de
    // `parser/inicializador.rs` para el diseño y para qué hicieron GCC, Clang,
    // chibicc, TCC y MSVC con esto mismo.

    #[test]
    fn una_lista_posicional_llena_un_array() {
        let out = run_c("int main() { int a[4] = {10, 20, 30, 40}; \
                         printf(\"%d %d %d\\n\", a[0], a[2], a[3]); return 0; }");
        assert_eq!(out.trim(), "10 30 40");
    }

    #[test]
    fn una_lista_posicional_llena_un_struct() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int main() { struct P p = {1, 2, 3}; \
                         printf(\"%d %d %d\\n\", p.x, p.y, p.z); return 0; }");
        assert_eq!(out.trim(), "1 2 3");
    }

    /// ★ C99 §6.7.9/21: lo NO mencionado vale CERO.
    ///
    /// Sin el borrado previo, `q.x` y `q.z` traerían lo que hubiera en la pila
    /// — basura distinta en cada ejecución, y un bug que no se repite.
    #[test]
    fn lo_no_mencionado_vale_cero() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int main() { struct P q = {.y = 7}; \
                         printf(\"%d %d %d\\n\", q.x, q.y, q.z); return 0; }");
        assert_eq!(out.trim(), "0 7 0");
    }

    /// Los designadores pueden ir en cualquier orden: el offset lo pone el
    /// nombre, no la posición.
    #[test]
    fn los_designadores_van_en_el_orden_que_quieran() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int main() { struct P r = {.z = 9, .x = 5}; \
                         printf(\"%d %d %d\\n\", r.x, r.y, r.z); return 0; }");
        assert_eq!(out.trim(), "5 0 9");
    }

    /// ★ La regla que más se olvida al implementar esto a mano: un designador
    /// **reposiciona el cursor**, y lo siguiente sin designador sigue DESDE
    /// AHÍ. La `d` va al índice 3, no al 0.
    #[test]
    fn tras_un_designador_se_sigue_desde_ahi() {
        let out = run_c("int main() { int b[5] = {[2] = 30, 40}; \
                         printf(\"%d %d %d %d\\n\", b[0], b[2], b[3], b[4]); return 0; }");
        assert_eq!(out.trim(), "0 30 40 0");
    }

    /// El último gana, y sale solo de emitir en orden.
    #[test]
    fn si_un_campo_se_inicializa_dos_veces_gana_el_ultimo() {
        let out = run_c("struct P { int x; int y; }; \
                         int main() { struct P p = {.x = 1, .y = 2, .x = 9}; \
                         printf(\"%d %d\\n\", p.x, p.y); return 0; }");
        assert_eq!(out.trim(), "9 2");
    }

    /// Anidado: `{ {..}, {..} }` sobre un array de structs.
    #[test]
    fn una_lista_anidada_recorre_los_subobjetos() {
        let out = run_c("struct P { int x; int y; }; \
                         int main() { struct P v[2] = { {1, 2}, {.y = 4} }; \
                         printf(\"%d %d %d %d\\n\", v[0].x, v[0].y, v[1].x, v[1].y); return 0; }");
        assert_eq!(out.trim(), "1 2 0 4");
    }

    /// Cadena de designadores: `[1].y = …` es legal C99.
    #[test]
    fn una_cadena_de_designadores_baja_dos_niveles() {
        let out = run_c("struct P { int x; int y; }; \
                         int main() { struct P v[3] = {[2].y = 8}; \
                         printf(\"%d %d\\n\", v[2].x, v[2].y); return 0; }");
        assert_eq!(out.trim(), "0 8");
    }

    /// Una cadena inicializa un `char[]` **byte a byte**. Es la única forma en
    /// C de inicializar un agregado sin llaves.
    #[test]
    fn una_cadena_llena_un_array_de_char() {
        let out = run_c("int main() { char s[8] = \"hola\"; \
                         printf(\"%s|%d\\n\", s, s[4]); return 0; }");
        assert_eq!(out.trim(), "hola|0");
    }

    /// Y si no cabe, se dice. Escribir uno de más pisaría lo de al lado.
    #[test]
    fn una_cadena_que_no_cabe_es_un_error() {
        let err = compile_source_to_bef("int main() { char s[3] = \"hola\"; return 0; }")
            .expect_err("cinco bytes en un array de tres tiene que fallar");
        assert!(err.message.contains("array"), "mensaje: {}", err.message);
    }

    /// Un escalar entre llaves es legal.
    #[test]
    fn un_escalar_admite_llaves() {
        let out = run_c("int main() { int x = {5}; printf(\"%d\\n\", x); return 0; }");
        assert_eq!(out.trim(), "5");
    }

    /// Sobrarse del array es un error, no un desbordamiento silencioso.
    #[test]
    fn pasarse_del_final_del_array_es_un_error() {
        let err = compile_source_to_bef("int main() { int a[2] = {1,2,3}; return a[0]; }")
            .expect_err("tres valores en un array de dos tiene que fallar");
        assert!(err.message.contains("elementos"), "mensaje: {}", err.message);
    }

    /// Un campo que no existe se dice con el nombre delante.
    #[test]
    fn un_campo_inventado_es_un_error() {
        let err = compile_source_to_bef(
            "struct P { int x; }; int main() { struct P p = {.pepe = 1}; return 0; }",
        )
        .expect_err("un campo que no existe tiene que fallar");
        assert!(err.message.contains("pepe"), "mensaje: {}", err.message);
    }

    /// ★ La declaración se parsea en TRES sitios (cuerpo de función, bloque
    /// anidado, `parse_stmt`) y estaba copiada en los tres. Al añadir las
    /// listas sólo aprendió uno: dentro de un `if`, `int a[2] = {…}` no
    /// compilaba. Ahora los tres llaman a `terminar_declaracion`.
    #[test]
    fn una_lista_tambien_compila_dentro_de_un_bloque() {
        let out = run_c("int main() { if (1) { int a[2] = {7, 8}; \
                         printf(\"%d %d\\n\", a[0], a[1]); } return 0; }");
        assert_eq!(out.trim(), "7 8");
    }

    /// ★ Y el emulador tiene que escribir el tamaño EXACTO.
    ///
    /// `mov [mem], eax` toca CUATRO bytes; el emulador escribía ocho rellenando
    /// de ceros. En un registro eso es correcto —escribir uno de 32 bits borra
    /// la mitad alta— pero en memoria destruye lo de al lado. Este caso lo
    /// destapó: con `{.x = 1, .y = 2, .x = 9}`, la última escritura de `x`
    /// borraba la `y` de detrás y salía `9 0`.
    ///
    /// Un emulador que hace fallar código correcto es peor que uno que no
    /// existe: manda a buscar el bug al sitio equivocado.
    #[test]
    fn escribir_un_campo_no_toca_al_de_al_lado() {
        let out = run_c("struct P { int x; int y; }; \
                         int main() { struct P p; p.y = 77; p.x = 5; \
                         printf(\"%d %d\\n\", p.x, p.y); return 0; }");
        assert_eq!(out.trim(), "5 77");
    }

    // ═══════════════ Structs POR VALOR ═══════════════
    //
    // Ver `codegen/agregados.rs` para la ABI de agregados de BMO y para qué
    // hacen SysV (clasificación por eightbytes) y Win64 (referencia oculta).

    /// `q = p` copia TODOS los bytes. Antes emitía `mov rax,[p]; mov [q],rax`
    /// — ocho— y un struct de 12 se copiaba a medias, en silencio.
    #[test]
    fn asignar_un_struct_copia_todos_sus_bytes() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int main() { struct P p = {1, 2, 3}; struct P q; q = p; \
                         printf(\"%d %d %d\\n\", q.x, q.y, q.z); return 0; }");
        assert_eq!(out.trim(), "1 2 3");
    }

    /// Y es una COPIA: tocar el destino no toca el origen.
    #[test]
    fn la_copia_de_un_struct_es_independiente() {
        let out = run_c("struct P { int x; int y; }; \
                         int main() { struct P p = {1, 2}; struct P q; q = p; q.y = 99; \
                         printf(\"%d %d\\n\", p.y, q.y); return 0; }");
        assert_eq!(out.trim(), "2 99");
    }

    /// Pasarlo a una función manda sus bytes, no su primera palabra.
    #[test]
    fn un_struct_viaja_entero_a_una_funcion() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int suma(struct P p) { return p.x + p.y + p.z; } \
                         int main() { struct P p = {1, 2, 3}; \
                         printf(\"%d\\n\", suma(p)); return 0; }");
        assert_eq!(out.trim(), "6");
    }

    /// ★ Y corre a los que vienen detrás. Un agregado de 12 bytes ocupa DOS
    /// ranuras; con el `16 + i*8` de antes, el parámetro siguiente se leía
    /// desde la mitad del anterior.
    #[test]
    fn un_struct_corre_los_parametros_que_van_detras() {
        let out = run_c("struct P { int x; int y; int z; }; \
                         int mezcla(int a, struct P p, int b) { return a * 100 + p.y + b; } \
                         int main() { struct P p = {1, 2, 3}; \
                         printf(\"%d\\n\", mezcla(7, p, 5)); return 0; }");
        assert_eq!(out.trim(), "707");
    }

    /// La función recibe una COPIA: modificarla no toca la del llamante.
    #[test]
    fn la_funcion_recibe_una_copia_no_el_original() {
        let out = run_c("struct P { int x; int y; }; \
                         int rompe(struct P p) { p.x = 99; return p.x; } \
                         int main() { struct P p = {1, 2}; int r; r = rompe(p); \
                         printf(\"%d %d\\n\", r, p.x); return 0; }");
        assert_eq!(out.trim(), "99 1");
    }

    /// Devolver un struct es un TERCER mecanismo (puntero oculto) y todavía no
    /// está. Se dice con el nombre delante: devolver ocho bytes de un struct de
    /// doce sería exactamente la mentira que este compilador no cuenta.
    #[test]
    fn devolver_un_struct_por_valor_se_rechaza_con_motivo() {
        let err = compile_source_to_bef(
            "struct P { int x; int y; }; \
             struct P haz() { struct P p = {1,2}; return p; } \
             int main() { struct P q; q = haz(); return q.x; }",
        )
        .expect_err("devolver un struct todavia no se compila");
        assert!(err.message.contains("haz"), "mensaje: {}", err.message);
    }

    // ═══════════════ La ENTRADA: getchar y scanf ═══════════════
    //
    // La mitad que le faltaba a `printf`. Ver `codegen/entrada.rs`.

    /// Un byte cada vez, en orden.
    #[test]
    fn getchar_entrega_los_bytes_en_orden() {
        let fuente = "int main() { int c; for (;;) { c = getchar(); \
                      if (c == 10) break; printf(\"[%c]\", c); } return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("hola\n"));
        assert_eq!(out, "[h][o][l][a]");
    }

    /// ★ La puerta entrega **hasta 7 bytes de una vez y los CONSUME**. Sin el
    /// buffer, un lector de un byte se comería seis de cada siete pulsaciones y
    /// parecería un teclado que pierde letras. Trece bytes son dos paquetes.
    #[test]
    fn getchar_no_pierde_los_bytes_que_sobran_del_paquete() {
        let fuente = "int main() { int c; int n; n = 0; \
                      for (;;) { c = getchar(); if (c == 10) break; n = n + 1; } \
                      printf(\"%d\\n\", n); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("abcdefghijklm\n"));
        assert_eq!(out.trim(), "13");
    }

    /// El buffer es UNO: dos `getchar()` distintos comparten los bytes que
    /// sobraron. Si cada sitio tuviera el suyo, el segundo empezaría a leer
    /// desde cero y se perderían los del primero.
    #[test]
    fn dos_getchar_distintos_comparten_el_mismo_buffer() {
        let fuente = "int main() { int a; int b; a = getchar(); b = getchar(); \
                      printf(\"%c%c\\n\", a, b); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("xy\n"));
        assert_eq!(out.trim(), "xy");
    }

    #[test]
    fn scanf_lee_un_entero() {
        let fuente = "int main() { int x; scanf(\"%d\", &x); \
                      printf(\"leido=%d\\n\", x * 2); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("21\n"));
        assert_eq!(out.trim(), "leido=42");
    }

    /// Un negativo tecleado es negativo. Sin el signo, `-5` daría 5 y la cuenta
    /// saldría al revés sin una palabra.
    #[test]
    fn scanf_lee_un_entero_negativo() {
        let fuente = "int main() { int x; scanf(\"%d\", &x); \
                      printf(\"%d\\n\", x); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("-5\n"));
        assert_eq!(out.trim(), "-5");
    }

    /// `%s` lee la línea al buffer del llamante **con su cero final**: en C una
    /// cadena sin terminador no es una cadena, y el `%s` de después imprimiría
    /// hasta el primer cero que hubiera por ahí.
    #[test]
    fn scanf_lee_una_cadena_y_la_termina() {
        let fuente = "int main() { char s[16]; scanf(\"%s\", s); \
                      printf(\"<%s>\\n\", s); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("mundo\n"));
        assert_eq!(out.trim(), "<mundo>");
    }

    #[test]
    fn scanf_lee_un_caracter() {
        let fuente = "int main() { char c; scanf(\"%c\", &c); \
                      printf(\"%c%c\\n\", c, c); return 0; }";
        let out = run_c_sembrado(fuente, |m| m.poner_entrada("Z\n"));
        assert_eq!(out.trim(), "ZZ");
    }

    /// Más de una conversión se RECHAZA. Un `scanf` que ignora la mitad de su
    /// formato es un programa que lee mal en silencio — y las reglas de espacio
    /// en blanco de §7.21.6.2 ocupan página y media que aquí no están.
    #[test]
    fn scanf_con_dos_conversiones_se_rechaza_con_motivo() {
        let err = compile_source_to_bef(
            "int main() { int a; int b; scanf(\"%d %d\", &a, &b); return 0; }",
        )
        .expect_err("dos conversiones todavia no se compilan");
        assert!(err.message.contains("UNA conversion"), "mensaje: {}", err.message);
    }

    /// Y una conversión que no está se dice con cuál es.
    #[test]
    fn scanf_con_una_conversion_desconocida_se_rechaza() {
        let err = compile_source_to_bef("int main() { float f; scanf(\"%f\", &f); return 0; }")
            .expect_err("%f todavia no se compila");
        assert!(err.message.contains("%f"), "mensaje: {}", err.message);
    }

    /// Y las escrituras llevan el tamaño EXACTO del campo: escribir 8 bytes
    /// donde hay un `int` pisaría el campo siguiente.
    #[test]
    fn cada_escritura_usa_el_tamano_de_su_campo() {
        let out = run_c("struct M { char a; int b; char c; }; \
                         int main() { struct M m = {.a = 65, .b = 1000, .c = 66}; \
                         printf(\"%d %d %d\\n\", m.a, m.b, m.c); return 0; }");
        assert_eq!(out.trim(), "65 1000 66");
    }


    // ═══════════════ La tabla de intrinsecos, ENTERA ═══════════════

    /// ★ Compila una llamada a **cada fila** de `intrinsics.toml`.
    ///
    /// Es la matriz de conformidad de la tabla, y hacía falta desde que dejó de
    /// tener doce filas: el codegen valida el nombre de cada registro **al
    /// emitir**, así que una fila con `"rex"` en vez de `"rax"` no falla hasta
    /// que alguien la usa — y en una tabla de driver "alguien la usa" puede ser
    /// dentro de seis meses, en metal, buscando otra cosa.
    ///
    /// No comprueba que los bytes sean los correctos: eso lo dice el manual de
    /// Intel y está en la fila. Comprueba que la fila es **emitible**.
    #[test]
    fn cada_intrinseco_de_la_tabla_compila() {
        let tabla = bmo_sem_asm::Intrinsics::load_x86_64().expect("la tabla tiene que cargar");
        let nombres = tabla.names();
        assert!(nombres.len() >= 40, "la tabla se ha quedado corta: {}", nombres.len());

        for nombre in nombres {
            let def = tabla.get(nombre).unwrap();
            let ceros: Vec<&str> = vec!["0"; def.args.len()];
            let fuente = format!(
                "int main() {{ __{nombre}({}); return 0; }}",
                ceros.join(", ")
            );
            match compile_source_to_bef(&fuente) {
                Ok(bef) => assert!(!bef.is_empty(), "__{nombre} no produjo nada"),
                Err(e) => panic!("__{nombre} no compila: {}", e.message),
            }
        }
    }

    /// Y la aridad se valida contra la tabla: pasarle un argumento de más a una
    /// instrucción que no lo tiene es un error, no un argumento ignorado.
    #[test]
    fn un_intrinseco_con_argumentos_de_mas_se_rechaza() {
        let err = compile_source_to_bef("int main() { __hlt(1, 2); return 0; }")
            .expect_err("hlt no toma argumentos");
        assert!(err.message.contains("hlt"), "mensaje: {}", err.message);
    }

    /// Un nombre con `__` que no está en la tabla se dice, y se dice DÓNDE
    /// mirar. El namespace `__` es de la implementación, así que aquí no puede
    /// caer a "función desconocida".
    #[test]
    fn un_intrinseco_que_no_existe_dice_donde_estan() {
        let err = compile_source_to_bef("int main() { __inventado(); return 0; }")
            .expect_err("no existe");
        assert!(err.message.contains("intrinsics.toml"), "mensaje: {}", err.message);
    }

    // ═══════════════ La libreria SEMANTIC ═══════════════
    //
    // Cada funcion ES una instruccion. Ver `tables/semantic/semantic.h` para
    // que hacen GCC, MSVC y Clang con esto mismo, y en que se diferencia BMO
    // (aqui es una fila de TOML, alli son miles de lineas de C++).

    #[test]
    fn semantic_compila_entera() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { respira(); barrera_total(); barrera_escrituras();              barrera_lecturas(); printf(\"ok\n\"); return 0; }",
        );
        assert_eq!(out.trim(), "ok");
    }

    /// ★ Un atomico devuelve **lo que HABIA**, no lo que se puso. Es lo que se
    /// escribe al reves sin notarlo, y no se ve en un volcado de bytes.
    #[test]
    fn xchg_devuelve_lo_que_habia() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { u64 c; c = 7;              printf(\"%d %d\n\", (int)atomico_xchg(&c, 42), (int)c); return 0; }",
        );
        assert_eq!(out.trim(), "7 42");
    }

    /// El compara-e-intercambia, en sus dos caminos: cuando cuadra cambia, y
    /// cuando no cuadra **deja el valor y devuelve el de verdad**, que es lo que
    /// permite reintentar sin releer.
    #[test]
    fn cas_cambia_solo_si_cuadra_y_siempre_dice_lo_que_habia() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { u64 c; c = 5;              printf(\"%d %d \", (int)atomico_cas(&c, 5, 9), (int)c);              printf(\"%d %d\n\", (int)atomico_cas(&c, 5, 77), (int)c); return 0; }",
        );
        assert_eq!(out.trim(), "5 9 9 9");
    }

    /// `xadd` suma y entrega lo ANTERIOR: un contador que reparte numeros sin
    /// dar el mismo dos veces.
    #[test]
    fn xadd_entrega_el_valor_anterior() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { u64 c; c = 100;              printf(\"%d %d %d\n\", (int)atomico_sumar_y_devolver(&c, 1),              (int)atomico_sumar_y_devolver(&c, 1), (int)c); return 0; }",
        );
        assert_eq!(out.trim(), "100 101 102");
    }

    #[test]
    fn los_atomicos_sin_retorno_modifican_la_memoria() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { u64 c; c = 8; atomico_sumar(&c, 4);              atomico_encender(&c, 1);              printf(\"%d\n\", (int)c); return 0; }",
        );
        assert_eq!(out.trim(), "13");
    }

    /// Contar y buscar bits — de lo que vive un asignador de marcos.
    #[test]
    fn los_intrinsecos_de_bits_cuentan_bien() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { printf(\"%d %d %d %x\n\", bits_contar(0xF0F0),              bits_primero(0x00100000), bits_ultimo(0x00100000),              bytes_al_reves(0x11223344)); return 0; }",
        );
        assert_eq!(out.trim(), "8 20 20 44332211");
    }

    /// ★ `bsf` con cero es INDEFINIDO y el emulador lo modela como el silicio:
    /// deja el destino intacto. `tzcnt` sí está definido y da 32. La diferencia
    /// es la que hace que un mapa de bits lleno reserve un marco ya dado.
    #[test]
    fn tzcnt_esta_definido_en_cero_y_bsf_no() {
        let out = run_c_con_pp(
            "#include <semantic/semantic.h>
             int main() { printf(\"%d\n\", bits_ceros_derecha(0)); return 0; }",
        );
        assert_eq!(out.trim(), "32");
    }
}

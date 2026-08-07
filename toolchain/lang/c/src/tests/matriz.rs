//! La MATRIZ: el programa entero, de punta a punta
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

/// El ejemplo del repositorio, ejecutado. Si alguien vuelve a invertir
/// un operador, este test lo dice antes de que haga falta flashear nada.
#[test]
fn hola_example_produces_its_documented_output() {
    let out = run_c(include_str!("../../examples/hola.c"));
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
        // ★ Dos filas que faltaban de siempre, y el hueco era invisible: el
        // codegen emitía `~` y los desplazamientos BIEN —el .bef se escribe
        // sin quejarse— pero el emulador no decodificaba el grupo F7 /2, así
        // que no había forma de EJECUTARLOS y nadie les puso fila. Lo destapó
        // C++ al escribir su matriz desde cero (fix en `bmo-lower::emu`).
        ("bitnot", "printf(\"%d %d\", ~0, ~5);", "-1 -6"),
        ("shifts", "printf(\"%d %d\", 21 << 1, 84 >> 1);", "42 42"),
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
    let out = run_c(include_str!("../../examples/hola_C.c"));
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
fn profile_is_c() {
    assert_eq!(profile().name, "C");
}

/// ★ **Sin `main` no hay programa.**
///
/// Un fichero vacío producía un BEF de 8 240 bytes con `entry_offset = 0`, o
/// sea apuntando a lo primero que hubiera en la sección de código, y se
/// escribía sin quejarse. Un binario con punto de entrada inventado falla en
/// el metal y no en la compilación — que es donde se puede leer el motivo.
#[test]
fn sin_main_no_hay_programa() {
    for fuente in ["", "int suma(int a, int b) { return a + b; }"] {
        let e = compile_source_to_bef(fuente)
            .expect_err("sin punto de entrada no se puede escribir un .bef");
        assert!(e.message.contains("main"), "el error tiene que nombrar `main`: {}", e.message);
    }
}


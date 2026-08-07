//! Punteros a FUNCION: el despacho virtual en C puro
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

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

/// ★ **El ladrillo de las vtables, probado en C.**
///
/// Una tabla de punteros a función en una global, rellenada en ejecución y
/// llamada por índice. Es EXACTAMENTE la forma que una función virtual de C++
/// necesita, y por eso se prueba aquí: si esto no corre, el paso 5 de C++ no
/// tiene dónde apoyarse.
///
/// Se rellena en ejecución y no con un inicializador estático porque las
/// globales de BMO C sólo admiten `Expr::Int` — una dirección de función no se
/// conoce hasta que se emite el código.
#[test]
fn una_tabla_de_punteros_a_funcion_en_una_global() {
    let src = r#"
long tabla[2];
long doble(long x) { return x * 2; }
long mitad(long x) { return x / 2; }
int main() {
    tabla[0] = doble;
    tabla[1] = mitad;
    long (*f)(long) = tabla[0];
    long (*g)(long) = tabla[1];
    printf("%d %d", f(21), g(84));
    return 0;
}
"#;
    assert_eq!(run_c(src).trim(), "42 42");
}

/// Y con el índice calculado en EJECUCIÓN, que es lo que hace un despacho
/// virtual de verdad: la ranura sale del tipo dinámico, no de una constante.
#[test]
fn la_tabla_se_indexa_con_un_indice_de_ejecucion() {
    let src = r#"
long tabla[2];
long doble(long x) { return x * 2; }
long mitad(long x) { return x / 2; }
int main() {
    tabla[0] = doble;
    tabla[1] = mitad;
    int i = 0;
    long (*f)(long) = tabla[i];
    i = i + 1;
    long (*g)(long) = tabla[i];
    printf("%d %d", f(21), g(84));
    return 0;
}
"#;
    assert_eq!(run_c(src).trim(), "42 42");
}

/// ★★ **El despacho virtual entero, escrito en C.**
///
/// Dos objetos del mismo tipo estático con tablas distintas: la misma línea de
/// código llama a funciones distintas según lo que haya en el `vptr`. Eso es
/// una función virtual, y no hace falta nada más que esto.
///
/// Se prueba en C —y no sólo en C++— porque es el suelo sobre el que el paso 5
/// se apoya: si esta forma no corre, la vtable de C++ no tiene dónde pisar. Es
/// lo mismo que se escribiría a mano en C para hacer polimorfismo, que es la
/// razón por la que Bjarne pudo implementarlo como una traducción.
#[test]
fn el_despacho_virtual_entero_en_c() {
    let src = r#"
struct Animal { long vptr; long edad; };
long tabla_perro[1];
long tabla_gato[1];
long perro_habla(struct Animal *self) { return self->edad * 2; }
long gato_habla(struct Animal *self) { return self->edad + 100; }
int main() {
    tabla_perro[0] = perro_habla;
    tabla_gato[0] = gato_habla;
    struct Animal a; a.vptr = tabla_perro; a.edad = 21;
    struct Animal b; b.vptr = tabla_gato;  b.edad = 21;
    long *tp = a.vptr;
    long *tg = b.vptr;
    long (*f)(struct Animal*) = tp[0];
    long (*g)(struct Animal*) = tg[0];
    printf("%d %d", f(&a), g(&b));
    return 0;
}
"#;
    assert_eq!(run_c(src).trim(), "42 121");
}


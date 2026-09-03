//! Punteros a FUNCION: el despacho virtual en C puro
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

// ---- Punteros a funcion (Fase 2) ----

#[test]
fn parses_function_pointer_declarator() {
    // int (*op)(int, int); -- variable de tipo puntero.
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
    // op = add; -- 'add' como valor = lea rax,[rip+add] (48 8D 05).
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
    // op(3,4) donde op es variable -> call rax (FF D0), no call rel32.
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
    // &myfunc tambien da la direccion (equivalente a la decadencia).
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
    // (*fp)(args) -- forma explicita del puntero a funcion.
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

/// * **El ladrillo de las vtables, probado en C.**
///
/// Una tabla de punteros a funcion en una global, rellenada en ejecucion y
/// llamada por indice. Es EXACTAMENTE la forma que una funcion virtual de C++
/// necesita, y por eso se prueba aqui: si esto no corre, el paso 5 de C++ no
/// tiene donde apoyarse.
///
/// Se rellena en ejecucion y no con un inicializador estatico porque las
/// globales de BMO C solo admiten `Expr::Int` -- una direccion de funcion no se
/// conoce hasta que se emite el codigo.
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

/// Y con el indice calculado en EJECUCION, que es lo que hace un despacho
/// virtual de verdad: la ranura sale del tipo dinamico, no de una constante.
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

/// ** **El despacho virtual entero, escrito en C.**
///
/// Dos objetos del mismo tipo estatico con tablas distintas: la misma linea de
/// codigo llama a funciones distintas segun lo que haya en el `vptr`. Eso es
/// una funcion virtual, y no hace falta nada mas que esto.
///
/// Se prueba en C --y no solo en C++-- porque es el suelo sobre el que el paso 5
/// se apoya: si esta forma no corre, la vtable de C++ no tiene donde pisar. Es
/// lo mismo que se escribiria a mano en C para hacer polimorfismo, que es la
/// razon por la que Bjarne pudo implementarlo como una traduccion.
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

/// *** LA TABLA DE PUNTEROS A FUNCION INICIALIZADA -- el muro de DOOM (03-09).
///
/// `f_wipe.c` declara dentro de una funcion:
///
/// ```c
///    static int (*wipes[])(int, int, int) =
///        { wipe_initColorXForm, wipe_doColorXForm, ... };
///    rc = (*wipes[wipeno*3+1])(width, height, ticks);
/// ```
///
/// El metal contesto con un `#GP` en `wipe_ScreenWipe+0xa8`, `ff d0` = `call
/// rax`, y el veredicto **PUNTERO NO CANONICO**: se llama a una direccion que
/// nunca se escribio.
///
/// [!] Y el muro **ya estaba documentado desde el otro lado**: el descenso de
/// C++ dice, para las tablas virtuales, que *"no se pueden emitir como un
/// inicializador estatico porque las globales de BMO C solo admiten un entero,
/// y la direccion de una funcion no se conoce hasta emitir el codigo"*. C++ lo
/// rodea rellenando la tabla al principio de `main`. **DOOM no puede: su tabla
/// la escribe el programador.**
#[test]
#[ignore = "bug abierto: una tabla de punteros a funcion inicializada estaticamente sale a ceros -- es el #GP de wipe_ScreenWipe en el Ryzen (03-09)"]
fn una_tabla_estatica_de_punteros_a_funcion_se_puede_llamar() {
    let salida = run_c(
        "int uno(int a) { return a + 1; }
         int dos(int a) { return a + 2; }
         static int (*tabla[])(int) = { uno, dos };
         int main() {
             printf(\"%d %d\", (*tabla[0])(10), (*tabla[1])(10));
             return 0;
         }",
    );
    assert_eq!(salida, "11 12", "la tabla tiene que traer las DOS direcciones");
}

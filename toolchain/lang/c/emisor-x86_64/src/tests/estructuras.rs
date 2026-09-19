//! `struct`, campos y subindices: donde esta el byte
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

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

/// ** `o->in->y` ANIDADO -- antes devolvia offset 0 EN SILENCIO.
///
/// Hasta el 2026-09-02 esta casilla leia los dos offsets del AST (`off1`,
/// `off2`, los dos 8) porque el nodo `Expr::Arrow` los cargaba dentro. Ya no
/// los carga: los resuelve el codegen contra su tabla.
///
/// [!] Y la comprobacion **mejora al mudarse**: leer el numero del nodo no
/// probaba que se usara al emitir. Esto escribe en los DOS campos del `Inner`
/// --que estan a 0 y a 8, con relleno por medio-- y los lee. Con un offset
/// equivocado se pisan; con offset 0, `y` devuelve lo que vale `x`.
#[test]
fn la_flecha_anidada_llega_al_campo_de_dentro() {
    let salida = run_c(
        "struct Inner { int x; long y; };
         struct Outer { int pad; struct Inner *in; };
         int main() {
             struct Inner dentro; struct Outer fuera; struct Outer *o;
             o = &fuera; o->in = &dentro;
             o->in->x = 11; o->in->y = 22;
             printf(\"%d %d\", o->in->x, (int)o->in->y);
             return 0;
         }",
    );
    assert_eq!(salida, "11 22", "los dos campos de Inner no se pueden pisar");
}

/// ** `o.in.y` con agregados POR VALOR, y el relleno cuenta.
///
/// Gemela de la de arriba: leia `off1`/`off2` del arbol y ahora ejecuta.
/// `Outer` empieza con un `long pad`, asi que el `Inner` no cae en 0 -- si el
/// offset del salto de fuera se pierde, se escribe encima del relleno y lo que
/// se lee es basura.
#[test]
fn el_punto_anidado_llega_al_campo_de_dentro() {
    let salida = run_c(
        "struct Inner { int x; long y; };
         struct Outer { long pad; struct Inner in; };
         int main() {
             struct Outer o;
             o.pad = 99; o.in.x = 11; o.in.y = 22;
             printf(\"%d %d %d\", (int)o.pad, o.in.x, (int)o.in.y);
             return 0;
         }",
    );
    assert_eq!(salida, "99 11 22", "el relleno de fuera no se puede pisar");
}

#[test]
fn subscript_on_compound_base_now_works() {
    // p->arr[i] con arr: int* -- antes ERROR honesto, ahora compila.
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
        Stmt::Expr(Expr::Assign(n, v)) if n == "x" && matches!(v.as_ref(), Expr::IndexPtr(_, _))));
    assert!(ok, "s->arr[2] debe ser IndexPtr");
    // [!] Que el ELEMENTO sea `Int` ya no se puede leer del arbol: desde el
    // 2026-09-02 el nodo no lo carga, lo contesta el juez unico a partir de la
    // base. La garantia se comprueba ejecutando, en
    // `el_elemento_de_un_indice_compuesto_mide_lo_que_dice_su_tipo`.
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

#[test]
fn subscript_compound_base_assign_and_compound() {
    // p->arr[i] = v  y  p->arr[i] += v -- no se descartan.
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
    // Misma leccion que en `subscript_compound_assign`: la llana sigue siendo
    // `AssignIndexPtr` y la compuesta paso a `AssignOp` para no clonar el
    // lvalue. Se cuentan las dos formas, no se relaja el numero.
    let llana = p.functions[0].body.iter().filter(|st| matches!(st,
        Stmt::Expr(Expr::AssignIndexPtr(_, _, _)))).count();
    let compuesta = p.functions[0].body.iter().filter(|st| matches!(st,
        Stmt::Expr(Expr::AssignOp(lv, _, _)) if matches!(**lv, Expr::IndexPtr(_, _)))).count();
    assert_eq!(llana, 1, "`s->arr[0] = 5` sigue siendo AssignIndexPtr");
    assert_eq!(compuesta, 1, "`s->arr[0] += 3` es AssignOp sobre un IndexPtr");
    compile_source_to_bef(src).unwrap();
}

#[test]
fn field_assign_carries_exact_type() {
    // pt.x = 10 con x:int -- el AssignField lleva TypeSpec::Int para que
    // ** EL ANCHO DEL STORE, comprobado EJECUTANDO (2026-09-02).
    //
    // Antes se leia `ft == TypeSpec::Int` del nodo `AssignField`. El nodo ya no
    // lo carga, y de todas formas el numero en el arbol nunca probo que se
    // usara: lo que hay que ver es que `pt.x = 10` escriba CUATRO bytes y no
    // ocho. Se pone `y` primero; si el store fuera de 8, se la lleva por
    // delante.
    let salida = run_c(
        "struct Point { int x; long y; };
         int main() {
             struct Point pt;
             pt.y = 777; pt.x = 10;
             printf(\"%d %d\", pt.x, (int)pt.y);
             return 0;
         }",
    );
    assert_eq!(salida, "10 777", "un store de 8 bytes en `x` se llevaria `y`");
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
fn array_decl_records_size() {
    // int arr[4] debe ser Array(Int, 4) -- antes el tamano se TIRABA.
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
        if let Stmt::Expr(Expr::AssignSubscript(name, _, val)) = stmt {
            assert_eq!(name, "arr");
            assert_eq!(**val, Expr::Int(7));
            found = true;
        }
    }
    assert!(found, "arr[2] = 7 debe producir AssignSubscript, no descartarse");
    // y debe compilar a BEF
    let bef = compile_source_to_bef(src).unwrap();
    assert!(bef.len() > 48);
}

/// ** EL PASO DE UN `int` SIGUE SIENDO 4, y ahora se comprueba EJECUTANDO.
///
/// Hasta el 2026-09-02 esto era `assert_eq!(*scale, 4)` sobre el nodo del AST,
/// porque el paso viajaba dentro. Ya no viaja: lo contesta el codegen, que es
/// quien tiene la tabla de tamanos.
///
/// [!] La garantia **no se relaja, cambia de sitio**. Un test que solo miraba
/// el numero en el arbol no comprobaba que se USARA; este escribe en dos
/// casillas contiguas y las lee. Con un paso equivocado se pisan, y con un
/// paso de 0 las dos leen la primera.
#[test]
fn el_paso_de_un_int_son_cuatro_bytes_y_se_ve_al_ejecutar() {
    let salida = run_c(
        "int main() { int arr[4]; arr[2] = 7; arr[3] = 9; printf(\"%d %d\", arr[2], arr[3]); return 0; }",
    );
    assert_eq!(salida, "7 9", "dos casillas contiguas no se pueden pisar");
}

/// Las tres asignaciones a `arr[1]` sobreviven -- y **dos de ellas cambiaron de
/// FORMA el 2026-08-13**, que es lo que este test tuvo que aprender.
///
/// `arr[1] = 1` sigue siendo `AssignSubscript`. `arr[1] += 5` y `arr[1] <<= 2`
/// son ahora `AssignOp`, porque desazucararlas a `arr[1] = arr[1] + 5` clonaba
/// el lvalue y con un indice con efectos eso es incorrecto (C11 6.5.16.2p3).
///
/// [!] La INTENCION del test no cambia --que ninguna se descarte en silencio--
/// y por eso se cuentan las dos formas en vez de relajar el numero a 1: un test
/// que cuenta menos porque el codigo cambio de forma deja de comprobar lo que
/// dice su nombre.
#[test]
fn subscript_compound_assign() {
    let src = "int main() { int arr[4]; arr[1] = 1; arr[1] += 5; arr[1] <<= 2; return arr[1]; }";
    let p = parse(src).unwrap();
    let llanas = p.functions[0].body.iter().filter(|s| {
        matches!(s, Stmt::Expr(Expr::AssignSubscript(_, _, _)))
    }).count();
    let compuestas = p.functions[0].body.iter().filter(|s| {
        matches!(s, Stmt::Expr(Expr::AssignOp(lv, _, _)) if matches!(**lv, Expr::Subscript(_, _)))
    }).count();
    assert_eq!(llanas, 1, "`arr[1] = 1` sigue siendo AssignSubscript");
    assert_eq!(compuestas, 2, "`+=` y `<<=` son AssignOp sobre un Subscript");
    compile_source_to_bef(src).unwrap();
}

/// ** Y la que de verdad importa: el lvalue se evalua UNA vez.
///
/// El test de arriba mira la forma; este mira el COMPORTAMIENTO, que es lo que
/// se rompio durante meses sin que nadie lo viera. `probe_assignment` lo lleva
/// como fila del censo; aqui queda al lado de sus hermanos de forma para que
/// quien toque el desazucarado tropiece con los dos.
#[test]
fn el_lvalue_de_un_op_igual_se_evalua_una_vez() {
    let src = "int main() { int g[4]; int i;                  g[0]=0; g[1]=0; g[2]=0; g[3]=0; i = 1;                  g[i++] += 7;                  printf(\"%d %d %d\", i, g[1], g[2]); return 0; }";
    // `i` acaba en 2 (no en 3) y el 7 cae en `g[1]` (no en `g[2]`).
    assert_eq!(run_c(src).trim(), "2 7 0");
}

#[test]
fn subscript_on_compound_base_via_field() {
    // s.arr[0] con arr: int* -- evolucion del test de Fase 0: antes se
    // rechazaba (honesto pero limitado), en Fase 2 ya COMPILA como IndexPtr.
    let src = r#"
struct S { int* arr; };
int main() { struct S s; int x; x = s.arr[0]; return x; }
"#;
    let p = parse(src).unwrap();
    let ok = p.functions[0].body.iter().any(|st| matches!(st,
        Stmt::Expr(Expr::Assign(n, v)) if n == "x" && matches!(v.as_ref(), Expr::IndexPtr(_, _))));
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
fn parses_array_decl() {
    let src = "int main() { int arr[4]; arr[0] = 1; return arr[0]; }";
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

/// ** EL ELEMENTO DE UN INDICE COMPUESTO MIDE LO QUE DICE SU TIPO.
///
/// Hasta el 2026-09-02 esto era `matches!(.., Expr::IndexPtr(_, _, TypeSpec::Int))`
/// sobre el arbol, porque el tipo del elemento viajaba dentro del nodo. Ya no
/// viaja: `p[i]` es `*(p + i)`, y a que apunta `p` lo contesta `crate::tipos`.
///
/// [!] La garantia no se relaja, **cambia de sitio y ademas mejora**: mirar el
/// numero en el nodo no comprobaba que se USARA. Esto escribe en tres casillas
/// contiguas de un array dentro de un struct y las lee. Con un elemento de 8
/// bytes donde toca 4, se pisan.
#[test]
fn el_elemento_de_un_indice_compuesto_mide_lo_que_dice_su_tipo() {
    let salida = run_c(
        "struct S { int arr[4]; };
         int main() {
             struct S s; struct S *p;
             p = &s;
             p->arr[0] = 10; p->arr[1] = 20; p->arr[2] = 30;
             printf(\"%d %d %d\", p->arr[0], p->arr[1], p->arr[2]);
             return 0;
         }",
    );
    assert_eq!(salida, "10 20 30", "tres int contiguos no se pueden pisar");
}

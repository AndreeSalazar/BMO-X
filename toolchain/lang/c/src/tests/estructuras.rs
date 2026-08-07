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


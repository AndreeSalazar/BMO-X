//! **El OBJETO (`.bo`) de BMO C** -- E2 de `docs/plan/PLAN_EL_ENLAZADOR.md`.
//!
//! Lo que estas filas comprueban es lo unico que distingue un objeto de una
//! imagen: **quien cierra cada referencia**. Se lee el `.bo` con el contrato
//! (`bmo_abi::bef::objeto`), que es el mismo lector que usara `bmo-enlazar`.

use super::*;
use bmo_abi::bef::objeto::{self, REL_CODE};
use bmo_abi::bef::relocations::RelocationKind;
use bmo_abi::bef::sections::SectionKind;

fn objeto_de(fuente: &str) -> objeto::Object<'static> {
    let bytes = crate::compile_source_to_object(fuente).expect("la unidad debe compilar");
    // El lector presta del buffer; el buffer vive lo que dure la prueba.
    let bytes: &'static [u8] = Box::leak(bytes.into_boxed_slice());
    objeto::read(bytes).expect("el objeto debe cumplir su contrato")
}

/// Una unidad que USA lo que no define: llama a `suma`, toma la direccion de
/// `otra` y lee un `extern`. Nada de eso es un error aqui -- es lo que un
/// objeto existe para decir.
const USA: &str = r#"
int suma(int a, int b);
int otra(void);
extern int contador_global;
static int privado = 7;
int publico = 3;
const char *saludo = "hola";
int main() {
    int (*p)(void) = otra;
    privado = privado + contador_global + p();
    const char *local = "mundo";
    printf("%d %s %s", privado, saludo, local);
    return suma(publico, privado);
}
"#;

#[test]
fn lo_que_la_unidad_no_define_sale_como_simbolo_indefinido() {
    let o = objeto_de(USA);
    let indefinidos: Vec<&str> = o
        .symbols
        .iter()
        .filter(|s| s.section.is_none())
        .map(|s| s.name)
        .collect();
    assert!(indefinidos.contains(&"suma"), "{indefinidos:?}");
    assert!(indefinidos.contains(&"otra"), "{indefinidos:?}");
    assert!(indefinidos.contains(&"contador_global"), "{indefinidos:?}");
    // Y un indefinido es SIEMPRE global: un local que nadie define seria un
    // fallo de esta unidad, no una pregunta para el enlazador.
    assert!(o.symbols.iter().filter(|s| s.section.is_none()).all(|s| s.global));
}

/// `static` = de esta unidad. Sin esto, dos ficheros con el mismo `static int
/// n` chocarian al enlazar -- y el nombre es justo el que el autor eligio
/// PARA que no saliera.
#[test]
fn static_es_privado_y_lo_demas_es_publico() {
    let o = objeto_de(USA);
    let busca = |n: &str| o.symbols.iter().find(|s| s.name == n).unwrap_or_else(|| panic!("falta {n}"));
    assert!(!busca("privado").global, "un static no sale de su unidad");
    assert!(busca("publico").global);
    assert!(busca("main").global);
    assert!(busca("main").function);
    // Y lo que el compilador se invento (el stub de syscalls, los buferes
    // `__bmo_*`) tampoco sale: cada unidad lleva su copia.
    for s in &o.symbols {
        if s.name.starts_with("__bmo") || s.name.contains('.') {
            assert!(!s.global, "{} no puede ser publico", s.name);
        }
    }
}

/// *** LA FILA QUE DA SENTIDO A TODAS: las distancias a los datos ya NO las
/// calcula el compilador. En una imagen, un `lea [rip+cadena]` se resuelve
/// aqui contando paginas; en un objeto, otro codigo se va a meter en medio y
/// esa cuenta seria basura sin fallar. Sale como relocacion contra `.rodata`.
#[test]
fn las_referencias_a_datos_quedan_como_relocaciones() {
    let o = objeto_de(USA);
    let indice = |k: SectionKind| {
        o.symbols
            .iter()
            .position(|s| s.section == Some(k) && s.name.starts_with('.'))
            .unwrap_or_else(|| panic!("falta el simbolo de seccion de {k:?}")) as u32
    };
    let rodata = indice(SectionKind::RoData);
    let rel32: Vec<_> = o
        .relocs
        .iter()
        .filter(|r| r.kind == RelocationKind::Rel32 as u8)
        .collect();
    assert!(!rel32.is_empty(), "un programa con cadenas y globales deja relocs");
    assert!(
        rel32.iter().any(|r| r.symbol_idx == rodata),
        "la cadena del printf tiene que apuntar a .rodata"
    );
    // Y un global leido desde el codigo, a .data o a .bss.
    let datos = indice(SectionKind::Data);
    assert!(rel32.iter().any(|r| r.symbol_idx == datos), "un global vive en .data");
    // La cadena que INICIALIZA un global no es un `lea`: es un puntero dentro
    // de .data, y eso ya era una SeccionAbs64 antes de que hubiera objetos.
    assert!(o
        .relocs
        .iter()
        .any(|r| r.kind == RelocationKind::SeccionAbs64 as u8));
    // Todas parchean dentro del codigo, que es donde vive un `lea`.
    assert!(rel32.iter().all(|r| r.target_section == REL_CODE));
    // Y el `call suma` apunta al simbolo indefinido, no a un hueco en cero.
    let suma = o.symbols.iter().position(|s| s.name == "suma").unwrap() as u32;
    assert!(rel32.iter().any(|r| r.symbol_idx == suma), "el call a suma");
}

/// Un objeto no es un programa: no necesita `main`. Una imagen si, y eso no
/// cambia.
#[test]
fn un_objeto_no_necesita_main_y_una_imagen_si() {
    let solo_una_funcion = "int suma(int a, int b) { return a + b; }";
    assert!(crate::compile_source_to_object(solo_una_funcion).is_ok());
    let e = crate::compile_source_to_bef(solo_una_funcion).unwrap_err();
    assert!(e.message.contains("main"), "{}", e.message);
}

/// El mismo fuente compilado dos veces da los mismos bytes. Sin esto, dos
/// compilaciones del mismo objeto darian `.bex` distintos y la reproducibilidad
/// (C7 de PLAN_SEGURIDAD) se rompe en el primer escalon.
#[test]
fn compilar_dos_veces_da_los_mismos_bytes() {
    let a = crate::compile_source_to_object(USA).unwrap();
    let b = crate::compile_source_to_object(USA).unwrap();
    assert_eq!(a, b);
}

/// *** Y EL CERO CALLADO QUE SALIO AL ESCRIBIR ESTO (2026-09-17): `&x` de un
/// nombre que no existe emitia `xor eax,eax` -- la direccion CERO entregada
/// como si fuera un dato. Ahora es un error con su nombre, en los dos modos.
#[test]
fn la_direccion_de_un_nombre_que_no_existe_no_es_cero() {
    let fuente = "int main() { int *p = &no_existe_en_ningun_sitio; return *p; }";
    let e = crate::compile_source_to_bef(fuente).unwrap_err();
    assert!(e.message.contains("no_existe_en_ningun_sitio"), "{}", e.message);
}

//! El CARGADOR: lo que sale de aqui tiene que poder cargarse
//!
//! Parte del banco de pruebas de BMO C. Los ayudantes (`run_c`,
//! `run_c_sembrado`, `ejecutar_bef`) viven en `tests/mod.rs`.

use super::*;

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


// ── El relleno a página, FUERA ────────────────────────────────────────
//
// Hasta el 2026-08-07 el codegen rellenaba cada tramo hasta la página con
// `0xCC` y ese relleno viajaba dentro del `.bex`. No era capricho: los
// `lea [rip+disp]` se contaban asumiendo que los datos van pegados detrás del
// código, y el cargador (`ring0/task/proc.rs`) pone cada sección en la página
// siguiente. Rellenar hacía coincidir las dos cuentas.
//
// Ahora el compilador MODELA la regla del cargador en vez de forzarla, y el
// arnés de pruebas coloca las secciones por página como el cargador real — sin
// eso, estos tests no probarían nada.

/// El tamaño declarado de la sección `kind`.
fn tamano_seccion(bef: &[u8], kind: bmo_abi::bef::sections::SectionKind) -> Option<usize> {
    use bmo_abi::bef::sections::SectionEntry;
    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let sec_off = hdr.section_table_offset as usize;
    for i in 0..hdr.section_count as usize {
        let e = sec_off + i * SectionEntry::SIZE;
        if bef[e] == kind as u8 {
            return Some(u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize);
        }
    }
    None
}

/// ★ Un programa pequeño ocupa lo que ocupa.
///
/// Antes la sección de código de CUALQUIER programa era múltiplo de 4096, así
/// que un `hola` medía una página entera y no se podía distinguir de un
/// programa cuarenta veces mayor. Ese redondeo es lo que hacía invisible
/// cualquier ahorro de código por debajo de una página.
#[test]
fn la_seccion_de_codigo_ya_no_se_redondea_a_pagina() {
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int main() { printf(\"hola\"); return 0; }").unwrap();
    let code = tamano_seccion(&bef, SectionKind::Code).expect("tiene que haber seccion code");
    assert!(
        code % 4096 != 0,
        "un programa de este tamano no puede medir un multiplo exacto de pagina: {code}"
    );
    assert!(code < 4096, "y tiene que caber de sobra en una pagina: {code}");
}

/// ★ Y LA PRUEBA QUE IMPORTA: la cadena se sigue alcanzando.
///
/// Es el `%s` el que fallaría si la aritmética nueva estuviera mal. El código
/// de este programa NO llena la página, así que rodata empieza en 4096 mientras
/// el código acaba mucho antes: si el compilador contara "pegado detrás del
/// código" —como hacía cuando rellenaba— el puntero caería en el hueco y se
/// imprimiría basura o nada.
///
/// Este test no habría podido fallar antes del 2026-08-07: el arnés concatenaba
/// las secciones, así que la cuenta equivocada también habría acertado.
#[test]
fn una_cadena_se_alcanza_aunque_el_codigo_no_llene_la_pagina() {
    let fuente = "int main() { char *s; s = \"cadena en rodata\"; \
                  printf(\"[%s]\", s); return 0; }";
    assert_eq!(run_c(fuente), "[cadena en rodata]");
}

/// Lo mismo para los GLOBALES, que van en la tercera sección — o sea que su
/// dirección depende de DOS redondeos, no de uno: la página tras el código y la
/// página tras rodata. Un error en el segundo sumando sólo se ve aquí.
///
/// Las dos secciones se ejercitan en la misma línea: el `%d` lee el global (de
/// `data`, tras dos fronteras) y el `%s` la cadena (de `rodata`, tras una).
#[test]
fn un_global_se_alcanza_tras_dos_fronteras_de_pagina() {
    let fuente = "int contador = 41; \
                  int main() { contador = contador + 1; \
                  printf(\"%d %s\", contador, \"eltexto\"); return 0; }";
    assert_eq!(run_c(fuente), "42 eltexto");
}

// ── El global que valía CERO en silencio ──────────────────────────────
//
// Estos tres nacieron de escribir el test de arriba con `char *texto =
// "eltexto"` y ver salir `42 UH\x89åH\x8d\x05õ\x1f` — bytes de codigo maquina.
// El global valía 0, y el byte 0 de la imagen es el `push rbp` de la primera
// funcion.
//
// NO era una regresion: fallaba igual con el codegen anterior. Un
// inicializador que este codegen no sabia convertir se rellenaba de ceros y no
// se decia, y nada lo miraba porque `globales.rs` sólo comprobaba que el
// programa COMPILARA.

/// ★★ EL GLOBAL QUE VALIA CERO, AHORA APUNTA DONDE DEBE.
///
/// Este test nacio al reves: comprobaba que `char *texto = "eltexto"` **se
/// RECHAZARA**, porque el codegen no sabia poner una direccion y rellenar de
/// ceros en silencio era peor que negarse. Con las relocations `SeccionAbs64`
/// ya se puede poner, asi que el test cambio de sentido — y se deja dicho,
/// porque un test que un dia exigio lo contrario es la mejor prueba de que algo
/// avanzo de verdad.
///
/// Lo que se arreglo por debajo: el compilador deja el hueco a cero y anota la
/// reloc; la direccion la escribe el cargador, que es el unico que la sabe.
#[test]
fn un_global_inicializado_con_cadena_apunta_a_la_cadena() {
    let fuente = "char *texto = \"eltexto\";                   int main() { printf(\"[%s]\", texto); return 0; }";
    assert_eq!(run_c(fuente), "[eltexto]");
}

/// Y lo que SIGUE rechazado, para que el mensaje no se pierda: un literal de
/// coma flotante en un global. Ahi no falta una reloc — falta convertir el
/// valor, que es otro trabajo.
#[test]
fn un_global_con_float_sigue_rechazado_diciendolo() {
    let err = compile_source_to_bef("double d = 1.5; int main() { return 0; }")
        .expect_err("un cero inventado es peor que un error");
    let msg = format!("{err:?}");
    assert!(msg.contains("'d'"), "tiene que decir QUE global: {msg}");
}

/// Y de paso, lo que sí se puede poner y antes valía cero: un entero negativo.
///
/// `int x = -5;` es `Neg(Int(5))` en el AST, no `Int(-5)`, así que caía en el
/// mismo agujero. Ahora se convierte, que es gratis y claramente correcto.
#[test]
fn un_global_negativo_ya_no_vale_cero() {
    let fuente = "int frio = -40; int main() { printf(\"%d\", frio); return 0; }";
    assert_eq!(run_c(fuente), "-40");
}

/// Y el viaje de ida y vuelta, que es lo que cubre la pareja completa: escribir
/// un negativo en un global EN EJECUCIÓN y volver a leerlo. El `store` guarda 4
/// bytes (correcto para con y sin signo); el que fallaba era el `load`.
#[test]
fn un_global_int_conserva_el_signo_al_releerlo() {
    let fuente = "int v = 0; \
                  int main() { v = 0 - 7; printf(\"%d,\", v); \
                               v = v - 1; printf(\"%d\", v); return 0; }";
    assert_eq!(run_c(fuente), "-7,-8");
}

/// El contraste que prueba que no se ha roto lo otro: `unsigned int` NO se
/// extiende con signo, y ahí `mov eax,[rax]` es la instrucción correcta.
#[test]
fn un_global_unsigned_no_se_extiende_con_signo() {
    let fuente = "unsigned int u = 0; \
                  int main() { u = 0 - 1; printf(\"%u\", u); return 0; }";
    assert_eq!(run_c(fuente), "4294967295");
}

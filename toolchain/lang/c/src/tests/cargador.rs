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


// -- El relleno a pagina, FUERA ----------------------------------------
//
// Hasta el 2026-08-07 el codegen rellenaba cada tramo hasta la pagina con
// `0xCC` y ese relleno viajaba dentro del `.bex`. No era capricho: los
// `lea [rip+disp]` se contaban asumiendo que los datos van pegados detras del
// codigo, y el cargador (`ring0/task/proc.rs`) pone cada seccion en la pagina
// siguiente. Rellenar hacia coincidir las dos cuentas.
//
// Ahora el compilador MODELA la regla del cargador en vez de forzarla, y el
// arnes de pruebas coloca las secciones por pagina como el cargador real -- sin
// eso, estos tests no probarian nada.

/// El tamano declarado de la seccion `kind`.
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

/// * Un programa pequeno ocupa lo que ocupa.
///
/// Antes la seccion de codigo de CUALQUIER programa era multiplo de 4096, asi
/// que un `hola` media una pagina entera y no se podia distinguir de un
/// programa cuarenta veces mayor. Ese redondeo es lo que hacia invisible
/// cualquier ahorro de codigo por debajo de una pagina.
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

/// * Y LA PRUEBA QUE IMPORTA: la cadena se sigue alcanzando.
///
/// Es el `%s` el que fallaria si la aritmetica nueva estuviera mal. El codigo
/// de este programa NO llena la pagina, asi que rodata empieza en 4096 mientras
/// el codigo acaba mucho antes: si el compilador contara "pegado detras del
/// codigo" --como hacia cuando rellenaba-- el puntero caeria en el hueco y se
/// imprimiria basura o nada.
///
/// Este test no habria podido fallar antes del 2026-08-07: el arnes concatenaba
/// las secciones, asi que la cuenta equivocada tambien habria acertado.
#[test]
fn una_cadena_se_alcanza_aunque_el_codigo_no_llene_la_pagina() {
    let fuente = "int main() { char *s; s = \"cadena en rodata\"; \
                  printf(\"[%s]\", s); return 0; }";
    assert_eq!(run_c(fuente), "[cadena en rodata]");
}

// -- LA SECCION `Bss`: los ceros se declaran, no se guardan --------------
//
// Estas dos miran la FORMA del `.bex`, que es lo que un test de comportamiento
// no puede ver: un compilador que metiera los ceros en `.data` daria
// exactamente los mismos numeros en pantalla y un fichero 37,9% mayor.
//
// El numero de DOOM, medido el 2026-08-09 con el mismo compilador y solo este
// cambio en medio:  1.299.768 -> 807.072 B, con `data` de 645.008 a 152.224 y
// una `Bss` de 492.784 que no ocupa ni un byte del fichero. La memoria del
// proceso NO cambia (1.268.584 las dos veces), que es justo lo correcto: lo que
// se quita es el transporte, no el sitio.

/// El tamano DECLARADO en memoria de la seccion `kind` (`mem_size`), que para
/// una `Bss` es lo unico que dice algo -- su `file_size` es cero por definicion.
fn memoria_seccion(bef: &[u8], kind: bmo_abi::bef::sections::SectionKind) -> Option<usize> {
    use bmo_abi::bef::sections::SectionEntry;
    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let sec_off = hdr.section_table_offset as usize;
    for i in 0..hdr.section_count as usize {
        let e = sec_off + i * SectionEntry::SIZE;
        if bef[e] == kind as u8 {
            return Some(u64::from_le_bytes(bef[e + 24..e + 32].try_into().unwrap()) as usize);
        }
    }
    None
}

/// ** Una tabla de 32 KiB a cero **no engorda el fichero**.
///
/// Es la fila que mide el escalon 0 de `docs/LA_RAM.md`. Sin `Bss`, este `.bex`
/// pasaria de 32.768 bytes; con ella cabe de sobra y la tabla sigue existiendo
/// entera en memoria.
#[test]
fn una_tabla_grande_a_cero_no_engorda_el_fichero() {
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int enorme[8192]; int main() { return enorme[0]; }").unwrap();
    let bss = memoria_seccion(&bef, SectionKind::Bss).expect("tiene que haber seccion bss");
    assert!(
        bss >= 32768,
        "la tabla son 8192 enteros = 32 KiB, y la bss mide {bss}"
    );
    assert!(
        bef.len() < 8192,
        "los 32 KiB de ceros no pueden estar en el fichero, y mide {}",
        bef.len()
    );
}

/// Y al reves: un programa cuyos globales TIENEN valor no lleva `Bss` ninguna.
///
/// Una seccion vacia declarada igualmente seria una pagina reservada para nada
/// en cada proceso del sistema.
#[test]
fn sin_globales_a_cero_no_se_declara_bss() {
    use bmo_abi::bef::sections::SectionKind;
    let bef = compile_source_to_bef("int g = 42; int main() { return g; }").unwrap();
    assert!(
        memoria_seccion(&bef, SectionKind::Bss).is_none(),
        "no hay ningun global a cero: no debe declararse seccion bss"
    );
}

/// Lo mismo para los GLOBALES, que van en la tercera seccion -- o sea que su
/// direccion depende de DOS redondeos, no de uno: la pagina tras el codigo y la
/// pagina tras rodata. Un error en el segundo sumando solo se ve aqui.
///
/// Las dos secciones se ejercitan en la misma linea: el `%d` lee el global (de
/// `data`, tras dos fronteras) y el `%s` la cadena (de `rodata`, tras una).
#[test]
fn un_global_se_alcanza_tras_dos_fronteras_de_pagina() {
    let fuente = "int contador = 41; \
                  int main() { contador = contador + 1; \
                  printf(\"%d %s\", contador, \"eltexto\"); return 0; }";
    assert_eq!(run_c(fuente), "42 eltexto");
}

// -- El global que valia CERO en silencio ------------------------------
//
// Estos tres nacieron de escribir el test de arriba con `char *texto =
// "eltexto"` y ver salir `42 UH\x89aH\x8d\x05o\x1f` -- bytes de codigo maquina.
// El global valia 0, y el byte 0 de la imagen es el `push rbp` de la primera
// funcion.
//
// NO era una regresion: fallaba igual con el codegen anterior. Un
// inicializador que este codegen no sabia convertir se rellenaba de ceros y no
// se decia, y nada lo miraba porque `globales.rs` solo comprobaba que el
// programa COMPILARA.

/// ** EL GLOBAL QUE VALIA CERO, AHORA APUNTA DONDE DEBE.
///
/// Este test nacio al reves: comprobaba que `char *texto = "eltexto"` **se
/// RECHAZARA**, porque el codegen no sabia poner una direccion y rellenar de
/// ceros en silencio era peor que negarse. Con las relocations `SeccionAbs64`
/// ya se puede poner, asi que el test cambio de sentido -- y se deja dicho,
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

/// ** Y ESTE TAMBIEN CAMBIO DE SENTIDO, el mismo dia y por el mismo motivo.
///
/// Exigia que un global de coma flotante **se RECHAZARA**, y el motivo escrito
/// era *"falta convertir el valor"*. Convertir es justo lo que hace `to_bits`:
/// un `float` son los cuatro bytes de su representacion IEEE y un `double` los
/// ocho. Lo que faltaba no era saber hacerlo, era hacerlo.
///
/// Salio pidiendolo DOOM: `float mouse_acceleration = 2.0;` en `i_video.c`.
///
/// Se comprueba **por el valor y no por el patron de bits**: se multiplica y se
/// baja a entero, asi que si los bytes guardados fueran los de otro numero, o
/// la anchura fuera la del `double` en un `float`, el resultado no saldria.
#[test]
fn un_global_de_coma_flotante_guarda_su_valor() {
    let fuente = "float f = 2.5; double d = 1.5; \
                  int main() { printf(\"%d %d\", (int)(f * 2), (int)(d * 4)); return 0; }";
    assert_eq!(run_c(fuente), "5 6");
}

/// Y de paso, lo que si se puede poner y antes valia cero: un entero negativo.
///
/// `int x = -5;` es `Neg(Int(5))` en el AST, no `Int(-5)`, asi que caia en el
/// mismo agujero. Ahora se convierte, que es gratis y claramente correcto.
#[test]
fn un_global_negativo_ya_no_vale_cero() {
    let fuente = "int frio = -40; int main() { printf(\"%d\", frio); return 0; }";
    assert_eq!(run_c(fuente), "-40");
}

/// Y el viaje de ida y vuelta, que es lo que cubre la pareja completa: escribir
/// un negativo en un global EN EJECUCION y volver a leerlo. El `store` guarda 4
/// bytes (correcto para con y sin signo); el que fallaba era el `load`.
#[test]
fn un_global_int_conserva_el_signo_al_releerlo() {
    let fuente = "int v = 0; \
                  int main() { v = 0 - 7; printf(\"%d,\", v); \
                               v = v - 1; printf(\"%d\", v); return 0; }";
    assert_eq!(run_c(fuente), "-7,-8");
}

/// El contraste que prueba que no se ha roto lo otro: `unsigned int` NO se
/// extiende con signo, y ahi `mov eax,[rax]` es la instruccion correcta.
#[test]
fn un_global_unsigned_no_se_extiende_con_signo() {
    let fuente = "unsigned int u = 0; \
                  int main() { u = 0 - 1; printf(\"%u\", u); return 0; }";
    assert_eq!(run_c(fuente), "4294967295");
}

// == EL PAQUETE: un .bex con los DATOS de la app dentro ================

/// **** LA FILA QUE DECIDE SI EL PAQUETE SIRVE: **empaquetado, el programa
/// sigue corriendo IGUAL**.
///
/// Un paquete es un `.bex` con una seccion `Resources` (`0x0B`) dentro -- el
/// codigo y los datos en un solo fichero. Que eso no rompa nada no es evidente:
/// anadir una seccion hace crecer la tabla, y **todos los offsets en fichero se
/// mueven**. Si el cargador leyera un offset de otro sitio, o si las secciones
/// cambiaran de indice, el programa cargaria y haria otra cosa.
///
/// Por eso se comprueba EJECUTANDO y no mirando bytes: la salida tiene que ser
/// la misma con y sin recursos dentro.
#[test]
fn un_programa_empaquetado_corre_igual() {
    let src = r#"
int main() {
    int i;
    for (i = 0; i < 3; i = i + 1) { printf("%d,", i * 7); }
    printf("\n");
    return 0;
}
"#;
    let desnudo = compile_source_to_bef(src).unwrap();
    let esperado = ejecutar_bef(&desnudo);
    assert_eq!(esperado, "0,7,14,\n");

    let wad = vec![0x5Au8; 4096];
    let paquete = bmo_abi::bef::paquete::empaquetar(
        &desnudo,
        &[("datos.wad", &wad), ("leeme.txt", b"hola")],
    )
    .expect("debe empaquetar");

    assert!(paquete.len() > desnudo.len() + 4096, "los datos tienen que estar dentro");
    assert_eq!(
        ejecutar_bef(&paquete),
        esperado,
        "el programa empaquetado tiene que hacer EXACTAMENTE lo mismo"
    );
}

/// Y los datos se encuentran por nombre en el fichero resultante. Sin esto lo
/// anterior solo probaria que los recursos son inertes, que es la mitad barata.
#[test]
fn los_recursos_se_recuperan_del_paquete() {
    let desnudo = compile_source_to_bef("int main() { printf(\"x\"); return 0; }").unwrap();
    let wad: Vec<u8> = (0..1000u32).map(|i| (i % 251) as u8).collect();
    let paquete =
        bmo_abi::bef::paquete::empaquetar(&desnudo, &[("doom1.wad", &wad)]).unwrap();

    let d = bmo_abi::bef::paquete::directorio(&paquete).expect("trae directorio");
    let i = d.buscar("doom1.wad").expect("esta");
    assert_eq!(d.datos(i).unwrap(), &wad[..]);
}

/// ** El cargador solo mapea Code/RoData/Data/Bss, asi que **un paquete de
/// cuatro megas no le cuesta al proceso ni una pagina mas** que la imagen
/// desnuda. Se comprueba sobre las secciones que el kernel llama cargables: su
/// suma no puede cambiar al empaquetar.
#[test]
fn los_recursos_no_ocupan_memoria_del_proceso() {
    use bmo_abi::bef::sections::SectionKind;

    fn memoria_mapeada(bex: &[u8]) -> u64 {
        let tabla = u64::from_le_bytes(bex[32..40].try_into().unwrap()) as usize;
        let count = u32::from_le_bytes(bex[40..44].try_into().unwrap()) as usize;
        let mut total = 0u64;
        for i in 0..count {
            let e = &bex[tabla + i * 48..tabla + (i + 1) * 48];
            let cargable = matches!(
                e[0],
                x if x == SectionKind::Code as u8
                    || x == SectionKind::RoData as u8
                    || x == SectionKind::Data as u8
                    || x == SectionKind::Bss as u8
            );
            if cargable {
                total += u64::from_le_bytes(e[24..32].try_into().unwrap());
            }
        }
        total
    }

    let desnudo = compile_source_to_bef("int main() { printf(\"x\"); return 0; }").unwrap();
    let gordo = vec![0u8; 4 * 1024 * 1024];
    let paquete = bmo_abi::bef::paquete::empaquetar(&desnudo, &[("gordo", &gordo)]).unwrap();

    assert!(paquete.len() > 4 * 1024 * 1024, "el fichero SI crece");
    assert_eq!(
        memoria_mapeada(&desnudo),
        memoria_mapeada(&paquete),
        "y la memoria del proceso NO"
    );
}

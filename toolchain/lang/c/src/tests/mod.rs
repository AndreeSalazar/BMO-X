//! **El banco de pruebas de BMO C.**
//!
//! Estaba entero dentro de `lib.rs`: **2520 de sus 2628 lineas eran este
//! modulo**, y encontrar un test era buscar por nombre en un fichero donde
//! ya no cabia nada mas. Aqui viven los AYUDANTES --los que compilan y
//! ejecutan-- y cada tema tiene su fichero.
//!
//! No se movio a `tests/` de integracion a proposito: alli solo se ve la API
//! publica, y la mitad de estas pruebas miran el AST o el preprocesador por
//! dentro. Un banco que solo puede probar lo publico no puede probar lo que
//! mas se rompe.
//!
//! ## El reparto se TERMINO el 2026-08-06
//!
//! El trabajo estaba a medias: habia nueve ficheros por tema al lado y este
//! `mod.rs` seguia quedandose con **112 tests sueltos en 1784 lineas**. Un
//! fallo decia su nombre y ya; de que iba, a buscarlo. Ahora **el fichero es
//! la categoria** y `cargo test printf::` corre esa parte sola.
//!
//! Aqui dentro no queda ni un `#[test]`: solo los NUEVE AYUDANTES, que es lo
//! unico que este fichero deberia haber tenido nunca.
//!
//! | | |
//! |---|---|
//! | El lenguaje se reconoce | `parseo` 28 - `estructuras` 16 - `enumeraciones` 3 |
//! | El programa CORRE | `ejecucion` 6 - `flotante` 10 - `globales` 3 - `matriz` 5 |
//! | El sistema debajo | `syscalls` 6 - `intrinsecos` 8 - `puerta` - `cargador` 5 |
//! | Lo de siempre | `printf` 11 - `punteros_funcion` 8 - `preprocesador` |
//!
//! Y los que ya estaban: `agregados`, `almacenamiento`, `entrada`,
//! `inicializadores`, `memoria`, `semantic`, `silencios`.
//!
//! 238 verdes antes del corte, 238 despues. No se reescribio ni un test.

use super::*;

mod agregados;
mod almacenamiento;
mod cadenas;
mod cargador;
mod doom_probe;
mod ejecucion;
mod entrada;
mod enumeraciones;
mod estructuras;
mod flotante;
mod globales;
mod inicializadores;
/// Las de `<string.h>`, `<ctype.h>` y `<stdlib.h>` que faltaban para que el
/// unity build de DOOM llegue al final. Escritas en C, no en el codegen.
mod libc;
mod intrinsecos;
mod matriz;
mod memoria;
/// El MONTON de Ring 3 (`<bmo/monton.h>`): `malloc` de verdad sobre UN bloque
/// de `KIND_MEMORIA`. `memoria` prueba el contrato del kernel; esto, el reparto
/// que se escribe encima.
mod monton;
/// `<bmo/musica.h>`: notas, figuras y tempo sobre `KIND_AUDIO`. Aqui no suena
/// nada -- se comprueba la PARTITURA, que es lo unico que una libreria de
/// musica puede prometer sin un altavoz delante.
mod musica;
mod parseo;
mod preprocesador;
mod printf;
mod puerta;
mod punteros_funcion;
mod semantic;
mod silencios;
/// Las funciones SINTETIZADAS: emitidas una vez, alcanzadas con `call`. Aqui
/// se cuenta **cuantas veces sale el cuerpo**, que es lo que un test de
/// comportamiento no puede ver.
mod sintetizadas;
mod syscalls;
/// La tabla de configuracion de DOOM en ocho lineas: la forma exacta con la
/// que murio en el Ryzen el 2026-08-13.
mod tabla_de_config;
/// El operador de negacion que valia -256, y por que DOOM iba a escribir su
/// configuracion encima del WAD.
mod argv_de_doom;
mod varargs_de_doom;
/// LA SONDA DEL LENGUAJE: la matriz contenedor x operacion, y el censo de lo
/// que BMO C sabe hacer -- escrito para que no pueda quedarse viejo.
mod sonda_del_lenguaje;

// -- Banco de pruebas: EJECUTAR el programa, no mirarlo --------------
//
// Mismo criterio que en COBOL: un formateo que produce digitos erroneos
// se ve perfectamente sano en un volcado de bytes.

/// Compila y ejecuta un programa C, devolviendo lo que el kernel habria
/// pintado.
fn run_c(source: &str) -> String {
    let bef = compile_source_to_bef(source).expect("el programa debe compilar");
    ejecutar_bef(&bef)
}

/// Igual, pero pasando ANTES por el preprocesador -- que es lo que hace la
/// linea de ordenes y lo que el camino de biblioteca NO hace.
fn run_c_con_pp(source: &str) -> String {
    let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("con preprocesador debe compilar");
    ejecutar_bef(&bef)
}

/// Compila con preprocesador y ejecuta SEMBRANDO la maquina antes.
///
/// Hace falta desde que C puede emitir la puerta: un programa que lee el
/// raton necesita que haya un raton que leer. Sin esto, todo lo que use
/// `<bmo/entrada.h>` se probaria contra ceros, que es indistinguible de
/// un driver muerto.
fn run_c_sembrado(source: &str, sembrar: impl FnOnce(&mut bmo_lower::emu::Machine)) -> String {
    let bef = compile_with_preprocessor(source, std::path::Path::new("prueba.c"), CStandard::C11)
        .expect("con preprocesador debe compilar");
    ejecutar_bef_con(&bef, sembrar)
}

fn ejecutar_bef(bef: &[u8]) -> String {
    ejecutar_bef_con(bef, |_| {})
}

/// Igual que [`run_c`], pero devuelve **la maquina entera**.
///
/// Para lo que un programa no puede contarse a si mismo: cuanta memoria le
/// entrego el kernel, que llamadas cruzaron la puerta y en que orden. Un
/// programa que imprime "todo bien" es un testigo, no una prueba.
fn run_c_maquina(source: &str) -> bmo_lower::emu::Machine {
    let bef = compile_source_to_bef(source).expect("el programa debe compilar");
    maquina_de_bef(&bef)
}

fn ejecutar_bef_con(
    bef: &[u8],
    sembrar: impl FnOnce(&mut bmo_lower::emu::Machine),
) -> String {
    maquina_de_bef_con(bef, sembrar).console
}

fn maquina_de_bef(bef: &[u8]) -> bmo_lower::emu::Machine {
    maquina_de_bef_con(bef, |_| {})
}

fn maquina_de_bef_con(
    bef: &[u8],
    sembrar: impl FnOnce(&mut bmo_lower::emu::Machine),
) -> bmo_lower::emu::Machine {
    use bmo_abi::bef::sections::{SectionEntry, SectionKind};
    use bmo_lower::emu::{run, Machine};

    let hdr = unsafe { &*(bef.as_ptr() as *const bmo_abi::bef::header::BefHeader) };
    let entry = hdr.entry_offset as usize;
    let sec_off = hdr.section_table_offset as usize;

    // La imagen se rearma en el MISMO orden en que el codegen la dispuso:
    // codigo, luego rodata, luego data. El `lea rax,[rip+disp]` con el que se
    // alcanzan las cadenas se calculo contando desde el codigo; cargar solo la
    // seccion CODE dejaba esos punteros apuntando al vacio y un `%s` imprimia
    // cadena vacia.
    //
    // * Y CADA SECCION EMPIEZA EN SU PROPIA PAGINA, porque es lo que hace el
    // cargador de verdad.
    //
    // Antes se pegaban una detras de otra. Con el relleno a pagina que emite
    // `pad_to_page` eso daba el mismo resultado --el codigo ya es multiplo de
    // 4096-- asi que el banco de pruebas pasaba igual. Pero era una coincidencia,
    // no una equivalencia: `ring0/task/proc.rs` hace
    //
    //     va_cursor = va_start + pages * PAGE
    //
    // o sea que coloca cada seccion en la pagina siguiente **sea cual sea** el
    // tamano de la anterior. Un compilador que dejara de rellenar habria
    // seguido pasando aqui y habria fallado en el Ryzen, que es exactamente el
    // punto ciego que denuncia la cabecera de `patch_all_fixups`: *"esto NO lo
    // puede detectar el emulador de pruebas"*.
    //
    // Ahora si lo puede. El hueco se rellena con `0xCC` y no con ceros por el
    // mismo motivo que lo hace el compilador: si el flujo se sale del codigo,
    // la maquina para en vez de seguir por basura interpretable.
    const PAGE: usize = 4096;
    let mut code = Vec::new();
    // Donde acabo cada seccion en la imagen, indexado por el CODIGO DE SECCION
    // DE LAS RELOCS (`0` = code, `1` = data, `2` = rodata), que **no es** el de
    // `SectionKind` -- ver la nota en `bef::relocations`.
    let mut base = [usize::MAX; 3];
    for (kind, cod_reloc) in [
        (SectionKind::Code, 0usize),
        (SectionKind::RoData, 2usize),
        (SectionKind::Data, 1usize),
        // * `Bss` va la ULTIMA y no tiene codigo de reloc, porque no puede ser
        // ni origen ni destino de una: el codigo de seccion de una relocation
        // solo sabe decir code/data/rodata. Ver `Codegen::separar_bss`.
        //
        // Y no trae bytes: `file_size` es 0 y lo que cuenta es `mem_size`. Se
        // rellena de CEROS y no de `0xCC` como los huecos entre secciones,
        // porque aqui el cero no es relleno -- es el valor que el programa va a
        // leer, y es justamente lo que hay que comprobar que llega.
        (SectionKind::Bss, usize::MAX),
    ] {
        for i in 0..hdr.section_count as usize {
            let e = sec_off + i * SectionEntry::SIZE;
            if bef[e] == kind as u8 {
                let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap()) as usize;
                let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize;
                let mem = u64::from_le_bytes(bef[e + 24..e + 32].try_into().unwrap()) as usize;
                while !code.is_empty() && code.len() % PAGE != 0 {
                    code.push(0xCC);
                }
                if cod_reloc != usize::MAX {
                    base[cod_reloc] = code.len();
                }
                code.extend_from_slice(&bef[off..off + size]);
                for _ in size..mem {
                    code.push(0);
                }
            }
        }
    }
    assert!(!code.is_empty(), "el BEF no tiene seccion CODE");

    // * LAS RELOCATIONS, aplicadas como las aplicara el cargador.
    //
    // Sin esto un `char *p = "x"` global se quedaria en cero y el test lo veria
    // como el mapa del raycaster: leyendo desde el byte 0 de la imagen. Aqui la
    // "direccion virtual" de una seccion es su offset en esta imagen plana,
    // porque el emulador direcciona desde cero.
    //
    // Se hace en el arnes y no dentro del emulador a proposito: aplicar
    // relocations es trabajo del CARGADOR, y el emulador es un CPU. Meterlo
    // dentro seria darle al modelo de la maquina un trabajo que la maquina no
    // hace.
    for i in 0..hdr.section_count as usize {
        let e = sec_off + i * SectionEntry::SIZE;
        if bef[e] != SectionKind::Relocs as u8 {
            continue;
        }
        let off = u64::from_le_bytes(bef[e + 8..e + 16].try_into().unwrap()) as usize;
        let size = u64::from_le_bytes(bef[e + 16..e + 24].try_into().unwrap()) as usize;
        let n = size / bmo_abi::bef::relocations::Relocation::SIZE;
        for k in 0..n {
            let r = off + k * bmo_abi::bef::relocations::Relocation::SIZE;
            let donde_off = u64::from_le_bytes(bef[r..r + 8].try_into().unwrap()) as usize;
            let destino_sec = u32::from_le_bytes(bef[r + 8..r + 12].try_into().unwrap()) as usize;
            let kind = bef[r + 12];
            let donde_sec = bef[r + 13] as usize;
            let addend = i64::from_le_bytes(bef[r + 16..r + 24].try_into().unwrap());
            assert_eq!(
                kind,
                bmo_abi::bef::relocations::RelocationKind::SeccionAbs64 as u8,
                "el arnes solo sabe aplicar SeccionAbs64; salio kind={kind}"
            );
            assert!(
                donde_sec < 3 && destino_sec < 3,
                "codigo de seccion fuera de rango en una reloc"
            );
            assert!(
                base[donde_sec] != usize::MAX && base[destino_sec] != usize::MAX,
                "una reloc nombra una seccion que este .bex no lleva"
            );
            let donde = base[donde_sec] + donde_off;
            let valor = (base[destino_sec] as i64 + addend) as u64;
            assert!(donde + 8 <= code.len(), "reloc fuera de la imagen");
            code[donde..donde + 8].copy_from_slice(&valor.to_le_bytes());
        }
    }

    let mut machine = Machine::new(code);
    machine.rip = entry; // `main` no tiene por que estar al principio
    sembrar(&mut machine);
    let machine = run(machine, 500_000);
    assert!(machine.exited, "el programa debe terminar por INVOKE(EXIT)");
    machine
}

/// Busca una subsecuencia de bytes dentro del BEF ya escrito.
fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty() && haystack.windows(needle.len()).any(|w| w == needle)
}


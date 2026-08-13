//! **Los dos lectores del BEF leen los mismos bytes.**
//!
//! El formato tiene dos lectores y **ningun compilador entre ellos**:
//!
//! ```text
//!    bmo-abi/bef/*.rs      structs con `repr(C)`, para el toolchain
//!    kernel/task/bex.rs    bytes a mano, porque el kernel NO importa bmo-abi
//! ```
//!
//! Y no importarlo es correcto --`bmo-abi` trae `alloc` y el kernel no puede--
//! pero tenia un precio que nadie pagaba: **los offsets estaban escritos dos
//! veces**, una como campos de un struct y otra como literales dentro de
//! `leer_reloc`. El comentario del kernel lo decia con todas las letras:
//!
//! > *"si el struct cambiara de forma, estos offsets son el unico sitio a
//! > tocar"*
//!
//! **"El unico sitio a tocar" es la definicion de una duplicacion que se
//! olvida.** Mover un campo del struct compila igual, pasa todos los tests del
//! toolchain, y el cargador escribe la direccion equivocada dentro de un proceso
//! -- que es corrupcion silenciosa, no un fallo.
//!
//! # Lo que hace esta prueba
//!
//! Los offsets viven ahora en `bmo-bex-gate`, que **los dos lados ya importan**
//! y no tiene dependencias. Esta prueba los clava al struct con `offset_of!`.
//!
//! De dos verdades que hay que mantener a mano se pasa a **una verdad y una
//! prueba que la ata**. Si alguien mueve un campo del struct, esto falla aqui --
//! en el anfitrion, en un segundo-- en vez de en el Ryzen tres arranques
//! despues.
//!
//! # Por que no basta con `size_of`
//!
//! Ya habia un `const _: () = assert!(size_of::<Relocation>() == 24)`. Eso caza
//! que la struct crezca, y **no caza que dos campos se intercambien** -- que es
//! justo el error que produce una direccion plausible y equivocada.

use bmo_abi::bef::relocations::Relocation;
use bmo_abi::bef::sections::SectionEntry;
use bmo_bex_gate as gate;

/// ** EL TAMANO, que es lo unico que estaba cubierto hasta hoy.
#[test]
fn el_tamano_de_una_reloc_es_el_mismo_en_los_dos_lados() {
    assert_eq!(
        core::mem::size_of::<Relocation>(),
        gate::RELOC_SIZE,
        "el struct y el lector del kernel no miden lo mismo: el cargador leeria \
         una reloc por vuelta desalineandose una vez por entrada"
    );
}

/// ** Y AQUI LO QUE NO ESTABA: CADA CAMPO EN SU SITIO.
///
/// `size_of` no distingue un struct correcto de uno con dos campos
/// intercambiados. Y esa es exactamente la clase de error que no falla: la
/// reloc se lee, el numero es plausible, y el cargador escribe una direccion
/// equivocada en la memoria de un proceso.
#[test]
fn cada_campo_de_una_reloc_esta_donde_el_kernel_lo_busca() {
    assert_eq!(core::mem::offset_of!(Relocation, offset), gate::reloc::OFFSET, "offset");
    assert_eq!(core::mem::offset_of!(Relocation, symbol_idx), gate::reloc::SYMBOL_IDX, "symbol_idx");
    assert_eq!(core::mem::offset_of!(Relocation, kind), gate::reloc::KIND, "kind");
    assert_eq!(
        core::mem::offset_of!(Relocation, target_section),
        gate::reloc::TARGET_SECTION,
        "target_section"
    );
    assert_eq!(core::mem::offset_of!(Relocation, addend), gate::reloc::ADDEND, "addend");
}

/// La tabla de secciones es el otro sitio donde el kernel lee bytes a mano, y
/// el que decide **donde aterriza cada trozo del programa**. Un offset movido
/// aqui no da una direccion mala: da una seccion entera en el sitio que no es.
#[test]
fn cada_campo_de_una_seccion_esta_donde_el_kernel_lo_busca() {
    assert_eq!(core::mem::size_of::<SectionEntry>(), gate::SECTION_ENTRY_SIZE);
    assert_eq!(core::mem::offset_of!(SectionEntry, kind), gate::seccion::KIND, "kind");
    assert_eq!(core::mem::offset_of!(SectionEntry, flags), gate::seccion::FLAGS, "flags");
    assert_eq!(
        core::mem::offset_of!(SectionEntry, file_offset),
        gate::seccion::FILE_OFFSET,
        "file_offset"
    );
    assert_eq!(core::mem::offset_of!(SectionEntry, file_size), gate::seccion::FILE_SIZE, "file_size");
    assert_eq!(core::mem::offset_of!(SectionEntry, mem_size), gate::seccion::MEM_SIZE, "mem_size");
    assert_eq!(core::mem::offset_of!(SectionEntry, virt_addr), gate::seccion::VIRT_ADDR, "virt_addr");
    assert_eq!(
        core::mem::offset_of!(SectionEntry, alignment),
        gate::seccion::ALIGNMENT,
        "alignment"
    );
    assert_eq!(
        core::mem::offset_of!(SectionEntry, hash_index),
        gate::seccion::HASH_INDEX,
        "hash_index"
    );
}

/// ** EL KERNEL NO PUEDE SEGUIR LLEVANDO SUS PROPIOS LITERALES.
///
/// Esta prueba lee el fichero del cargador y comprueba que **no queda ni un
/// offset escrito a mano** en `leer_reloc`: tiene que usar los del crate
/// compartido.
///
/// Es la misma inversion que el guardian de `bmo.h`: la lista no vale de nada si
/// alguien puede anadir una copia sin que nadie se entere. Aqui la fuente de la
/// verdad es `bmo-bex-gate`, y esto vigila que el kernel no se invente otra.
///
/// [!] No falla si el fichero no esta: `bmo-abi` se puede compilar sin el arbol
/// del kernel al lado, y una prueba que exige un fichero de otro proyecto rompe
/// el `cargo test` de quien acaba de clonar solo esta carpeta.
#[test]
fn el_cargador_del_kernel_no_lleva_offsets_propios() {
    let ruta = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../Ultra_kernel_x86-64/kernel/src/ring0/task/bex.rs");
    let Ok(fuente) = std::fs::read_to_string(&ruta) else {
        eprintln!("sin el arbol del kernel al lado: no hay nada que comprobar");
        return;
    };
    let Some(i) = fuente.find("pub fn leer_reloc(") else {
        panic!("`leer_reloc` ya no se llama asi: esta prueba mira el sitio equivocado");
    };
    let cuerpo: String = fuente[i..].chars().take(900).collect();

    // `base + N` con N literal es exactamente lo que se quiere prohibir.
    let mut sueltos = Vec::new();
    for trozo in cuerpo.split("base + ").skip(1) {
        let n: String = trozo.chars().take_while(|c| c.is_ascii_digit()).collect();
        if !n.is_empty() {
            sueltos.push(n);
        }
    }
    assert!(
        sueltos.is_empty(),
        "`leer_reloc` todavia lleva offsets a mano ({}). Tienen que salir de \
         `bmo_bex_gate::reloc::*`, que es lo unico que esta prueba puede atar al \
         struct -- un literal aqui es una segunda verdad que nadie vigila.",
        sueltos.join(", ")
    );
}

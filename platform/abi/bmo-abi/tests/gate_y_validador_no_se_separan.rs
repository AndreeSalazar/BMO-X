//! **LAS DOS COPIAS DE UNA MISMA REGLA, ATADAS POR UNA PRUEBA.**
//!
//! # Por que hay dos copias, y por que NO se arreglo juntandolas
//!
//! La regla es *"una relocation tiene que caber dentro de la seccion que dice
//! parchear"*, y el 2026-08-25 se descubrio que vivia en **un solo sitio**:
//! `bef::validator`, que es quien llama el toolchain. El cargador del kernel no
//! lo llama --no puede: este crate usa `alloc`-- asi que un `.bex` copiado a
//! mano al FAT32 entraba con sus relocations sin mirar.
//!
//! La regla se escribio en `bmo-bex-gate`, que es el juez sin `alloc` al que el
//! kernel SI llama. Lo obvio despues era que este validador delegara en el.
//! **No se hizo, y el motivo esta escrito en el `Cargo.toml` de este crate:**
//!
//! > *"Solo para las PRUEBAS: los offsets compartidos con el cargador del
//! > kernel. No es dependencia de la libreria -- el contrato no depende de la
//! > puerta."*
//!
//! Delegar habria invertido esa flecha en silencio: el CONTRATO pasaria a
//! depender de la PUERTA. Y esa flecha no es un detalle de empaquetado -- es lo
//! que permite que exista mas de una puerta sin tocar el contrato.
//!
//! # Entonces que impide que se separen
//!
//! Esto. `bmo-bex-gate` es dev-dependency, o sea que **en las pruebas si esta**,
//! y aqui se le pregunta lo mismo a los dos y se exige la misma respuesta.
//!
//! > Dos copias de una decision son dos decisiones esperando a separarse.
//! > Cuando la arquitectura no deja juntarlas, lo que queda no es confiar:
//! > es **atarlas por fuera**.
//!
//! [!] Y si algun dia el contrato deja de tener enforcement dentro --que seria
//! lo correcto-- esta prueba se borra con el. Mientras exista, existe.

/// La regla **tal y como la escribe el validador**, copiada de
/// `validate_reloc_section` a proposito.
///
/// Si alguien cambia alla y no aqui, esta funcion deja de representarlo y la
/// prueba de abajo se vuelve mentira. Es el limite de este metodo y se dice:
/// **ata las dos implementaciones, no vigila que esta copia siga siendo fiel.**
/// La linea de alla lleva un comentario que manda aqui.
fn como_lo_dice_el_validador(offset: u64, parche: usize, file_size: u64, mem_size: u64) -> bool {
    let end = offset as usize + parche;
    // El validador ERRA cuando se pasa de las dos; o sea que "cabe" es lo
    // contrario de eso.
    !(end > file_size as usize && end > mem_size as usize)
}

/// Los dos jueces, la misma pregunta, la misma respuesta.
///
/// Se barren los bordes de verdad --el ultimo byte que cabe, el primero que no,
/// la `.bss` que no ocupa en fichero-- y no una nube de numeros al azar: un
/// desacuerdo vive en un borde, no en el medio.
#[test]
fn reloc_cabe_dice_lo_mismo_que_este_validador() {
    let casos: &[(u64, usize, u64, u64)] = &[
        // (offset, parche, file_size, mem_size)
        (0, 8, 0x400, 0x400),        // el principio
        (0x3F8, 8, 0x400, 0x400),    // el ultimo que cabe, justo
        (0x3F9, 8, 0x400, 0x400),    // el primero que no
        (0x400, 8, 0x400, 0x400),    // el borde exacto por arriba
        (0x9000, 8, 0x400, 0x400),   // *** el que cae en la seccion de al lado
        (0x100, 8, 0, 0x1000),       // una .bss: manda `mem`
        (0xFFC, 4, 0x1000, 0x1000),  // un parche de 4, que existe en el formato
        (0xFFD, 4, 0x1000, 0x1000),
        (0, 8, 0, 0),                // una seccion vacia no admite ninguna
    ];
    for &(off, parche, fs, ms) in casos {
        let gate = bmo_bex_gate::reloc_cabe(off, parche as u64, fs, ms);
        let val = como_lo_dice_el_validador(off, parche, fs, ms);
        assert_eq!(
            gate, val,
            "los dos jueces discrepan en offset={:#x} parche={} file={} mem={}: \
             gate dice {} y el validador {}",
            off, parche, fs, ms, gate, val
        );
    }
}

/// **El desbordamiento es lo unico donde NO coinciden, y el gate es el bueno.**
///
/// El validador hace `rel.offset as usize + patch_size` con un `+` normal. En
/// `debug` eso entra en panico y en `release` da la vuelta y contesta que SI
/// cabe -- con un `offset` que viene del fichero, o sea de fuera.
///
/// *** No se arregla alla porque este crate no se ejecuta en la maquina: corre
/// en el anfitrion, dentro del compilador. Donde importa es en el cargador, y
/// ahi manda el gate, que usa `checked_add`. **Se deja escrito para que el dia
/// que alguien toque esa linea sepa que hay un caso que no comparten.**
#[test]
fn en_el_desbordamiento_el_gate_es_mas_estricto_y_es_a_proposito() {
    assert!(
        !bmo_bex_gate::reloc_cabe(u64::MAX, 8, 0x400, 0x400),
        "un offset imposible no puede caber"
    );
}

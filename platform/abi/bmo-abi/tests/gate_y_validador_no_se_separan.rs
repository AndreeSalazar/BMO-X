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

/// **LA VERSION DEL ABI, QUE ES LA OTRA COPIA -- y la que tenia la grieta.**
///
/// # Lo que se encontro el 2026-08-26
///
/// `bmo-abi` declara la regla en la misma frase que la define:
///
/// > *"Major versions are incompatible; minor versions are additive."*
///
/// Y el cargador --que es quien de verdad decide, porque el kernel llama al
/// gate y no a este crate-- la tenia escrita a mano y **de otra forma**:
///
/// ```text
///    if !((abi_mayor == 1 || abi_mayor == 2) && abi_menor == 0)
/// ```
///
/// *** `abi_menor == 0` no es aditivo: es exacto. El dia que el ABI subiera a
/// `2.1` --o sea el primer dia que se "mejorase" de la forma que el contrato
/// declara segura-- un `.bex` de `2.1` habria sido rechazado por el cargador
/// mientras el contrato decia que tenia que entrar.
///
/// No habia hecho dano porque nadie ha subido el menor nunca. Eso no es que
/// estuviera bien: es que todavia no se habia cobrado.
///
/// ** Esta prueba barre TODO el espacio pequeno de versiones en vez de mirar
/// las de hoy. Comprobar `(2,0)` no habria encontrado nada -- ahi las dos
/// copias coincidian. Lo que separa dos reglas no es el caso que se usa: es el
/// que todavia no.
#[test]
fn el_gate_y_el_contrato_admiten_las_mismas_versiones_del_abi() {
    for mayor in 0u8..=4 {
        for menor in 0u8..=4 {
            let gate = bmo_bex_gate::abi_admisible(mayor, menor);
            let contrato = bmo_abi::supports_abi((mayor, menor));
            assert_eq!(
                gate, contrato,
                "el cargador y el contrato discrepan en el ABI {}.{}: \
                 el gate dice {} y `supports_abi` {}",
                mayor, menor, gate, contrato
            );
        }
    }
}

/// **Y que la regla sea DE VERDAD aditiva en el menor**, no solo igual en los
/// dos sitios. Dos copias equivocadas de la misma forma tambien coinciden.
#[test]
fn el_menor_es_aditivo_en_los_dos() {
    let (mayor, menor) = bmo_abi::BMO_ABI_VERSION;
    for m in 0..=menor {
        assert!(
            bmo_abi::supports_abi((mayor, m)),
            "el contrato tendria que admitir {}.{}", mayor, m
        );
        assert!(
            bmo_bex_gate::abi_admisible(mayor, m),
            "el cargador tendria que admitir {}.{}", mayor, m
        );
    }
    assert!(
        !bmo_bex_gate::abi_admisible(mayor, menor + 1),
        "un binario que pide mas menor del que hay NO puede entrar: pide algo que \
         este sistema no implementa"
    );
}

/// **LA PRUEBA QUE DE VERDAD HABRIA CAZADO LA GRIETA.**
///
/// Las dos de arriba comparan las dos copias entre si, y eso **no basta**: con
/// el menor de hoy en cero, `menor <= 0` y `menor == 0` contestan lo mismo en
/// todas las versiones que existen. Las dos copias podian estar de acuerdo *y
/// las dos equivocadas*.
///
/// Aqui se le pregunta a la REGLA, con unos limites inventados, la unica
/// pregunta que la distingue: **si este sistema fuera el 2.2, entraria un
/// binario compilado contra el 2.1?** El contrato dice que si --el menor es
/// aditivo-- y una comprobacion de igualdad diria que no.
#[test]
fn el_menor_es_aditivo_de_verdad_y_no_solo_por_casualidad() {
    // Un sistema hipotetico 2.2, con el 1.0 heredado.
    let admite = |mayor, menor| bmo_bex_gate::admisible_con(mayor, menor, 2, 2, 1, 0);

    assert!(admite(2, 0), "2.0 tiene que entrar en un sistema 2.2");
    assert!(admite(2, 1), "*** 2.1 tiene que entrar en un sistema 2.2: EL MENOR ES ADITIVO");
    assert!(admite(2, 2), "2.2 es el de casa");
    assert!(!admite(2, 3), "2.3 pide algo que este sistema no implementa");
    assert!(!admite(3, 0), "un mayor distinto es incompatible por definicion");
    assert!(admite(1, 0), "el heredado sigue entrando");
    assert!(!admite(1, 1), "pero solo hasta su propio menor");
}

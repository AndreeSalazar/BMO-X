//! Pruebas de `arquitectura`, y viven FUERA de `src/` a proposito.
//!
//! `tests/agnostico.rs` prohibe que el frontend nombre una maquina, y estas
//! pruebas nombran `x86_64` en cada linea -- porque prueban **contra una
//! concreta**. La regla no es "aqui nadie dice x86": es **el compilador no
//! nombra maquinas, y quien las prueba si**.
//!
//! Que la frontera caiga en `src/` contra `tests/` no es una casualidad comoda:
//! es la misma linea que separa lo que se entrega de lo que lo comprueba.

use bmo_inti_front::arquitectura::Maquina;
use bmo_mods::Roots;

fn x86() -> Maquina {
    Maquina::buscar(&Roots::find(), "x86_64").expect("no encuentro arch/x86_64/inti.toml")
}

#[test]
fn la_tabla_trae_los_nombres() {
    let m = x86();
    assert_eq!(m.nombre(), "x86_64");
    assert!(m.cuantos_nombres() >= 30, "solo {}", m.cuantos_nombres());
    assert_eq!(m.instruccion("escribe_puerto"), Some("outb"));
    assert_eq!(m.instruccion("lee_reloj"), Some("rdtsc"));
}

/// La regla: no es "de bajo nivel", es "aqui nadie comprueba por ti".
#[test]
fn lo_que_pide_crudo_y_lo_que_no() {
    let m = x86();
    assert!(m.pide_crudo("escribe_puerto"), "un aparato hace lo que le mandes");
    assert!(m.pide_crudo("sin_interrupciones"), "puede dejar la maquina sorda");
    assert!(!m.pide_crudo("lee_reloj"), "leer la hora no rompe nada");
    assert!(!m.pide_crudo("cuenta_unos"), "es aritmetica");
}

/// La puerta del sistema NO es de la arquitectura. Al otro lado hay un kernel
/// que valida una capability, asi que ni pide `crudo` ni sale de esta tabla.
#[test]
fn la_puerta_no_vive_en_la_arquitectura() {
    let m = x86();
    assert!(!m.conoce("invoca"));
    assert!(!m.conoce("espera_a"));
}

#[test]
fn el_perfil_de_maquina_llega_por_la_tabla() {
    let m = x86();
    assert_eq!(m.ancho_de_puntero(), 8);
    assert_eq!(m.alineacion_maxima(), 16);
}

#[test]
fn una_maquina_que_no_existe_se_dice_que_no() {
    assert!(Maquina::buscar(&Roots::find(), "z80").is_none());
}

/// Un `usa` no puede convertirse en una ruta.
#[test]
fn un_nombre_con_separadores_no_es_una_arquitectura() {
    let r = Roots::find();
    assert!(Maquina::buscar(&r, "../secreto").is_none());
    assert!(Maquina::buscar(&r, "x86_64/..").is_none());
}

// ===================================================================
//  ** `usa x86_64` carga la maquina ENTERA
// ===================================================================

/// Peticion de Eddi: *"que `usa x86_64` CARGUE TODO lo que x86-64 representa"*.
/// Los dieciseis registros, con su numero.
#[test]
fn la_tabla_trae_los_dieciseis_registros() {
    let m = x86();
    assert_eq!(m.cuantos_registros(), 16);
    assert_eq!(m.registro("rax"), Some(0));
    assert_eq!(m.registro("rsp"), Some(4));
    assert_eq!(m.registro("r15"), Some(15));
    assert_eq!(m.registro("no_existe"), None);
}

/// ** Y el reparto: el emisor no DECIDE que registros usar, los LEE.
///
/// Es lo que arregla el asignador de F3, que llevaba la lista escrita a mano en
/// Rust. Anadir una instruccion es una fila de TOML; un registro tambien.
#[test]
fn el_reparto_sale_de_la_tabla_y_no_del_emisor() {
    let m = x86();
    // rdx, rsi, rdi
    assert_eq!(m.temporales(), vec![2, 6, 7]);
    // rax, rcx
    assert_eq!(m.trabajo(), vec![0, 1]);
}

/// Los de trabajo y los de reparto **no se solapan**. Si se solaparan, una
/// operacion binaria pisaria un temporal vivo -- y el fallo aparece solo cuando
/// la expresion es lo bastante larga.
#[test]
fn los_de_trabajo_no_estan_en_el_reparto() {
    let m = x86();
    for t in m.temporales() {
        assert!(
            !m.trabajo().contains(&t),
            "el registro {} esta en las dos listas",
            t
        );
    }
}

/// Ni la pila ni el marco se reparten jamas. Repartir `rsp` no da un programa
/// lento: da uno que no vuelve.
#[test]
fn la_pila_y_el_marco_no_se_reparten_nunca() {
    let m = x86();
    let rsp = m.registro("rsp").unwrap();
    let rbp = m.registro("rbp").unwrap();
    assert!(!m.temporales().contains(&rsp));
    assert!(!m.temporales().contains(&rbp));
    assert!(!m.trabajo().contains(&rsp));
    assert!(!m.trabajo().contains(&rbp));
}

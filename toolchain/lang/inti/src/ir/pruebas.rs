//! Pruebas del descenso a IR.
//!
//! El criterio: **cada prueba fija una promesa del lenguaje que ahora se puede
//! VER**. Hasta aqui, "sin comportamiento indefinido" era un documento; en la
//! IR es una instruccion que se cuenta.

use super::*;
use crate::{lexico, palabras::Vocabulario, sintaxis};

fn ir(fuente: &str) -> ModuloIr {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    bajar(&arbol.valor).valor
}

fn en_funcion(cuerpo: &str) -> FuncionIr {
    let mut f = String::from("perfil llano\n\nfuncion prueba(a es entero32, b es entero32) devuelve entero32\n");
    for l in cuerpo.lines() {
        f.push_str("    ");
        f.push_str(l);
        f.push('\n');
    }
    let m = ir(&f);
    m.funciones.into_iter().next().expect("no hay funcion")
}

fn comprobaciones(f: &FuncionIr) -> Vec<Comprobacion> {
    f.instrucciones
        .iter()
        .filter_map(|i| match i {
            Instr::Comprueba { que, .. } => Some(*que),
            _ => None,
        })
        .collect()
}

// ===================================================================
//  Lo basico
// ===================================================================

#[test]
fn una_funcion_baja_con_sus_parametros_como_locales() {
    let f = en_funcion("devuelve a\n");
    assert_eq!(f.nombre, "prueba");
    assert_eq!(f.locales, 2, "los dos parametros");
    assert!(matches!(
        f.instrucciones.last(),
        Some(Instr::Devuelve(Some(Valor::Local(Local(0)))))
    ));
}

#[test]
fn una_suma_usa_un_temporal() {
    let f = en_funcion("devuelve a + b\n");
    assert!(f.temporales >= 1);
    assert!(f.instrucciones.iter().any(|i| matches!(
        i,
        Instr::Binaria {
            op: Op::Suma,
            izquierda: Valor::Local(Local(0)),
            derecha: Valor::Local(Local(1)),
            ..
        }
    )));
}

/// El decimal sigue siendo TEXTO, por el mismo motivo que en el lexer: pasarlo
/// por un binario intermedio perderia la exactitud que el lenguaje promete.
#[test]
fn el_decimal_no_se_convierte_por_el_camino() {
    let m = ir("perfil pleno\n\nfuncion f devuelve numero\n    devuelve 0.1\n");
    let f = &m.funciones[0];
    assert!(f.instrucciones.iter().any(|i| matches!(
        i,
        Instr::Devuelve(Some(Valor::Const(Const::Decimal(t)))) if t == "0.1"
    )));
}

#[test]
fn los_textos_van_a_un_pozo_y_no_se_repiten() {
    let m = ir(
        "perfil pleno\n\nfuncion f\n    escribe(\"hola\")\n    escribe(\"hola\")\n    escribe(\"adios\")\n",
    );
    assert_eq!(m.textos, vec!["hola", "adios"], "el pozo no repite");
}

// ===================================================================
//  ** Las reglas, hechas instrucciones
// ===================================================================

/// La promesa mas grande del lenguaje, y aqui se puede CONTAR.
#[test]
fn una_suma_trae_su_comprobacion_de_desborde() {
    let f = en_funcion("devuelve a + b\n");
    assert_eq!(comprobaciones(&f), vec![Comprobacion::Desborde]);
}

/// **UNA DIVISION TRAE DOS REGLAS, y en este orden.**
///
/// ** Las dos son de la division y ninguna sobra:
///
/// ```text
///    EntreCero   el divisor es cero        -> E1003
///    Cociente    `-2^63 entre -1` no cabe  -> E1001, y es la Regla 1
/// ```
///
/// *** La segunda no la pedia nadie hasta el 2026-08-22. El cociente que no
/// cabe se escribe con una barra, asi que se colaba entre las dos: de la
/// division solo se miraba el divisor. En metal eso era una autopsia del
/// kernel en vez de una trampa, porque `idiv` levanta `#DE` -- el mismo
/// vector que dividir entre cero.
///
/// El ORDEN se fija a proposito: las dos van ANTES de la division, porque
/// despues de dividir mal ya no queda programa que mire nada.
#[test]
fn una_division_trae_sus_dos_reglas() {
    let f = en_funcion("devuelve a / b\n");
    assert_eq!(
        comprobaciones(&f),
        vec![Comprobacion::EntreCero, Comprobacion::Cociente]
    );
}

/// Comparar y los bits no pueden salirse, asi que no pagan nada.
#[test]
fn comparar_no_paga_comprobacion() {
    let f = en_funcion("si a < b\n    devuelve 1\ndevuelve 0\n");
    assert!(comprobaciones(&f).is_empty(), "{:?}", comprobaciones(&f));

    let f = en_funcion("devuelve a bits_y b\n");
    assert!(comprobaciones(&f).is_empty());
}

/// Un indice SIEMPRE se comprueba al bajar. Lo que el compilador pueda
/// demostrar se quitara despues -- pero se quita, no se olvida.
#[test]
fn un_indice_trae_la_suya() {
    let m = ir("perfil pleno\n\nfuncion f(notas)\n    escribe(notas[0])\n");
    let c = comprobaciones(&m.funciones[0]);
    assert_eq!(c, vec![Comprobacion::Indice]);
}

/// ** El numero que se podra medir: cuantas comprobaciones cuesta un modulo.
#[test]
fn el_modulo_sabe_cuantas_comprobaciones_emitio() {
    let m = ir(
        "perfil llano\n\n\
         funcion f(a es entero32, b es entero32) devuelve entero32\n\
         \x20   devuelve a + b * a\n",
    );
    assert_eq!(m.comprobaciones(), 2, "una por cada operacion que se pasa");
}

#[test]
fn cada_comprobacion_lleva_su_codigo_y_su_sitio() {
    let f = en_funcion("devuelve a + b\n");
    match f.instrucciones.iter().find(|i| matches!(i, Instr::Comprueba { .. })) {
        Some(Instr::Comprueba { que, sitio, .. }) => {
            assert_eq!(que.codigo(), "E1001");
            assert!(sitio.linea > 0, "sin sitio no hay [DONDE]");
        }
        otro => panic!("{:?}", otro),
    }
}

// ===================================================================
//  Control
// ===================================================================

#[test]
fn un_si_baja_a_saltos_y_etiquetas() {
    let f = en_funcion("si a < b\n    devuelve 1\ndevuelve 0\n");
    assert!(f
        .instrucciones
        .iter()
        .any(|i| matches!(i, Instr::SaltaSi { .. })));
    let etiquetas = f
        .instrucciones
        .iter()
        .filter(|i| matches!(i, Instr::Etiqueta(_)))
        .count();
    assert!(etiquetas >= 3, "una por rama, una de salida: {}", etiquetas);
}

#[test]
fn un_repite_mientras_cierra_su_bucle() {
    let f = en_funcion("repite mientras a < b\n    devuelve 1\ndevuelve 0\n");
    // La ultima instruccion del bucle vuelve al principio.
    assert!(f.instrucciones.iter().any(|i| matches!(i, Instr::Salta(_))));
}

/// `corta` salta a la salida del bucle en el que esta, y `continua` a su
/// vuelta. Con bucles anidados, al del de dentro.
#[test]
fn corta_salta_a_la_salida_de_su_bucle() {
    let f = en_funcion("repite\n    corta\n");
    let saltos: Vec<u32> = f
        .instrucciones
        .iter()
        .filter_map(|i| match i {
            Instr::Salta(Etiqueta(e)) => Some(*e),
            _ => None,
        })
        .collect();
    assert!(!saltos.is_empty());
}

// ===================================================================
//  Lo que este modulo se niega a saber
// ===================================================================
//
//  (El test de que la IR no nombra ninguna maquina vive en
//  `tests/agnostico.rs`, con los demas: nombra maquinas para comprobarlo, y
//  por eso no puede estar dentro de `src/`.)

/// Una ranura local es un INDICE, no una direccion: el ancho lo pone el emisor
/// con el perfil de la maquina.
#[test]
fn las_locales_son_indices_y_no_direcciones() {
    let f = en_funcion("cambiante t = a\nt = t + b\ndevuelve t\n");
    assert_eq!(f.locales, 3, "a, b y t");
    assert!(f
        .instrucciones
        .iter()
        .any(|i| matches!(i, Instr::Guarda { destino: Local(2), .. })));
}

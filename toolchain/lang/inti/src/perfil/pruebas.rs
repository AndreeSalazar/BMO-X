//! Pruebas del analisis de perfiles.

use super::*;
use crate::{arquitectura::Maquina, lexico, palabras::Vocabulario, sintaxis};

fn comprueba(fuente: &str) -> Cosecha<Informe> {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    // Las pruebas cargan la maquina si el fuente la declaro, igual que hace el
    // compilador de verdad.
    // El mismo camino que hace el compilador de verdad: un `usa` que es una
    // arquitectura conocida trae su maquina; uno que no, no.
    let maquinas: Vec<Maquina> = arbol
        .valor
        .usa
        .iter()
        .filter_map(|(n, _)| Maquina::buscar(&bmo_mods::Roots::find(), n))
        .collect();
    comprobar(&arbol.valor, &Catalogo::por_defecto(), &maquinas)
}

fn codigos_de(fuente: &str) -> Vec<&'static str> {
    comprueba(fuente).codigos()
}

// ===================================================================
//  Lo que `llano` no admite
// ===================================================================

#[test]
fn en_llano_no_hay_lista() {
    let c = codigos_de("perfil llano\n\nfuncion principal\n    notas = [1, 2, 3]\n");
    assert_eq!(c, vec!["E0070"]);
}

#[test]
fn en_llano_no_hay_texto() {
    let c = codigos_de("perfil llano\n\nfuncion principal\n    saludo = \"hola\"\n");
    assert_eq!(c, vec!["E0070"]);
}

/// Sin medida no hay pila. La obligacion sale del perfil, no del gusto.
#[test]
fn en_llano_hay_que_decir_la_medida() {
    let c = codigos_de("perfil llano\n\nfuncion cuenta(x es numero) devuelve numero\n    devuelve x\n");
    assert!(c.iter().all(|x| *x == "E0020"), "{:?}", c);
    assert!(!c.is_empty());
}

#[test]
fn en_llano_un_parametro_sin_tipo_se_denuncia() {
    let c = codigos_de("perfil llano\n\nfuncion suma(a, b) devuelve entero32\n    devuelve a\n");
    assert_eq!(c, vec!["E0020", "E0020"], "uno por parametro");
}

#[test]
fn en_llano_no_hay_tareas() {
    let c = codigos_de("perfil llano\n\nfuncion principal\n    en paralelo\n        espera()\n");
    assert_eq!(c, vec!["E0070"]);
}

/// Y lo mismo escrito en `pleno` no dice nada.
#[test]
fn en_pleno_todo_eso_vale() {
    let c = codigos_de(
        "perfil pleno\n\n\
         funcion media(notas es lista de numero) devuelve numero\n\
         \x20   saludo = \"hola\"\n\
         \x20   devuelve 0\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

// ===================================================================
//  `crudo`
// ===================================================================

#[test]
fn crudo_no_existe_en_pleno() {
    let c = codigos_de("perfil pleno\n\nfuncion principal\n    crudo\n        espera()\n");
    assert_eq!(c, vec!["E0071"]);
}

/// La regla que decide: `crudo` no marca "bajo nivel", marca "aqui nadie
/// comprueba por ti".
#[test]
fn tocar_un_puerto_fuera_de_crudo_se_denuncia() {
    let c = codigos_de(
        "perfil llano\nusa x86_64\n\nfuncion lee devuelve natural8\n    devuelve entrada_puerto(0x60)\n",
    );
    assert_eq!(c, vec!["E0072"]);
}

#[test]
fn dentro_de_crudo_el_puerto_vale() {
    let c = codigos_de(
        "perfil llano\n\n\
         funcion lee devuelve natural8\n\
         \x20   crudo\n\
         \x20       devuelve entrada_puerto(0x60)\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

/// `invoca` NO pide `crudo` aunque sea la puerta del sistema: al otro lado hay
/// un kernel que valida una capability. Esa es la diferencia entera.
#[test]
fn la_puerta_no_pide_crudo() {
    let c = codigos_de(
        "perfil llano\nusa bmo\n\n\
         funcion manda(cap es natural64) devuelve natural64\n\
         \x20   devuelve invoca(cap, 7, 0, 0, 0)\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

// ===================================================================
//  El informe
// ===================================================================

/// ** El numero que convierte "cuanto de mi programa esta atado a esta
/// maquina?" en un dato.
#[test]
fn los_bloques_crudo_se_cuentan() {
    let c = comprueba(
        "perfil llano\n\n\
         funcion a devuelve natural8\n\
         \x20   crudo\n\
         \x20       devuelve entrada_puerto(0x60)\n\
         funcion b devuelve natural8\n\
         \x20   crudo\n\
         \x20       devuelve entrada_puerto(0x64)\n",
    );
    assert_eq!(c.valor.bloques_crudo, 2);
    assert!(!c.hay_errores());
}

#[test]
fn un_programa_sin_crudo_lo_dice_con_un_cero() {
    let c = comprueba("perfil pleno\n\nfuncion principal\n    escribe(\"hola\")\n");
    assert_eq!(c.valor.bloques_crudo, 0);
}

// ===================================================================
//  La tabla
// ===================================================================

#[test]
fn el_catalogo_incrustado_carga() {
    let cat = Catalogo::por_defecto();
    assert!(cat.crecen.contains("texto"));
    assert!(cat.without_size.contains("numero"));
    // Lo que pide `crudo` ya no vive aqui: se mudo a la arquitectura, que es
    // de donde depende. Ver `arquitectura::pruebas`.
}

/// Una tabla ilegible no puede convertirse en "todo esta prohibido": eso
/// pararia compilaciones correctas con un mensaje sobre el programa del
/// usuario, cuando el problema es de la instalacion.
#[test]
fn una_tabla_rota_no_acusa_al_programa() {
    let cat = Catalogo::desde_texto("esto no es toml [[[");
    let v = Vocabulario::por_defecto().unwrap();
    let fuente = "perfil llano\n\nfuncion principal\n    saludo = \"hola\"\n";
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    let c = comprobar(&arbol.valor, &cat, &[]);
    // El texto sigue siendo texto y `llano` sigue sin monton, asi que eso se
    // denuncia igual; lo que no puede pasar es que la tabla rota invente
    // prohibiciones nuevas.
    assert_eq!(c.codigos(), vec!["E0070"]);
}

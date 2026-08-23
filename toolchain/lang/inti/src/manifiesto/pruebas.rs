//! Pruebas del manifiesto: **que lo escrito se pueda volver a leer.**

use super::*;
use crate::arbol::{Perfil, Pieza};
use crate::{lexico, palabras::Vocabulario, sintaxis};

fn modulo(fuente: &str) -> Modulo {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(!arbol.hay_errores(), "el fuente de la prueba no se lee");
    arbol.valor
}

/// **La ida y la vuelta.** Un manifiesto que se escribe y no se puede releer no
/// es un contrato: es un comentario largo dentro de un fichero binario.
#[test]
fn lo_escrito_se_vuelve_a_leer_igual() {
    let mut m = modulo("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    m.piezas.push(Pieza {
        fichero: "monton/origen.inti".to_string(),
        usa: "monton".to_string(),
        perfil: Perfil::Llano,
        desde: 0,
        hasta: 1,
    });
    let mut informe = Informe::default();
    // El perfil RESULTANTE, que es lo que va al manifiesto desde P2: no lo que
    // el fichero declaro, sino lo que el binario ES.
    informe.perfil_resultante = "llano".to_string();
    informe.bloques_crudo = 3;
    informe.arquitecturas = vec!["x86_64".to_string()];

    let antes = de(&m, &informe, "cpu.inti");
    let texto = antes.a_toml();
    let despues = Manifiesto::de_toml(&texto).expect("el TOML que escribi no se parsea");
    assert_eq!(antes, despues, "la ida y la vuelta no coinciden:\n{}", texto);
}

/// **Se declara lo que hace falta para no abrir el fuente.**
#[test]
fn dice_perfil_crudo_arquitecturas_y_piezas() {
    let mut m = modulo("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    m.piezas.push(Pieza {
        fichero: "monton/reparto.inti".to_string(),
        usa: "monton".to_string(),
        // ** La pieza declara OTRO perfil que el modulo. Hoy no se juzga, y por
        // eso hay que comprobar que al menos se GUARDA: es el dato encima del
        // que se escribe la regla del mezclado.
        perfil: Perfil::Pleno,
        desde: 0,
        hasta: 1,
    });
    let mut informe = Informe::default();
    informe.bloques_crudo = 1;
    informe.arquitecturas = vec!["x86_64".to_string()];

    let leido = Manifiesto::de_toml(&de(&m, &informe, "sonda.inti").a_toml()).unwrap();
    assert_eq!(leido.lenguaje, "inti");
    assert_eq!(leido.perfil, "llano");
    assert_eq!(leido.crudo, 1);
    assert_eq!(leido.arquitecturas, vec!["x86_64".to_string()]);
    assert_eq!(leido.piezas.len(), 1);
    assert_eq!(leido.piezas[0].usa, "monton");
    assert_eq!(
        leido.piezas[0].perfil, "pleno",
        "la pieza tiene que llevar SU perfil, no el del modulo"
    );
}

/// **UNA RUTA DE WINDOWS NO PUEDE ROMPER EL MANIFIESTO.**
///
/// ** `fuente` es una ruta y en esta maquina lleva `\`. Sin escapar, el TOML que
/// el compilador acaba de escribir no se puede volver a leer -- y el `.bex`
/// seguiria pasando el gate, porque el validador solo mira que sea UTF-8.
///
/// Es un fallo que solo aparece en la maquina donde se desarrolla, que es la
/// peor clase: el que lo escribio no lo ve.
#[test]
fn una_ruta_con_barras_invertidas_sobrevive() {
    let m = modulo("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    let ruta = "C:\\Users\\Salazar\\Documents\\BMO\\cpu.inti";
    let escrito = de(&m, &Informe::default(), ruta);
    let texto = escrito.a_toml();
    let leido = Manifiesto::de_toml(&texto)
        .unwrap_or_else(|| panic!("el TOML no se parsea:\n{}", texto));
    assert_eq!(leido.fuente, ruta);
}

/// **El mismo modulo da el mismo texto.** Dos compilaciones de la misma fuente
/// tienen que dar el mismo fichero, o *"este `.bex` es el que audite"* deja de
/// poder decirse.
#[test]
fn el_mismo_modulo_da_el_mismo_texto() {
    let m = modulo("perfil llano\n\nfuncion principal devuelve entero32\n    devuelve 0\n");
    let a = de(&m, &Informe::default(), "x.inti").a_toml();
    let b = de(&m, &Informe::default(), "x.inti").a_toml();
    assert_eq!(a, b);
}

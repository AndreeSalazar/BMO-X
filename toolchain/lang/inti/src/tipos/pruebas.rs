//! Pruebas de `tipos`.
//!
//! ## ** Las dos mitades, y las dos hacen falta
//!
//! Un comprobador se juzga por dos cosas, no por una:
//!
//! ```text
//!    lo que DENUNCIA     que atrape lo que esta mal
//!    lo que DEJA PASAR   que no denuncie lo que esta bien
//! ```
//!
//! La segunda es la que decide si el aviso sobrevive: uno que salta de mas se
//! desactiva en una semana, y entonces ya no vigila nada. Por eso aqui hay tantas
//! pruebas de lo que **no** se denuncia como de lo que si.

use super::*;
use crate::disposicion::{comprobar as medir, Medidas};
use crate::{lexico, palabras::Vocabulario, sintaxis};

fn codigos_de(fuente: &str) -> Vec<&'static str> {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    let plano = medir(&arbol.valor, Medidas::por_defecto()).valor;
    comprobar(&arbol.valor, &plano).codigos()
}

/// Envuelve un cuerpo en una funcion con dos parametros de cada clase.
fn con(cuerpo: &str) -> String {
    format!(
        "perfil llano\n\nfuncion f(n es entero64, x es flotante64) devuelve entero64\n{}",
        cuerpo
    )
}

// ===================================================================
//  ** LO QUE SE DENUNCIA
// ===================================================================

/// **EL CASO QUE ABRIO EL MODULO.**
///
/// Antes esto compilaba y hacia aritmetica de coma flotante sobre los bits de un
/// entero: `n` valiendo 5 no entra como `5.0`, entra como `2,47e-323`. Un
/// flotante perfectamente valido, y otro numero.
#[test]
fn mezclar_flotante_y_entero_se_denuncia() {
    assert!(codigos_de(&con("    devuelve x + n\n")).contains(&"E0022"));
    assert!(codigos_de(&con("    devuelve n + x\n")).contains(&"E0022"));
}

/// Las cuatro operaciones, no solo la suma.
#[test]
fn las_cuatro_operaciones_mezcladas_se_denuncian() {
    for op in ["+", "-", "*", "/"] {
        let c = codigos_de(&con(&format!("    devuelve x {} n\n", op)));
        assert!(c.contains(&"E0022"), "`{}` mezclado paso: {:?}", op, c);
    }
}

/// ** Y COMPARAR TAMBIEN, que es el que mas se cuela porque "parece que no
/// opera".
///
/// `x < n` lee los mismos ocho bytes con dos alfabetos y contesta una pregunta
/// que no significa nada. Que devuelva un `logico` no lo hace inofensivo.
#[test]
fn comparar_mezclado_se_denuncia() {
    for op in ["<", ">", "<=", ">=", "="] {
        let c = codigos_de(&con(&format!(
            "    si x {} n\n        devuelve 1\n    devuelve 0\n",
            op
        )));
        assert!(c.contains(&"E0022"), "`{}` mezclado paso: {:?}", op, c);
    }
}

/// Guardar un entero donde va un flotante.
#[test]
fn asignar_la_clase_ajena_se_denuncia() {
    assert!(codigos_de(&con("    y es flotante64 = n\n    devuelve 0\n")).contains(&"E0022"));
    assert!(codigos_de(&con("    y es entero64 = x\n    devuelve 0\n")).contains(&"E0022"));
}

/// ** SIN VERACIDAD: una condicion es una pregunta, no "algo que no es cero".
///
/// Es la sorpresa de Python que INTI quita a proposito. Alli `si lista`
/// pregunta si esta vacia, `si 0` es falso y `si "0"` es cierto: tres reglas
/// distintas que hay que recordar, en el sitio donde equivocarse sale mas caro.
#[test]
fn una_condicion_que_no_es_logica_se_denuncia() {
    let c = codigos_de(&con("    si n\n        devuelve 1\n    devuelve 0\n"));
    assert!(c.contains(&"E0040"), "{:?}", c);
}

/// Y en el bucle igual, que es donde mas se escribe.
#[test]
fn la_condicion_de_un_bucle_tambien() {
    let c = codigos_de(&con(
        "    repite mientras n\n        n = n - 1\n    devuelve 0\n",
    ));
    assert!(c.contains(&"E0040"), "{:?}", c);
}

// ===================================================================
//  ** LO QUE NO SE DENUNCIA -- la mitad que hace que el aviso sobreviva
// ===================================================================

/// **UN LITERAL NO TIENE TIPO TODAVIA.**
///
/// `x * 2` con `x` de coma flotante es correcto: el `2` no es "un entero que se
/// convierte", es un numero que aun no ha elegido forma. Eso NO es la conversion
/// implicita que prohibe el censo `v05` -- alli hay DOS tipos escritos, y aqui
/// hay uno y un literal.
///
/// ** Si esto se denunciara, el modulo entero seria inservible: no se podria
/// escribir `x / 2.0` ni `n + 1`.
#[test]
fn un_literal_se_adapta_y_no_es_una_conversion() {
    assert!(codigos_de(&con("    devuelve x * 2\n")).is_empty());
    assert!(codigos_de(&con("    devuelve x + 0.5\n")).is_empty());
    assert!(codigos_de(&con("    devuelve n + 1\n")).is_empty());
    assert!(codigos_de(&con("    devuelve n * 2 - 3\n")).is_empty());
}

/// Dos de la misma clase, aunque el ancho no coincida.
///
/// ** Pasa a proposito y esta escrito en la cabecera del modulo: INTI opera en
/// el ancho de la maquina y ninguno de los dos deja de ser un numero por el
/// camino. Lo que produce basura es mezclar CLASES.
#[test]
fn mezclar_anchos_de_la_misma_clase_pasa() {
    let f = "perfil llano\n\nfuncion f(a es entero64, b es natural8) devuelve entero64\n    devuelve a + b\n";
    assert!(codigos_de(f).is_empty());
}

/// La conversion pedida por su nombre es exactamente la salida que el aviso
/// sugiere. Si no funcionara, el mensaje estaria mandando a un sitio cerrado.
#[test]
fn pedir_la_conversion_por_su_nombre_arregla_el_aviso() {
    assert!(codigos_de(&con("    devuelve x + flotante64(n)\n")).is_empty());
    assert!(codigos_de(&con("    y es flotante64 = flotante64(n)\n    devuelve 0\n")).is_empty());
    assert!(codigos_de(&con("    y es entero64 = entero64(x)\n    devuelve 0\n")).is_empty());
}

/// Una comparacion SI es una condicion, que es el caso normal.
#[test]
fn una_comparacion_vale_como_condicion() {
    assert!(codigos_de(&con("    si n > 0\n        devuelve 1\n    devuelve 0\n")).is_empty());
    assert!(codigos_de(&con("    si x < 1.0\n        devuelve 1\n    devuelve 0\n")).is_empty());
    assert!(
        codigos_de(&con("    si n > 0 y n < 10\n        devuelve 1\n    devuelve 0\n")).is_empty()
    );
}

/// Y un `logico` declarado, tambien.
#[test]
fn un_logico_declarado_vale_como_condicion() {
    let f = "perfil llano\n\nfuncion f(hay es logico, b es entero64) devuelve entero64\n    si hay\n        devuelve 1\n    devuelve 0\n";
    assert!(codigos_de(f).is_empty());
}

/// ** Y una LLAMADA no se denuncia, aunque no se sepa que devuelve.
///
/// Su tipo de retorno no se resuelve todavia. Denunciar `si hay_tecla()` seria
/// denunciar un programa correcto -- y eso es como se desactiva un aviso.
#[test]
fn una_llamada_no_se_denuncia_por_no_saber_que_devuelve() {
    let f = "\
perfil llano

funcion hay_algo(a es entero64, b es entero64) devuelve logico
    devuelve a > b

funcion f(n es entero64, m es entero64) devuelve entero64
    si hay_algo(n, m)
        devuelve 1
    devuelve 0
";
    assert!(codigos_de(f).is_empty());
}

/// ** `pleno` no se toca, y no es una excepcion comoda.
///
/// Alli un valor puede cambiar de forma en ejecucion, y medirlo con estas reglas
/// denunciaria programas correctos. El dia que `pleno` tenga su modelo, esta
/// puerta se abre.
#[test]
fn en_pleno_no_se_comprueba_nada() {
    let f = "perfil pleno\n\nfuncion principal\n    escribe(1 + 2.5)\n";
    assert!(codigos_de(f).is_empty());
}

/// El bucle de pixeles de F5a, entero, no se queja. Es la prueba de que esto no
/// rompe lo que ya funcionaba.
#[test]
fn el_bucle_de_pixeles_sigue_pasando() {
    let f = "\
perfil llano
usa memoria

funcion pinta(pantalla es bufer de natural32, cuantos es entero64, color es entero64)
    cambiante i = 0
    repite mientras i < cuantos
        crudo
            pantalla[i] = color
        i = i + 1
";
    assert!(codigos_de(f).is_empty());
}

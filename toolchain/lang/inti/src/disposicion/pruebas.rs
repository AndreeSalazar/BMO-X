//! Pruebas de la disposicion.

use super::*;
use crate::{lexico, palabras::Vocabulario, sintaxis};

fn plano_de(fuente: &str) -> Cosecha<Plano> {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    comprobar(&arbol.valor, Medidas::por_defecto())
}

fn codigos_de(fuente: &str) -> Vec<&'static str> {
    plano_de(fuente).codigos()
}

const CABECERA: &str = "perfil llano\n\n";

// ===================================================================
//  Medir
// ===================================================================

#[test]
fn los_tipos_con_medida_en_el_nombre_miden_lo_que_dicen() {
    let m = Medidas::por_defecto();
    assert_eq!(m.de("natural8"), Some(1));
    assert_eq!(m.de("natural16"), Some(2));
    assert_eq!(m.de("natural32"), Some(4));
    assert_eq!(m.de("natural64"), Some(8));
    assert_eq!(m.de("logico"), Some(1), "un byte, no un bit");
}

/// Lo que no dice su medida NO mide cero: no se sabe.
///
/// ** La diferencia importa mas de lo que parece. Con `Some(0)`, un registro con
/// un campo `texto` mediria lo mismo con el campo y sin el, y los
/// desplazamientos de los de detras cuadrarian igual -- asi que nadie se
/// enteraria hasta ver datos raros.
#[test]
fn lo_que_no_dice_su_medida_no_mide_cero() {
    let m = Medidas::por_defecto();
    assert_eq!(m.de("texto"), None);
    assert_eq!(m.de("numero"), None);
}

#[test]
fn un_registro_llano_se_mide_y_sus_campos_tienen_sitio() {
    let p = plano_de(&format!(
        "{}registro Punto\n    x es entero64\n    y es entero64\n",
        CABECERA
    ));
    assert!(!p.hay_errores(), "{:?}", p.codigos());
    let r = p.valor.registro("Punto").expect("sin registro");
    assert_eq!(r.campo("x").unwrap().desplazamiento, 0);
    assert_eq!(r.campo("y").unwrap().desplazamiento, 8);
    assert_eq!(r.medida, 16);
}

/// ** LA ALINEACION, que es donde se ve si esto esta hecho de verdad.
///
/// Un byte y luego una palabra: la palabra NO empieza en 1. Se sube al
/// siguiente multiplo de 8, y el registro entero se redondea a 8 para que uno
/// detras de otro en un array siga cuadrando.
///
/// Y el motivo no es la elegancia: **hay maquinas donde un acceso desalineado
/// falla**, y otras donde funciona y va mas lento. Cual de las dos te toca es
/// exactamente la clase de cosa que INTI promete no dejar al azar.
///
/// (Este comentario nombraba una maquina concreta y lo tumbo
/// `tests/agnostico.rs`. Tenia razon dos veces: el frontend no puede nombrarla,
/// y ademas la frase queda mejor sin ella -- lo que importa es que VARIA.)
#[test]
fn los_campos_se_alinean_y_el_registro_se_redondea() {
    let p = plano_de(&format!(
        "{}registro Mezcla\n    a es natural8\n    b es entero64\n    c es natural8\n",
        CABECERA
    ));
    let r = p.valor.registro("Mezcla").unwrap();
    assert_eq!(r.campo("a").unwrap().desplazamiento, 0);
    assert_eq!(r.campo("b").unwrap().desplazamiento, 8, "no en el 1");
    assert_eq!(r.campo("c").unwrap().desplazamiento, 16);
    assert_eq!(r.alineacion, 8);
    assert_eq!(r.medida, 24, "redondeado, para que un array cuadre");
}

/// Y sin huecos cuando no hacen falta: alinear no es rellenar por costumbre.
#[test]
fn sin_huecos_cuando_no_hacen_falta() {
    let p = plano_de(&format!(
        "{}registro Color\n    r es natural8\n    g es natural8\n    b es natural8\n    a es natural8\n",
        CABECERA
    ));
    let r = p.valor.registro("Color").unwrap();
    assert_eq!(r.campo("a").unwrap().desplazamiento, 3);
    assert_eq!(r.medida, 4);
    assert_eq!(r.alineacion, 1);
}

/// ** Los campos van EN EL ORDEN ESCRITO, y no reordenados para ahorrar huecos.
///
/// Reordenar `Mezcla` ahorraria 8 bytes. Y romperia lo unico que un registro
/// promete de verdad: que su disposicion se pueda predecir mirando el fuente.
/// Un `registro` de INTI tiene que poder describir algo que YA existe --una
/// estructura del kernel, una cabecera de fichero-- y para eso el orden es el
/// contrato, no una oportunidad.
#[test]
fn el_orden_de_los_campos_es_el_escrito() {
    let p = plano_de(&format!(
        "{}registro Mezcla\n    a es natural8\n    b es entero64\n",
        CABECERA
    ));
    let r = p.valor.registro("Mezcla").unwrap();
    let nombres: Vec<&str> = r.campos().iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(nombres, vec!["a", "b"]);
}

// ===================================================================
//  Lo que se denuncia
// ===================================================================

#[test]
fn un_campo_sin_tipo_se_denuncia() {
    let c = codigos_de(&format!("{}registro Punto\n    x\n", CABECERA));
    assert_eq!(c, vec!["E0122"]);
}

#[test]
fn un_campo_que_no_se_puede_medir_se_denuncia() {
    let c = codigos_de(&format!("{}registro Punto\n    x es texto\n", CABECERA));
    assert_eq!(c, vec!["E0121"]);
}

/// ** Sin el tipo escrito, `p.x` no se puede resolver -- y se DICE.
///
/// Antes de este modulo se bajaba a `p`: el campo se ignoraba, sin una queja, y
/// el programa compilaba y hacia otra cosa.
#[test]
fn un_campo_sobre_algo_sin_tipo_se_denuncia() {
    let c = codigos_de(&format!(
        "{}registro Punto\n    x es entero64\n\nfuncion f(p)\n    devuelve p.x\n",
        CABECERA
    ));
    assert_eq!(c, vec!["E0121"]);
}

#[test]
fn un_campo_que_el_registro_no_tiene_se_denuncia() {
    let c = codigos_de(&format!(
        "{}registro Punto\n    x es entero64\n\nfuncion f(p es Punto)\n    devuelve p.z\n",
        CABECERA
    ));
    assert_eq!(c, vec!["E0120"]);
}

#[test]
fn con_el_tipo_escrito_el_campo_vale() {
    let c = codigos_de(&format!(
        "{}registro Punto\n    x es entero64\n\nfuncion f(p es Punto) devuelve entero64\n    devuelve p.x\n",
        CABECERA
    ));
    assert!(c.is_empty(), "{:?}", c);
}

// ===================================================================
//  `bufer`
// ===================================================================

/// Un `bufer` mide lo que un puntero; lo que mide su ELEMENTO es otra pregunta.
#[test]
fn un_bufer_mide_lo_que_un_puntero_y_su_elemento_lo_suyo() {
    let p = Plano {
        medidas: Medidas::por_defecto(),
        registros: HashMap::new(),
    };
    let t = Tipo::Bufer(Box::new(Tipo::Nombre("natural32".into())));
    assert_eq!(p.medida_de(&t), Some(8), "es una direccion");
    assert_eq!(p.elemento(&t).unwrap().1, 4, "y dentro hay cuatros");
}

/// ** Indexar un `bufer` pide `crudo`, y aqui esta el motivo entero.
///
/// No lleva su longitud dentro, asi que **no hay contra que comprobar el
/// indice**. No es que la comprobacion se haya olvidado: no existe la
/// informacion para hacerla.
///
/// `lista de <tipo>` si la lleva, y por eso esa no pide `crudo` -- pero es de
/// `pleno`. La misma regla de siempre: al otro lado, hay alguien que comprueba?
#[test]
fn indexar_un_bufer_fuera_de_crudo_se_denuncia() {
    let c = codigos_de(&format!(
        "{}funcion f(a es bufer de natural32) devuelve natural32\n    devuelve a[0]\n",
        CABECERA
    ));
    assert_eq!(c, vec!["E0072"]);
}

#[test]
fn dentro_de_crudo_el_indice_vale() {
    let c = codigos_de(&format!(
        "{}funcion f(a es bufer de natural32) devuelve natural32\n    crudo\n        devuelve a[0]\n",
        CABECERA
    ));
    assert!(c.is_empty(), "{:?}", c);
}

#[test]
fn indexar_algo_que_no_es_bufer_se_denuncia() {
    let c = codigos_de(&format!(
        "{}funcion f(a es entero64) devuelve entero64\n    crudo\n        devuelve a[0]\n",
        CABECERA
    ));
    assert_eq!(c, vec!["E0120"]);
}

// ===================================================================
//  El perfil
// ===================================================================

/// ** En `pleno` este modulo NO trabaja, y no es una excepcion comoda.
///
/// Alli un campo no es "una direccion mas un desplazamiento fijo": un `texto`
/// crece, lo que se guarda es una referencia, y ese modelo no esta construido.
/// Medir un registro de `pleno` con las reglas de `llano` daria dos cosas y las
/// dos malas: denunciar `nombre es texto` --que alli es correcto-- o inventarle
/// una disposicion que luego no sera la suya.
#[test]
fn en_pleno_no_se_mide_nada_todavia() {
    let c = codigos_de("perfil pleno\n\nregistro Alumno\n    nombre es texto\n    nota es numero\n");
    assert!(c.is_empty(), "{:?}", c);
}

//! LA SEGUNDA MAQUINA, que todavia no existe -- y por eso hay que sondearla.
//!
//! ## De donde sale este fichero
//!
//! `medidas.toml` lleva escrita una promesa desde el dia que nacio:
//!
//! > *"mientras la disposicion sea una tabla, dos maquinas distintas pueden dar
//! > disposiciones distintas SIN QUE EL COMPILADOR CAMBIE. El dia que INTI
//! > compile para 32 bits, lo que cambia es `puntero = 4` en una tabla."*
//!
//! Eso es **una tesis, no un hecho**, y hasta hoy no la comprobaba nadie. La
//! forma de comprobarla sin tener la segunda maquina es darle al compilador la
//! tabla de una maquina que no existe y mirar si la obedece.
//!
//! ## ** La diferencia con `agnostico.rs`, que es la que importa
//!
//! ```text
//!    agnostico.rs        nadie ESCRIBIO la linea que ataria el compilador
//!    segunda_maquina.rs  y ademas, cambiar la tabla CAMBIA lo que emite
//! ```
//!
//! La primera es una prohibicion y se comprueba leyendo. La segunda es una
//! capacidad y solo se comprueba ejercitandola. Se puede aprobar la primera y
//! suspender la segunda sin contradiccion: bastaria que el compilador leyera la
//! tabla y luego no usara lo leido -- que es exactamente el fallo que este
//! proyecto ya ha visto varias veces, el de la pieza que se calcula bien y no
//! la lee nadie.
//!
//! ## Lo que este fichero NO prueba, dicho por delante
//!
//! Que INTI compile para una maquina de 32 bits. No compila: no hay emisor, ni
//! convencion de llamada, ni marco. Lo que prueba es mas estrecho y es lo unico
//! que se puede saber hoy:
//!
//! > **Las medidas de la maquina entran por la tabla y salen en el resultado.**
//! > Ni una sola de ellas esta escrita en Rust.
//!
//! Portar de verdad seguira siendo trabajo. Lo que esta sonda decide es si ese
//! trabajo sera *escribir un emisor* o *desenterrar ochos repartidos por el
//! compilador* -- que es la diferencia entre un mes y un ano.

use bmo_inti_front::disposicion::{self, Medidas, Plano};
use bmo_inti_front::ir::{self, Instr};
use bmo_inti_front::{lexico, palabras::Vocabulario, sintaxis, tablas};
use bmo_mods::Roots;

// ===================================================================
//  La tabla de la maquina que no existe
// ===================================================================

/// El texto de la tabla **que se entrega**, no una copia dentro del test.
///
/// Copiarla aqui haria la sonda mas comoda y la dejaria sin valor: comprobaria
/// que el compilador obedece a un texto del test, no a la tabla del producto.
fn tabla_real() -> String {
    let p = Roots::find()
        .locate(disposicion::RUTA)
        .unwrap_or_else(|| panic!("no encuentro `{}`", disposicion::RUTA));
    std::fs::read_to_string(p).expect("la tabla no se lee")
}

/// La misma tabla, con las medidas **de la maquina** puestas a 32 bits.
///
/// ** Y solo esas. Si esta funcion tuviera que tocar una linea mas --un tipo con
/// la medida en el nombre, una regla de alineacion, cualquier cosa que no sea
/// una fila de `[bytes_de_esta_maquina]`-- la tesis seria falsa y esta sonda
/// seria la mentira que la tapa.
fn tabla_de_32() -> String {
    cambia(&tabla_real(), &[("puntero", 4), ("bufer", 4)])
}

/// Reescribe filas de la tabla por clave. Los comentarios no se tocan: una
/// linea que empieza por `#` es prosa, y ahi `puntero = 4` aparece **contando**
/// esta misma historia.
fn cambia(tabla: &str, filas: &[(&str, u32)]) -> String {
    let mut fuera = String::new();
    let mut tocadas = 0usize;
    for linea in tabla.lines() {
        let mut puesta = false;
        if !linea.trim_start().starts_with('#') {
            if let Some((cabeza, _)) = linea.split_once('=') {
                for (clave, valor) in filas {
                    if cabeza.trim() == *clave {
                        fuera.push_str(&format!("{} = {}", clave, valor));
                        puesta = true;
                        tocadas += 1;
                        break;
                    }
                }
            }
        }
        if !puesta {
            fuera.push_str(linea);
        }
        fuera.push('\n');
    }
    assert_eq!(
        tocadas,
        filas.len(),
        "una fila que se queria cambiar no esta en la tabla; la sonda estaria \
         midiendo la tabla de 64 contra si misma"
    );
    fuera
}

// ===================================================================
//  Andamio: de fuente a plano, y de fuente a IR, con LA TABLA QUE SE DIGA
// ===================================================================

fn arbol_de(fuente: &str) -> bmo_inti_front::arbol::Modulo {
    let v = Vocabulario::por_defecto().expect("sin vocabulario");
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la sonda no se lee: {}",
        arbol.pintar("sonda.inti")
    );
    arbol.valor
}

fn plano_con(fuente: &str, tabla: &str) -> Plano {
    let m = arbol_de(fuente);
    let c = disposicion::comprobar(&m, Medidas::desde_tabla(tabla));
    assert!(
        !c.hay_errores(),
        "la sonda no puede medir su propio fuente: {:?}",
        c.codigos()
    );
    c.valor
}

fn anchos_leidos(fuente: &str, tabla: &str) -> Vec<u32> {
    let m = arbol_de(fuente);
    let plano = plano_con(fuente, tabla);
    let c = ir::bajar_con(&m, &tablas::Modulos::por_defecto(), &plano, &[], &nec());
    assert!(!c.hay_errores(), "la sonda no baja: {:?}", c.codigos());
    c.valor
        .funciones
        .iter()
        .flat_map(|f| f.instrucciones.iter())
        .filter_map(|i| match i {
            Instr::Lee { ancho, .. } => Some(*ancho),
            _ => None,
        })
        .collect()
}

const CABECERA: &str = "perfil llano\n\n";

// ===================================================================
//  1. La tabla entra
// ===================================================================

/// Lo primero es lo mas tonto y sin ello no vale nada lo demas: que el
/// compilador **lea** la tabla que se le da, y no la suya.
#[test]
fn la_maquina_entra_por_la_tabla() {
    let m = Medidas::desde_tabla(&tabla_de_32());
    assert_eq!(m.de("puntero"), Some(4), "la fila no entro");
    assert_eq!(m.de("bufer"), Some(4));
}

/// ** Y lo que NO se mueve, que es la mitad del diseno.
///
/// `natural64` mide ocho bytes en toda maquina **porque lo dice su nombre**. Si
/// cambiar de maquina le cambiara la medida, INTI no tendria tipos exactos --
/// tendria los `int` de C, que es de lo que se escapo con la regla 9.
#[test]
fn lo_que_dice_su_medida_en_el_nombre_no_lo_mueve_ninguna_maquina() {
    let m = Medidas::desde_tabla(&tabla_de_32());
    assert_eq!(m.de("natural64"), Some(8), "el nombre manda, no la maquina");
    assert_eq!(m.de("entero64"), Some(8));
    assert_eq!(m.de("natural32"), Some(4));
    assert_eq!(m.de("logico"), Some(1));
}

// ===================================================================
//  2. La tabla sale -- LA DISPOSICION
// ===================================================================

/// ** EL CORAZON DE LA SONDA.
///
/// El mismo fuente, dos tablas, dos planos distintos. Y no es solo que los
/// campos se muevan: **cambia la alineacion del registro, y con ella su medida
/// total** -- que es lo que decide si un array de estos cuadra o no.
///
/// ```text
///    registro Enlace          64 bits          32 bits
///       antes  es bufer         @0  (8)          @0  (4)
///       luego  es bufer         @8  (8)          @4  (4)
///       marca  es natural8     @16  (1)          @8  (1)
///                             ---------        ---------
///                             medida 24        medida 12
///                             alinea  8        alinea  4
/// ```
///
/// La mitad. Y ni una linea de Rust cambio entre las dos columnas.
#[test]
fn el_mismo_fuente_da_dos_disposiciones() {
    let fuente = format!(
        "{}registro Enlace\n    antes es bufer de natural8\n    \
         luego es bufer de natural8\n    marca es natural8\n",
        CABECERA
    );

    let de64 = plano_con(&fuente, &tabla_real());
    let r = de64.registro("Enlace").expect("sin registro en 64");
    assert_eq!(r.campo("antes").unwrap().desplazamiento, 0);
    assert_eq!(r.campo("luego").unwrap().desplazamiento, 8);
    assert_eq!(r.campo("marca").unwrap().desplazamiento, 16);
    assert_eq!(r.alineacion, 8);
    assert_eq!(r.medida, 24);

    let de32 = plano_con(&fuente, &tabla_de_32());
    let r = de32.registro("Enlace").expect("sin registro en 32");
    assert_eq!(r.campo("antes").unwrap().desplazamiento, 0);
    assert_eq!(r.campo("luego").unwrap().desplazamiento, 4, "no en el 8");
    assert_eq!(r.campo("marca").unwrap().desplazamiento, 8);
    assert_eq!(r.alineacion, 4, "la alineacion tambien es de la maquina");
    assert_eq!(r.medida, 12, "la mitad, no 24 con huecos");
}

/// Un campo que NO es de la maquina no se mueve por cambiarla, y el de detras
/// tampoco. Es la prueba de que el cambio esta acotado: si al bajar el puntero
/// a 4 se moviera un `entero64`, la tabla no seria una tabla -- seria un
/// interruptor global.
#[test]
fn lo_que_no_es_de_la_maquina_se_queda_donde_estaba() {
    let fuente = format!(
        "{}registro Fijo\n    a es entero64\n    b es natural32\n    c es natural8\n",
        CABECERA
    );
    for tabla in [tabla_real(), tabla_de_32()] {
        let r = plano_con(&fuente, &tabla);
        let r = r.registro("Fijo").unwrap();
        assert_eq!(r.campo("a").unwrap().desplazamiento, 0);
        assert_eq!(r.campo("b").unwrap().desplazamiento, 8);
        assert_eq!(r.campo("c").unwrap().desplazamiento, 12);
        assert_eq!(r.medida, 16);
    }
}

// ===================================================================
//  3. ** Y EL DESCENSO OBEDECE -- que es lo que no se puede leer
// ===================================================================

/// Aqui es donde una sonda floja se habria parado, y donde este proyecto ya se
/// ha quemado: **calcular bien y que no lo lea nadie**.
///
/// Que la disposicion salga distinta no sirve de nada si el descenso emite un
/// `Lee` de ocho bytes de todos modos. Asi que se mira lo que emite.
///
/// `xs es bufer de bufer de natural8` -- indexarlo lee **un puntero**, y un
/// puntero mide lo que diga la maquina. Ocho en una tabla, cuatro en la otra, y
/// el fuente es el mismo caracter por caracter.
#[test]
fn el_descenso_lee_el_ancho_de_la_maquina_y_no_uno_suyo() {
    let fuente = format!(
        "{}funcion primero(xs es bufer de bufer de natural8) devuelve natural64\n    \
         crudo\n        devuelve xs[0]\n",
        CABECERA
    );

    assert_eq!(
        anchos_leidos(&fuente, &tabla_real()),
        vec![8],
        "en 64 bits se lee un puntero de 8"
    );
    assert_eq!(
        anchos_leidos(&fuente, &tabla_de_32()),
        vec![4],
        "** si esto dice 8, el plano se calculo y no lo leyo nadie"
    );
}

/// Y el paso del array tambien. Leer `xs[1]` en una maquina de 32 tiene que
/// mirar cuatro bytes mas alla, no ocho -- si no, el segundo elemento del array
/// no es el segundo elemento de nada.
///
/// ** Esto NO se comprueba mirando el ancho del `Lee`, que seria el mismo. Se
/// comprueba en la medida del elemento, que es de donde sale el paso.
#[test]
fn el_paso_del_array_es_el_de_la_maquina_cuando_el_elemento_es_suyo() {
    use bmo_inti_front::arbol::Tipo;
    let puntero_a_byte = Tipo::Bufer(Box::new(Tipo::Nombre("natural8".to_string())));
    let de_punteros = Tipo::Bufer(Box::new(puntero_a_byte));

    let fuente = format!("{}registro Vacio\n    x es natural8\n", CABECERA);

    let de64 = plano_con(&fuente, &tabla_real());
    assert_eq!(de64.elemento(&de_punteros).map(|(_, m)| m), Some(8));

    let de32 = plano_con(&fuente, &tabla_de_32());
    assert_eq!(de32.elemento(&de_punteros).map(|(_, m)| m), Some(4));
}

// ===================================================================
//  4. La honestidad de la sonda
// ===================================================================

/// OJO: Una sonda que no puede fallar no vigila nada.
///
/// Esta comprueba lo contrario de lo que comprueban las de arriba: que las dos
/// tablas son **de verdad distintas**. Si algun dia alguien "simplifica"
/// `cambia()` y deja de cambiar nada, las cinco pruebas anteriores seguirian
/// en verde comparando la tabla de 64 consigo misma, y estarian mintiendo las
/// cinco a la vez.
#[test]
fn las_dos_tablas_no_son_la_misma() {
    let a = tabla_real();
    let b = tabla_de_32();
    assert_ne!(a, b, "la sonda se estaria midiendo a si misma");
    assert_eq!(
        Medidas::desde_tabla(&a).de("puntero"),
        Some(8),
        "y la de hoy sigue siendo de 64: si esto cambia, cambio el producto"
    );
}

/// La tabla de necesidades de las pruebas: **la incrustada**.
///
/// ** Y no la del disco a proposito. Una prueba que leyera `$BMO_MODS` diria
/// cosas distintas segun quien la corra, que es justo lo que un test no puede
/// hacer. La que se comprueba contra el disco es otra, y esta declarada aparte.
fn nec() -> bmo_inti_front::necesidades::Necesidades {
    bmo_inti_front::necesidades::Necesidades::por_defecto()
}

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

/// ** El ejemplo dejo de ser `texto` el 2026-08-23, y el cambio ES la noticia.
///
/// `texto` YA MIDE --una referencia-- asi que este modulo no tiene nada que
/// decirle. Quien lo rechaza en `llano` es `perfil`, y con el motivo bueno:
/// *"lo que crece pide memoria, y `llano` no tiene monton"*, en vez de *"no se
/// cuanto mide"*. Son dos frases distintas y solo una manda a hacer algo.
///
/// Lo que sigue sin medida es un nombre que **no existe en ningun sitio**, que
/// es de lo que este aviso hablaba de verdad desde el principio.
#[test]
fn un_campo_que_no_se_puede_medir_se_denuncia() {
    let c = codigos_de(&format!("{}registro Punto\n    x es Inventado\n", CABECERA));
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


// ===================================================================
//  ** F5c -- LA CLASE. De que son los numeros de una operacion.
// ===================================================================

/// Lo que devuelve la primera funcion del fuente, con sus tipos. El andamio de
/// las tres pruebas de abajo.
fn devuelto(fuente: &str) -> (Plano, Expr, HashMap<String, Tipo>) {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(!arbol.hay_errores(), "{}", arbol.pintar("prueba.inti"));
    let plano = comprobar(&arbol.valor, Medidas::por_defecto()).valor;
    let Decl::Funcion(f) = &arbol.valor.declaraciones[0] else {
        panic!("la prueba no encuentra su funcion");
    };
    let tipos = tipos_de(f);
    let Sent::Devuelve { valor: Some(e), .. } = &f.cuerpo[0] else {
        panic!("la prueba no encuentra su `devuelve`");
    };
    (plano, e.clone(), tipos)
}

/// Un literal con punto es de coma flotante y uno sin punto no. Es la respuesta
/// mas simple de las cuatro, y la que sostiene las demas.
#[test]
fn el_punto_es_lo_que_hace_flotante_a_un_literal() {
    let (p, e, t) = devuelto(&format!(
        "{}funcion f devuelve flotante64\n    devuelve 1.5\n",
        CABECERA
    ));
    assert!(p.es_flotante(&e, &t));

    let (p, e, t) = devuelto(&format!(
        "{}funcion f devuelve entero64\n    devuelve 15\n",
        CABECERA
    ));
    assert!(!p.es_flotante(&e, &t));
}

/// ** LA PREGUNTA SE CONTESTA CON EL TIPO ESCRITO, no con el aspecto del valor.
///
/// `a * 2` con `a es flotante64` es de coma flotante aunque el `2` no lleve
/// punto. **Lo decide lo que esta declarado**, que es la unica cosa que se puede
/// leer sin ejecutar nada -- y en un lenguaje que escribe sistema, el ancho de
/// una operacion tiene que estar escrito en algun sitio que se pueda leer.
#[test]
fn el_tipo_declarado_decide_la_clase_y_no_el_literal() {
    let (p, e, t) = devuelto(&format!(
        "{}funcion f(a es flotante64, b es entero64) devuelve flotante64\n    devuelve a * 2\n",
        CABECERA
    ));
    assert!(
        p.es_flotante(&e, &t),
        "`a * 2` con `a` flotante es de coma flotante"
    );
}

#[test]
fn dos_enteros_no_se_vuelven_flotantes_por_dividirse() {
    let (p, e, t) = devuelto(&format!(
        "{}funcion f(a es entero64, b es entero64) devuelve entero64\n    devuelve a / b\n",
        CABECERA
    ));
    assert!(!p.es_flotante(&e, &t));
}

/// ** E0123: LOS BITS SOBRE UN FLOTANTE SE DENUNCIAN.
///
/// Y el motivo no es que falte emitirlos. Es que la pregunta no significa nada:
/// los ocho bytes de un flotante son signo, exponente y mantisa, asi que `f | 1`
/// no enciende el bit de las unidades -- toca el exponente y devuelve un numero
/// que no se parece a ninguno de los dos.
///
/// Sin este aviso, el emisor no tendria que emitir para ese caso, **no emitiria
/// nada**, y el programa compilaria y daria basura. Es el mismo agujero que F5b
/// cerro en los campos, visto en otro sitio.
#[test]
fn los_bits_sobre_un_flotante_se_denuncian() {
    let codigos = codigos_de(&format!(
        "{}funcion f(a es flotante64, b es entero64) devuelve flotante64\n    devuelve a bits_o 1\n",
        CABECERA
    ));
    assert!(codigos.contains(&"E0123"), "no se denuncio: {:?}", codigos);
}

/// Y el cociente entero tampoco, que es el que mas se cuela: `entre` sobre
/// flotantes parece razonable y no lo es.
#[test]
fn el_cociente_entero_sobre_flotantes_tambien_se_denuncia() {
    let codigos = codigos_de(&format!(
        "{}funcion f(a es flotante64, b es entero64) devuelve flotante64\n    devuelve a entre 2\n",
        CABECERA
    ));
    assert!(codigos.contains(&"E0123"), "{:?}", codigos);
}

/// ** Y LO QUE NO SE DENUNCIA, que es la mitad que hace util al aviso.
///
/// Un aviso que salta de mas se desactiva en una semana, y entonces ya no
/// vigila nada. Las cuatro operaciones y las seis comparaciones estan todas en
/// IEEE-754 con su resultado escrito, y ninguna puede quejarse.
#[test]
fn las_operaciones_que_si_existen_no_se_denuncian() {
    for op in ["+", "-", "*", "/", "<", ">", "<=", ">="] {
        let codigos = codigos_de(&format!(
            "{}funcion f(a es flotante64, b es flotante64) devuelve logico\n    devuelve a {} b\n",
            CABECERA, op
        ));
        assert!(
            !codigos.contains(&"E0123"),
            "`{}` se denuncio y no debia: {:?}",
            op,
            codigos
        );
    }
}

/// Y los bits sobre ENTEROS siguen siendo legales, que es lo que el aviso no
/// puede haber roto de paso.
#[test]
fn los_bits_sobre_enteros_siguen_siendo_legales() {
    let codigos = codigos_de(&format!(
        "{}funcion f(a es natural64, b es natural64) devuelve natural64\n    devuelve a bits_o b\n",
        CABECERA
    ));
    assert!(!codigos.contains(&"E0123"), "{:?}", codigos);
}

// -------------------------------------------------------------------
//  Las conversiones, que salen de la misma tabla
// -------------------------------------------------------------------

#[test]
fn la_tabla_dice_cuales_son_conversiones() {
    let m = Medidas::por_defecto();
    assert!(m.es_conversion("flotante64"));
    assert!(m.es_conversion("entero32"));
    assert!(m.es_conversion("natural8"));
    // Un puntero mide, pero no se convierte: no esta en ninguna de las dos
    // listas de `[clase]`, y ese es el criterio.
    assert!(!m.es_conversion("puntero"));
    assert!(!m.es_conversion("texto"));
}

#[test]
fn una_conversion_dice_a_que_clase_va() {
    let m = Medidas::por_defecto();
    assert!(m.es_flotante("flotante64"));
    assert!(!m.es_flotante("entero64"));
    assert!(m.es_entero("natural16"));
    assert!(!m.es_entero("flotante32"));
}

// ===================================================================
//  ** LO QUE CRECE SE MIDE POR REFERENCIA (2026-08-22)
// ===================================================================

use crate::arbol::Tipo;

fn plano_vacio() -> Plano {
    plano_de(&format!("{}funcion f devuelve entero32
    devuelve 0
", CABECERA)).valor
}

/// **Una `lista de T` mide lo que una referencia, no lo que la lista.**
///
/// *** Es la propiedad que hace posible que exista un campo de lista: si midiera
/// lo que la lista, el registro que la contiene cambiaria de tamano cada vez que
/// alguien anadiera un elemento -- y un registro que cambia de tamano no se
/// puede colocar en un marco.
///
/// La lista de verdad vive en el monton, con su contador y su capacidad, y lo
/// que se guarda es DONDE esta. Ver `bmo_abi::dynobj::lista`.
#[test]
fn una_lista_mide_lo_que_una_referencia() {
    let p = plano_vacio();
    let de_enteros = Tipo::Lista(Box::new(Tipo::Nombre("entero32".to_string())));
    let de_flotantes = Tipo::Lista(Box::new(Tipo::Nombre("flotante64".to_string())));

    assert_eq!(p.medida_de(&de_enteros), Some(8));
    // ** Y las dos miden lo MISMO aunque sus elementos midan distinto: eso es
    // exactamente lo que dice que se guarda la direccion y no la cosa.
    assert_eq!(p.medida_de(&de_enteros), p.medida_de(&de_flotantes));
    assert_eq!(p.alineacion_de(&de_enteros), Some(8));
}

/// **Y una `tabla` igual**: lo que crece se guarda donde esta.
#[test]
fn una_tabla_tambien_mide_lo_que_una_referencia() {
    let p = plano_vacio();
    let t = Tipo::Tabla(
        Box::new(Tipo::Nombre("entero32".to_string())),
        Box::new(Tipo::Nombre("entero64".to_string())),
    );
    assert_eq!(p.medida_de(&t), Some(8));
}

/// **`texto` sigue SIN medida, y hoy es la respuesta correcta.**
///
/// *** No es un olvido y es la puerta que falta. `lista` y `tabla` se saben por
/// la FORMA del tipo --son variantes del arbol-- y `texto` es un `Tipo::Nombre`,
/// asi que pide una lista de nombres: la de *"lo que crece se guarda por
/// referencia"*, que hoy vive en `biblioteca.toml` como `tipos_que_crecen`.
///
/// ** Y hasta que `texto` mida, `disposicion` no puede abrir su puerta a
/// `pleno`: la abriria denunciando `nombre es texto`, que alli es correcto. Esta
/// prueba fija el estado real para que el dia que cambie, se vea.
/// *** `texto` MIDE, y mide lo que mide una direccion (2026-08-23).
///
/// La prueba que habia aqui se llamaba `texto_sigue_sin_medida_y_por_eso_la_
/// puerta_de_pleno_no_se_abre` y fijaba el estado CONTRARIO a proposito, para
/// que el dia que cambiara se viera. Es hoy.
///
/// ** Y lo que cambio no es una fila: es de DONDE sale la respuesta. `texto` no
/// tiene --ni va a tener-- una fila en `medidas.toml`; lo que tiene es un sitio
/// en `tipos_que_crecen`, y lo que crece se guarda por referencia. La tabla de
/// medidas sigue diciendo que no lo sabe y el PLANO si. Son dos preguntas
/// distintas, y ahora tienen dos respuestas distintas.
#[test]
fn texto_mide_una_referencia_porque_crece() {
    let p = plano_vacio();
    assert!(p.crece("texto"), "`texto` sale de `tipos_que_crecen`");
    assert_eq!(
        p.medida_de(&Tipo::Nombre("texto".to_string())),
        Some(8),
        "lo que crece se guarda por referencia, y una referencia mide 8"
    );
    assert_eq!(
        p.alineacion_de(&Tipo::Nombre("texto".to_string())),
        Some(8),
        "y se alinea como lo que es: una direccion"
    );

    // [!] Y LA TABLA DE MEDIDAS NO SE ENTERO, que es justo lo que se buscaba:
    // `texto` no mide 8 porque alguien escribiera `texto = 8`, sino porque
    // crece. El dia que esto conteste `Some(8)`, la lista se habra copiado --
    // que es el fallo que se quiso evitar, no un detalle de estilo.
    assert_eq!(Medidas::por_defecto().de("texto"), None);

    // Un bufer mide lo mismo por OTRO motivo: es una direccion cruda, sin
    // cabecera y sin contador. La diferencia entre los dos no es el numero.
    assert_eq!(
        p.medida_de(&Tipo::Bufer(Box::new(Tipo::Nombre("natural8".to_string())))),
        Some(8)
    );

    // Y un nombre que ni crece ni esta en la tabla sigue sin medida: la lista
    // no es un comodin que conteste que si a todo.
    assert_eq!(p.medida_de(&Tipo::Nombre("Inventado".to_string())), None);
}

/// *** LA PUERTA DE `pleno` YA SOLO ESPERA AL DECIMAL (2026-08-23).
///
/// La puerta de `comprobar` lleva su condicion escrita desde que existe, y
/// hasta hoy la condicion era `texto`. Ya no: `texto` mide. Esta prueba fija
/// **cual es la que queda**, para que el dia que tambien caiga no haya que
/// deducirlo leyendo el comentario.
///
/// ** Se mide por las piezas y no abriendo la puerta, a proposito: abrirla para
/// probar que se puede abrir seria abrirla. Lo que se comprueba es que las dos
/// mitades del registro de `pleno` ya saben decir su sitio, y que la que falta
/// es exactamente una.
#[test]
fn la_puerta_de_pleno_ya_solo_espera_al_decimal() {
    let p = plano_vacio();

    // Los tres tipos de `pleno` que crecen: los tres saben medir, y miden lo
    // mismo, que es lo que hace posible un campo de ellos.
    for t in [
        Tipo::Nombre("texto".to_string()),
        Tipo::Lista(Box::new(Tipo::Nombre("entero64".to_string()))),
        Tipo::Tabla(
            Box::new(Tipo::Nombre("texto".to_string())),
            Box::new(Tipo::Nombre("entero64".to_string())),
        ),
    ] {
        assert_eq!(
            p.medida_de(&t),
            Some(8),
            "lo que crece se guarda por referencia: {t:?}"
        );
    }

    // *** Y LA QUE FALTA. `numero` en `pleno` es decimal exacto --coeficiente
    // de 128 bits mas escala-- y esa disposicion no esta decidida: cuantos
    // bytes, en que orden, y donde va el signo.
    //
    // No es un olvido y no se tapa con un numero cualquiera: mientras no se
    // decida, la respuesta honrada es "no lo se".
    assert_eq!(
        p.medida_de(&Tipo::Nombre("numero".to_string())),
        None,
        "el decimal no tiene disposicion todavia, y por eso la puerta sigue \
         cerrada. El dia que la tenga, esto pasa a ser Some(..) y la puerta \
         se abre entera."
    );
    assert_eq!(p.medida_de(&Tipo::Nombre("decimal".to_string())), None);

    // [!] Y `numero` NO crece: no es que pida monton, es que cuesta. Las dos
    // listas contestan preguntas distintas y esta prueba no las confunde.
    assert!(!p.crece("numero"));
}

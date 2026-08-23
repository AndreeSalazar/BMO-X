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
    comprobar(
        &arbol.valor,
        &Catalogo::por_defecto(),
        &maquinas,
        &crate::tablas::Modulos::por_defecto(),
    )
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

/// *** PERO SUS BYTES SI SE PUEDEN LEER, y esa es la linea (2026-08-23).
///
/// Hasta hoy CUALQUIER literal de texto se denunciaba en `llano` con *"lo que
/// crece pide memoria"*. **Y un literal no crece**: es CONGELADO, sus bytes
/// viven en `RoData` con el bit de INMORTAL puesto, y llegar a ellos es una
/// direccion y nada mas.
///
/// ** Era el mismo fallo que se cerro el 22-08 con `PRIMOS = [2, 3, 5]`, un tipo
/// mas alla -- y a los dos se llega igual: con `crudo`.
///
/// La linea que queda trazada:
///
/// ```text
///    lee_natural8("hola" + i)   dentro de `crudo`   ->  VALE
///    saludo = "hola"                                ->  E0070
/// ```
///
/// Lo que no cabe en `llano` no son los bytes: es la VARIABLE. Y que hoy solo se
/// le pueda meter un literal no es una propiedad del tipo `texto`, es una
/// carencia del perfil que manana puede no serlo.
#[test]
fn en_llano_los_bytes_de_un_literal_si_se_pueden_leer() {
    let fuente = concat!(
        "perfil llano\n",
        "usa memoria\n\n",
        "funcion b(i es natural64) devuelve natural64\n",
        "    crudo\n",
        "        devuelve lee_natural8(\"hola\" + i)\n"
    );
    let c = codigos_de(fuente);
    assert!(c.is_empty(), "un literal congelado cabe en `llano`: {c:?}");
}

/// **`numero` en `llano` se rechaza POR LO QUE CUESTA, no por lo que le falta.**
///
/// ## ** Lo que esta prueba corrige, y era un mensaje que mentia
///
/// Hasta el 2026-08-22 esto daba `E0020` --*"hay que decir la medida exacta"*--
/// y el motivo era falso. El maestro tiene decidido desde que se escribio que
/// `numero` es `coeficiente 128b + escala`: **eso es una medida**.
///
/// Lo que pasa en `llano` es otra cosa: una suma decimal cuesta 5-20 veces una
/// entera de 64 bits, y `llano` escribe drivers.
///
/// *** Un error que dice "dime la medida" manda a buscar una medida que no
/// falta. El que dice el precio explica una decision. Y el mensaje **es** la
/// interfaz principal de este lenguaje.
#[test]
fn en_llano_numero_se_rechaza_por_lo_que_cuesta() {
    let c = codigos_de("perfil llano\n\nfuncion cuenta(x es numero) devuelve numero\n    devuelve x\n");
    assert!(c.iter().all(|x| *x == "E0074"), "{:?}", c);
    assert!(!c.is_empty());
}

/// Y el mensaje trae **el precio y la salida**, no una queja.
#[test]
fn el_mensaje_de_numero_dice_el_precio_y_la_salida() {
    let t = comprueba("perfil llano\n\nfuncion f(x es numero)\n    devuelve\n")
        .pintar("prueba.inti");
    assert!(t.contains("E0074"), "{}", t);
    assert!(t.contains("5 y 20"), "no dice el precio:\n{}", t);
    assert!(t.contains("drivers"), "no dice por que aqui no:\n{}", t);
    assert!(t.contains("perfil pleno"), "no dice a donde ir:\n{}", t);
    assert!(
        !t.contains("medida exacta"),
        "sigue mandando a buscar una medida que no falta:\n{}",
        t
    );
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
///
/// ** Lo unico que sale es `E0073`, que no habla del programa sino del
/// COMPILADOR: no sabe bajar `pleno` a bytes todavia. El dia que llegue, esta
/// lista se queda vacia y la prueba falla -- que es como tiene que enterarse.
#[test]
fn en_pleno_todo_eso_vale() {
    let c = codigos_de(
        "perfil pleno\n\n\
         funcion media(notas es lista de numero) devuelve numero\n\
         \x20   saludo = \"hola\"\n\
         \x20   devuelve 0\n",
    );
    assert_eq!(c, vec!["E0073"], "en `pleno` no sale nada del PROGRAMA: solo lo que el compilador no sabe hacer todavia");
}

// ===================================================================
//  `crudo`
// ===================================================================

/// OJO: desde el 22-08 TODO fuente de `pleno` trae ademas `E0073` -- el
/// compilador no sabe bajar ese perfil a bytes todavia. Se mira que `E0071`
/// este, no que sea el unico: exigir la lista exacta ataria esta prueba a una
/// limitacion temporal del compilador, que no es lo que prueba.
#[test]
fn crudo_no_existe_en_pleno() {
    let c = codigos_de("perfil pleno\n\nfuncion principal\n    crudo\n        espera()\n");
    assert!(c.contains(&"E0071"), "{:?}", c);
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
    assert!(cat.crece("texto"));
    // ** `numero` ya NO esta en `sin_medida`: esta en `cuestan`, que es su
    // motivo de verdad. La lista vieja se queda vacia y con su sitio hecho.
    assert!(!cat.sin_medida("numero"));
    assert!(cat.cuesta("numero"));
    assert!(cat.cuesta("decimal"));
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
    let c = comprobar(
        &arbol.valor,
        &cat,
        &[],
        &crate::tablas::Modulos::por_defecto(),
    );
    // El texto sigue siendo texto y `llano` sigue sin monton, asi que eso se
    // denuncia igual; lo que no puede pasar es que la tabla rota invente
    // prohibiciones nuevas.
    assert_eq!(c.codigos(), vec!["E0070"]);
}

/// ** Tocar una direccion cruda pide `crudo`, Y SIN NOMBRAR NINGUNA MAQUINA.
///
/// Aqui se ve que la regla no era "lo de la maquina pide crudo". Era:
///
///     al otro lado, hay alguien que comprueba?
///
/// De una direccion cruda no hay nadie -- no hay kernel que valide una
/// capability como en `invoca`, y no hay comprobacion de limites como en un
/// indice, porque no hay lista: hay un numero. Y eso es verdad en toda maquina,
/// asi que la prohibicion viaja con el modulo que trae el nombre y no con la
/// arquitectura.
#[test]
fn escribir_en_una_direccion_fuera_de_crudo_se_denuncia() {
    let c = codigos_de(
        "perfil llano
usa memoria

funcion principal
    escribe_natural64(0x200000, 1)
",
    );
    assert_eq!(c, vec!["E0072"]);
}

#[test]
fn leer_una_direccion_fuera_de_crudo_tambien() {
    let c = codigos_de(
        "perfil llano
usa memoria

funcion lee devuelve natural64
    devuelve lee_natural64(0x200000)
",
    );
    assert_eq!(c, vec!["E0072"]);
}

#[test]
fn dentro_de_crudo_la_memoria_vale() {
    let c = codigos_de(
        "perfil llano
usa memoria

         funcion principal
             crudo
                 escribe_natural64(0x200000, 1)
",
    );
    assert!(c.is_empty(), "{:?}", c);
}

// ===================================================================
//  ** LA COSTURA -- de que fichero es la linea que se acusa
// ===================================================================

/// **Un fallo que vive en una pieza traida NO acusa al fichero del usuario.**
///
/// ## Que se rompia, medido
///
/// `armar` mete las declaraciones de las piezas en el mismo modulo que las del
/// usuario. Sin costuras, este analisis --que corre sobre el arbol ya
/// fusionado-- no puede distinguirlas, y el aviso salia asi:
///
/// ```text
///    E0070 En el perfil `llano` no se puede usar `texto`.
///       en usuario.inti, linea 3:        <- y la linea 3 estaba EN BLANCO
/// ```
///
/// *** El mensaje de cuatro partes tiene un hueco para el DONDE. Un donde que
/// senala a otro fichero no es un detalle de formato: es la parte del mensaje
/// que decide a que fichero va a mirar quien lo lee.
#[test]
fn un_fallo_de_una_pieza_dice_de_que_pieza_es() {
    let v = Vocabulario::por_defecto().unwrap();
    // Lo que escribio el usuario: `llano` y limpio.
    let mio = lexico::barrer("perfil llano

funcion principal
    devuelve 0
", &v);
    let mut arbol = sintaxis::leer(&mio.valor, &v);
    // Lo que traeria un `usa`: se declara `pleno` y usa `texto`.
    let suyo = lexico::barrer("perfil pleno

funcion saluda(a es texto)
    devuelve a
", &v);
    let mut pieza = sintaxis::leer(&suyo.valor, &v);

    // La misma fusion que hace `armar`, con su costura.
    let desde = arbol.valor.declaraciones.len();
    arbol.valor.declaraciones.append(&mut pieza.valor.declaraciones);
    let hasta = arbol.valor.declaraciones.len();
    arbol.valor.piezas.push(crate::arbol::Pieza {
        fichero: "saludos/cortesia.inti".to_string(),
        usa: "saludos".to_string(),
        perfil: pieza.valor.perfil,
        desde,
        hasta,
    });

    let c = comprobar(
        &arbol.valor,
        &Catalogo::por_defecto(),
        &[],
        &crate::tablas::Modulos::por_defecto(),
    );
    let texto = c.pintar("usuario.inti");
    assert!(
        texto.contains("saludos/cortesia.inti"),
        "el aviso no dice de que pieza es:
{}",
        texto
    );
    assert!(
        texto.contains("`usa saludos`"),
        "el aviso no dice quien la trajo:
{}",
        texto
    );
    assert!(
        !texto.contains("en usuario.inti, linea"),
        "el aviso sigue acusando al fichero del usuario:
{}",
        texto
    );
}

/// **La pieza se lleva escrito el perfil que declaro para si misma.**
///
/// ** Hoy no se juzga, y por eso hace falta comprobar que se GUARDA: un dato
/// que nadie mira todavia es exactamente el que se pierde en la siguiente
/// refactorizacion. La regla del mezclado se escribe encima de esto.
#[test]
fn la_costura_recuerda_el_perfil_que_declaro_la_pieza() {
    let v = Vocabulario::por_defecto().unwrap();
    let mio = lexico::barrer("perfil llano

funcion principal
    devuelve 0
", &v);
    let mut arbol = sintaxis::leer(&mio.valor, &v);
    let suyo = lexico::barrer("perfil pleno

funcion dos devuelve entero32
    devuelve 2
", &v);
    let mut pieza = sintaxis::leer(&suyo.valor, &v);

    let desde = arbol.valor.declaraciones.len();
    arbol.valor.declaraciones.append(&mut pieza.valor.declaraciones);
    let hasta = arbol.valor.declaraciones.len();
    arbol.valor.piezas.push(crate::arbol::Pieza {
        fichero: "x/y.inti".to_string(),
        usa: "x".to_string(),
        perfil: pieza.valor.perfil,
        desde,
        hasta,
    });

    // El modulo dice `llano` y lleva dentro un trozo que se declaro `pleno`.
    assert_eq!(arbol.valor.perfil, Perfil::Llano);
    assert_eq!(arbol.valor.piezas[0].perfil, Perfil::Pleno);
    // Y sabe de que trozo es cada declaracion.
    assert!(arbol.valor.pieza_de(desde).is_some(), "la traida no tiene pieza");
    assert!(
        arbol.valor.pieza_de(0).is_none(),
        "la del usuario no puede tener pieza"
    );
}

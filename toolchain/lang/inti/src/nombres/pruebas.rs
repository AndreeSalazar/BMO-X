//! Pruebas del analisis de nombres.

use super::*;
use crate::{lexico, palabras::Vocabulario, sintaxis};

fn comprueba(fuente: &str) -> Cosecha<()> {
    let v = Vocabulario::por_defecto().unwrap();
    let piezas = lexico::barrer(fuente, &v);
    let arbol = sintaxis::leer(&piezas.valor, &v);
    assert!(
        !arbol.hay_errores(),
        "el fuente de la prueba no se lee: {}",
        arbol.pintar("prueba.inti")
    );
    comprobar(&arbol.valor, &Comun::por_defecto(), &[])
}

fn codigos_de(fuente: &str) -> Vec<&'static str> {
    comprueba(fuente).codigos()
}

fn en_principal(cuerpo: &str) -> Vec<&'static str> {
    let mut f = String::from("perfil pleno\n\nfuncion principal\n");
    for l in cuerpo.lines() {
        f.push_str("    ");
        f.push_str(l);
        f.push('\n');
    }
    codigos_de(&f)
}

// ===================================================================
//  `cambiante`
// ===================================================================

#[test]
fn sin_cambiante_no_se_reasigna() {
    assert_eq!(en_principal("x = 5\nx = 6\n"), vec!["E0030"]);
}

#[test]
fn con_cambiante_si() {
    assert!(en_principal("cambiante x = 5\nx = x + 1\n").is_empty());
}

/// ** UN PARAMETRO DA `E0033`, NO `E0030`, y la diferencia es el consejo.
///
/// Esta prueba exigia `E0030` --el aviso generico de "se fijo y no se puede
/// cambiar"--, y ese aviso manda a *"la linea donde nace, sin `cambiante`"*. La
/// linea donde nace un parametro es la FIRMA, donde no hay ningun `=` que
/// quitar: el consejo era correcto y no se podia seguir, que es la peor clase
/// de mensaje.
///
/// El codigo propio existia desde F0 y el comentario de `declara` prometia
/// usarlo desde F2b. No se usaba porque la ficha no guardaba de donde venia el
/// nombre -- y esta prueba, al exigir el generico, **congelaba el fallo en su
/// sitio**. Lo destapo la matriz del censo: `f04_parametro_fijo` declara
/// `E0033` desde el primer dia y nadie lo comparaba.
#[test]
fn un_parametro_no_se_cambia_dentro() {
    let c = codigos_de(
        "perfil pleno\n\nfuncion suma_uno(x)\n    x = x + 1\n",
    );
    assert_eq!(c, vec!["E0033"]);
}

#[test]
fn un_parametro_cambiante_si() {
    let c = codigos_de(
        "perfil pleno\n\nfuncion suma_uno(cambiante x)\n    x = x + 1\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

// ===================================================================
//  El alcance
// ===================================================================

/// ** Un bloque puede LEER lo de fuera.
#[test]
fn un_bloque_lee_lo_de_fuera() {
    let c = en_principal("limite = 10\nsi limite > 5\n    escribe(limite)\n");
    assert!(c.is_empty(), "{:?}", c);
}

/// Y SI puede escribir un `cambiante` de su funcion. Sin esto, `repite
/// mientras` no podria cambiar su propia condicion y el bucle no terminaria
/// nunca -- lo cazo el censo el 2026-08-19.
#[test]
fn un_bloque_escribe_lo_cambiante_de_su_funcion() {
    let c = en_principal("cambiante total = 0
si cierto
    total = 1
");
    assert!(c.is_empty(), "{:?}", c);

    let c = en_principal("cambiante quedan = cierto
repite mientras quedan
    quedan = falso
");
    assert!(c.is_empty(), "el bucle tiene que poder terminar: {:?}", c);
}

/// Lo que no se puede es escribir lo de FUERA de la funcion: el nivel superior,
/// que esta congelado. Por eso no hacen falta `global` ni `nonlocal`.
#[test]
fn ninguna_funcion_escribe_lo_de_nivel_superior() {
    let c = comprueba(
        "perfil pleno

maximo_notas = 10

funcion principal
    maximo_notas = 20
",
    );
    assert_eq!(c.codigos(), vec!["E0032"]);
    assert!(c.avisos[0].que_habia.contains("congela"));
}

/// ** Una variable puede tapar un nombre de la biblioteca: `cambiante suma = 0`
/// tiene que poder escribirse aunque exista `suma`. Si no, cincuenta nombres
/// comunes dejarian de servir como variables.
#[test]
fn una_variable_puede_taparle_el_nombre_a_la_biblioteca() {
    let c = en_principal("cambiante suma = 0
suma = suma + 1
escribe(suma)
");
    assert!(c.is_empty(), "{:?}", c);
}

/// Lo que no vale es dos veces el mismo nombre en la misma funcion: ahi el
/// lector no sabria cual es cual.
#[test]
fn dos_veces_el_mismo_nombre_en_una_funcion_no() {
    let c = en_principal("cambiante total = 0
si cierto
    cambiante total = 1
");
    assert_eq!(c, vec!["E0030"]);
}

#[test]
fn el_nombre_del_bucle_vive_en_el_bucle() {
    let c = en_principal("para cada n en [1, 2]\n    escribe(n)\n");
    assert!(c.is_empty(), "{:?}", c);
    // Y fuera ya no existe.
    let c = en_principal("para cada n en [1, 2]\n    escribe(n)\nescribe(n)\n");
    assert_eq!(c, vec!["E0110"]);
}

// ===================================================================
//  La biblioteca comun -- la facilidad vive aqui
// ===================================================================

#[test]
fn los_nombres_de_siempre_estan_sin_pedirlos() {
    let c = en_principal(
        "notas = [8, 6, 9]\nescribe(cuenta de notas)\nescribe(media de notas)\nescribe(ordena(notas))\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

#[test]
fn un_nombre_que_no_existe_se_dice() {
    assert_eq!(en_principal("escribe(chocolate)\n"), vec!["E0110"]);
}

/// ** La caracteristica de Rust y Elm que mas se cita, y aqui sale casi gratis
/// porque la biblioteca comun esta en una tabla.
#[test]
fn un_nombre_mal_escrito_sugiere_el_bueno() {
    let c = comprueba("perfil pleno\n\nfuncion principal\n    escrib(\"hola\")\n");
    assert_eq!(c.codigos(), vec!["E0110"]);
    assert_eq!(c.avisos[0].que_hacer, "escribe `escribe`");
}

#[test]
fn tambien_sugiere_un_nombre_del_propio_programa() {
    let c = comprueba(
        "perfil pleno\n\nfuncion principal\n    cantidad = 5\n    escribe(cantidd)\n",
    );
    assert_eq!(c.codigos(), vec!["E0110"]);
    assert!(c.avisos[0].que_hacer.contains("cantidad"));
}

/// ** Existe, pero en el otro perfil. Decirlo asi vale mucho mas que "no se que
/// es": el que escribe ya sabe que existe.
#[test]
fn en_llano_los_de_pleno_se_explican_en_vez_de_negarse() {
    let c = comprueba(
        "perfil llano\n\nfuncion f(x es entero32) devuelve entero32\n    devuelve cuenta(x)\n",
    );
    assert_eq!(c.codigos(), vec!["E0070"]);
    assert!(c.avisos[0].que_paso.contains("existe, pero no en el perfil"));
}

/// Y los que valen en los dos perfiles siguen valiendo en `llano`.
#[test]
fn en_llano_quedan_los_de_los_dos_perfiles() {
    let c = comprueba(
        "perfil llano\n\nfuncion f(a es entero32, b es entero32) devuelve entero32\n\
         \x20   devuelve maximo(a, b)\n",
    );
    assert!(c.codigos().is_empty(), "{:?}", c.codigos());
}

// ===================================================================
//  La tabla
// ===================================================================

#[test]
fn la_tabla_comun_carga() {
    let c = Comun::por_defecto();
    let pleno = c.en(Perfil::Pleno);
    let llano = c.en(Perfil::Llano);
    assert!(pleno.contains(&"escribe"));
    assert!(pleno.contains(&"cuenta"));
    assert!(pleno.contains(&"maximo"));
    assert!(llano.contains(&"maximo"), "las cuentas valen en los dos");
    assert!(!llano.contains(&"escribe"), "escribir pide monton");
    assert!(pleno.len() > llano.len());
}

#[test]
fn la_distancia_de_edicion_cuenta_bien() {
    assert_eq!(distancia("escribe", "escribe"), 0);
    assert_eq!(distancia("escrib", "escribe"), 1);
    assert_eq!(distancia("escrbe", "escribe"), 1);
    assert_eq!(distancia("", "hola"), 4);
}

/// Con nombres cortos, distancia 2 empareja cualquier cosa: `x` y `si` estarian
/// "cerca". El tope se aprieta a proposito.
#[test]
fn no_sugiere_disparates_con_nombres_cortos() {
    let c = comprueba("perfil pleno\n\nfuncion principal\n    escribe(qq)\n");
    assert_eq!(c.codigos(), vec!["E0110"]);
    assert!(
        !c.avisos[0].que_hacer.starts_with("escribe `"),
        "no deberia sugerir nada: {}",
        c.avisos[0].que_hacer
    );
}

// ===================================================================
//  Las tres promesas que faltaban
// ===================================================================

/// ** Ignorar un error es error de COMPILACION. Sin esto, `o si no` seria una
/// costumbre en vez de una regla.
#[test]
fn ignorar_un_error_no_compila() {
    let c = comprueba(
        "perfil pleno\n\n\
         funcion divide(a, b) devuelve numero o error\n\
         \x20   si b = 0\n\
         \x20       falla \"entre cero no\"\n\
         \x20   devuelve a / b\n\
         \n\
         funcion principal\n\
         \x20   divide(10, 0)\n",
    );
    assert_eq!(c.codigos(), vec!["E0060"]);
    assert!(c.avisos[0].que_habia.contains("un error es un DATO"));
}

/// Y mirarlo con `o si no` es exactamente mirarlo.
#[test]
fn con_o_si_no_ya_se_mira() {
    let c = comprueba(
        "perfil pleno\n\n\
         funcion divide(a, b) devuelve numero o error\n\
         \x20   devuelve a / b\n\
         \n\
         funcion principal\n\
         \x20   divide(10, 0) o si no 0\n",
    );
    assert!(c.codigos().is_empty(), "{:?}", c.codigos());
}

/// Las de la BIBLIOTECA tambien: la lista sale de la tabla.
#[test]
fn una_de_la_biblioteca_que_falla_tambien_se_exige() {
    let c = comprueba("perfil pleno\n\nfuncion principal\n    guarda(\"x.txt\", \"hola\")\n");
    assert_eq!(c.codigos(), vec!["E0060"]);
}

/// ** Borrar mientras se itera: el bug que en otros lenguajes se salta un
/// elemento sin avisar. Aqui no compila.
#[test]
fn mutar_la_coleccion_que_se_recorre_no_compila() {
    let c = en_principal("cambiante notas = [1, 2, 3]\npara cada n en notas\n    quita(notas, n)\n");
    assert_eq!(c, vec!["E0050"]);
}

/// Y tocar la del bucle de FUERA desde el de dentro es igual de malo.
#[test]
fn tampoco_la_del_bucle_de_fuera() {
    let c = en_principal(
        "cambiante a = [1]\ncambiante b = [2]\npara cada x en a\n    para cada y en b\n        agrega(a, 1)\n",
    );
    assert!(c.contains(&"E0050"), "{:?}", c);
}

/// Modificar OTRA coleccion mientras se recorre una vale.
#[test]
fn modificar_otra_coleccion_si_vale() {
    let c = en_principal(
        "notas = [1, 2]\ncambiante buenas = []\npara cada n en notas\n    agrega(buenas, n)\n",
    );
    assert!(c.is_empty(), "{:?}", c);
}

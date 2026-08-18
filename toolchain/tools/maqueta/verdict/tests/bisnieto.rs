//! Lo que el bisnieto considera mal, y por que lo dice asi.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_layout::lay;
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

/// Compila hasta el final y devuelve el veredicto YA RENDERIZADO, con los
/// espacios colapsados: el formato lo prueba `diag`, aqui se juzgan LAS PALABRAS.
fn veredicto(src: &str) -> String {
    let doc = parse(src.as_bytes()).unwrap_or_else(|e| {
        panic!("el padre tenia que aceptarlo:\n{}", render("t.maqueta", src.as_bytes(), &e))
    });
    let c = cascade(&doc).unwrap_or_else(|e| {
        panic!("el hijo tenia que aceptarlo:\n{}", render("t.maqueta", src.as_bytes(), &e))
    });
    let v = judge(&lay(&c), &c);
    render("t.maqueta", src.as_bytes(), &v)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn limpio(src: &str) {
    let v = veredicto(src);
    assert!(v.is_empty(), "esto tenia que salir limpio:\n{v}");
}

// ------------------------------------------------------------------------
//  ★★ La que mas vale
// ------------------------------------------------------------------------

#[test]
fn el_texto_que_no_cabe_se_caza_con_los_dos_numeros() {
    // La unica clase de fallo de este sistema que se ve BONITA en pantalla y
    // esta mal: un navegador lo esconde reajustando lineas y BMO-X no puede.
    let v = veredicto(
        "<maqueta><span class=\"a\">demasiado largo</span></maqueta>\
         <style>.a{width:40px;height:16px;color:#FFFFFF}</style>",
    );
    assert!(v.contains("el texto no cabe"));
    assert!(v.contains("120 px"), "el ancho que pide:\n{v}");
    assert!(v.contains("40 px"), "y el que hay:\n{v}");
    assert!(v.contains("BONITO"), "y por que importa:\n{v}");
}

#[test]
fn el_texto_que_no_cabe_de_alto_tambien() {
    let v = veredicto(
        "<maqueta><span class=\"a\">x</span></maqueta>\
         <style>.a{width:80px;height:8px;color:#FFFFFF}</style>",
    );
    assert!(v.contains("no cabe de alto"));
}

#[test]
fn un_texto_que_cabe_justo_no_se_queja() {
    // 5 letras x 8 px = 40. El limite es <=, no <.
    limpio(
        "<maqueta><span class=\"a\">hola!</span></maqueta>\
         <style>.a{width:40px;height:16px;color:#E6EDF6}</style>",
    );
}

// ------------------------------------------------------------------------
//  Cabe todo?
// ------------------------------------------------------------------------

#[test]
fn una_caja_fuera_de_su_padre_sale_con_las_dos_geometrias() {
    let v = veredicto(
        "<maqueta><div class=\"p\"><div class=\"h\"></div></div></maqueta>\
         <style>.p{width:50px;height:50px} .h{width:200px;height:10px}</style>",
    );
    assert!(v.contains("se sale de su padre"));
    assert!(v.contains("mide 200x10"), "el rect real:\n{v}");
    assert!(v.contains("no recorta"), "y por que no hay overflow:\n{v}");
}

#[test]
fn una_caja_de_tamano_cero_se_caza_porque_no_se_puede_ver() {
    let v = veredicto("<maqueta><div class=\"a\"></div></maqueta><style>.a{height:0}</style>");
    assert!(v.contains("no se va a ver"));
    assert!(v.contains("propiedad que se olvido"));
}

#[test]
fn una_caja_absoluta_se_juzga_contra_el_lienzo_y_no_contra_su_padre() {
    let v = veredicto(
        "<maqueta ancho=\"100\" alto=\"100\"><div class=\"p\"><div class=\"f\"></div></div></maqueta>\
         <style>.p{width:20px;height:20px} .f{position:absolute;left:90px;top:0;width:50px;height:10px}</style>",
    );
    assert!(v.contains("se sale de el lienzo") || v.contains("se sale de"), "{v}");
}

// ------------------------------------------------------------------------
//  Los nombres responden?
// ------------------------------------------------------------------------

#[test]
fn dos_ids_iguales_porque_un_clic_contestaria_lo_que_no_es() {
    let v = veredicto(
        "<maqueta><div class=\"a\" id=\"k\"></div><div class=\"a\" id=\"k\"></div></maqueta>\
         <style>.a{width:10px;height:10px}</style>",
    );
    assert!(v.contains("el id `k` ya estaba usado"));
    assert!(v.contains("tabla de golpeo"));
}

#[test]
fn una_isla_sin_sitio_cita_la_decision_del_director() {
    let v = veredicto("<maqueta><island nombre=\"v\"></island></maqueta>");
    assert!(v.contains("la isla `v` mide 0x0"));
    assert!(v.contains("lo pone LA MAQUETA"), "{v}");
}

#[test]
fn dos_islas_con_el_mismo_nombre() {
    let v = veredicto(
        "<maqueta><island nombre=\"v\" class=\"a\"></island>\
         <island nombre=\"v\" class=\"a\"></island></maqueta>\
         <style>.a{width:10px;height:10px}</style>",
    );
    assert!(v.contains("la isla `v` ya estaba"));
}

#[test]
fn una_regla_muerta_es_error_en_un_documento_que_maqueta_algo() {
    let v = veredicto(
        "<maqueta><div class=\"a\"></div></maqueta>\
         <style>.a{width:10px;height:10px} .fantasma{gap:0}</style>",
    );
    assert!(v.contains("la regla `.fantasma` no llega a ninguna caja"));
    assert!(v.contains("dice algo y no hace nada"));
}

#[test]
fn una_clase_que_nadie_define_es_el_otro_lado_de_la_misma_errata() {
    let v = veredicto(
        "<maqueta><div class=\"a tecla\"></div></maqueta>\
         <style>.a{width:10px;height:10px}</style>",
    );
    assert!(v.contains("la clase `tecla` no la define ninguna regla"));
}

#[test]
fn una_paleta_sin_cajas_no_se_juzga_por_reglas_sin_usar() {
    // `tema/tema.maqueta` no tiene ni una caja, asi que TODAS sus reglas salen
    // sin usar. Eso no es un hallazgo sobre el fichero: es la consecuencia
    // trivial de no tener cajas. El bisnieto juzga maquetaciones terminadas, y
    // donde no hay maquetacion no hay nada que juzgar.
    limpio(include_str!("../../tema/tema.maqueta"));
}

// ------------------------------------------------------------------------
//  Hay algo escrito que no hace nada?
// ------------------------------------------------------------------------

#[test]
fn gap_en_una_caja_de_bloque_no_hace_nada_y_se_dice() {
    let v = veredicto(
        "<maqueta><div class=\"a\"><div class=\"b\"></div></div></maqueta>\
         <style>.a{gap:8px;width:20px;height:20px} .b{width:5px;height:5px}</style>",
    );
    assert!(v.contains("`gap:8px` aqui no hace nada"));
    assert!(v.contains("alli no te lo dice nadie y aqui si"), "{v}");
}

#[test]
fn una_absoluta_sin_left_ni_top_cae_en_un_cero_que_nadie_eligio() {
    let v = veredicto(
        "<maqueta ancho=\"100\" alto=\"100\"><div class=\"f\"></div></maqueta>\
         <style>.f{position:absolute;width:10px;height:10px}</style>",
    );
    assert!(v.contains("tiene que decir `left` y `top`"));
    assert!(v.contains("no es una decision de nadie"));
}

#[test]
fn un_texto_sin_color_apunta_a_la_paleta() {
    // El precio de no tener herencia, cobrado aqui en vez de pintando de un
    // color que nadie eligio.
    let v = veredicto("<maqueta><span class=\"a\">hola</span></maqueta><style>.a{width:80px;height:16px}</style>");
    assert!(v.contains("este texto no tiene color"));
    assert!(v.contains("tema.maqueta"), "hay que decir donde esta la paleta:\n{v}");
    assert!(v.contains("herencia de ninguna parte"));
}

// ------------------------------------------------------------------------
//  ★★ La calculadora entera, juzgada
// ------------------------------------------------------------------------

#[test]
fn la_calculadora_pasa_el_veredicto_entera() {
    // 322x446, cinco filas de cuatro, diecisiete ids, diecisiete textos con su
    // color, y ni una caja fuera de sitio. Es la prueba de que las diez
    // comprobaciones no son un muro: un fichero escrito con cuidado las pasa.
    limpio(include_str!("../../pruebas/calc.maqueta"));
}

#[test]
fn un_solo_cambio_en_la_calculadora_la_rompe_y_se_ve_donde() {
    // [!] Ensanchar las teclas a secas NO rompe nada, y esa fue mi primera
    // version de esta prueba: el panel no declara tamano, asi que crece con
    // ellas y todo sigue encajando. Es correcto, y util de saber -- una
    // maquetacion que se deduce del arbol se defiende sola.
    //
    // Lo que si rompe es lo de siempre: un tamano CLAVADO y un contenido que
    // crece por debajo. Con el lienzo fijo en los 322x446 que hoy calcula el
    // compilador, unas teclas de 96 px ya no caben en su fila.
    let roto = include_str!("../../pruebas/calc.maqueta")
        .replace("<maqueta>", "<maqueta ancho=\"322\" alto=\"446\">")
        .replace("width:72px", "width:96px");
    let v = veredicto(&roto);
    assert!(v.contains("se sale de su padre"), "{v}");
    // Caza exactamente la CUARTA tecla de cada fila, que es la unica que se
    // sale, con su rect y el sitio que habia. El alto sigue siendo 72: solo se
    // ensancharon.
    assert!(v.contains("mide 96x72"), "y dice cual y cuanto:\n{v}");
    assert!(v.contains("k_sub"), "con el id de la que sobra:\n{v}");
    assert!(v.contains("(8, 54) a (314, 126)"), "y el sitio que habia:\n{v}");
}

// ------------------------------------------------------------------------
//  El comportamiento del juez
// ------------------------------------------------------------------------

#[test]
fn todos_los_reparos_llevan_sus_dos_notas() {
    for malo in [
        "<maqueta><span class=\"a\">largo de mas</span></maqueta><style>.a{width:8px;height:16px;color:#FFFFFF}</style>",
        "<maqueta><div class=\"a\"></div></maqueta><style>.a{height:0}</style>",
        "<maqueta><island nombre=\"v\"></island></maqueta>",
        "<maqueta><div class=\"a\"><div class=\"b\"></div></div></maqueta><style>.a{gap:8px;width:20px;height:20px} .b{width:5px;height:5px}</style>",
    ] {
        let v = veredicto(malo);
        assert!(!v.is_empty(), "tenia que quejarse: {malo}");
        assert!(v.contains("= por que:"), "sin razon:\n{v}");
        assert!(v.contains("= en su lugar:"), "sin salida:\n{v}");
    }
}

#[test]
fn los_reparos_salen_en_orden_de_fichero() {
    let v = veredicto(
        "<maqueta><div class=\"a\" id=\"k\"></div><div class=\"a\" id=\"k\"></div>\
         <island nombre=\"v\"></island></maqueta>\
         <style>.a{width:10px;height:10px}</style>",
    );
    assert!(v.find("el id `k`").unwrap() < v.find("la isla `v`").unwrap());
}

#[test]
fn una_maquetacion_sana_no_da_ni_un_reparo() {
    limpio(
        "<maqueta><div class=\"f\"><span class=\"t\">ok</span><island nombre=\"v\" class=\"i\"></island></div></maqueta>\
         <style>.f{display:flex;gap:6px;width:200px;height:40px} \
         .t{width:40px;height:16px;color:#E6EDF6} .i{width:100px;height:30px}</style>",
    );
}

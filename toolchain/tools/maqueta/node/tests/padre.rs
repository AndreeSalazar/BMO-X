//! What the father accepts, and what it refuses to name.
//!
//! These go through the public `parse` rather than the internals on purpose: if
//! the API is awkward to test, the generation above will find it awkward to use.

use bmo_maqueta_diag::render;
use bmo_maqueta_node::{parse, Keyword, Prop, Selector, Tag, Value};

/// Every rejection is checked through the RENDERED message, not the enum. The
/// message is the product -- a correct error nobody can read is a wall.
///
/// [!] Whitespace is collapsed before the check, and that is not laziness: the
/// renderer wraps notes at 66 columns, so asserting on a phrase that happens to
/// straddle a line break fails for a message that is perfectly correct. It bit
/// twice while writing these. Layout of the message is `diag`'s business and is
/// tested there; here what is on trial is the WORDS.
fn errs(src: &str) -> String {
    let raw = match parse(src.as_bytes()) {
        Ok(_) => panic!("esto tenia que fallar:\n{src}"),
        Err(e) => render("t.maqueta", src.as_bytes(), &e),
    };
    raw.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn ok(src: &str) -> bmo_maqueta_node::Document {
    match parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    }
}

// ------------------------------------------------------------------------
//  What it names
// ------------------------------------------------------------------------

const PAD: &str = r#"
<maqueta>
  <style>
    div    { background-color:#182434 }
    .pad   { display:flex; flex-direction:column; gap:6px; padding:6px }
    .fila  { display:flex; flex-direction:row; gap:6px }
    .tecla { width:72px; height:72px; background-color:#2B3B52; color:#E6EDF6 }
  </style>
  <div class="pad">
    <div class="fila">
      <span class="tecla" id="k_7">7</span>
      <span class="tecla" id="k_8">8</span>
    </div>
    <island nombre="vitals"></island>
  </div>
</maqueta>
"#;

#[test]
fn a_whole_document_gets_named() {
    let d = ok(PAD);
    assert_eq!(d.root.tag, Tag::Maqueta);
    assert_eq!(d.rules.len(), 4);

    let pad = &d.root.children[0];
    assert_eq!(pad.classes, ["pad"]);
    let fila = &pad.children[0];
    assert_eq!(fila.children.len(), 2);
    assert_eq!(fila.children[0].text.as_deref(), Some("7"));
    assert_eq!(fila.children[0].id.as_deref(), Some("k_7"));
    assert_eq!(pad.children[1].island.as_deref(), Some("vitals"));
}

#[test]
fn values_come_out_as_integers_and_words_not_text() {
    let d = ok(PAD);
    let tecla = d
        .rules
        .iter()
        .find(|r| r.selectors == [Selector::Class("tecla".into())])
        .unwrap();
    assert!(tecla.decls.iter().any(|d| d.prop == Prop::Width && d.value == Value::Px(72)));
    assert!(tecla
        .decls
        .iter()
        .any(|d| d.prop == Prop::BackgroundColor && d.value == Value::Color(0x2B3B52)));

    let pad = d
        .rules
        .iter()
        .find(|r| r.selectors == [Selector::Class("pad".into())])
        .unwrap();
    assert!(pad
        .decls
        .iter()
        .any(|d| d.prop == Prop::Display && d.value == Value::Word(Keyword::Flex)));
}

#[test]
fn a_tag_selector_is_a_tag_and_not_a_string() {
    let d = ok(PAD);
    assert_eq!(d.rules[0].selectors, [Selector::Tag(Tag::Div)]);
}

#[test]
fn one_padding_value_means_four_sides() {
    let d = ok("<maqueta><div class=\"a\"></div></maqueta>\n<style>.a{padding:6px}</style>");
    assert_eq!(d.rules[0].decls[0].value, Value::Px4([6, 6, 6, 6]));
}

#[test]
fn four_padding_values_keep_the_css_order() {
    let d = ok("<maqueta><div class=\"a\"></div></maqueta>\n<style>.a{padding:1px 2px 3px 4px}</style>");
    assert_eq!(d.rules[0].decls[0].value, Value::Px4([1, 2, 3, 4]));
}

#[test]
fn zero_needs_no_unit() {
    let d = ok("<maqueta><div class=\"a\"></div></maqueta>\n<style>.a{gap:0}</style>");
    assert_eq!(d.rules[0].decls[0].value, Value::Px(0));
}

#[test]
fn the_canvas_size_is_optional_and_read_when_given() {
    let d = ok("<maqueta ancho=\"322\" alto=\"446\"><div></div></maqueta>");
    assert_eq!((d.root.width, d.root.height), (Some(322), Some(446)));

    let d = ok("<maqueta><div></div></maqueta>");
    assert_eq!((d.root.width, d.root.height), (None, None));
}

#[test]
fn comments_and_indentation_do_not_become_content() {
    let d = ok("<maqueta>\n  <!-- una nota -->\n  <div>\n  </div>\n</maqueta>");
    assert_eq!(d.root.children.len(), 1);
    assert_eq!(d.root.children[0].text, None);
}

// ------------------------------------------------------------------------
//  What it cannot name -- and it is the MESSAGE that is on trial
// ------------------------------------------------------------------------

#[test]
fn a_tag_that_promises_semantics_says_so() {
    let e = errs("<maqueta><h1>Hola</h1></maqueta>");
    assert!(e.contains("etiqueta no soportada -- `<h1>`"));
    assert!(e.contains("PROMETE semantica"));
    assert!(e.contains("`<div>` o `<span>`"), "tiene que dar la salida:\n{e}");
}

#[test]
fn an_invented_tag_gets_the_closed_list() {
    let e = errs("<maqueta><caja></caja></maqueta>");
    assert!(e.contains("etiqueta no soportada -- `<caja>`"));
    assert!(e.contains("CERRADA"));
}

#[test]
fn a_real_css_property_gets_a_real_reason_and_a_way_out() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{box-shadow:0 2px}</style>");
    assert!(e.contains("propiedad no soportada -- `box-shadow`"));
    assert!(e.contains("mezcla alfa"), "tiene que decir POR QUE:\n{e}");
    assert!(e.contains("escalon 4"));
    assert!(e.contains("dos `<div>`"), "y que hacer en su lugar:\n{e}");
}

#[test]
fn the_font_rejection_says_the_limit_is_the_foundation_not_a_gap() {
    // This one matters: someone will read "no font-size" as poverty, when it is
    // the reason compile-time text measurement is possible at all.
    let e = errs("<maqueta><div></div></maqueta><style>.a{font-size:12px}</style>");
    assert!(e.contains("ancho fijo"));
    assert!(e.contains("cimiento"), "no puede sonar a carencia:\n{e}");
}

#[test]
fn an_invented_property_gets_the_closed_list() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{ancho-total:4px}</style>");
    assert!(e.contains("propiedad no soportada -- `ancho-total`"));
    assert!(e.contains("diecisiete"));
}

#[test]
fn a_percentage_is_refused_with_the_reason_that_chose_it() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{width:50%}</style>");
    assert!(e.contains("unidad no soportada -- `%`"));
    assert!(e.contains("no sabe que tiene padre"));
    assert!(e.contains("`gap`"));
}

#[test]
fn em_and_rem_are_refused_by_name() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{width:2em}</style>");
    assert!(e.contains("unidad no soportada -- `em`"));
    assert!(e.contains("tipografia"));
}

#[test]
fn a_measure_without_a_unit_is_caught() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{width:72}</style>");
    assert!(e.contains("falta la unidad"));
    assert!(e.contains("72px"));
}

#[test]
fn inherit_is_caught_as_a_value_not_as_a_property() {
    // It is a VALUE, so a table keyed by property name would never see it, and
    // the author would get "quiere un color" and go looking in the wrong place.
    let e = errs("<maqueta><div></div></maqueta><style>.a{color:inherit}</style>");
    assert!(e.contains("`inherit` no existe"));
    assert!(e.contains("no sabe que tiene padre"));
}

#[test]
fn an_id_selector_says_why_id_is_reserved() {
    let e = errs("<maqueta><div></div></maqueta><style>#pad{gap:6px}</style>");
    assert!(e.contains("los selectores de id no existen"));
    assert!(e.contains("tabla de golpeo"));
}

#[test]
fn a_descendant_selector_names_l7_and_offers_the_comma() {
    let e = errs("<maqueta><div></div></maqueta><style>.panel .boton{gap:6px}</style>");
    assert!(e.contains("los selectores de descendencia no existen"));
    assert!(e.contains("ancestros"));
    assert!(e.contains("`,`"));
}

#[test]
fn a_word_outside_its_list_gets_the_list() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{flex-direction:diagonal}</style>");
    assert!(e.contains("no es un valor de `flex-direction`"));
    assert!(e.contains("`row`") && e.contains("`column`"));
}

#[test]
fn a_colour_word_is_refused_with_the_palette_pointed_at() {
    let e = errs("<maqueta><div></div></maqueta><style>.a{color:red}</style>");
    assert!(e.contains("quiere un color `#RRGGBB`"));
    assert!(e.contains("tema.maqueta"), "hay que decir donde esta la paleta:\n{e}");
}

// ------------------------------------------------------------------------
//  Shape of the document
// ------------------------------------------------------------------------

#[test]
fn a_tag_left_open_is_not_closed_for_you() {
    let e = errs("<maqueta><div></maqueta>");
    assert!(e.contains("se cierra") || e.contains("no se cerro"));
    assert!(e.contains("L7") || e.contains("navegador"));
}

#[test]
fn badly_nested_tags_are_not_reordered() {
    let e = errs("<maqueta><div><span></div></span></maqueta>");
    assert!(e.contains("se cierra `</div>` pero lo abierto es `<span>`"));
}

#[test]
fn text_and_boxes_in_the_same_tag_are_refused() {
    let e = errs("<maqueta><div>Total: <span>0</span></div></maqueta>");
    assert!(e.contains("mezclar texto y cajas"));
    assert!(e.contains("flujo en linea"));
}

#[test]
fn an_island_holds_nothing() {
    let e = errs("<maqueta><island nombre=\"v\"><div></div></island></maqueta>");
    assert!(e.contains("no puede llevar cajas dentro"));
    assert!(e.contains("OTRO proceso"));
}

#[test]
fn the_root_has_to_be_maqueta() {
    let e = errs("<div><span>hola</span></div>");
    assert!(e.contains("la raiz es `<div>`"));
}

#[test]
fn two_style_blocks_are_refused_because_order_would_decide() {
    let e = errs("<maqueta><style>.a{gap:1px}</style><style>.a{gap:2px}</style><div></div></maqueta>");
    assert!(e.contains("un solo bloque `<style>`") || e.contains("solo puede haber un bloque"));
}

#[test]
fn an_unknown_attribute_is_refused_because_a_silent_one_would_lie() {
    let e = errs("<maqueta><div style=\"gap:6px\"></div></maqueta>");
    assert!(e.contains("atributo no soportado -- `style`"));
    assert!(e.contains("parece hacer algo y no hace nada"));
}

#[test]
fn an_attribute_without_a_value_is_refused() {
    let e = errs("<maqueta><div hidden></div></maqueta>");
    assert!(e.contains("no tiene valor") || e.contains("atributo no soportado"));
}

#[test]
fn nombre_belongs_to_islands_only() {
    let e = errs("<maqueta><div nombre=\"v\"></div></maqueta>");
    assert!(e.contains("`nombre` es solo de `<island>`"));
}

#[test]
fn the_canvas_size_belongs_to_the_root_only() {
    let e = errs("<maqueta><div ancho=\"10\"></div></maqueta>");
    assert!(e.contains("`ancho` es solo de `<maqueta>`"));
    assert!(e.contains("`width`"));
}

#[test]
fn non_ascii_is_refused_with_the_bex_that_grew() {
    // Bytes as escapes, so this test file does not itself break the rule it is
    // testing. \xC3\xB1 is an n with a tilde in UTF-8.
    let src = b"<maqueta><div>ma\xC3\xB1ana</div></maqueta>";
    let e = parse(src).unwrap_err();
    let msg = render("t.maqueta", src, &e);
    assert!(msg.contains("byte fuera de ASCII"));
    assert!(msg.contains("492.032"), "el numero es lo que convence:\n{msg}");
}

// ------------------------------------------------------------------------
//  Behaviour of the compiler itself
// ------------------------------------------------------------------------

#[test]
fn five_mistakes_cost_one_compilation_not_five() {
    let src = "<maqueta><h1></h1><div style=\"x\"></div></maqueta>\
               <style>.a{width:50%;box-shadow:0;color:red}</style>";
    let e = parse(src.as_bytes()).unwrap_err();
    assert!(e.len() >= 5, "se esperaban cinco o mas, hubo {}", e.len());
}

#[test]
fn one_broken_rule_does_not_poison_the_ones_after_it() {
    let src = "<maqueta><div class=\"b\"></div></maqueta>\
               <style>.a{width:50%} .b{gap:6px}</style>";
    let e = parse(src.as_bytes()).unwrap_err();
    // One complaint about the percentage, and nothing about `.b`.
    assert_eq!(e.len(), 1, "{}", render("t.maqueta", src.as_bytes(), &e));
}

#[test]
fn every_message_carries_both_notes() {
    // The contract says a rejection shows the way out. This checks the whole
    // corpus at once rather than trusting each test above.
    for bad in [
        "<maqueta><h1></h1></maqueta>",
        "<maqueta><div></div></maqueta><style>.a{width:50%}</style>",
        "<maqueta><div></div></maqueta><style>#x{gap:0}</style>",
        "<maqueta><div></div></maqueta><style>.a{box-shadow:0}</style>",
        "<maqueta><div>a<span>b</span></div></maqueta>",
    ] {
        let msg = errs(bad);
        assert!(msg.contains("= por que:"), "sin razon:\n{msg}");
        assert!(msg.contains("= en su lugar:"), "sin salida:\n{msg}");
    }
}

#[test]
fn nothing_here_panics_however_broken_the_input() {
    for bad in [
        "", "<", "</", "<>", "<maqueta", "<style>", "<style>.a{", "<maqueta></div>",
        "<maqueta><div class=></div></maqueta>", "<style>{}</style>", "<style>.a{;;;}</style>",
        "<maqueta><island></island></maqueta>", "<style>.{}</style>", "<style>a b c</style>",
    ] {
        let _ = parse(bad.as_bytes());
    }
}

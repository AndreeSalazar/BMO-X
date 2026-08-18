//! What the son settles, and the one thing it refuses to settle.

use bmo_maqueta_cascade::{cascade, Cascaded, Direction, Display, Justify, Style, Styled};
use bmo_maqueta_diag::render;
use bmo_maqueta_node::parse;

fn run(src: &str) -> Cascaded {
    let doc = match parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => panic!("el padre no lo acepto:\n{}", render("t.maqueta", src.as_bytes(), &e)),
    };
    match cascade(&doc) {
        Ok(c) => c,
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    }
}

fn errs(src: &str) -> String {
    let doc = parse(src.as_bytes()).expect("el padre tenia que aceptarlo");
    let e = cascade(&doc).err().expect("esto tenia que fallar");
    render("t.maqueta", src.as_bytes(), &e)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `<maqueta>` wrapping one styled `<div class="...">`, with the sheet given.
fn one(sheet: &str, classes: &str) -> Style {
    let src = format!("<maqueta><div class=\"{classes}\"></div></maqueta><style>{sheet}</style>");
    run(&src).root.children[0].style
}

// ------------------------------------------------------------------------
//  Matching
// ------------------------------------------------------------------------

#[test]
fn a_class_rule_reaches_its_box() {
    assert_eq!(one(".a{width:72px}", "a").width, Some(72));
}

#[test]
fn a_tag_rule_reaches_every_box_of_that_tag() {
    let c = run("<maqueta><div></div><div></div></maqueta><style>div{gap:6px}</style>");
    assert!(c.root.children.iter().all(|k| k.style.gap == 6));
    // ...and not the root, which is a `maqueta` and not a `div`.
    assert_eq!(c.root.style.gap, 0);
}

#[test]
fn two_classes_on_one_box_both_apply() {
    let s = one(".a{width:10px} .b{height:20px}", "a b");
    assert_eq!((s.width, s.height), (Some(10), Some(20)));
}

#[test]
fn a_comma_group_reaches_both() {
    let c = run("<maqueta><div class=\"a\"></div><span class=\"b\">x</span></maqueta>\
                 <style>.a, .b{gap:4px}</style>");
    assert_eq!(c.root.children[0].style.gap, 4);
    assert_eq!(c.root.children[1].style.gap, 4);
}

#[test]
fn the_last_rule_wins() {
    // The whole of MAQUETA's cascade, in one assertion.
    assert_eq!(one(".a{width:10px} .b{width:20px}", "a b").width, Some(20));
    assert_eq!(one(".b{width:20px} .a{width:10px}", "a b").width, Some(10));
}

#[test]
fn a_class_after_a_tag_wins_and_both_readings_agree() {
    // Order says the class (it is later); specificity says the class (0,1,0
    // beats 0,0,1). Same answer, which is exactly what the guardian protects.
    let src = "<maqueta><div class=\"a\"></div></maqueta>\
               <style>div{width:10px} .a{width:20px}</style>";
    assert_eq!(run(src).root.children[0].style.width, Some(20));
}

// ------------------------------------------------------------------------
//  * The consequence of the law, made observable
// ------------------------------------------------------------------------

#[test]
fn a_child_does_not_inherit_from_its_parent() {
    // This is what "no inheritance" MEANS, and it is the only place it can be
    // seen from outside. The father made it impossible by leaving out `parent`;
    // this is the proof that nothing put it back.
    let c = run("<maqueta><div class=\"a\"><span class=\"b\">x</span></div></maqueta>\
                 <style>.a{color:#FFFFFF} .b{width:8px}</style>");
    let padre = &c.root.children[0];
    let hijo = &padre.children[0];
    assert_eq!(padre.style.color, Some(0xFFFFFF));
    assert_eq!(hijo.style.color, None, "el hijo NO hereda el color");
}

#[test]
fn classes_do_not_survive_the_cascade() {
    // `Styled` has no `classes` field, so this is really a compile-time
    // guarantee -- the class went in and only its effect came out. If classes
    // survived, "which rule wins" would end up with two implementations.
    let c: Styled = run("<maqueta><div class=\"a\"></div></maqueta><style>.a{gap:1px}</style>").root;
    assert_eq!(c.children[0].style.gap, 1);
}

#[test]
fn an_undeclared_width_stays_unsaid_and_is_not_invented() {
    // `None` is not `auto`: it means the file did not say, and what it becomes
    // depends on the boxes around it -- which is the grandson's question.
    let s = one(".a{gap:6px}", "a");
    assert_eq!(s.width, None);
    assert_eq!(s.height, None);
}

#[test]
fn there_is_no_default_text_colour() {
    // A default would be inheritance from nowhere: a value nobody wrote that
    // looks intentional. Text without a colour is a finding for `verdict/`.
    assert_eq!(one("div{gap:0}", "").color, None);
}

#[test]
fn the_defaults_are_the_ones_css_uses() {
    let s = one("div{gap:0}", "");
    assert_eq!(s.display, Display::Block);
    assert_eq!(s.direction, Direction::Row);
    assert_eq!(s.justify, Justify::Start);
    assert_eq!(s.padding, [0, 0, 0, 0]);
    assert_eq!(s.background, None, "una caja sin fondo no pinta nada");
    assert_eq!(s.border_width, 0);
}

#[test]
fn the_canvas_only_appears_when_it_was_declared() {
    assert_eq!(run("<maqueta ancho=\"322\" alto=\"446\"><div></div></maqueta>").root.canvas,
               Some((322, 446)));
    assert_eq!(run("<maqueta><div></div></maqueta>").root.canvas, None);
}

// ------------------------------------------------------------------------
//  The guardian: the preview must not be able to lie
// ------------------------------------------------------------------------

#[test]
fn a_tag_rule_after_a_class_rule_is_refused() {
    let e = errs("<maqueta><div class=\"a\"></div></maqueta>\
                  <style>.a{width:10px} div{width:20px}</style>");
    assert!(e.contains("`div` va despues de `.a`"));
    assert!(e.contains("ESPECIFICIDAD"));
    assert!(e.contains("MIENTE"), "hay que decir que la previsualizacion miente:\n{e}");
    assert!(e.contains("etiqueta arriba"), "y como se arregla:\n{e}");
}

#[test]
fn tag_rules_grouped_at_the_top_are_fine() {
    let c = run("<maqueta><div class=\"a\"></div></maqueta>\
                 <style>div{gap:1px} span{gap:2px} .a{gap:3px}</style>");
    assert_eq!(c.root.children[0].style.gap, 3);
}

#[test]
fn a_mixed_rule_counts_as_a_class_rule() {
    // `div, .a` is scored by its HIGHEST selector, because it is the highest
    // score a browser compares. So a plain tag rule after it is refused.
    let e = errs("<maqueta><div class=\"a\"></div></maqueta>\
                  <style>div, .a{width:10px} span{width:20px}</style>");
    assert!(e.contains("`span` va despues de `div, .a`"));
}

#[test]
fn the_guardian_stops_before_settling_anything() {
    // A stylesheet it cannot read faithfully produces no tree at all, rather
    // than a tree that quietly disagrees with the preview.
    let doc = parse(
        "<maqueta><div class=\"a\"></div></maqueta><style>.a{width:1px} div{width:2px}</style>"
            .as_bytes(),
    )
    .unwrap();
    assert!(cascade(&doc).is_err());
}

// ------------------------------------------------------------------------
//  Findings: facts, not opinions
// ------------------------------------------------------------------------

#[test]
fn a_rule_that_matches_nothing_is_a_finding_and_not_an_error() {
    // It compiles: the cascade computed perfectly well. Whether a dead rule is
    // a typo or a work in progress is a judgement, and judgements are the
    // great-grandson's.
    let c = run("<maqueta><div class=\"a\"></div></maqueta>\
                 <style>.a{gap:1px} .fantasma{gap:2px}</style>");
    assert_eq!(c.dead_rules.len(), 1);
    assert_eq!(c.dead_rules[0].what, ".fantasma");
}

#[test]
fn a_class_no_rule_mentions_is_a_finding_too() {
    let c = run("<maqueta><div class=\"a huerfana\"></div></maqueta><style>.a{gap:1px}</style>");
    assert_eq!(c.orphan_classes.len(), 1);
    assert_eq!(c.orphan_classes[0].what, "huerfana");
}

#[test]
fn a_clean_sheet_leaves_no_findings() {
    let c = run("<maqueta><div class=\"a\"></div></maqueta><style>div{gap:0} .a{gap:1px}</style>");
    assert!(c.dead_rules.is_empty() && c.orphan_classes.is_empty());
}

#[test]
fn a_rule_used_deep_in_the_tree_is_not_reported_dead() {
    let c = run("<maqueta><div><div><span class=\"a\">x</span></div></div></maqueta>\
                 <style>.a{gap:1px}</style>");
    assert!(c.dead_rules.is_empty());
}

#[test]
fn nothing_here_panics_on_a_document_with_no_rules() {
    let c = run("<maqueta><div><span>x</span></div></maqueta>");
    assert_eq!(c.root.children[0].children[0].text.as_deref(), Some("x"));
    assert!(c.dead_rules.is_empty());
}

// ------------------------------------------------------------------------
//  The system palette is not fiction
// ------------------------------------------------------------------------

/// `tema/tema.maqueta` was written by hand before a compiler existed to read it.
///
/// * This is the check that keeps it honest. A palette file nobody compiles is
/// exactly the thing MAQUETA was built to stop existing: twelve lines that look
/// authoritative and might not parse. It costs one test and it can never rot.
#[test]
fn the_system_theme_compiles() {
    let src = include_str!("../../tema/tema.maqueta");
    let c = run(src);

    // Its rules only make sense against markup that wears the classes, so the
    // probe borrows the theme's sheet verbatim and hangs one box off it.
    let body = src
        .split("<style>")
        .nth(1)
        .and_then(|s| s.split("</style>").next())
        .expect("el tema tiene un bloque de estilo");
    let wearing = |class: &str| -> Style {
        run(&format!(
            "<maqueta><div class=\"{class}\"></div></maqueta><style>{body}</style>"
        ))
        .root
        .children[0]
        .style
    };

    // The names that carry the desktop, against the real constants of `scene/`.
    assert_eq!(wearing("ink").color, Some(0xE6EDF6), "INK, 63 usos");
    assert_eq!(wearing("ink-dim").color, Some(0x8A9BB4), "INK_DIM, 127 usos");
    assert_eq!(wearing("ink-ok").color, Some(0x7EE787));
    assert_eq!(wearing("ink-bad").color, Some(0xFF8A7A));
    assert_eq!(wearing("accent").color, Some(0x60A5FA));
    assert_eq!(wearing("field").background, Some(0x161C28));
    assert_eq!(wearing("taskbar").background, Some(0x0F131D));
    assert_eq!(wearing("bg-top").background, Some(0x1B2233));

    let boxed = wearing("box");
    assert_eq!(boxed.background, Some(0x1E2534));
    assert_eq!(boxed.border_color, Some(0x333D52));
    assert_eq!(boxed.border_width, 1);

    // On its own the theme has no markup, so every rule looks unused from here
    // -- which is exactly what a palette meant for OTHER files should look like.
    assert_eq!(c.dead_rules.len(), 9, "las nueve reglas del tema");
    assert!(c.orphan_classes.is_empty());
}

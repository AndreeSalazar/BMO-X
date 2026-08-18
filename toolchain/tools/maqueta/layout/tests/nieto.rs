//! Where the boxes land.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_layout::{lay, Laid, Rect, GLIFO_ALTO, GLIFO_ANCHO};
use bmo_maqueta_node::parse;

fn run(src: &str) -> Laid {
    let doc = match parse(src.as_bytes()) {
        Ok(d) => d,
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    };
    let c = match cascade(&doc) {
        Ok(c) => c,
        Err(e) => panic!("{}", render("t.maqueta", src.as_bytes(), &e)),
    };
    lay(&c)
}

fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
    Rect { x, y, w, h }
}

// ------------------------------------------------------------------------
//  The box model
// ------------------------------------------------------------------------

#[test]
fn width_names_the_content_and_padding_and_border_add_to_it() {
    // CSS's default `content-box`. It is the confusing one of the two and it is
    // chosen anyway, because it is what a browser does when nobody says.
    let l = run("<maqueta><div class=\"a\"></div></maqueta>\
                 <style>.a{width:100px;height:20px;padding:6px;border-width:2px}</style>");
    assert_eq!(l.canvas, (100 + 12 + 4, 20 + 12 + 4));
    assert_eq!(l.root.children[0].rect, r(0, 0, 116, 36));
    assert_eq!(l.root.children[0].content, r(8, 8, 100, 20));
}

#[test]
fn a_canvas_that_was_declared_wins_over_the_content() {
    let l = run("<maqueta ancho=\"400\" alto=\"300\"><div class=\"a\"></div></maqueta>\
                 <style>.a{width:10px;height:10px}</style>");
    assert_eq!(l.canvas, (400, 300));
}

#[test]
fn text_is_measured_by_arithmetic() {
    // The measurement that makes the whole compiler possible.
    let l = run("<maqueta><span>hola</span></maqueta>");
    assert_eq!(l.canvas, (4 * GLIFO_ANCHO, GLIFO_ALTO));
}

// ------------------------------------------------------------------------
//  What an undeclared size becomes -- the son left this to the grandson
// ------------------------------------------------------------------------

#[test]
fn a_block_child_without_a_width_fills_its_container() {
    let l = run("<maqueta ancho=\"200\" alto=\"100\"><div class=\"a\"><div class=\"b\"></div></div></maqueta>\
                 <style>.a{padding:10px} .b{height:5px}</style>");
    let a = &l.root.children[0];
    let b = &a.children[0];
    assert_eq!(a.rect.w, 200, "el bloque exterior llena el lienzo");
    assert_eq!(b.rect.w, 180, "y el interior llena el contenido de su padre");
}

#[test]
fn a_flex_item_without_a_main_size_shrinks_to_its_content() {
    // [!] La regla de etiqueta va ARRIBA. La primera version de esta prueba la
    // puso debajo de `.f` y el guardian del hijo la rechazo -- con razon, y me
    // la cazo a mi antes que a nadie.
    let l = run("<maqueta ancho=\"400\" alto=\"50\"><div class=\"f\"><span>abc</span></div></maqueta>\
                 <style>span{height:16px} .f{display:flex}</style>");
    let item = &l.root.children[0].children[0];
    assert_eq!(item.rect.w, 3 * GLIFO_ANCHO, "se encoge al contenido, no llena");
}

#[test]
fn a_flex_item_without_a_cross_size_stretches_because_css_stretches() {
    // The default that nearly diverged: CSS's `align-items` is `stretch`.
    let l = run("<maqueta><div class=\"f\"><div class=\"i\"></div></div></maqueta>\
                 <style>.f{display:flex;height:80px;width:200px} .i{width:20px}</style>");
    assert_eq!(l.root.children[0].children[0].rect.h, 80);
}

#[test]
fn align_start_leaves_the_item_at_its_own_height() {
    let l = run("<maqueta><div class=\"f\"><div class=\"i\">x</div></div></maqueta>\
                 <style>.f{display:flex;align-items:start;height:80px;width:200px} .i{width:20px}</style>");
    assert_eq!(l.root.children[0].children[0].rect.h, GLIFO_ALTO);
}

// ------------------------------------------------------------------------
//  Flow
// ------------------------------------------------------------------------

#[test]
fn a_flex_row_places_items_with_the_gap_between_them() {
    let l = run("<maqueta><div class=\"f\"><div class=\"i\"></div><div class=\"i\"></div>\
                 <div class=\"i\"></div></div></maqueta>\
                 <style>.f{display:flex;gap:6px} .i{width:72px;height:72px}</style>");
    let k = &l.root.children[0].children;
    assert_eq!(k[0].rect, r(0, 0, 72, 72));
    assert_eq!(k[1].rect, r(78, 0, 72, 72));
    assert_eq!(k[2].rect, r(156, 0, 72, 72));
    assert_eq!(l.canvas, (72 * 3 + 12, 72));
}

#[test]
fn a_flex_column_stacks_downwards() {
    let l = run("<maqueta><div class=\"f\"><div class=\"i\"></div><div class=\"i\"></div></div></maqueta>\
                 <style>.f{display:flex;flex-direction:column;gap:4px} .i{width:10px;height:20px}</style>");
    let k = &l.root.children[0].children;
    assert_eq!(k[0].rect.y, 0);
    assert_eq!(k[1].rect.y, 24);
}

#[test]
fn block_children_stack_and_gap_does_nothing_exactly_as_in_css() {
    let l = run("<maqueta><div class=\"b\"><div class=\"i\"></div><div class=\"i\"></div></div></maqueta>\
                 <style>.b{gap:50px} .i{height:10px}</style>");
    let k = &l.root.children[0].children;
    assert_eq!(k[0].rect.y, 0);
    assert_eq!(k[1].rect.y, 10, "`gap` no hace nada fuera de flex, como en CSS");
}

#[test]
fn justify_center_and_end_move_the_whole_run() {
    let sheet = "<style>.f{display:flex;width:100px;height:10px;justify-content:JJ} \
                 .i{width:20px;height:10px}</style>";
    for (j, x) in [("start", 0), ("center", 40), ("end", 80)] {
        let l = run(&format!(
            "<maqueta><div class=\"f\"><div class=\"i\"></div></div></maqueta>{}",
            sheet.replace("JJ", j)
        ));
        assert_eq!(l.root.children[0].children[0].rect.x, x, "justify-content:{j}");
    }
}

#[test]
fn space_between_spreads_the_free_room() {
    let l = run("<maqueta><div class=\"f\"><div class=\"i\"></div><div class=\"i\"></div></div></maqueta>\
                 <style>.f{display:flex;width:100px;height:10px;justify-content:space-between} \
                 .i{width:20px;height:10px}</style>");
    let k = &l.root.children[0].children;
    assert_eq!(k[0].rect.x, 0);
    assert_eq!(k[1].rect.x, 80);
}

#[test]
fn a_run_bigger_than_its_room_lands_at_a_negative_coordinate() {
    // * And it must. A browser overflows symmetrically too, and clamping to zero
    // here would hide exactly the overflow `verdict/` is built to catch. Same
    // trap the rasterizer wrote down: the wrong width flips the sign.
    let l = run("<maqueta><div class=\"f\"><div class=\"i\"></div></div></maqueta>\
                 <style>.f{display:flex;width:20px;height:10px;justify-content:center} \
                 .i{width:100px;height:10px}</style>");
    assert_eq!(l.root.children[0].children[0].rect.x, -40);
}

#[test]
fn an_absolute_box_is_anchored_to_the_canvas_and_leaves_the_flow() {
    // Not a simplification: CSS anchors to the nearest POSITIONED ancestor, and
    // MAQUETA has no `position:relative`, so there never is one.
    let l = run("<maqueta ancho=\"400\" alto=\"300\">\
                 <div class=\"b\"><div class=\"flota\"></div><div class=\"i\"></div></div></maqueta>\
                 <style>.b{padding:20px} .flota{position:absolute;left:10px;top:5px;width:8px;height:8px} \
                 .i{height:12px}</style>");
    let b = &l.root.children[0];
    let flota = b.children.iter().find(|f| f.rect.w == 8).unwrap();
    let normal = b.children.iter().find(|f| f.rect.h == 12).unwrap();
    assert_eq!(flota.rect, r(10, 5, 8, 8), "contra el lienzo, no contra el padre");
    assert_eq!(normal.rect.y, 20, "y no ocupa sitio en el flujo");
}

// ------------------------------------------------------------------------
//  * The two tables, from one pass
// ------------------------------------------------------------------------

#[test]
fn the_hit_table_and_the_paint_tree_are_the_same_arithmetic() {
    let l = run("<maqueta><div class=\"f\"><div class=\"i\" id=\"uno\"></div>\
                 <div class=\"i\" id=\"dos\"></div></div></maqueta>\
                 <style>.f{display:flex;gap:6px} .i{width:72px;height:72px}</style>");
    let hits = l.hits();
    assert_eq!(hits.len(), 2);
    assert_eq!(hits[0], ("uno", r(0, 0, 72, 72)));
    assert_eq!(hits[1], ("dos", r(78, 0, 72, 72)));

    // The point: no separate computation exists to disagree with.
    for (id, rect) in &hits {
        let painted = l.all().into_iter().find(|f| f.id.as_deref() == Some(id)).unwrap();
        assert_eq!(painted.rect, *rect);
    }
}

#[test]
fn islands_come_out_with_their_rects() {
    let l = run("<maqueta><div class=\"f\"><island nombre=\"vitals\" class=\"i\"></island></div></maqueta>\
                 <style>.f{display:flex} .i{width:300px;height:200px}</style>");
    assert_eq!(l.islands(), vec![("vitals", r(0, 0, 300, 200))]);
}

// ------------------------------------------------------------------------
//  ** The calculator: the number that judges the whole idea
// ------------------------------------------------------------------------

#[test]
fn the_calculator_lands_exactly_where_calc_rs_puts_it() {
    let l = run(include_str!("../../pruebas/calc.maqueta"));

    // The size nobody declared, worked out from the tree:
    //    ancho  4*72 + 3*6 (gap) + 2*6 (padding) + 2*2 (borde) = 322
    //    alto   40 + 6 + 5*72 + 4*6 + 2*6 + 2*2                   = 446
    assert_eq!(l.canvas, (322, 446), "el tamano que hoy calcula una persona");

    let pad = &l.root.children[0];
    assert_eq!(pad.rect, r(0, 0, 322, 446));
    assert_eq!(pad.content, r(8, 8, 306, 430));

    let hits: std::collections::HashMap<_, _> = l.hits().into_iter().collect();

    // The visor, then five rows of four at CALC_BTN=72 and CALC_GAP=6.
    assert_eq!(hits["k_c"], r(8, 54, 72, 72));
    assert_eq!(hits["k_div"], r(86, 54, 72, 72));
    assert_eq!(hits["k_mul"], r(164, 54, 72, 72));
    assert_eq!(hits["k_sub"], r(242, 54, 72, 72));

    // ...and the last row, which is where an off-by-one would surface.
    assert_eq!(hits["k_0"], r(8, 366, 72, 72));
    assert_eq!(hits["k_dot"], r(86, 366, 72, 72));

    // The three that used to be `.hueco`: they took the exact place the empty
    // boxes were holding, which is why the canvas above did not move a pixel.
    assert_eq!(hits["k_pct"], r(242, 210, 72, 72));
    assert_eq!(hits["k_neg"], r(164, 366, 72, 72));
    assert_eq!(hits["k_money"], r(242, 366, 72, 72));

    // Every key answers: five rows of four, and no gap left.
    assert_eq!(hits.len(), 20, "cinco filas de cuatro, sin un solo hueco");
    for rect in hits.values() {
        assert!(rect.inside(&pad.rect), "ninguna tecla se sale del panel");
    }
}

// ------------------------------------------------------------------------
//  The copied constant that cannot diverge in silence
// ------------------------------------------------------------------------

#[test]
fn las_medidas_del_glifo_siguen_siendo_las_del_kernel() {
    // [!] `GLIFO_ANCHO` and `GLIFO_ALTO` are a SECOND COPY: the originals live in
    // Ring 3, in another workspace, built for another target. There is no shared
    // home for them today.
    //
    // A second copy of a number is what turned `bmo.h` into a fourth copy, so
    // this reads the original off disk. It fails the day somebody changes the
    // font and forgets this crate -- which is the whole job of a guardian.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../../Ultra_userspace/userland/src/pantalla.rs"
    ))
    .expect("pantalla.rs tiene que estar donde dice");

    let leer = |name: &str| -> u32 {
        src.lines()
            .find_map(|l| l.split_once(&format!("pub const {name}: u32 = ")))
            .and_then(|(_, rest)| rest.trim_end_matches(';').trim().parse().ok())
            .unwrap_or_else(|| panic!("no encontre {name} en pantalla.rs"))
    };

    assert_eq!(GLIFO_ANCHO, leer("GLIFO_ANCHO"));
    assert_eq!(GLIFO_ALTO, leer("GLIFO_ALTO"));
}

// ------------------------------------------------------------------------
//  Donde caen las letras
// ------------------------------------------------------------------------

#[test]
fn en_una_caja_de_bloque_el_texto_empieza_arriba_a_la_izquierda() {
    let l = run("<maqueta><div class=\"a\">hola</div></maqueta>                 <style>.a{width:200px;height:60px;padding:4px;color:#FFFFFF}</style>");
    let t = l.root.children[0].text_at.unwrap();
    assert_eq!((t.x, t.y), (4, 4));
    assert_eq!((t.w, t.h), (4 * GLIFO_ANCHO, GLIFO_ALTO));
}

#[test]
fn una_etiqueta_centrada_cae_donde_calc_rs_la_pone() {
    // ** La comprobacion que vale: `calc.rs` centra a mano con
    //
    //     bx + CALC_BTN/2 - GLIFO_ANCHO/2  ,  by + CALC_BTN/2 - GLIFO_ALTO/2
    //
    // y en flex, un texto es un ELEMENTO ANONIMO --concepto real de CSS, no un
    // invento-- asi que `justify-content` y `align-items` lo mueven igual. Si
    // los dos numeros no coincidieran, el emisor produciria algo que no se
    // parece a lo que hay hoy.
    let l = run(include_str!("../../pruebas/calc.maqueta"));
    let c = l.all().into_iter().find(|f| f.id.as_deref() == Some("k_c")).unwrap();

    let (bx, by, btn) = (c.rect.x, c.rect.y, 72i32);
    let esperado_x = bx + btn / 2 - GLIFO_ANCHO as i32 / 2;
    let esperado_y = by + btn / 2 - GLIFO_ALTO as i32 / 2;

    let t = c.text_at.unwrap();
    assert_eq!((t.x, t.y), (esperado_x, esperado_y), "la etiqueta no cae donde calc.rs");
    assert_eq!((t.x, t.y), (40, 82), "y ese sitio es este");
}

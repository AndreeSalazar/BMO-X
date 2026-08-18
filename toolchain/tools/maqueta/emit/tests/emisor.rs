//! Lo que sale del compilador, y si sirve.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::rust;
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

/// Compila de verdad: padre, hijo, nieto **y bisnieto**. Un emisor que acepta
/// una maquetacion que el veredicto rechaza estaria emitiendo el fallo.
fn compilar(src: &str) -> Laid {
    let doc = parse(src.as_bytes())
        .unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let c = cascade(&doc)
        .unwrap_or_else(|e| panic!("{}", render("x.maqueta", src.as_bytes(), &e)));
    let l = lay(&c);
    let v = judge(&l, &c);
    assert!(v.is_empty(), "{}", render("x.maqueta", src.as_bytes(), &v));
    l
}

const CALC: &str = include_str!("../../pruebas/calc.maqueta");

fn generado() -> String {
    rust::modulo("toolchain/tools/maqueta/pruebas/calc.maqueta", &compilar(CALC))
}

// ------------------------------------------------------------------------
//  Lo que sale
// ------------------------------------------------------------------------

#[test]
fn el_fichero_dice_de_donde_salio_y_que_no_se_toca() {
    let g = generado();
    assert!(g.starts_with("//! GENERADO POR MAQUETA DESDE"));
    assert!(g.contains("NO EDITAR A MANO"));
    assert!(g.contains("calc.maqueta"), "y cual es la fuente");
}

#[test]
fn el_tamano_que_nadie_escribio_sale_como_constante() {
    let g = generado();
    assert!(g.contains("pub const ANCHO: u32 = 322;"));
    assert!(g.contains("pub const ALTO: u32 = 446;"));
}

#[test]
fn el_panel_sale_como_los_dos_rects_concentricos_que_escribe_calc_rs() {
    // calc.rs:
    //     p.rect(cc.x, cc.y, cc.width, cc.height, BOX_EDGE);
    //     p.rect(cc.x + 2, cc.y + 2, cc.width - 4, cc.height - 4, CALC_BG);
    let g = generado();
    assert!(g.contains("p.rect(ox + 0, oy + 0, 322, 446, 0x00333D52);"), "{g}");
    assert!(g.contains("p.rect(ox + 2, oy + 2, 318, 442, 0x00182434);"), "{g}");
}

#[test]
fn cada_tecla_sale_con_su_rect_y_su_etiqueta_centrada() {
    let g = generado();
    // La tecla `C`: rect en (8,54) 72x72, y su letra centrada en (40,82).
    assert!(g.contains("p.rect(ox + 8, oy + 54, 72, 72, 0x002B3B52);"), "{g}");
    assert!(g.contains("p.texto(ox + 40, oy + 82, \"C\", 0x00E6EDF6);"), "{g}");
    // La de operador, con su otro fondo.
    assert!(g.contains("p.rect(ox + 86, oy + 54, 72, 72, 0x003A5878);"), "{g}");
    // Y la de igual.
    assert!(g.contains("0x004C9BE8"), "{g}");
}

#[test]
fn cada_caja_lleva_su_nombre_en_un_comentario() {
    let g = generado();
    assert!(g.contains("// #k_c"));
    assert!(g.contains("// #k_eq"));
}

// ------------------------------------------------------------------------
//  ** La tabla de golpeo, de la misma pasada
// ------------------------------------------------------------------------

#[test]
fn la_tabla_de_golpeo_usa_los_mismos_numeros_que_el_pintado() {
    // El punto entero del proyecto: no hay una segunda aritmetica que pueda
    // discrepar. Se comprueba comparando los numeros que salieron en `pintar`
    // con los que salieron en `golpe`.
    let l = compilar(CALC);
    let g = generado();

    for (id, r) in l.hits() {
        let rect = format!("p.rect(ox + {}, oy + {}, {}, {}", r.x, r.y, r.w, r.h);
        let hit = format!(
            "px >= ox + {} && px < ox + {} && py >= oy + {} && py < oy + {}",
            r.x,
            r.right(),
            r.y,
            r.bottom()
        );
        assert!(g.contains(&rect), "falta el pintado de {id}: {rect}");
        assert!(g.contains(&hit), "falta el golpe de {id}: {hit}");
    }
}

#[test]
fn las_diecisiete_teclas_estan_en_la_tabla() {
    let g = generado();
    for id in [
        "k_c", "k_div", "k_mul", "k_sub", "k_7", "k_8", "k_9", "k_add", "k_4", "k_5", "k_6",
        "k_1", "k_2", "k_3", "k_eq", "k_0", "k_dot",
    ] {
        assert!(g.contains(&format!("return Some({id:?});")), "falta {id}");
    }
}

#[test]
fn hay_un_dentro_que_reemplaza_al_contains_escrito_a_mano() {
    assert!(generado().contains("pub fn dentro(ox: u32, oy: u32, px: u32, py: u32) -> bool"));
}

// ------------------------------------------------------------------------
//  Las islas
// ------------------------------------------------------------------------

#[test]
fn el_visor_de_la_calculadora_es_una_isla_y_sale_con_su_rect() {
    // ** Al cablear la calculadora al escritorio quedo claro que la pantallita
    // no es una caja con texto: el NUMERO CAMBIA, asi que es dato vivo, asi que
    // es una isla. La frontera del proyecto cayendo justo donde tenia que caer.
    let g = generado();
    assert!(g.contains("pub const ISLAS: [(&str, u32, u32, u32, u32); 1] = ["));
    assert!(g.contains("(\"visor\", 8, 8, 306, 40),"), "{g}");

    // Y quien la rellena no tiene que saber su color: se lo pide al generado.
    assert!(g.contains("pub fn limpiar_isla("));
    assert!(g.contains("if nombre == \"visor\""));
    assert!(g.contains("pub fn isla(nombre: &str) -> Option<(u32, u32, u32, u32)>"));

    let con_isla = "<maqueta><div class=\"f\"><island nombre=\"vitals\" class=\"i\"></island></div></maqueta>\
                    <style>.f{display:flex;width:300px;height:200px} .i{width:300px;height:200px}</style>";
    let g = rust::modulo("x.maqueta", &compilar(con_isla));
    assert!(g.contains("(\"vitals\", 0, 0, 300, 200),"), "{g}");
}

// ------------------------------------------------------------------------
//  ** El numero que juzgaba la idea
// ------------------------------------------------------------------------

#[test]
fn el_numero_que_juzga_la_idea_medido_y_no_prometido() {
    // De las 214 lineas de `calc.rs`, 118 son maquetacion: `CalcPad`, `button`,
    // `key_at`, `contains`, las nueve constantes y el cuerpo de `paint_calc`.
    // Las otras ~96 son la maquina de estados y se quedan en Rust, porque el
    // motor sigue siendo el COBOL de `cobol/calcgui.bex`.
    const MAQUETACION_A_MANO: usize = 118;

    let escrito = CALC
        .lines()
        .skip_while(|l| !l.trim_start().starts_with("<maqueta"))
        .filter(|l| !l.trim().is_empty())
        .count();

    // [!] EL PLAN PROMETIA "bajar a un tercio" Y NO LLEGA: son 58 contra 118,
    // un 52% menos --y el `.maqueta` ya incluye el realce de `:hover`, que en
    // Rust son `lighten()` mas la constante `HIGHLIGHT`. Y la razon esta medida, no supuesta -- la calculadora es el
    // PEOR CASO posible para MAQUETA:
    //
    //   `calc.rs` pinta veinte teclas con `for row { for col }` sobre una tabla
    //   de etiquetas. Una rejilla REGULAR ya es maquetacion declarativa, y ahi
    //   un bucle gana en lineas a veinte `<div>` escritos.
    //
    // Donde MAQUETA gana de verdad es en lo IRREGULAR --`chrome.rs`, la barra,
    // los paneles-- que es donde no hay bucle que valga. El numero honesto de
    // esta prueba es el de su peor caso.
    assert!(escrito < MAQUETACION_A_MANO, "{escrito} contra {MAQUETACION_A_MANO}");
    assert_eq!(escrito, 58, "si esto cambia, el numero del plan hay que rehacerlo");

    // Lo que de verdad se cobra no son las lineas: son las TRES FUNCIONES que
    // dejan de existir, y con ellas la aritmetica escrita dos veces.
    let g = generado();
    assert!(g.contains("pub fn golpe("));
    assert!(g.contains("pub fn dentro("));
    println!(
        "escrito {escrito} | generado {} | a mano {MAQUETACION_A_MANO} | -{}%",
        g.lines().count(),
        100 - escrito * 100 / MAQUETACION_A_MANO
    );
}

// ------------------------------------------------------------------------
//  Que lo generado sea Rust de verdad
// ------------------------------------------------------------------------

#[test]
fn lo_generado_esta_equilibrado_y_no_tiene_sorpresas() {
    let g = generado();

    // [!] Se cuenta SIN los comentarios, y lo aprendi rompiendolo: al documentar
    // el recorte con `[x0, x1)` --el intervalo medio abierto-- el generado quedo
    // con un `)` de mas y esta prueba salto. El codigo estaba perfecto.
    //
    // Sigue siendo una HEURISTICA: un texto del `.maqueta` con un parentesis
    // suelto la volveria a enganar. Se queda porque avisa en tres milisegundos,
    // y quien dice la verdad de verdad es `bmo.ps1`, que compila esto de veras.
    let codigo: String = g
        .lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert_eq!(codigo.matches('{').count(), codigo.matches('}').count(), "llaves");
    assert_eq!(codigo.matches('(').count(), codigo.matches(')').count(), "parentesis");
    assert!(!g.contains("ox + -"), "una coordenada negativa saldria como `ox + -8`");
    assert!(g.is_ascii(), "las fuentes de BMO-X son ASCII");
}

#[test]
fn una_maquetacion_vacia_no_genera_codigo_que_no_compile() {
    // Sin cajas no hay golpes, y `golpe` se quedaria con parametros sin usar --
    // que en este arbol es un aviso, y los avisos se tratan como errores.
    let g = rust::modulo("x.maqueta", &compilar("<maqueta></maqueta>"));
    assert!(g.contains("let _ = (ox, oy, px, py);"));
}

// ------------------------------------------------------------------------
//  ** El pintado RECORTADO
// ------------------------------------------------------------------------

#[test]
fn hay_un_pintar_en_que_recorta_los_rects_y_deja_el_texto_entero() {
    let g = generado();
    assert!(g.contains(
        "pub fn pintar_en(p: &bmo::Pantalla, ox: u32, oy: u32, cx: u32, cy: u32, cw: u32, ch: u32)"
    ));

    // El panel entero se CORTA al danio: es la razon de existir de la funcion.
    assert!(
        g.contains("if let Some((x, y, w, h)) = corte(cx, cy, cw, ch, ox + 0, oy + 0, 322, 446) {"),
        "{g}"
    );
    assert!(g.contains("p.rect(x, y, w, h, 0x00333D52);"), "{g}");

    // El texto NO se corta: entero o nada, porque medio glifo no se pinta.
    assert!(g.contains("if cruza(cx, cy, cw, ch, ox + 40, oy + 82, 8, 16) {"), "{g}");
    assert!(g.contains("p.texto(ox + 40, oy + 82, \"C\", 0x00E6EDF6);"), "{g}");
}

#[test]
fn el_recorte_generado_usa_el_intervalo_medio_abierto() {
    // La misma regla que `bmo-dibujo`. Si el borde contara, cada reparacion
    // repintaria una fila de mas y se veria como una costura.
    let g = generado();
    assert!(g.contains("if x1 > x0 && y1 > y0 {"), "{g}");
    assert!(g.contains("fn cruza(cx: u32"), "y `cruza` sale de `corte`");
}

#[test]
fn pintar_y_pintar_en_dibujan_LO_MISMO() {
    // ** La comprobacion que vale: si las dos listas se separaran, reparar un
    // danio pintaria algo distinto de lo que habia -- y eso en pantalla se ve
    // como suciedad, no como un error.
    use bmo_maqueta_emit::orden::{lista, Estado};

    let l = compilar(CALC);
    let ordenes = lista(&l);
    let g = generado();

    for o in ordenes.iter().filter(|o| o.estado == Estado::Reposo) {
        let r = o.trazo.area();
        let en_pintar_en = format!("cx, cy, cw, ch, ox + {}, oy + {}, {}, {}", r.x, r.y, r.w, r.h);
        assert!(g.contains(&en_pintar_en), "falta en pintar_en: {en_pintar_en}");
    }
}

#[test]
fn el_realce_sale_de_la_misma_lista_y_con_el_id_del_golpeo() {
    // `de` es `#k_c` en los comentarios; el `id` que llega del raton es `k_c`.
    // Si no coincidieran, el realce no se dispararia nunca y nadie lo notaria
    // hasta mirar la pantalla.
    let g = generado();
    assert!(g.contains("if id == \"k_c\" {"), "{g}");
    assert!(g.contains("p.rect(ox + 8, oy + 54, 72, 72, 0x004B637E);"), "el color de :hover");
}

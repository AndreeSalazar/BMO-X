//! **La cara de la calculadora, emitida y LEIDA DE VUELTA.**
//!
//! El escalon 8 de `PLAN_MAQUETA.md` / 2 de `PLAN_LA_CARA_VIAJA.md`, comprobado
//! del unico modo que prueba algo: **el emisor escribe y el LECTOR abre**, y el
//! lector es el mismo codigo que correra en Ring 3.
//!
//! ## Por que la ida y vuelta y no unos bytes dorados
//!
//! Un fichero dorado dice *"salio lo mismo que la ultima vez"*, que es util para
//! cazar cambios y **no dice si lo que sale se puede leer**. La ida y vuelta
//! contesta la pregunta que importa: si el compositor va a poder pintar esto.
//!
//! [!] Y hay un limite que hay que decir: **las dos mitades las escribio el
//! mismo lado.** Que el lector acepte lo que el emisor produce no demuestra que
//! el formato sea bueno, demuestra que son consistentes. La prueba de verdad
//! llega cuando lo lea el escritorio en el Ryzen -- escalon 3.

use bmo_maqueta_cascade::cascade;
use bmo_maqueta_diag::render;
use bmo_maqueta_emit::{bef, orden};
use bmo_maqueta_layout::{lay, Laid};
use bmo_maqueta_node::parse;
use bmo_maqueta_verdict::judge;

use bmo_maqueta_cara as cara;

const CALC: &str = include_str!("../../pruebas/calc.maqueta");

/// La cadena entera, veredicto incluido. Un emisor que aceptara una maquetacion
/// que el juez rechaza estaria emitiendo el fallo.
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

/// El lienzo de la calculadora, tomado de la caja raiz.
fn lienzo(l: &Laid) -> (i64, i64) {
    let r = l.all().first().expect("algo tiene que haber").rect;
    (r.w as i64, r.h as i64)
}

fn cara_de_la_calculadora() -> (Vec<u8>, usize, usize) {
    let l = compilar(CALC);
    let ordenes = orden::lista(&l);
    let golpes = orden::golpes(&l);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &golpes, w, h).expect("la calculadora tiene que caber");
    (bytes, ordenes.len(), golpes.len())
}

/// *** LA PRUEBA ENTERA EN UNA: se escribe, se abre, y lo que hay dentro es lo
/// que se metio.
#[test]
fn la_cara_de_la_calculadora_va_y_vuelve() {
    let (bytes, n_ordenes, n_golpes) = cara_de_la_calculadora();

    let c = cara::leer(&bytes, 1920, 1080).expect("el lector tiene que abrirla");
    assert_eq!(c.trazos(), n_ordenes, "no se perdio ni se invento un trazo");
    assert_eq!(c.golpes(), n_golpes, "ni un golpe");

    // Y el contenido, no solo las cuentas: un formato puede cuadrar en numero de
    // registros y traerlos todos a cero.
    let l = compilar(CALC);
    let ordenes = orden::lista(&l);
    for (i, o) in ordenes.iter().enumerate() {
        let p = c.trazo(i).expect("el trazo tiene que estar");
        let r = o.trazo.area();
        assert_eq!(
            (p.x as i64, p.y as i64, p.w as i64, p.h as i64),
            (r.x as i64, r.y as i64, r.w as i64, r.h as i64),
            "el trazo {i} (de {}) cambio de sitio al viajar",
            o.de
        );
        match &o.trazo {
            orden::Trazo::Rect { color, .. } => {
                assert_eq!(p.clase, cara::CLASE_RECT);
                assert_eq!(p.color, *color);
            }
            orden::Trazo::Texto { texto, color, .. } => {
                assert_eq!(p.clase, cara::CLASE_TEXTO);
                assert_eq!(p.color, *color);
                assert_eq!(p.texto, texto.as_bytes(), "las letras del trazo {i}");
            }
        }
    }
}

/// Los nombres de los golpes llegan enteros, que es lo unico que el programa
/// recibe cuando alguien pulsa. Si esto se rompiera, los botones existirian y no
/// se sabria cual es cual.
#[test]
fn los_botones_llegan_con_su_nombre() {
    let l = compilar(CALC);
    let golpes = orden::golpes(&l);
    let ordenes = orden::lista(&l);
    let (w, h) = lienzo(&l);
    let bytes = bef::escribir(&ordenes, &golpes, w, h).unwrap();
    let c = cara::leer(&bytes, 1920, 1080).unwrap();

    assert!(c.golpes() > 0, "la calculadora tiene botones");
    for (i, g) in golpes.iter().enumerate() {
        let p = c.golpe(i).unwrap();
        assert_eq!(p.nombre, g.nombre.as_bytes(), "el golpe {i}");
        assert_eq!((p.x as i64, p.w as i64), (g.r.x as i64, g.r.w as i64));
    }
}

/// **El tamano, medido y no estimado.**
///
/// `PLAN_LA_CARA_VIAJA.md` seccion 3 predijo **~950 bytes** para esta cara,
/// contando a mano sobre la calculadora ya compilada. Esta prueba lo mide de
/// verdad y deja el numero escrito.
///
/// * El tope es generoso a proposito --no es un presupuesto, es un despertador--:
/// lo que tiene que saltar es que la cara se ponga de decenas de KiB, que
/// significaria que dejo de ser "el resultado" y volvio a ser "el documento".
#[test]
fn una_cara_entera_cabe_en_pocos_kilobytes() {
    let (bytes, trazos, golpes) = cara_de_la_calculadora();
    let n = bytes.len();
    std::println!("la cara de la calculadora: {n} B  ({trazos} trazos, {golpes} golpes)");
    assert!(
        n < 8 * 1024,
        "una cara son datos, no un documento: {n} B es demasiado"
    );
    // Y que no sea absurdamente pequena, que seria la senal de que se emitio
    // vacia y las cuentas cuadran solas.
    assert!(n > cara::CABECERA, "no puede ser solo la cabecera");
}

/// **Un lienzo que no cabe en la pantalla se rechaza al LEER, no al escribir.**
///
/// Es la comprobacion 5, y prueba la separacion que sostiene el diseno: el
/// emisor corre en el anfitrion y **no sabe** en que pantalla se pintara. El que
/// lo sabe es el lector, y por eso la pantalla es un parametro suyo.
#[test]
fn la_pantalla_la_pone_quien_lee_y_no_quien_escribe() {
    let (bytes, _, _) = cara_de_la_calculadora();
    assert!(cara::leer(&bytes, 1920, 1080).is_ok());
    assert_eq!(
        cara::leer(&bytes, 32, 32).unwrap_err(),
        cara::Falta::LienzoMasGrandeQueLaPantalla
    );
}

/// **Un byte cambiado y el lector lo dice o lo aguanta, pero no estalla.**
///
/// Se corrompe la cara de la calculadora byte a byte. Es la misma prueba que le
/// hace `bmo-bex-gate` a su cabecera, y esta aqui por el mismo motivo: en Ring 3
/// un panico del compositor no es un test rojo, **es el escritorio caido**.
#[test]
fn la_cara_de_la_calculadora_corrompida_no_tumba_al_lector() {
    let (base, _, _) = cara_de_la_calculadora();
    for i in 0..base.len() {
        for v in [0x00u8, 0xFF] {
            let mut b = base.clone();
            if b[i] == v {
                continue;
            }
            b[i] = v;
            let _ = cara::leer(&b, 1920, 1080);
        }
    }
}

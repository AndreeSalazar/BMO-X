//! **QUE cae dentro de un rectangulo.** La pieza que convierte "repinta el
//! fondo" en "repite lo que cruza esto".
//!
//! ## El numero que justifica este fichero
//!
//! Hoy `erase_box` devuelve el fondo recorriendo su rectangulo **pixel a
//! pixel**, preguntandole a `scene_color` por cada uno:
//!
//! ```text
//!    325.000 pixeles x 4 bytes                     = 1,3 MB
//!    al ancho de banda medido en el Ryzen (~300 MB/s) = 4,33 ms
//!    un fotograma a 60 Hz                          = 16,7 ms
//! ```
//!
//! **Un borrado se come la cuarta parte de un fotograma**, y arrastrar una
//! ventana hace uno por evento de raton. Con la lista recortada son unas quince
//! llamadas a `rect()`, que escriben por filas en vez de por pixeles.
//!
//! ## * Y ademas el texto viaja dentro
//!
//! `scene_color` "sabe de rectangulos, no de letras" -- lo dice su propia
//! cabecera. Por eso los iconos del escritorio se comian al arrastrar una
//! ventana por encima: su etiqueta no la podia devolver nadie. Una lista de
//! trazos lleva las letras, asi que restaurar las devuelve sin que nadie tenga
//! que acordarse.
//!
//! ## Recortar, no descartar
//!
//! Un `Rect` se **corta** a la parte visible; un `Texto` se deja entero o se
//! deja fuera. No es capricho: el fondo del escritorio es un rectangulo del
//! tamano de la pantalla, y descartarlo-o-pintarlo-entero haria que reparar un
//! danio de 40x40 volviera a pintar 1920x1080. Un glifo, en cambio, es atomico:
//! medio glifo no se puede pintar.
//!
//! [!] Es el mismo par de reglas que ya costo un fallo en este arbol:
//! `bmo-dibujo` nacio porque el previsualizador **recortaba** y el kernel
//! **descartaba**, y 2.625 de 8.775 rectangulos por fotograma se tiraban
//! enteros. Aqui se dice cual es cual y por que.

use bmo_maqueta_layout::Rect;

use crate::orden::{Estado, Orden, Trazo};

/// La parte de `r` que cae dentro de `limite`. `None` si no se tocan.
pub fn corte(r: Rect, limite: Rect) -> Option<Rect> {
    let x0 = r.x.max(limite.x);
    let y0 = r.y.max(limite.y);
    let x1 = r.right().min(limite.right());
    let y1 = r.bottom().min(limite.bottom());
    if x1 <= x0 as i64 || y1 <= y0 as i64 {
        return None;
    }
    Some(Rect {
        x: x0,
        y: y0,
        w: (x1 - x0 as i64) as u32,
        h: (y1 - y0 as i64) as u32,
    })
}

/// Se tocan?
pub fn cruza(r: Rect, limite: Rect) -> bool {
    corte(r, limite).is_some()
}

/// Las ordenes de un estado que tocan `limite`, ya recortadas.
///
/// * Esta funcion es el instrumento de diagnostico de un fotograma raro: en vez
/// de leer el codigo generado, se le pide la lista y se mira. Un trozo que no se
/// repinta es una orden que no sale aqui.
pub fn dentro(ordenes: &[Orden], estado: Estado, limite: Rect) -> Vec<Orden> {
    ordenes
        .iter()
        .filter(|o| o.estado == estado)
        .filter_map(|o| {
            let trazo = match &o.trazo {
                Trazo::Rect { r, color } => Trazo::Rect {
                    r: corte(*r, limite)?,
                    color: *color,
                },
                // Un glifo es atomico: entero o nada.
                Trazo::Texto { r, texto, color } => {
                    if !cruza(*r, limite) {
                        return None;
                    }
                    Trazo::Texto {
                        r: *r,
                        texto: texto.clone(),
                        color: *color,
                    }
                }
            };
            Some(Orden {
                trazo,
                de: o.de.clone(),
                estado: o.estado,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: u32, h: u32) -> Rect {
        Rect { x, y, w, h }
    }

    #[test]
    fn dos_rectangulos_que_no_se_tocan_no_dan_corte() {
        assert_eq!(corte(r(0, 0, 10, 10), r(20, 20, 10, 10)), None);
    }

    #[test]
    fn tocarse_por_el_borde_no_es_tocarse() {
        // `[x0, x1)`, medio abierto, como el scissor de Vulkan y como el
        // `Recorte` de `bmo-dibujo`. Si el borde contara, cada reparacion
        // repintaria una fila de mas y se veria como una costura.
        assert_eq!(corte(r(0, 0, 10, 10), r(10, 0, 10, 10)), None);
    }

    #[test]
    fn el_corte_es_la_parte_comun() {
        assert_eq!(corte(r(0, 0, 100, 100), r(50, 50, 100, 100)), Some(r(50, 50, 50, 50)));
    }

    #[test]
    fn una_caja_dentro_del_limite_sale_entera() {
        assert_eq!(corte(r(10, 10, 5, 5), r(0, 0, 100, 100)), Some(r(10, 10, 5, 5)));
    }

    #[test]
    fn el_fondo_de_la_pantalla_se_corta_al_danio_y_no_se_pinta_entero() {
        // ** El caso que justifica recortar en vez de descartar: reparar 40x40
        // no puede costar 1920x1080.
        let fondo = r(0, 0, 1920, 1080);
        let danio = r(300, 400, 40, 40);
        assert_eq!(corte(fondo, danio), Some(danio));
    }

    #[test]
    fn coordenadas_negativas_no_desbordan() {
        // Una caja centrada en algo mas pequeno cae en negativo -- el nieto lo
        // permite a proposito. Aqui no puede convertirse en un numero enorme.
        assert_eq!(corte(r(-50, -50, 100, 100), r(0, 0, 10, 10)), Some(r(0, 0, 10, 10)));
    }

    fn ordenes() -> Vec<Orden> {
        vec![
            Orden {
                trazo: Trazo::Rect { r: r(0, 0, 100, 100), color: 0x111111 },
                de: "fondo".into(),
                estado: Estado::Reposo,
            },
            Orden {
                trazo: Trazo::Texto { r: r(80, 80, 16, 16), texto: "ab".into(), color: 0x222222 },
                de: "#lejos".into(),
                estado: Estado::Reposo,
            },
            Orden {
                trazo: Trazo::Rect { r: r(0, 0, 100, 100), color: 0x333333 },
                de: "#lejos".into(),
                estado: Estado::Encima,
            },
        ]
    }

    #[test]
    fn el_filtro_recorta_los_rects_y_respeta_el_estado() {
        let d = dentro(&ordenes(), Estado::Reposo, r(0, 0, 10, 10));
        assert_eq!(d.len(), 1, "el texto de (80,80) no toca (0,0,10,10)");
        assert_eq!(d[0].trazo.area(), r(0, 0, 10, 10), "el fondo sale RECORTADO");
        assert_eq!(d[0].de, "fondo");
    }

    #[test]
    fn un_glifo_que_cruza_sale_ENTERO() {
        // Medio glifo no se puede pintar, asi que o entra entero o no entra.
        let d = dentro(&ordenes(), Estado::Reposo, r(88, 88, 4, 4));
        let texto = d.iter().find(|o| matches!(o.trazo, Trazo::Texto { .. })).unwrap();
        assert_eq!(texto.trazo.area(), r(80, 80, 16, 16));
    }

    #[test]
    fn las_ordenes_de_encima_no_salen_en_reposo() {
        let reposo = dentro(&ordenes(), Estado::Reposo, r(0, 0, 100, 100));
        assert!(reposo.iter().all(|o| o.estado == Estado::Reposo));
        let encima = dentro(&ordenes(), Estado::Encima, r(0, 0, 100, 100));
        assert_eq!(encima.len(), 1);
    }

    #[test]
    fn un_limite_vacio_no_deja_pasar_nada() {
        assert!(dentro(&ordenes(), Estado::Reposo, r(0, 0, 0, 0)).is_empty());
    }
}

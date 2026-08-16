//! **EL ENCUADRE** -- donde cae cada pieza del logo, en aritmetica pura.
//!
//! === Por que salio del kernel ===
//!
//! Esto vivia dentro de `splash.rs::pintar_escena`, mezclado con las llamadas a
//! `fill_rect`. Y mientras estuvo ahi, **la composicion solo se podia juzgar
//! reiniciando el Ryzen** -- que es exactamente lo que la cabecera de este crate
//! dice que no puede pasar:
//!
//! > *"Un arranque animado que solo se puede juzgar reiniciando la maquina es un
//! > arranque que nadie va a ajustar nunca."*
//!
//! La regla estaba escrita y esta parte no la cumplia. Se noto en el video del
//! 2026-08-15: el `BMO-X` escrito sobre los tejados y el kanji pisando una torre.
//! Un fallo de encuadre, o sea de numeros -- y los numeros se prueban.
//!
//! Ahora [`componer`] es una funcion del tamano de pantalla y de las piezas, sin
//! una sola escritura a memoria de video. El kernel la llama y pinta; el
//! previsualizador del anfitrion la llama y guarda un PNG. **Los dos ven el mismo
//! encuadre porque ejecutan el mismo codigo**, que es el mismo argumento por el
//! que `bmo-hash` es el unico BLAKE3 del sistema.
//!
//! === La regla que hace falta que sea codigo ===
//!
//! **El logo va SOBRE la ciudad, nunca dentro.** Un primer plano metido en el
//! fondo no es un primer plano. Aqui eso es una linea --restar el techo-- y una
//! prueba que lo vigila para todas las resoluciones a la vez.

/// Lo que mide cada pieza. Lo rellena quien conoce la fuente y las mascaras: los
/// anchos de texto dependen del tipo de letra, y este crate no tiene ninguno.
#[derive(Debug, Clone, Copy)]
pub struct Medidas {
    pub pantalla_w: u32,
    pub pantalla_h: u32,
    /// La `y` de la punta de torre mas alta. Ver `Ciudad::techo`.
    pub techo: u32,
    /// La `x` del canto interior del marco izquierdo (el derecho es simetrico).
    /// Ver `Marco::interior`.
    ///
    /// ** El aura es OPACA. Si se metiera debajo del marco, o el marco la borra
    /// --se pinta despues-- o ella borra el marco. Las dos se ven, y las dos son
    /// el mismo fallo: dos planos peleandose por el mismo sitio, que es de lo
    /// que iba todo esto.
    pub marco_interior: u32,
    pub gato_w: u32,
    pub gato_h: u32,
    pub kanji_w: u32,
    pub kanji_h: u32,
    /// Aire entre el gato y el kanji.
    pub hueco_kanji: u32,
    pub titulo_w: u32,
    pub titulo_h: u32,
    /// Alto de una linea de la fuente normal: el subtitulo.
    pub linea_h: u32,
}

/// Donde va cada cosa. Todo en pixeles de pantalla.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Encuadre {
    pub gato_x: u32,
    pub gato_y: u32,
    pub kanji_x: u32,
    pub kanji_y: u32,
    pub titulo_x: u32,
    pub titulo_y: u32,
    /// Alto del bloque entero: gato + hueco + titulo + regla + subtitulo.
    pub alto_total: u32,
    /// Centro y radios del aura. Ver [`crate::halo`].
    pub aura_cx: i32,
    pub aura_cy: i32,
    pub aura_rx: i32,
    pub aura_ry: i32,
}

/// Aire entre lo mas bajo del logo y los tejados. Sin esto el subtitulo queda
/// **tocando** las torres, que se lee igual de mal que estar dentro.
pub const RESPIRO: u32 = 28;
/// Aire entre el gato y el titulo.
pub const HUECO: u32 = 34;
/// Grosor de la regla de neon bajo el titulo, y sus dos margenes.
const REGLA: u32 = 10 + 3 + 14;

/// **Compone el logo sobre la ciudad.**
pub fn componer(m: &Medidas) -> Encuadre {
    let fila_w = m.gato_w + m.hueco_kanji + m.kanji_w;
    let alto_total = m.gato_h + HUECO + m.titulo_h + REGLA + m.linea_h;

    // La fila de arriba son DOS piezas --gato y kanji-- y el par se centra como
    // una unidad, porque en el logo la composicion no es simetrica.
    let gato_x = m.pantalla_w.saturating_sub(fila_w) / 2;

    // ** EL BLOQUE SE CENTRA EN EL CIELO DESPEJADO, no en la pantalla.
    //
    // Centrado en la pantalla entera --que es lo que habia-- la mitad de abajo
    // cae dentro de las torres en cuanto el horizonte sube un poco. El techo lo
    // dice la ciudad; aqui solo se resta el respiro.
    //
    // Si no cabe --una pantalla muy baja-- se pega arriba en vez de meterse en
    // la ciudad: peor encuadrado y legible antes que bien centrado y enredado.
    let libre = m.techo.saturating_sub(RESPIRO);
    let gato_y = libre.saturating_sub(alto_total) / 2;

    // La altura del kanji sale del logo: su centro cae al 75% del alto del gato.
    let kanji_y = (gato_y + (m.gato_h * 3) / 4).saturating_sub(m.kanji_h / 2);
    let kanji_x = gato_x + m.gato_w + m.hueco_kanji;

    let titulo_x = m.pantalla_w.saturating_sub(m.titulo_w) / 2;
    let titulo_y = gato_y + m.gato_h + HUECO;

    // -- EL AURA, que envuelve al bloque entero.
    //
    // Mas ancha que alta, como el logo. El radio vertical se RECORTA a lo que
    // quepa entre el borde de arriba y los tejados: el aura es opaca --no se
    // puede leer el framebuffer para mezclar-- asi que salirse del cielo
    // despejado significaria borrar torres. Ver `halo`.
    let aura_cy = gato_y + alto_total / 2;
    let aura_ry = (alto_total * 3 / 5)
        .min(aura_cy)
        .min(m.techo.saturating_sub(aura_cy))
        .max(1);

    // Y el radio horizontal se recorta al hueco que deja el marco, por lo mismo:
    // el aura es opaca y el marco se pinta antes, asi que meterse debajo seria
    // que uno borrase al otro.
    let aura_cx = gato_x + fila_w / 2;
    let hasta_el_marco = aura_cx
        .saturating_sub(m.marco_interior)
        .min((m.pantalla_w.saturating_sub(m.marco_interior)).saturating_sub(aura_cx));
    let aura_rx = (fila_w * 3 / 4).min(hasta_el_marco).max(1);

    Encuadre {
        gato_x,
        gato_y,
        kanji_x,
        kanji_y,
        titulo_x,
        titulo_y,
        alto_total,
        aura_cx: aura_cx as i32,
        aura_cy: aura_cy as i32,
        aura_rx: aura_rx as i32,
        aura_ry: aura_ry as i32,
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::Ciudad;

    /// Las medidas reales del logo a una escala dada. Son las del kernel:
    /// mascara de 152x180, kanji de 74x72, fuente de 8x16.
    fn medidas(w: u32, h: u32) -> Medidas {
        let c = Ciudad::nueva(w as i32, h as i32, 42);
        let escala = if h >= 900 { 2 } else { 1 };
        let escala_t = if h >= 900 { 5 } else { 4 };
        Medidas {
            pantalla_w: w,
            pantalla_h: h,
            techo: c.techo().max(0) as u32,
            marco_interior: c.marco().interior().max(0) as u32,
            gato_w: 152 * escala,
            gato_h: 180 * escala,
            kanji_w: 74 * escala,
            kanji_h: 72 * escala,
            hueco_kanji: 22 * escala,
            titulo_w: 5 * 8 * escala_t,
            titulo_h: 16 * escala_t,
            linea_h: 16,
        }
    }

    /// ** LA REGLA, Y ES EL FALLO DEL VIDEO DEL 2026-08-15: el logo va SOBRE la
    /// ciudad, nunca dentro.
    ///
    /// En el video se ve el `BMO-X` escrito justo encima de los tejados y el
    /// kanji pisando una torre. Se comprueba en las resoluciones que esta casa
    /// usa de verdad, porque el encuadre cambia de escala con la altura y un
    /// arreglo que solo vale para 1080 no es un arreglo.
    #[test]
    fn el_logo_nunca_se_mete_en_la_ciudad() {
        for (w, h) in [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)] {
            let m = medidas(w, h);
            let e = componer(&m);
            let abajo = e.gato_y + e.alto_total;
            assert!(
                abajo <= m.techo,
                "a {}x{} el logo acaba en {} y los tejados empiezan en {}",
                w,
                h,
                abajo,
                m.techo
            );
        }
    }

    /// ** Y EL AURA TAMPOCO. Es opaca: si bajara de los tejados, borraria
    /// torres y el arreglo de la separacion habria abierto un agujero en la
    /// ciudad.
    #[test]
    fn el_aura_no_borra_torres_ni_se_sale_por_arriba() {
        for (w, h) in [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)] {
            let m = medidas(w, h);
            let e = componer(&m);
            assert!(
                e.aura_cy + e.aura_ry <= m.techo as i32,
                "a {}x{} el aura baja hasta {} y los tejados estan en {}",
                w,
                h,
                e.aura_cy + e.aura_ry,
                m.techo
            );
            assert!(e.aura_cy - e.aura_ry >= 0, "a {}x{} el aura se sale por arriba", w, h);
        }
    }

    /// El bloque cabe a lo ancho y esta centrado. Un logo cortado por el borde es
    /// lo primero que se ve y lo ultimo que se mira en el codigo.
    #[test]
    fn el_logo_cabe_a_lo_ancho() {
        for (w, h) in [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)] {
            let m = medidas(w, h);
            let e = componer(&m);
            assert!(e.kanji_x + m.kanji_w <= w, "a {}x{} el kanji se sale", w, h);
            assert!(e.titulo_x + m.titulo_w <= w, "a {}x{} el titulo se sale", w, h);
        }
    }

    /// El kanji se apoya en el gato, a la altura que manda el logo. Si se
    /// separara, dejarian de leerse como una marca y pasarian a ser dos dibujos.
    #[test]
    fn el_kanji_va_pegado_al_gato_y_a_su_altura() {
        let m = medidas(1920, 1080);
        let e = componer(&m);
        assert_eq!(e.kanji_x, e.gato_x + m.gato_w + m.hueco_kanji);
        let centro_kanji = e.kanji_y + m.kanji_h / 2;
        assert_eq!(centro_kanji, e.gato_y + (m.gato_h * 3) / 4);
    }

    /// ** EL AURA NO SE METE DEBAJO DEL MARCO. Es opaca y el marco se pinta
    /// antes que ella, asi que solaparse significa que uno de los dos borra al
    /// otro -- dos planos peleandose por el mismo sitio, otra vez.
    #[test]
    fn el_aura_no_se_mete_debajo_del_marco() {
        for (w, h) in [(1920, 1080), (1600, 900), (1366, 768), (1280, 720)] {
            let m = medidas(w, h);
            let e = componer(&m);
            assert!(
                e.aura_cx - e.aura_rx >= m.marco_interior as i32,
                "a {}x{} el aura entra {} px debajo del marco izquierdo",
                w, h, m.marco_interior as i32 - (e.aura_cx - e.aura_rx)
            );
            assert!(
                e.aura_cx + e.aura_rx <= (w - m.marco_interior) as i32,
                "a {}x{} el aura entra debajo del marco derecho", w, h
            );
        }
    }

    /// Una pantalla absurdamente baja no rompe ni desborda: el bloque se pega
    /// arriba. Sale de un panel raro o de un modo de video que el firmware
    /// entrego mas pequeno de lo pedido.
    #[test]
    fn una_pantalla_diminuta_no_desborda() {
        let m = medidas(320, 200);
        let e = componer(&m);
        assert_eq!(e.gato_y, 0, "sin sitio, el bloque se pega arriba");
        assert!(e.aura_ry >= 1, "el aura degenerada seguiria siendo valida");
    }
}

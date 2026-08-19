//! **LA RESTA** -- un clic de pantalla se convierte en un clic de la app.
//!
//! generacion: hijo
//!
//! Relaciona dos hechos que le dan de fuera --donde quedo la caja y donde cayo
//! el dedo-- y devuelve un par de numeros. **No sabe que significa ese par**:
//! si eso es el boton `7` de una calculadora o el centro de un mapa lo decide
//! la app, que es el nieto, y esta al otro lado de la frontera del proceso.
//!
//! ## Por que esto es un crate y no cuatro lineas dentro del compositor
//!
//! Porque cuatro lineas dentro del compositor **no se pueden probar**: el
//! compositor es un `.bex`, `no_main` para un target sin sistema operativo, y
//! ahi no corre un test. Es la ley L7b, y ya se pago una vez -- la politica de
//! foco vive en `bmo-input` por exactamente este motivo.
//!
//! Aqui la resta corre en el anfitrion, con sus filas, en tres segundos.
//!
//! ## ** La regla que este modulo existe para cumplir
//!
//! De `docs/plan/PLAN_DIRECTOR.md`, paso 2c.1:
//!
//! > *Las coordenadas que salen tienen que caer DENTRO de la superficie que la
//! > app declaro. Mandarle un clic en (5000, 5000) a una app de 322x446 es
//! > darle un numero que no puede significar nada.*
//!
//! Por eso la funcion devuelve `Option` y no un par a secas: **fuera no es
//! (0,0), fuera es que no hay golpe.** Un compositor que mandara ceros haria
//! que cada app tuviera que descubrir por su cuenta que ese cero era mentira.
//!
//! ## Las dos cajas, que no son la misma y ahi esta todo
//!
//! ```text
//!    LA VISIBLE     lo que de verdad se esta viendo del interior de la ventana.
//!                   Ya viene recortada contra el marco Y contra la pantalla:
//!                   una ventana medio fuera del panel tiene visible mas
//!                   pequeno que su contenido.
//!
//!    LA DECLARADA   lo que la app dijo que mide su superficie (`BSUP`).
//!                   Es dato de OTRO proceso, asi que aqui no se cree: se usa
//!                   como tope y nada mas.
//! ```
//!
//! Un golpe vale si cae en la visible **y** el resultado cabe en la declarada.
//! Normalmente la visible ya es menor o igual, pero comprobar las dos cuesta
//! una comparacion y quita la necesidad de confiar en que quien llama recorto
//! bien. La frontera de confianza no se reparte entre dos modulos.

#![no_std]

/// El interior visible de una ventana, en coordenadas de PANTALLA.
///
/// `x`,`y` son el primer pixel del contenido --ya por dentro del marco-- y
/// `ancho`,`alto` lo que queda despues de recortar contra el marco y contra el
/// lienzo. Medio abierto: el punto `x + ancho` ya esta fuera.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Visible {
    pub x: u32,
    pub y: u32,
    pub ancho: u32,
    pub alto: u32,
}

/// Lo que la app declaro que mide su superficie. Se usa como tope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Declarada {
    pub ancho: u32,
    pub alto: u32,
}

/// **La resta.** `None` si el punto no cae dentro de esta superficie.
///
/// No hay `saturating_sub` disfrazando nada: si `px` fuera menor que `v.x` el
/// punto esta fuera y ya se contesto que no antes de restar. Un saturado aqui
/// convertiria un clic de fuera en un clic en el borde, que es la clase de
/// mentira que este modulo existe para no contar.
pub fn traducir(v: Visible, d: Declarada, px: u32, py: u32) -> Option<(u32, u32)> {
    if v.ancho == 0 || v.alto == 0 {
        return None;
    }
    if px < v.x || py < v.y {
        return None;
    }
    let lx = px - v.x;
    let ly = py - v.y;
    if lx >= v.ancho || ly >= v.alto {
        return None;
    }
    // El tope de la app, aparte del recorte de quien llama. Ver la cabecera.
    if lx >= d.ancho || ly >= d.alto {
        return None;
    }
    Some((lx, ly))
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La calculadora --322x446, la superficie que de verdad existe-- con su
    /// interior empezando en (100, 50).
    fn caja() -> (Visible, Declarada) {
        (
            Visible { x: 100, y: 50, ancho: 322, alto: 446 },
            Declarada { ancho: 322, alto: 446 },
        )
    }

    #[test]
    fn el_primer_pixel_del_interior_es_el_cero_de_la_app() {
        let (v, d) = caja();
        assert_eq!(traducir(v, d, 100, 50), Some((0, 0)));
    }

    /// El ejemplo literal del plan: *"este clic es de la superficie 2, en su
    /// pixel (81, 210)"*.
    #[test]
    fn un_punto_de_dentro_se_resta() {
        let (v, d) = caja();
        assert_eq!(traducir(v, d, 181, 260), Some((81, 210)));
    }

    #[test]
    fn el_ultimo_pixel_entra_y_el_siguiente_no() {
        let (v, d) = caja();
        assert_eq!(traducir(v, d, 421, 495), Some((321, 445)));
        assert_eq!(traducir(v, d, 422, 495), None);
        assert_eq!(traducir(v, d, 421, 496), None);
    }

    #[test]
    fn arriba_y_a_la_izquierda_no_se_saturan_a_cero() {
        let (v, d) = caja();
        assert_eq!(traducir(v, d, 99, 50), None);
        assert_eq!(traducir(v, d, 100, 49), None);
        assert_eq!(traducir(v, d, 0, 0), None);
    }

    /// El caso que el plan nombra con sus numeros.
    #[test]
    fn cinco_mil_por_cinco_mil_no_significa_nada() {
        let v = Visible { x: 10, y: 10, ancho: 322, alto: 446 };
        let d = Declarada { ancho: 322, alto: 446 };
        assert_eq!(traducir(v, d, 5000, 5000), None);
    }

    /// Una ventana arrastrada medio fuera del panel: lo VISIBLE es menor que lo
    /// declarado, y el golpe se recorta contra lo visible.
    #[test]
    fn arrastrada_medio_fuera_recorta_por_lo_visible() {
        let v = Visible { x: 700, y: 100, ancho: 40, alto: 200 };
        let d = Declarada { ancho: 300, alto: 200 };
        assert_eq!(traducir(v, d, 739, 100), Some((39, 0)));
        assert_eq!(traducir(v, d, 740, 100), None);
    }

    /// Y al reves: el marco se encogio por debajo de lo que mide la superficie.
    /// El tope sigue siendo lo visible, no lo declarado.
    #[test]
    fn un_marco_encogido_no_deja_pasar_lo_que_no_se_ve() {
        let v = Visible { x: 0, y: 0, ancho: 50, alto: 50 };
        let d = Declarada { ancho: 1000, alto: 1000 };
        assert_eq!(traducir(v, d, 49, 49), Some((49, 49)));
        assert_eq!(traducir(v, d, 50, 50), None);
    }

    /// Una superficie que declara MENOS de lo que se ve no puede recibir un
    /// golpe fuera de lo suyo. Es el caso que quita la confianza en quien
    /// llama: aunque el recorte de arriba viniera mal, aqui se para.
    #[test]
    fn lo_declarado_es_un_tope_aunque_lo_visible_mienta() {
        let v = Visible { x: 0, y: 0, ancho: 300, alto: 300 };
        let d = Declarada { ancho: 10, alto: 10 };
        assert_eq!(traducir(v, d, 9, 9), Some((9, 9)));
        assert_eq!(traducir(v, d, 10, 9), None);
    }

    #[test]
    fn una_ventana_sin_interior_visible_no_recibe_nada() {
        let v = Visible { x: 100, y: 50, ancho: 0, alto: 200 };
        let d = Declarada { ancho: 300, alto: 200 };
        assert_eq!(traducir(v, d, 100, 50), None);
    }
}

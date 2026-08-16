//! # Escalon 0,5 -- EL LIENZO: el recorte, aplicado
//!
//! ## El fallo que este fichero hace imposible
//!
//! El recorte (escalon 0) ya existia y era correcto. No sirvio de nada, porque
//! **aplicarlo seguia siendo cosa de quien llamaba**, y hubo dos que llamaron
//! distinto:
//!
//! ```text
//!    // el previsualizador, en el anfitrion
//!    for fy in y.max(0)..(y + alto).min(h) {
//!        for fx in x.max(0)..(x + ancho).min(w) { ... }     // RECORTA
//!    }
//!
//!    // el kernel, en Ring 0
//!    if ancho > 0 && alto > 0 && x >= 0 && y >= 0 && x < w && y < h {
//!        fill_rect(x as u32, y as u32, ...);                // DESCARTA
//!    }
//!```
//!
//! Los dos "comprueban los limites". Uno recorta el rectangulo a lo que se ve;
//! el otro lo tira entero en cuanto una esquina se sale. Y la geometria de la
//! ciudad **emite rectangulos que empiezan fuera a proposito**: el bloque
//! exterior del marco se estira hasta pasado el borde para que la deriva de la
//! camara no abra una rendija. Ese bloque, el unico cuyo trabajo es sellar el
//! borde, es el primero que el kernel tiraba.
//!
//! ## Por que un `trait` y no una funcion de recortar que todos llamen
//!
//! Porque una funcion que hay que acordarse de llamar es una funcion que
//! alguien no llama. Ya habia una.
//!
//! Aqui el reparto es al reves de lo natural, y esa inversion es todo el
//! diseno:
//!
//! ```text
//!    quien llama  ->  rect(x, y, ancho, alto, color)      coordenadas CRUDAS
//!                     [el trait recorta, una vez, aqui]
//!    quien pinta  <-  rect_dentro(recorte, color)         YA dentro, garantizado
//! ```
//!
//! El que escribe pixeles --el framebuffer de Ring 0, la `Pantalla` de Ring 3,
//! el array de una prueba-- **nunca ve una coordenada negativa**. No puede
//! decidir mal sobre un caso que no le llega. Y el que emite geometria no tiene
//! que saber donde acaba la pantalla, que es justo lo que le permite emitir el
//! sello del marco pasado el borde sin pensarlo.
//!
//! Un rectangulo que se sale se recorta. Un rectangulo entero fuera no llega.
//! Ninguna de las dos cosas es una opinion de quien implementa.
//!
//! ## Y sirve para ventanas, no solo para el borde del monitor
//!
//! [`Lienzo::recorte`] no tiene por que ser la pantalla entera: es el `scissor`.
//! Un lienzo que contesta la caja de una ventana recorta a la ventana, y la
//! primitiva que dibuja dentro no se entera de que hay un compositor.

use crate::recorte::Recorte;
use crate::Color;

/// **Donde caen los pixeles.**
///
/// Se implementa con dos metodos --el recorte activo y el relleno-- y se usa
/// con [`rect`](Lienzo::rect), que es el unico camino de entrada.
///
/// # El contrato, en las dos direcciones
///
/// * **Quien implementa** recibe en [`rect_dentro`](Lienzo::rect_dentro) un
///   recorte **no vacio y contenido entero** en el que devolvio
///   [`recorte`](Lienzo::recorte). Puede escribir esos pixeles sin comprobar
///   nada mas: no hay negativos, no hay desbordes, no hay ancho cero.
/// * **Quien llama** puede pasar cualquier cosa. Coordenadas negativas, anchos
///   que se salen por la derecha, rectangulos enteros fuera de la pantalla.
///   Todos son casos validos y todos hacen lo correcto.
///
/// El objetivo de la asimetria es que la unica forma de pintar mal sea
/// implementar `rect_dentro` mal, y `rect_dentro` es cuatro lineas sin
/// decisiones.
pub trait Lienzo {
    /// El `scissor` activo: fuera de aqui no se escribe un pixel.
    ///
    /// Normalmente la pantalla entera, `Recorte::nuevo(0, 0, ancho, alto)`.
    /// Puede ser la caja de una ventana.
    fn recorte(&self) -> Recorte;

    /// Rellena un rectangulo **que ya esta dentro**.
    ///
    /// [!] No lo llames tu. Es el metodo que implementas, no el que usas: si se
    /// llama a mano se salta el recorte, que es lo unico que este fichero
    /// existe para impedir. Usa [`rect`](Lienzo::rect).
    fn rect_dentro(&mut self, r: Recorte, color: Color);

    /// **La puerta.** Rellena un rectangulo, recortandolo antes.
    ///
    /// `ancho` y `alto` en pixeles; `x` e `y` pueden ser negativos. Un
    /// rectangulo que no deja nada visible no llega abajo.
    ///
    /// No se reimplementa. Esta escrito una vez y es el mismo para el kernel,
    /// para Ring 3 y para el previsualizador -- que es la unica forma de que
    /// las tres orillas pinten lo mismo.
    fn rect(&mut self, x: i32, y: i32, ancho: i32, alto: i32, color: Color) {
        self.caja(&Recorte::nuevo(x, y, ancho, alto), color);
    }

    /// Lo mismo que [`rect`](Lienzo::rect) para quien ya tiene el rectangulo
    /// hecho.
    fn caja(&mut self, r: &Recorte, color: Color) {
        let dentro = r.interseccion(&self.recorte());
        if !dentro.vacio() {
            self.rect_dentro(dentro, color);
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Un lienzo de mentira que apunta CADA pixel que se escribe. Es lo que
    /// permite preguntar lo que ninguna prueba preguntaba: *que quedo sin
    /// tocar*.
    struct Testigo {
        w: i32,
        h: i32,
        px: Vec<Option<Color>>,
        /// Cuantas veces se llamo al relleno. Sirve para comprobar que lo que
        /// no se ve no se pinta.
        llamadas: u32,
    }

    impl Testigo {
        fn nuevo(w: i32, h: i32) -> Self {
            Testigo { w, h, px: vec![None; (w * h) as usize], llamadas: 0 }
        }
        fn en(&self, x: i32, y: i32) -> Option<Color> {
            self.px[(y * self.w + x) as usize]
        }
        fn sin_tocar(&self) -> usize {
            self.px.iter().filter(|p| p.is_none()).count()
        }
    }

    impl Lienzo for Testigo {
        fn recorte(&self) -> Recorte {
            Recorte::nuevo(0, 0, self.w, self.h)
        }
        fn rect_dentro(&mut self, r: Recorte, color: Color) {
            self.llamadas += 1;
            // ** La prueba de que el contrato se cumple esta AQUI, en el sitio
            // donde el fallo del kernel vivia: si llegara algo fuera, revienta.
            assert!(r.x0 >= 0 && r.y0 >= 0, "llego un recorte con negativos: {:?}", r);
            assert!(r.x1 <= self.w && r.y1 <= self.h, "llego un recorte desbordado: {:?}", r);
            assert!(!r.vacio(), "llego un recorte vacio");
            for y in r.y0..r.y1 {
                for x in r.x0..r.x1 {
                    self.px[(y * self.w + x) as usize] = Some(color);
                }
            }
        }
    }

    const ROJO: Color = 0xFFFF0000;

    /// ** LA PRUEBA DE LA REGRESION DEL 2026-08-15, dicha al nivel del
    /// contrato.
    ///
    /// Un rectangulo que empieza fuera por la izquierda **pinta la parte que se
    /// ve**. Con la regla vieja del kernel (`if x >= 0`) esto pintaba cero
    /// pixeles, y eso es la franja muerta de 191 px del video.
    #[test]
    fn un_rectangulo_que_entra_desde_fuera_pinta_lo_que_se_ve() {
        let mut t = Testigo::nuevo(100, 50);
        // El sello del marco: empieza en -34 y mide 227 de ancho.
        t.rect(-34, 0, 227, 50, ROJO);
        assert_eq!(t.en(0, 0), Some(ROJO), "no se pinto la columna del borde");
        assert_eq!(t.en(99, 49), Some(ROJO), "no se pinto la esquina opuesta");
        assert_eq!(t.sin_tocar(), 0, "quedaron pixeles sin escribir");
    }

    /// Y por los cuatro lados, no solo por la izquierda.
    #[test]
    fn entra_desde_los_cuatro_lados() {
        for (x, y, w, h) in [(-10, 0, 20, 50), (90, 0, 20, 50), (0, -10, 100, 20), (0, 40, 100, 20)] {
            let mut t = Testigo::nuevo(100, 50);
            t.rect(x, y, w, h, ROJO);
            assert_eq!(t.llamadas, 1, "se descarto un rectangulo que se veia: {:?}", (x, y, w, h));
        }
    }

    /// Un rectangulo entero fuera NO llega abajo. Recortar no es "pintar
    /// siempre": es pintar la interseccion, y una interseccion vacia no se
    /// pinta.
    #[test]
    fn lo_que_no_se_ve_no_llega_al_relleno() {
        let mut t = Testigo::nuevo(100, 50);
        t.rect(-500, 0, 100, 50, ROJO);
        t.rect(1000, 0, 100, 50, ROJO);
        t.rect(0, -500, 100, 100, ROJO);
        t.rect(0, 500, 100, 100, ROJO);
        assert_eq!(t.llamadas, 0, "se pinto algo que no se ve");
        assert_eq!(t.sin_tocar(), 100 * 50);
    }

    /// Anchos y altos degenerados no pintan y no rompen. Salen solos de restar
    /// dos coordenadas que se cruzaron.
    #[test]
    fn los_tamanos_degenerados_no_pintan() {
        let mut t = Testigo::nuevo(100, 50);
        t.rect(10, 10, 0, 10, ROJO);
        t.rect(10, 10, 10, 0, ROJO);
        t.rect(10, 10, -5, -5, ROJO);
        assert_eq!(t.llamadas, 0);
    }

    /// El recorte no tiene por que ser la pantalla: un lienzo que contesta la
    /// caja de una ventana recorta a la ventana. Es el `scissor`, y es lo que
    /// hara falta el dia que el compositor pinte dentro de un marco.
    #[test]
    fn el_recorte_puede_ser_una_ventana() {
        struct Ventana {
            caja: Recorte,
            ultimo: Option<Recorte>,
        }
        impl Lienzo for Ventana {
            fn recorte(&self) -> Recorte {
                self.caja
            }
            fn rect_dentro(&mut self, r: Recorte, _c: Color) {
                self.ultimo = Some(r);
            }
        }
        let mut v = Ventana { caja: Recorte::nuevo(20, 20, 40, 40), ultimo: None };
        v.rect(0, 0, 1000, 1000, ROJO);
        assert_eq!(v.ultimo, Some(Recorte::nuevo(20, 20, 40, 40)), "no se recorto a la ventana");
    }
}

//! **LA TORRE** -- una sola, y como se dibuja su fachada.
//!
//! Aparte de `ciudad.rs` porque son dos preguntas distintas: alli se decide
//! **como se reparten** las torres, aqui **que es una**. La primera cambia
//! cuando alguien quiera otra skyline; la segunda, cuando cambie el estilo de
//! las ventanas. Mezcladas, tocar una obliga a leer la otra.

use crate::azar::mezclador;
use crate::paleta::*;

/// Separacion entre ventanas, y lado de cada una.
///
/// Cinco y dos: la ventana ocupa menos de la mitad del hueco. Con 3 de lado la
/// fachada se emborrona y deja de leerse como ventanas; con paso 8 la torre
/// parece deshabitada.
const PASO: i32 = 5;
const LADO: i32 = 2;

/// Una torre: donde empieza, cuanto mide, y de que capa es.
#[derive(Clone, Copy)]
pub struct Torre {
    pub x: i32,
    pub ancho: i32,
    pub alto: i32,
    /// `0` = fondo (silueta), `1` = frente (con ventanas).
    pub capa: u8,
    /// Si sus ventanas estan encendidas. **Es el dato del sistema**: quien
    /// construye la ciudad decide que significa.
    pub encendido: bool,
    /// El tono del neon de esta torre.
    pub tinte: Color,
}

impl Torre {
    pub const APAGADA: Torre = Torre {
        x: 0,
        ancho: 0,
        alto: 0,
        capa: 0,
        encendido: false,
        tinte: NEON_CIAN,
    };

    /// Dibuja la torre con su base en `horizonte`, desplazada `dx` a la
    /// izquierda por la camara.
    pub fn dibujar(&self, horizonte: i32, dx: i32, rect: &mut impl FnMut(i32, i32, i32, i32, Color)) {
        let x = self.x - dx;
        let y = horizonte - self.alto;
        let color = if self.capa == 0 { TORRE_FONDO } else { TORRE_FRENTE };
        rect(x, y, self.ancho, self.alto, color);
        if self.capa == 0 {
            return;
        }
        // El canto iluminado: una columna de un pixel a la izquierda. Cuesta un
        // rectangulo y **separa dos torres pegadas**, que sin el se leen como
        // una sola mas ancha.
        rect(x, y, 1, self.alto, TORRE_BORDE);
        self.ventanas(x, y, rect);
    }

    /// La rejilla de ventanas.
    ///
    /// [!] El patron sale de la posicion ABSOLUTA de la ventana en el mundo, no
    /// de su posicion en pantalla: si saliera de la pantalla, **la fachada
    /// cambiaria de dibujo mientras la camara avanza** -- las ventanas se
    /// encenderian y apagarian solas al deslizarse. Por eso se usa `self.x` y no
    /// la `x` ya desplazada.
    fn ventanas(&self, x: i32, y0: i32, rect: &mut impl FnMut(i32, i32, i32, i32, Color)) {
        if self.ancho < PASO * 2 || self.alto < PASO * 2 {
            return;
        }
        let mut fy = y0 + PASO;
        let mut mundo_y = PASO;
        while fy + LADO < y0 + self.alto - 2 {
            let mut fx = x + PASO;
            let mut mundo_x = self.x + PASO;
            while fx + LADO < x + self.ancho - 1 {
                let h = mezclador(mundo_x as u64, mundo_y as u64);
                let color = if !self.encendido || h % 5 == 0 {
                    VENTANA_APAGADA
                } else if h % 7 == 0 {
                    self.tinte
                } else if h % 3 == 0 {
                    VENTANA_FRIA
                } else {
                    VENTANA_CALIDA
                };
                rect(fx, fy, LADO, LADO, color);
                fx += PASO;
                mundo_x += PASO;
            }
            fy += PASO;
            mundo_y += PASO;
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// Una torre de fondo no gasta ni un rectangulo en ventanas: son siluetas.
    #[test]
    fn el_fondo_no_dibuja_ventanas() {
        let t = Torre { x: 10, ancho: 60, alto: 80, capa: 0, encendido: true, ..Torre::APAGADA };
        let mut n = 0;
        t.dibujar(200, 0, &mut |_, _, _, _, _| n += 1);
        assert_eq!(n, 1, "una silueta es UN rectangulo");
    }

    /// ** La fachada NO cambia cuando la camara avanza. Si cambiara, las
    /// ventanas se encenderian solas al deslizarse -- que es el defecto mas
    /// visible que puede tener un fondo con paralaje.
    #[test]
    fn la_fachada_no_cambia_al_avanzar_la_camara() {
        let t = Torre { x: 100, ancho: 40, alto: 90, capa: 1, encendido: true, ..Torre::APAGADA };
        let recoger = |dx: i32| {
            let mut v: [Color; 64] = [0; 64];
            let mut i = 0;
            t.dibujar(300, dx, &mut |_, _, _, _, c| {
                if i < 64 {
                    v[i] = c;
                    i += 1;
                }
            });
            v
        };
        assert_eq!(recoger(0), recoger(37), "la fachada cambio al desplazarse");
    }

    /// Apagada, ninguna ventana sale encendida.
    #[test]
    fn apagada_no_enciende_ni_una() {
        let t = Torre { x: 0, ancho: 40, alto: 90, capa: 1, encendido: false, ..Torre::APAGADA };
        t.dibujar(300, 0, &mut |_, _, _, _, c| {
            assert!(
                c != VENTANA_CALIDA && c != VENTANA_FRIA,
                "salio una ventana encendida en una torre apagada"
            );
        });
    }

    /// Una torre mas estrecha que dos ventanas no intenta dibujarlas ni entra
    /// en bucle.
    #[test]
    fn una_torre_diminuta_no_rompe_nada() {
        let t = Torre { x: 0, ancho: 4, alto: 6, capa: 1, encendido: true, ..Torre::APAGADA };
        let mut n = 0;
        t.dibujar(100, 0, &mut |_, _, _, _, _| n += 1);
        assert_eq!(n, 2, "solo el cuerpo y el canto");
    }
}

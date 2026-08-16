//! **LA NIEBLA** -- lo que despega los planos del todo.
//!
//! ## Las dos nieblas, que no son la misma cosa
//!
//! Se llaman igual y hacen trabajos distintos, asi que van separadas:
//!
//! 1. **La BRUMA** ([`velo`]) no se ve: se nota. Es el aire que hay entre el ojo
//!    y una torre, y lo que hace es **acercar su color al del cielo cuanto mas
//!    lejos esta**. Es la pista mas fuerte de profundidad que existe, y la usa
//!    cualquier pintor de paisajes desde hace quinientos anos. La escalera de
//!    valores de `paleta.rs` separa las capas; esto separa **dentro** de cada
//!    capa, porque la base de una torre esta mas lejos que su punta.
//!
//! 2. **Las BANDAS** ([`bandas`]) si se ven: son jirones horizontales que cruzan
//!    la escena **mas lentos que todo lo demas**. Y esa lentitud es su unico
//!    truco -- lo que dice "esto esta lejisimos" es que casi no se mueve
//!    mientras las torres pasan.
//!
//! ## Por que la niebla es lo que faltaba
//!
//! Del video del Ryzen: la ciudad seguia leyendose plana aunque las capas ya
//! tuvieran brillos distintos. Y es que **dos planos separados en valor pero con
//! el borde igual de nitido siguen pareciendo recortables pegados**. La bruma
//! difumina esa frontera sin difuminar nada: solo cambia el color.

use crate::paleta::*;

/// Cuanta bruma llega a haber en la base del horizonte, de 0 a 255.
///
/// [!] No llega a 255 a proposito. Con bruma total la base de las torres seria
/// **exactamente** el color del cielo y la ciudad pareceria flotar cortada por
/// abajo. 170 deja el suelo insinuado.
pub const BRUMA_MAX: u32 = 170;

/// **La bruma sobre un color, segun a que altura esta y de que capa es.**
///
/// `y` es la fila que se va a pintar y `horizonte` donde acaba la ciudad. Cuanto
/// mas cerca del horizonte, mas aire hay de por medio y mas se acerca el color
/// al del cielo.
///
/// La capa multiplica: lo lejano tiene **todo** el aire de la escena delante, lo
/// cercano solo un poco.
pub fn velo(color: Color, y: i32, horizonte: i32, capa: u8) -> Color {
    if horizonte <= 0 || y < 0 {
        return color;
    }
    // Cuanto vale 1 la base y 0 el borde de arriba de la pantalla.
    let alto = horizonte.max(1) as u32;
    let cerca = (y.min(horizonte) as u32 * 255) / alto;
    // El fondo recibe la bruma entera; el frente, un tercio. Es la diferencia
    // que hace que las dos capas dejen de tener el mismo borde.
    let peso = if capa == 0 { 255 } else { 85 };
    let bruma = cerca * BRUMA_MAX / 255 * peso / 255;
    mezcla(color, CIELO_BAJO, bruma, 255)
}

/// Cuantos jirones cruzan la escena.
pub const BANDAS: i32 = 5;

/// **Las bandas de niebla**, emitidas como rectangulos.
///
/// `avance` es el de la camara. Van a un **sexto** de su velocidad: mas lentas
/// que el fondo de la ciudad, que ya iba a un tercio. Ese escalon es lo que las
/// pone detras de todo sin necesidad de dibujarlas antes.
///
/// [!] Y se pintan DESPUES de las torres del fondo y ANTES de las del frente:
/// una niebla que no se mete entre las capas no separa nada, solo tine.
pub fn bandas(
    ancho: i32,
    horizonte: i32,
    avance: i32,
    mut rect: impl FnMut(i32, i32, i32, i32, Color),
) {
    if ancho <= 0 || horizonte <= 0 {
        return;
    }
    let dx = avance / 6;
    for i in 0..BANDAS {
        // Repartidas por el tercio de abajo del cielo, que es donde se acumula
        // la niebla de verdad: pegada al suelo, no a media altura.
        let y = horizonte - horizonte * (i + 1) / (BANDAS * 3);
        let grosor = 2 + (i % 3);
        // Cada banda arranca en un sitio distinto y da la vuelta. El modulo
        // sobre `ancho * 2` deja un hueco tan ancho como la pantalla entre una
        // vuelta y la siguiente: si dieran la vuelta pegadas seria una raya
        // continua, no un jiron.
        let largo = ancho / 3 + i * ancho / 12;
        let periodo = ancho * 2;
        let base = (i * periodo / BANDAS - dx).rem_euclid(periodo);
        // La banda es el cielo un poco mas claro. Ni blanca ni gris: la niebla
        // no tiene color propio, **devuelve el que le llega**.
        let c = mezcla(CIELO_BAJO, CIELO_ALTO, 60, 255);
        rect(base - periodo, y, largo, grosor, c);
        rect(base, y, largo, grosor, c);
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La bruma acerca al cielo, nunca aleja. Si alejara, lo lejano saldria mas
    /// contrastado que lo cercano y la escena se leeria del reves.
    #[test]
    fn la_bruma_acerca_al_color_del_cielo() {
        let torre = TORRE_FRENTE;
        let lejos = luminancia(velo(torre, 500, 500, 0));
        let cerca = luminancia(velo(torre, 0, 500, 0));
        let cielo = luminancia(CIELO_BAJO);
        assert!(lejos > cerca, "la base tiene que estar mas velada que la punta");
        assert!(lejos <= cielo, "la bruma no puede pasarse del cielo");
    }

    /// ** El fondo recibe MAS bruma que el frente. Es lo unico que separa dos
    /// capas que ya tienen el mismo borde nitido.
    #[test]
    fn el_fondo_recibe_mas_bruma_que_el_frente() {
        let f = luminancia(velo(TORRE_FRENTE, 400, 500, 0));
        let d = luminancia(velo(TORRE_FRENTE, 400, 500, 1));
        assert!(f > d, "el fondo tiene que velarse mas: {} contra {}", f, d);
    }

    /// Arriba del todo no hay bruma: el aire se acumula abajo.
    #[test]
    fn arriba_no_hay_bruma() {
        assert_eq!(velo(TORRE_FRENTE, 0, 500, 0), TORRE_FRENTE);
    }

    /// ** Las bandas van MAS LENTAS que el fondo de la ciudad. Es su unico
    /// truco: lo que dice "esto esta lejisimos" es que casi no se mueve.
    #[test]
    fn las_bandas_van_mas_lentas_que_el_fondo() {
        use crate::camara::{Camara, LENTITUD_DEL_FONDO};
        let avance = 600;
        let fondo = Camara::nueva(avance).desplazamiento(0);
        let niebla = avance / 6;
        assert!(niebla < fondo, "la niebla no puede adelantar al fondo");
        assert!(LENTITUD_DEL_FONDO < 6, "el escalon de velocidades se invirtio");
    }

    /// Cubren la pantalla en cualquier momento del recorrido: una banda que
    /// desaparece al dar la vuelta se ve como un parpadeo.
    #[test]
    fn siempre_hay_niebla_en_pantalla() {
        for avance in [0, 137, 640, 1500, 4000] {
            let mut vistas = 0;
            bandas(640, 480, avance, |x, _, w, _, _| {
                if x + w > 0 && x < 640 {
                    vistas += 1;
                }
            });
            assert!(vistas > 0, "sin niebla visible con avance {}", avance);
        }
    }

    /// Un lienzo degenerado no rompe ni entra en bucle.
    #[test]
    fn un_lienzo_vacio_no_rompe() {
        let mut n = 0;
        bandas(0, 0, 10, |_, _, _, _, _| n += 1);
        assert_eq!(n, 0);
    }
}

//! **LA CAMARA** -- lo que hace que la ciudad se sienta andada y no pintada.
//!
//! ## Que es el paralaje, y por que es lo unico que hace falta
//!
//! Pedido por el dueno: *"que se sienta que tiene animacion y como la camara
//! avanza"*.
//!
//! Mover la ciudad entera a un lado no da sensacion de avanzar: da sensacion de
//! que la imagen se desliza. Lo que el ojo lee como profundidad es que **lo
//! cercano pase mas deprisa que lo lejano** -- es la misma pista que usa mirando
//! por la ventanilla de un coche, donde la valla vuela y la montana casi no se
//! mueve.
//!
//! Asi que la camara no mueve nada: solo contesta **cuanto se ha desplazado cada
//! capa**, y la ciudad se dibuja en esa posicion. Dos numeros y una division.
//!
//! ## Por que un modulo aparte para tan poco
//!
//! Porque es la unica pieza que habla de TIEMPO en un crate que por lo demas es
//! geometria, y porque el dia que haya una tercera capa --o un movimiento
//! vertical, o un zoom-- el cambio es aqui y solo aqui. Un `avance / 3` suelto en
//! mitad del dibujado es una decision escondida en una expresion.

/// Cuanto mas lento va el fondo que el frente.
///
/// Tres, y el numero importa mas de lo que parece: con 2 las dos capas se leen
/// como una sola cosa mal alineada, y con 6 el fondo se queda tan quieto que
/// parece un telon pintado. Tres es donde se separan sin romperse.
pub const LENTITUD_DEL_FONDO: i32 = 3;

/// Cuanto deriva el marco respecto al avance, en centesimas. Ver
/// [`Camara::desplazamiento`]: la capa mas cercana se mueve la que menos, y hay
/// una razon de reloj detras.
pub const DERIVA_DEL_MARCO: i32 = 12;

/// La camara: un solo numero, los pixeles que lleva avanzados.
#[derive(Clone, Copy, Default)]
pub struct Camara {
    pub avance: i32,
}

impl Camara {
    pub fn nueva(avance: i32) -> Self {
        Camara { avance }
    }

    /// Cuanto se ha desplazado la capa `capa`, en pixeles hacia la izquierda.
    ///
    /// `0` es el fondo, `1` el frente y `2` el marco. Lo de delante se mueve
    /// entero; lo de detras, la fraccion.
    ///
    /// ** Y LA CAPA 2 ROMPE LA REGLA A PROPOSITO. El marco esta mas cerca que
    /// nada y sin embargo se mueve MENOS que el frente. No es un descuido: el
    /// marco esta anclado a los bordes de la pantalla, y con paralaje de primer
    /// plano de verdad --dos o tres veces el frente-- se saldria entero en los
    /// 2,4 segundos que dura la intro. El plano que existe para cerrar la escena
    /// la abriria por los lados. Ver la cabecera de [`crate::marco`].
    pub fn desplazamiento(&self, capa: u8) -> i32 {
        match capa {
            0 => self.avance / LENTITUD_DEL_FONDO,
            1 => self.avance,
            _ => self.avance * DERIVA_DEL_MARCO / 100,
        }
    }
}

#[cfg(test)]
mod pruebas {
    use super::*;

    /// La propiedad que define el paralaje: el frente SIEMPRE va por delante
    /// del fondo. Si esto se invierte, la escena se lee del reves.
    #[test]
    fn el_frente_avanza_mas_que_el_fondo() {
        for a in [1, 7, 60, 999] {
            let c = Camara::nueva(a);
            assert!(
                c.desplazamiento(1) > c.desplazamiento(0),
                "con avance {} el frente no adelanta al fondo",
                a
            );
        }
    }

    /// Sin avance, nada se mueve. Es el primer fotograma, y tiene que salir
    /// identico a la ciudad quieta.
    #[test]
    fn sin_avance_las_dos_capas_estan_en_cero() {
        let c = Camara::nueva(0);
        assert_eq!(c.desplazamiento(0), 0);
        assert_eq!(c.desplazamiento(1), 0);
    }
}

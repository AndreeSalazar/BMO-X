//! **La escalera de Linux para un aparato que esta DENTRO y falla** (2026-09-17).
//!
//! Eddi: *"analiza en linux como es reintento con teclado para que nunca
//! muera con mouse... eso se aplica con todos los USB"*. En Linux el que no
//! deja morir a un teclado no es `hub.c` (ese es el que lo hace ENTRAR): es
//! `usbhid`, `hid_io_error()` en `drivers/hid/usbhid/hid-core.c`:
//!
//! ```text
//!   un URB de entrada falla (-EPROTO, -ETIME, -EILSEQ...)
//!     -> se reintenta a intervalos CRECIENTES: 13, 26, 52, 104, 104... ms
//!     -> si hace mas de medio segundo del ultimo error, es una racha NUEVA
//!        y se empieza otra vez en 13 ms
//!     -> si los errores llevan UN SEGUNDO seguidos, se deja de reintentar
//!        el endpoint y se RESETEA EL APARATO ENTERO (usb_reset_device):
//!        reset del puerto y re-enumeracion en su sitio
//!   un -EPIPE (stall) -> clear_halt (nuestro `recuperar_endpoint`)
//! ```
//!
//! Y el raton y el teclado no se mueren juntos porque cada uno tiene SU
//! racha: la del raton no toca al teclado.
//!
//! Lo que hacia BMO-X hasta hoy: rearmar AL INSTANTE tras cada error, sin
//! cuenta ni tope. Un aparato roto giraba 250 veces por segundo y nadie se
//! enteraba de que estaba roto; y si el endpoint se quedaba parado sin
//! evento, solo se encendia una luz (E6) y no se hacia nada.
//!
//! Este modulo es la CUENTA, sin xHC: lo unico que decide es CUANDO rearmar
//! y CUANDO rendirse con el endpoint y reiniciar el aparato. Por eso se
//! prueba sola. Las vueltas son las del bombeo: una cada 4 ms.
//!
//! [carril]  VERDE     aritmetica sobre contadores; no toca el bus
//! [cuesta]  TAREA     una espera mal contada retrasa un rearme, no rompe nada
//! [riesgo]  RELOJ     esta calibrado a 4 ms por vuelta; si el bombeo cambia
//!                     de ritmo, los milisegundos de aqui cambian con el
//! [consumo] NADA      solo cuenta cuando hay errores

/// Vueltas del bombeo por milisegundo (el bus late cada 4 ms).
const VUELTAS_POR_MS: u64 = 1;
/// La primera espera tras un error: 13 ms (Linux). En vueltas de 4 ms, 3.
const PRIMERA_ESPERA: u64 = 13 / 4;
/// La espera no pasa de ~104 ms (Linux dobla hasta pasar de 100).
const ESPERA_TOPE: u64 = 104 / 4;
/// Medio segundo sin errores = la siguiente es una racha NUEVA.
const RACHA_NUEVA: u64 = 500 / 4 * VUELTAS_POR_MS;
/// Un segundo de errores seguidos = se rinde con el endpoint y se reinicia
/// el aparato.
const REINICIO: u64 = 1000 / 4 * VUELTAS_POR_MS;

/// La racha de errores de UN aparato.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Racha {
    /// Errores en esta racha.
    pub errores: u32,
    /// Vuelta del primer error de la racha.
    desde: u64,
    /// Vuelta del ultimo error.
    ultimo: u64,
    /// Espera actual entre rearmes, en vueltas.
    espera: u64,
    /// Vuelta a partir de la cual se puede rearmar.
    proximo: u64,
}

/// Lo que hay que hacer tras anotar un error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paso {
    /// Esperar y rearmar cuando `puede_rearmar` lo diga.
    Esperar,
    /// Un segundo de errores: reiniciar el aparato entero.
    Reiniciar,
}

impl Racha {
    pub const fn nueva() -> Self {
        Self { errores: 0, desde: 0, ultimo: 0, espera: 0, proximo: 0 }
    }

    /// Un informe bueno: la racha se cierra.
    pub fn bien(&mut self) {
        self.errores = 0;
        self.espera = 0;
        self.proximo = 0;
    }

    /// Un error en la vuelta `ahora`. Decide si se espera o se reinicia.
    pub fn error(&mut self, ahora: u64) -> Paso {
        if self.errores == 0 || ahora.saturating_sub(self.ultimo) > RACHA_NUEVA {
            self.errores = 0;
            self.desde = ahora;
            self.espera = PRIMERA_ESPERA;
        } else if self.espera < ESPERA_TOPE {
            // 13, 26, 52, 104 y ahi se queda (Linux dobla hasta pasar de 100).
            self.espera = (self.espera * 2).min(ESPERA_TOPE);
        }
        self.errores += 1;
        self.ultimo = ahora;
        self.proximo = ahora + self.espera;
        if ahora.saturating_sub(self.desde) >= REINICIO {
            Paso::Reiniciar
        } else {
            Paso::Esperar
        }
    }

    /// Ya paso la espera de esta racha? (sin racha, siempre si)
    pub fn puede_rearmar(&self, ahora: u64) -> bool {
        self.errores == 0 || ahora >= self.proximo
    }

    /// Hay una racha abierta?
    pub fn en_racha(&self) -> bool {
        self.errores > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn el_primer_error_espera_13_ms_y_los_siguientes_doblan_hasta_104() {
        let mut r = Racha::nueva();
        assert_eq!(r.error(100), Paso::Esperar);
        assert!(!r.puede_rearmar(100 + PRIMERA_ESPERA - 1));
        assert!(r.puede_rearmar(100 + PRIMERA_ESPERA));
        let mut esperas = [0u64; 6];
        let mut v = 100 + PRIMERA_ESPERA;
        for e in esperas.iter_mut() {
            r.error(v);
            *e = r.proximo - v;
            v = r.proximo;
        }
        assert_eq!(esperas, [6, 12, 24, 26, 26, 26], "doblan y topan en ~104 ms");
    }

    #[test]
    fn medio_segundo_sin_errores_abre_una_racha_nueva() {
        let mut r = Racha::nueva();
        r.error(0);
        r.error(3);
        r.error(9);
        assert_eq!(r.espera, 12);
        r.error(9 + RACHA_NUEVA + 1);
        assert_eq!(r.espera, PRIMERA_ESPERA, "vuelve a empezar en 13 ms");
        assert_eq!(r.errores, 1);
    }

    #[test]
    fn un_segundo_de_errores_seguidos_reinicia_el_aparato() {
        let mut r = Racha::nueva();
        let mut v = 0u64;
        let mut paso = r.error(v);
        while paso == Paso::Esperar {
            v = r.proximo;
            paso = r.error(v);
            assert!(v < 2 * REINICIO, "no puede tardar mas del doble");
        }
        assert!(v >= REINICIO, "no antes del segundo: {v}");
    }

    #[test]
    fn un_informe_bueno_cierra_la_racha() {
        let mut r = Racha::nueva();
        r.error(5);
        assert!(r.en_racha());
        r.bien();
        assert!(!r.en_racha());
        assert!(r.puede_rearmar(6));
    }
}

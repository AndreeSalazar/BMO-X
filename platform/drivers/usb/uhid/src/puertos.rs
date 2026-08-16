//! **La contabilidad de puertos**: a cual se puede tocar y a cual no.
//!
//! === Por que existe este modulo ===
//!
//! La foto del 2026-07-31 en el Ryzen: el teclado escribia **unos segundos** y
//! se moria, el RGB del raton parpadeando sin parar, y en el registro de
//! arranque los slots subiendo `0x30`, `0x31`, ... `0x40` hasta que el
//! controlador contesto `cc=0x9` (*No Slots Available*).
//!
//! Todo eso era **un solo bucle que se alimentaba a si mismo**:
//!
//! ```text
//!   el xHC avisa "cambio el puerto N"
//!     -> adoptar_puerto(N)
//!       -> port_reset(N)            <- y un reset ES un cambio de puerto
//!         -> el xHC avisa "cambio el puerto N"
//!           -> adoptar_puerto(N) ...
//! ```
//!
//! Y el puerto N era **el del propio teclado**, que ya estaba enumerado y
//! bombeando: cada vuelta lo reseteaba, lo volvia a direccionar en un slot
//! nuevo, encontraba que no habia nada que adoptar (su interfaz de teclado ya
//! estaba tomada), tiraba el slot y volvia a empezar. El teclado moria con el
//! primer reset -- de ahi los "unos segundos".
//!
//! === Las dos reglas, y por que son dos ===
//!
//! 1. **A un puerto que ya dio un aparato no se le vuelve a tocar.** Resetear
//!    un puerto que funciona no puede salir bien: en el mejor caso no cambia
//!    nada, en el peor mata lo que habia. Es la regla que salva al teclado.
//! 2. **Un puerto que no da nada se intenta un numero FINITO de veces.** La
//!    regla 1 no basta: un puerto con algo que no sabemos adoptar nunca queda
//!    "tomado", asi que sin un tope seguiria girando para siempre. Es la regla
//!    que corta la realimentacion.
//!
//! Ninguna de las dos impide el hot-plug de verdad: **desenchufar libera el
//! puerto y le devuelve los intentos**, que es lo que distingue "ya lo probe"
//! de "esto es otro aparato".
//!
//! Vive aparte del driver porque es la unica parte de todo esto que **se puede
//! probar sin un xHC delante** -- y era justo la parte que estaba mal.

/// Cuantos puertos se contabilizan.
///
/// * TREINTA Y DOS, y no dieciseis. El numero viejo era una suposicion --"el xHC
/// de esta placa declara menos"-- y un xHC declara los puertos USB2 y los USB3
/// por separado, asi que veinte o mas es corriente. Lo que hacia esa suposicion
/// no era desperdiciar memoria: era **cerrar el hot-plug de los puertos altos en
/// silencio**. Un puerto fuera de rango contesta `intentos = MAX_INTENTOS`, o
/// sea `se_puede_intentar = false`, o sea `adoptar_puerto` se va sin tocar el bus
/// y CABINA dice `nada que adoptar`. Y como el recorrido del arranque llama a
/// `cosechar_puerto` directamente --sin pasar por esta contabilidad--, el sintoma
/// era el peor posible: **enumera al arrancar y no vuelve nunca si se pierde**.
///
/// Cuesta un `u32` en vez de un `u16` y dieciseis bytes mas de array.
pub const MAX_PUERTOS: usize = 32;

/// Cuantas veces se intenta adoptar un puerto que no da nada.
///
/// Tres y no uno: la razon de que exista la re-enumeracion reactiva es que un
/// aparato puede tardar en engancharse (un raton con firmware RGB tarda mas
/// que un teclado). Tres y no infinito: ver la regla 2.
pub const MAX_INTENTOS: u8 = 3;

/// Que puertos estan tomados y cuantas veces se ha intentado cada uno.
#[derive(Debug, Clone, Copy)]
pub struct Puertos {
    tomados: u32,
    intentos: [u8; MAX_PUERTOS],
}

impl Default for Puertos {
    fn default() -> Self {
        Self::nuevo()
    }
}

impl Puertos {
    pub const fn nuevo() -> Self {
        Self { tomados: 0, intentos: [0; MAX_PUERTOS] }
    }

    fn cabe(port: u8) -> bool {
        (port as usize) < MAX_PUERTOS
    }

    /// Esta tomado? Es decir: salio de ahi un aparato que esta funcionando?
    pub fn tomado(&self, port: u8) -> bool {
        Self::cabe(port) && self.tomados & (1 << port) != 0
    }

    /// Intentos gastados en este puerto.
    pub fn intentos(&self, port: u8) -> u8 {
        if Self::cabe(port) { self.intentos[port as usize] } else { MAX_INTENTOS }
    }

    /// * La pregunta que corta el bucle: **merece la pena tocar este puerto?**
    ///
    /// No, si ya dio un aparato (tocarlo solo puede romperlo) y no, si ya se
    /// intento lo suficiente. Un puerto fuera de rango tampoco: mejor no
    /// enumerar que enumerar a ciegas.
    pub fn se_puede_intentar(&self, port: u8) -> bool {
        Self::cabe(port) && !self.tomado(port) && self.intentos[port as usize] < MAX_INTENTOS
    }

    /// Se va a intentar. Cuenta el intento **antes** de tocar el bus: si la
    /// enumeracion se cuelga o se sale por otro camino, el intento ya esta
    /// contado y el bucle no puede volver eternamente por el mismo sitio.
    pub fn anotar_intento(&mut self, port: u8) {
        if Self::cabe(port) {
            self.intentos[port as usize] = self.intentos[port as usize].saturating_add(1);
        }
    }

    /// De aqui salio un aparato que esta instalado: el puerto queda tomado.
    pub fn take(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados |= 1 << port;
        }
    }

    /// Se desenchufo: el puerto vuelve a estar libre **y con los intentos
    /// devueltos**. Sin esto, enchufar y desenchufar tres veces dejaria un
    /// puerto inservible hasta el siguiente reinicio.
    pub fn release(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados &= !(1 << port);
            self.intentos[port as usize] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// * El fallo que mato al teclado en el Ryzen: el bucle de adopcion
    /// reseteaba el puerto donde ya habia un teclado enumerado y bombeando.
    #[test]
    fn a_un_puerto_que_ya_dio_un_aparato_no_se_le_vuelve_a_tocar() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(2);
        p.take(2);
        assert!(!p.se_puede_intentar(2), "resetearlo solo puede romperlo");
        assert!(p.tomado(2));
    }

    /// * Y la otra mitad: un puerto que NUNCA llega a estar tomado no puede
    /// girar para siempre. Es lo que pasaba con el puerto que enumeraba bien y
    /// no tenia nada que adoptar.
    #[test]
    fn un_puerto_que_no_da_nada_se_intenta_un_numero_finito_de_veces() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            assert!(p.se_puede_intentar(3));
            p.anotar_intento(3);
        }
        assert!(!p.se_puede_intentar(3), "aqui es donde se corta la realimentacion");
    }

    /// Pero se reintenta lo justo: la re-enumeracion reactiva existe porque un
    /// aparato puede tardar en engancharse. Un solo intento seria volver al
    /// fallo anterior.
    #[test]
    fn se_reintenta_mas_de_una_vez() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(1);
        assert!(p.se_puede_intentar(1), "el primer fallo no puede ser el ultimo");
    }

    /// Desenchufar devuelve el puerto Y los intentos: lo que se enchufe despues
    /// es otro aparato y merece sus oportunidades.
    #[test]
    fn desenchufar_libera_el_puerto_y_devuelve_los_intentos() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            p.anotar_intento(4);
        }
        p.take(4);
        p.release(4);
        assert!(p.se_puede_intentar(4));
        assert_eq!(p.intentos(4), 0);
        assert!(!p.tomado(4));
    }

    /// Los puertos son independientes: agotar uno no puede cerrar el de al lado.
    #[test]
    fn los_puertos_no_se_pisan() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            p.anotar_intento(0);
        }
        p.take(5);
        assert!(!p.se_puede_intentar(0));
        assert!(!p.se_puede_intentar(5));
        assert!(p.se_puede_intentar(6), "este no tiene nada que ver");
    }

    /// Un puerto fuera de rango no rompe nada y no se enumera.
    #[test]
    fn un_puerto_fuera_de_rango_no_se_toca_ni_desborda() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(200);
        p.take(200);
        p.release(200);
        assert!(!p.se_puede_intentar(200));
        assert!(!p.tomado(200));
    }

    /// El estado inicial: todos libres. Si esto fallara, el arranque no
    /// enumeraria nada y la maquina se quedaria sin teclado.
    #[test]
    fn al_arrancar_todos_los_puertos_son_intentables() {
        let p = Puertos::nuevo();
        for puerto in 0..MAX_PUERTOS as u8 {
            assert!(p.se_puede_intentar(puerto));
        }
    }
}

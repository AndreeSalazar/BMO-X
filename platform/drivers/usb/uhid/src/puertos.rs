//! **La contabilidad de puertos**: a cuál se puede tocar y a cuál no.
//!
//! ═══ Por qué existe este módulo ═══
//!
//! La foto del 2026-07-31 en el Ryzen: el teclado escribía **unos segundos** y
//! se moría, el RGB del ratón parpadeando sin parar, y en el registro de
//! arranque los slots subiendo `0x30`, `0x31`, … `0x40` hasta que el
//! controlador contestó `cc=0x9` (*No Slots Available*).
//!
//! Todo eso era **un solo bucle que se alimentaba a sí mismo**:
//!
//! ```text
//!   el xHC avisa "cambió el puerto N"
//!     -> adoptar_puerto(N)
//!       -> port_reset(N)            <- y un reset ES un cambio de puerto
//!         -> el xHC avisa "cambió el puerto N"
//!           -> adoptar_puerto(N) ...
//! ```
//!
//! Y el puerto N era **el del propio teclado**, que ya estaba enumerado y
//! bombeando: cada vuelta lo reseteaba, lo volvía a direccionar en un slot
//! nuevo, encontraba que no había nada que adoptar (su interfaz de teclado ya
//! estaba tomada), tiraba el slot y volvía a empezar. El teclado moría con el
//! primer reset — de ahí los "unos segundos".
//!
//! ═══ Las dos reglas, y por qué son dos ═══
//!
//! 1. **A un puerto que ya dio un aparato no se le vuelve a tocar.** Resetear
//!    un puerto que funciona no puede salir bien: en el mejor caso no cambia
//!    nada, en el peor mata lo que había. Es la regla que salva al teclado.
//! 2. **Un puerto que no da nada se intenta un número FINITO de veces.** La
//!    regla 1 no basta: un puerto con algo que no sabemos adoptar nunca queda
//!    "tomado", así que sin un tope seguiría girando para siempre. Es la regla
//!    que corta la realimentación.
//!
//! Ninguna de las dos impide el hot-plug de verdad: **desenchufar libera el
//! puerto y le devuelve los intentos**, que es lo que distingue "ya lo probé"
//! de "esto es otro aparato".
//!
//! Vive aparte del driver porque es la única parte de todo esto que **se puede
//! probar sin un xHC delante** — y era justo la parte que estaba mal.

/// Cuántos puertos se contabilizan. El xHC de esta placa declara menos, y
/// pasarse por arriba no cuesta nada: es un bitmask y un array de bytes.
pub const MAX_PUERTOS: usize = 16;

/// Cuántas veces se intenta adoptar un puerto que no da nada.
///
/// Tres y no uno: la razón de que exista la re-enumeración reactiva es que un
/// aparato puede tardar en engancharse (un ratón con firmware RGB tarda más
/// que un teclado). Tres y no infinito: ver la regla 2.
pub const MAX_INTENTOS: u8 = 3;

/// Qué puertos están tomados y cuántas veces se ha intentado cada uno.
#[derive(Debug, Clone, Copy)]
pub struct Puertos {
    tomados: u16,
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

    /// ¿Está tomado? Es decir: ¿salió de ahí un aparato que está funcionando?
    pub fn tomado(&self, port: u8) -> bool {
        Self::cabe(port) && self.tomados & (1 << port) != 0
    }

    /// Intentos gastados en este puerto.
    pub fn intentos(&self, port: u8) -> u8 {
        if Self::cabe(port) { self.intentos[port as usize] } else { MAX_INTENTOS }
    }

    /// ★ La pregunta que corta el bucle: **¿merece la pena tocar este puerto?**
    ///
    /// No, si ya dio un aparato (tocarlo sólo puede romperlo) y no, si ya se
    /// intentó lo suficiente. Un puerto fuera de rango tampoco: mejor no
    /// enumerar que enumerar a ciegas.
    pub fn se_puede_intentar(&self, port: u8) -> bool {
        Self::cabe(port) && !self.tomado(port) && self.intentos[port as usize] < MAX_INTENTOS
    }

    /// Se va a intentar. Cuenta el intento **antes** de tocar el bus: si la
    /// enumeración se cuelga o se sale por otro camino, el intento ya está
    /// contado y el bucle no puede volver eternamente por el mismo sitio.
    pub fn anotar_intento(&mut self, port: u8) {
        if Self::cabe(port) {
            self.intentos[port as usize] = self.intentos[port as usize].saturating_add(1);
        }
    }

    /// De aquí salió un aparato que está instalado: el puerto queda tomado.
    pub fn tomar(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados |= 1 << port;
        }
    }

    /// Se desenchufó: el puerto vuelve a estar libre **y con los intentos
    /// devueltos**. Sin esto, enchufar y desenchufar tres veces dejaría un
    /// puerto inservible hasta el siguiente reinicio.
    pub fn soltar(&mut self, port: u8) {
        if Self::cabe(port) {
            self.tomados &= !(1 << port);
            self.intentos[port as usize] = 0;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ★ El fallo que mató al teclado en el Ryzen: el bucle de adopción
    /// reseteaba el puerto donde ya había un teclado enumerado y bombeando.
    #[test]
    fn a_un_puerto_que_ya_dio_un_aparato_no_se_le_vuelve_a_tocar() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(2);
        p.tomar(2);
        assert!(!p.se_puede_intentar(2), "resetearlo solo puede romperlo");
        assert!(p.tomado(2));
    }

    /// ★ Y la otra mitad: un puerto que NUNCA llega a estar tomado no puede
    /// girar para siempre. Es lo que pasaba con el puerto que enumeraba bien y
    /// no tenía nada que adoptar.
    #[test]
    fn un_puerto_que_no_da_nada_se_intenta_un_numero_finito_de_veces() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            assert!(p.se_puede_intentar(3));
            p.anotar_intento(3);
        }
        assert!(!p.se_puede_intentar(3), "aqui es donde se corta la realimentacion");
    }

    /// Pero se reintenta lo justo: la re-enumeración reactiva existe porque un
    /// aparato puede tardar en engancharse. Un solo intento sería volver al
    /// fallo anterior.
    #[test]
    fn se_reintenta_mas_de_una_vez() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(1);
        assert!(p.se_puede_intentar(1), "el primer fallo no puede ser el ultimo");
    }

    /// Desenchufar devuelve el puerto Y los intentos: lo que se enchufe después
    /// es otro aparato y merece sus oportunidades.
    #[test]
    fn desenchufar_libera_el_puerto_y_devuelve_los_intentos() {
        let mut p = Puertos::nuevo();
        for _ in 0..MAX_INTENTOS {
            p.anotar_intento(4);
        }
        p.tomar(4);
        p.soltar(4);
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
        p.tomar(5);
        assert!(!p.se_puede_intentar(0));
        assert!(!p.se_puede_intentar(5));
        assert!(p.se_puede_intentar(6), "este no tiene nada que ver");
    }

    /// Un puerto fuera de rango no rompe nada y no se enumera.
    #[test]
    fn un_puerto_fuera_de_rango_no_se_toca_ni_desborda() {
        let mut p = Puertos::nuevo();
        p.anotar_intento(200);
        p.tomar(200);
        p.soltar(200);
        assert!(!p.se_puede_intentar(200));
        assert!(!p.tomado(200));
    }

    /// El estado inicial: todos libres. Si esto fallara, el arranque no
    /// enumeraría nada y la máquina se quedaría sin teclado.
    #[test]
    fn al_arrancar_todos_los_puertos_son_intentables() {
        let p = Puertos::nuevo();
        for puerto in 0..MAX_PUERTOS as u8 {
            assert!(p.se_puede_intentar(puerto));
        }
    }
}

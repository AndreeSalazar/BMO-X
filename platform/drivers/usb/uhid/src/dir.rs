//! La DIRECCION de un periferico dentro del controlador.
//!
//! ## Por que esto es un tipo y no dos `u8` sueltos
//!
//! Un aparato USB, para el xHC, es el par `(slot, dci)`: el slot lo asigna el
//! controlador al direccionar el dispositivo, y el DCI identifica uno de sus
//! endpoints. Todo Transfer Event trae ese par, y **es lo unico que dice de
//! quien es el informe**.
//!
//! Mientras fueron dos campos sueltos repetidos en cada struct, la comparacion
//! se escribia a mano en cada sitio (`ev_slot == k.slot && ev_ep == k.dci`), y
//! **nada impedia que dos perifericos llevaran el mismo par**. No es teorico:
//! con el bug de enumeracion del teclado compuesto, el raton y el teclado
//! acabaron los dos en el slot 2 con el mismo DCI -- y como el despacho probaba
//! las dos ramas con `if` independientes, **el mismo informe de 8 bytes se leia
//! como teclado Y como raton a la vez**.
//!
//! Con un tipo propio, "son el mismo aparato?" es una pregunta que se hace una
//! vez y se responde igual en todas partes. Ver [`Direccion::choca_con`].

/// El par `(slot, dci)` que identifica un endpoint concreto de un aparato.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Direccion {
    pub slot: u8,
    pub dci: u8,
}

impl Direccion {
    pub const fn nueva(slot: u8, dci: u8) -> Self {
        Self { slot, dci }
    }

    /// Este Transfer Event es de este endpoint?
    #[inline]
    pub fn es_mio(&self, slot: u8, ep: u8) -> bool {
        self.slot == slot && self.dci == ep
    }

    /// Dos perifericos estarian compartiendo el mismo endpoint?
    ///
    /// Si esto es cierto, uno de los dos esta mal enumerado: dos aparatos
    /// distintos no pueden tener la misma direccion. Aceptarlo hace que cada
    /// informe se decodifique dos veces, con dos formatos distintos, y ninguno
    /// de los dos resultados significa nada.
    #[inline]
    pub fn choca_con(&self, otra: Direccion) -> bool {
        *self == otra
    }
}

//! **Lo que CABINA lee del USB.** Solo lectura, y eso es el contrato.
//!
//! Salio de `dev/usb/mod.rs` el 2026-08-12. Se puede sacar solo porque **ni una
//! de estas funciones cambia un byte de estado**: son ventanas. Tenerlas juntas
//! y aparte hace visible una regla que antes habia que confiar en que se
//! cumpliera -- si algun dia una de estas escribe, se vera en el `diff` de un
//! fichero que dice en su primera linea que no escribe.

use super::*;

pub fn hid_stats() -> (bool, bool, u8, u8, u32, i32, i32, u8, u32) {
    unsafe {
        (KBD_RDY, MOUSE_RDY, KBD_SLOT, MOUSE_SLOT,
         MOUSE_EVENTS, MOUSE_X, MOUSE_Y, MOUSE_BTN, KEY_EVENTS)
    }
}

/// Contadores de bajo nivel del xHC + HID para cazar el corte del teclado:
/// (transfer_events_del_xHC, raw_events_del_xHC, hid_events_totales).
/// Si al teclear TEV no sube -> el xHC no completa la interrupcion (endpoint/
/// ring/doorbell). Si TEV sube pero HEV no -> el evento no matchea al teclado.
/// Si HEV sube pero kev no -> mapeo (ya no deberia tras el keypad).
pub fn xfer_stats() -> (u32, u32, u32) {
    (bmo_xhci::xfer_events(), bmo_xhci::raw_events(), unsafe { HID_EVENTS })
}

/// El reparto de informes: `(bombea el teclado, bombea el raton, huerfanos)`.
///
/// Los dos primeros son la pregunta que no se podia hacer: un periferico sin
/// transferencia encolada esta enumerado, con el endpoint en `Running`, y mudo
/// para siempre. El tercero cuenta los Transfer Events que no eran de ningun
/// periferico conocido -- antes se descartaban sin dejar rastro.
pub fn reparto_stats() -> (bool, bool, u32) {
    unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        let (k, r) = hid.bombeando();
        (k, r, hid.huerfanos())
    }
}

/// **Las puertas del bus**: `(avisos esperando, avisos PERDIDOS, barridos,
/// barridos que repararon algo)`.
///
/// Es lo que hay que mirar cuando el teclado "se olvida":
///
/// * `perdidos > 0` -- la cola de avisos se lleno. No es fatal (para eso esta el
///   barrido), pero significa que los enchufes llegaron mas rapido de lo que se
///   atendieron.
/// * `barridos` subiendo y `utiles` en cero -- el bus esta sano y el barrido no
///   ha tenido que reparar nada. Es lo normal.
/// * `utiles` subiendo -- **se estan perdiendo avisos de verdad**. El sistema se
///   repara solo, pero ahi hay algo que investigar: cada uno de esos es medio
///   segundo en que el teclado no respondia.
pub fn puertas_stats() -> (usize, u32, u64, u64) {
    let (esperando, perdidos) = bmo_xhci::avisos_stats();
    let (hechos, utiles) = super::barrido_stats();
    (esperando, perdidos, hechos, utiles)
}

/// El aparcadero de eventos del xHC: `(aparcados en total, PERDIDOS, ahora)`.
///
/// El anillo de eventos es uno para todo el controlador, asi que quien espera
/// una complecion de comando se cruza con los informes de los aparatos que ya
/// estan bombeando. Antes los descartaba, y descartar el primer informe de un
/// endpoint lo deja mudo para siempre -- nadie vuelve a encolar la
/// transferencia. Ahora se aparcan; `PERDIDOS` es lo que hay que vigilar.
pub fn park_stats() -> (u32, u32, u32) {
    bmo_xhci::evt_park_stats()
}

/// Salud del endpoint de interrupcion del teclado leida DEL HARDWARE, no de
/// nuestras suposiciones: `(ep_state, bInterval_del_descriptor,
/// Interval_programado, speed, usbsts)`.
///
/// `ep_state` sale del Device Context que mantiene el xHC: 1=Running es lo
/// unico aceptable. 2=Halted, 3=Stopped o 4=Error significan que el endpoint no
/// esta agendado y ningun doorbell lo va a revivir. `bi`/`iv` delatan el bug
/// clasico del Interval (ver `bmo_xhci::encode_interval`).
pub fn kbd_ep_debug() -> (u8, u8, u8, u8, u32) {
    let (slot, dci) = unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        (KBD_SLOT, hid.kbd_dci())
    };
    let st = unsafe { bmo_xhci::ep_state(slot, dci) };
    let (bi, iv, sp) = bmo_xhci::last_ep_timing();
    (st, bi, iv, sp, unsafe { bmo_xhci::usbsts() })
}

/// DCI del teclado + ultimo Transfer Event (slot, ep, cc) del xHC. Si el ep del
/// ultimo evento != dci del teclado, el evento no matchea y no se re-encola ->
/// tev pegado en 1. Ese es el corte que buscamos.
pub fn kbd_debug() -> (u8, u8, u8, u8) {
    let dci = unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        hid.kbd_dci()
    };
    let (s, e, c) = bmo_xhci::last_event();
    (dci, s, e, c)
}

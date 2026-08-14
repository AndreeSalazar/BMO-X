//! **THE RECORDER** -- the event ring CABINA writes into.
//!
//! === Why this is the first file of the folder ===
//!
//! Because everything else here is a reader. `info`, `warn`, `fault` and
//! `panic` push into this ring; the cockpit paints it; the black box writes it
//! to disk. If this is wrong, the other five are faithfully reporting garbage.
//!
//! It records **from the first second**, before there is a framebuffer to show
//! it on, and shows it when a screen exists. If the kernel dies between the
//! BootContext check and the shell, the ring already holds what happened.

use super::*;

// -- Buffer de EVENTOS: la grabadora -----------------------------------------
// Ring de eventos con severidad/capa/entidad. `cabina-core::Event` ya trae
// severidad, capa (from_module), modulo, mensaje y valor.

pub(crate) const EVENT_RING: usize = 48;
pub(crate) static mut EVENTS: [Event; EVENT_RING] = [Event::ZERO; EVENT_RING];
pub(crate) static mut EV_WRITE: usize = 0;
pub(crate) static mut EV_SEQ: u64 = 0;
pub(crate) static mut EV_TOTAL: u64 = 0;
pub(crate) static mut EV_LOST: u64 = 0;
pub(crate) static mut BUSY: bool = false;

#[inline]
pub(crate) fn irq_save() -> u64 {
    let f: u64;
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) f); }
    f
}

#[inline]
pub(crate) fn irq_restore(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
}

/// Graba un evento. Seguro desde IRQ y desde el manejador de faults. La capa
/// se infiere del nombre del modulo (`Layer::from_module`): "usb"->ring0,
/// "lang"->lang, "cap"->sec, etc.
/// ** DE DONDE SALIO CADA EVENTO, SIN TOCAR NI UNA LLAMADA (2026-08-11).
///
/// `#[track_caller]` le pide al compilador el fichero y la linea **del que
/// llama**, no de esta funcion. Sale gratis --se resuelve al compilar-- y lo
/// mejor es lo que NO hay que hacer: las doscientas llamadas a `info`, `warn` y
/// `fault` repartidas por el kernel se quedan exactamente como estan.
///
/// == Por que hacia falta, contado con lo que costo ==
///
/// El 2026-08-10 una linea decia `cabecera invalida (magic, version o 0
/// secciones)` y para saber quien la habia escrito hubo que buscar la frase por
/// todo el arbol. Funciona hasta que dos sitios dicen lo mismo, o hasta que
/// alguien reescribe la frase y el `grep` deja de encontrarla.
///
/// == Y esto NO es darle un cerebro al kernel ==
///
/// No hay nada que deducir: el sitio lo sabe el compilador en el momento de
/// emitir. Guardarlo no es analizar, es **dejar de tirar un dato que ya se
/// tenia**. El kernel sigue sin interpretar nada -- apunta hechos, y quien los
/// agrupe, encadene y narre puede vivir en Ring 3 y leerlos.
///
/// Es el mismo movimiento de esta semana, por cuarta vez: `bex::necesita`
/// deducia lo que el fichero podia declarar; `tramo_dma` preguntaba una
/// traduccion que el mapeo ya garantizaba; una falta de cabecera no ensenaba los
/// bytes que la provocaron. **Quitar la pregunta, no mejorarla.**
#[track_caller]
pub fn record(sev: Severity, module: &str, msg: &str, value: u64) {
    record_fmt(sev, module, msg, value, Fmt::Raw);
}

/// [`record`] saying **how its number is read**. See [`Fmt`].
///
/// The old entry point stays exactly as it was and forwards with `Fmt::Raw`, so
/// none of the two hundred existing call sites has to change to keep working.
/// What changes is that from here on a call site CAN say what it always knew.
#[track_caller]
pub fn record_fmt(sev: Severity, module: &str, msg: &str, value: u64, fmt: Fmt) {
    let sitio = core::panic::Location::caller();
    let flags = irq_save();
    unsafe {
        // Reentrancia (excepcion a media escritura): contar y salir. Nunca
        // dejar el anillo a medio escribir ni girar en un lock imposible.
        if BUSY {
            EV_LOST = EV_LOST.wrapping_add(1);
            irq_restore(flags);
            return;
        }
        BUSY = true;

        let layer = Layer::from_module(module);
        let mut ev = Event::new(sev, layer, Entity::Module, module, 0, msg, value)
            .en(sitio.file(), sitio.line())
            .como(fmt);
        ev.intento = INTENTO_ACTUAL;
        EV_SEQ = EV_SEQ.wrapping_add(1);
        ev.seq = EV_SEQ;
        ev.tick_ns = crate::ring0::plat::timer::ticks();
        let arr = core::ptr::addr_of_mut!(EVENTS) as *mut Event;
        core::ptr::write(arr.add(EV_WRITE), ev);
        EV_WRITE = (EV_WRITE + 1) % EVENT_RING;
        EV_TOTAL = EV_TOTAL.wrapping_add(1);

        BUSY = false;
    }
    irq_restore(flags);
}

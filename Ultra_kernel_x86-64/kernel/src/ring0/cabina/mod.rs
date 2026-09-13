//! CABINA -- el registrador omnisciente del sistema (lado Ring 0).
//!
//! [familia] cabina  nivel 1 -- el REGISTRO: todo el kernel apunta aqui lo que pasa, y solo sabe la hora
//! [conecta] reloj
//!
//! [carril]  AMARILLO  la fachada del registrador
//! [consumo] NADA      apunta o pinta cuando alguien lo llama
//!
//! Le da VIDA a `cabina-core`: mantiene el ANILLO DE EVENTOS del kernel y lo
//! pinta como un cockpit permanente en el framebuffer. La vision del usuario:
//! un observador que "ve todo" entre Ring 0 y Ring 3 -- para dejar de debuggear
//! a ciegas.
//!
//! ## Grabadora, no encuestadora
//!
//! CABINA no adivina el estado comparando contadores al repintar: los modulos
//! EMPUJAN su evento en el instante exacto en que ocurre el hecho
//! (`cabina::info/warn/fault` desde usb, proc, faults, phase...). Consecuencia
//! importante: un hecho queda grabado aunque el shell nunca llegue a correr --
//! justo el escenario donde antes quedabamos ciegos.
//!
//! Lo unico que sigue siendo POLLING es `watch()`, y a proposito: son
//! vigilancias de una CONDICION que se sostiene en el tiempo (RAM baja, un
//! teclado que enumero pero lleva rato mudo), no hechos puntuales.
//!
//! ## Reentrancia
//!
//! `record()` se llama desde el shell, desde `init` y desde el manejador de
//! faults. `cli` cubre la preempcion por IRQ, pero una EXCEPCION no se
//! enmascara: un #PF a media escritura del anillo re-entraria aqui. Por eso
//! hay un flag `BUSY` en vez de un spinlock -- un lock se auto-bloquearia para
//! siempre en ese caso. Los eventos perdidos por reentrancia se CUENTAN y se
//! muestran: preferimos un numero honesto a un hueco silencioso.
//!
//! A futuro: volcado del anillo a disco (NVMe+FAT32) = la caja negra forense,
//! y el buffer de shared-memory para que Ring 3 aporte su parte.

use cabina_core::{Event, Severity, Layer, Entity};
use cabina_core::event::Fmt;

/// THE RECORDER: the event ring everything else here reads from.
/// EL BARRIDO: lo que no se escapa de ningun filtro. Ver su cabecera.
pub mod radar;
pub(crate) mod ring;
pub(crate) use ring::*;
/// THE ATTEMPT: the only thing in CABINA with a lifetime. Its `Drop` marks it
/// "left OPEN" -- the absence of an ending IS the report.
pub(crate) mod attempt;
/// LA CAIDA: lo ultimo que dijo la maquina, en RAM que sobrevive al reinicio.
/// Es la respuesta a *"que escriba en tiempo real"*: no cuesta nada mientras
/// vive, y se lee cuando vuelve. Ver su cabecera -- es una hipotesis sobre la
/// placa hasta que el Ryzen conteste.
pub mod caida;
pub use attempt::*;
/// FORMATTING WITHOUT `std`: a line built by hand in a fixed byte buffer.
pub(crate) mod format;
pub(crate) use format::*;
/// LA LECTURA: lo que Ring 3 pregunta del anillo (`TASK_OP_CABINA_*`). Solo LEE
/// lo apuntado; vivia al final de `cockpit.rs`.
pub(crate) mod lectura;
pub use lectura::*;
// ** El cockpit, las vigilancias y la caja negra se fueron al MIRADOR
// (`ring0/mirador`, L8b 2026-09-13): leen a todo el kernel para ensenarlo, y
// eso no puede vivir en la familia que todo el kernel llama.

// -- ** THE SAME VOCABULARY, SAYING WHAT THE NUMBER IS -----------------------
//
// One per unit rather than one function taking a `Fmt`, and it is deliberate:
// `bytes("arch", "el WAD", n)` reads as a sentence at the call site, while
// `info_fmt("arch", "el WAD", n, Fmt::Bytes)` reads as a call with a flag. The
// call site is where somebody has to remember to say it, so that is where it has
// to be cheap.
//
// Severity stays Info for all of them: a size is not a warning. When something
// IS wrong, `warn`/`fault` still take the raw value -- and the day one of those
// needs a unit too, it gets its own line here and not an extra argument
// everywhere.

/// A count of things. Decimal.
#[track_caller]
pub fn count(module: &str, msg: &str, n: u64) {
    record_fmt(Severity::Info, module, msg, n, Fmt::Count);
}

/// A size in bytes. Prints the scale AND the exact number.
#[track_caller]
pub fn bytes(module: &str, msg: &str, n: u64) {
    record_fmt(Severity::Info, module, msg, n, Fmt::Bytes);
}

/// A memory address. Hex, plus its offset inside the page -- which is the fact
/// that took a day to see in the split-relocation bug.
#[track_caller]
pub fn addr(module: &str, msg: &str, a: u64) {
    record_fmt(Severity::Info, module, msg, a, Fmt::Addr);
}

/// Milliseconds.
#[track_caller]
pub fn millis(module: &str, msg: &str, ms: u64) {
    record_fmt(Severity::Info, module, msg, ms, Fmt::Millis);
}

/// A MAC, packed with byte 0 at the top.
#[track_caller]
pub fn mac(module: &str, msg: &str, m: u64) {
    record_fmt(Severity::Info, module, msg, m, Fmt::Mac);
}

/// A bitfield. Binary, because which bits are set is the whole point and hex
/// hides exactly that.
#[track_caller]
pub fn bits(module: &str, msg: &str, b: u64) {
    record_fmt(Severity::Info, module, msg, b, Fmt::Bits);
}

/// A process or thread id.
#[track_caller]
pub fn id(module: &str, msg: &str, n: u64) {
    record_fmt(Severity::Info, module, msg, n, Fmt::Id);
}

/// Evento `n` posiciones antes del mas reciente (0 = el ultimo). Para mostrar
/// el HISTORIAL, no solo la ultima linea.
pub(crate) fn event_back(n: usize) -> Option<Event> {
    unsafe {
        if n as u64 >= EV_TOTAL || n >= EVENT_RING { return None; }
        let idx = (EV_WRITE + EVENT_RING - 1 - n) % EVENT_RING;
        let arr = core::ptr::addr_of!(EVENTS) as *const Event;
        Some(core::ptr::read(arr.add(idx)))
    }
}

// ** Aqui vivia `boot_probe`, el censo PCI de almacenamiento. Se fue a
// `dev::pci::censo_almacenamiento` (L8b): contar lo que hay en el bus es oficio
// del bus, y era lo que hacia a este registro importar `dev`.

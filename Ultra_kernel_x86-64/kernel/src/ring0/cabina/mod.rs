//! CABINA -- el registrador omnisciente del sistema (lado Ring 0).
//!
//! [carril]  AMARILLO  la fachada del registrador
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

use cabina_core::{TelemetrySnapshot, Event, Severity, Layer, Entity};
use cabina_core::event::Fmt;
use crate::ring0::core::splash::{splash_dashboard_log_color, DASH_LOG_W};

/// THE RECORDER: the event ring everything else here reads from.
/// EL BARRIDO: lo que no se escapa de ningun filtro. Ver su cabecera.
pub mod radar;
pub(crate) mod ring;
pub(crate) use ring::*;
/// THE ATTEMPT: the only thing in CABINA with a lifetime. Its `Drop` marks it
/// "left OPEN" -- the absence of an ending IS the report.
pub(crate) mod attempt;
pub use attempt::*;
/// THE BLACK BOX: the ring, on the disk. The only part that survives a power
/// cut, and the only one that can fail for reasons unrelated to logging.
pub(crate) mod blackbox;
pub use blackbox::*;
/// THE WATCHES: this IS polling, and the reason is that a device which stops
/// answering does not send an event saying so.
pub(crate) mod watch;
pub use watch::*;
/// FORMATTING WITHOUT `std`: a line built by hand in a fixed byte buffer.
pub(crate) mod format;
pub(crate) use format::*;
/// THE COCKPIT: severity colours, filters, layout. Presentation only -- none of
/// it changes what is recorded.
pub(crate) mod cockpit;
pub use cockpit::*;

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

// -- Censo de arranque -------------------------------------------------------

/// Primer paso hacia la CAJA NEGRA: que controlador de disco hay? Saberlo
/// decide el driver a cablear (AHCI vs NVMe). Se llama UNA vez desde
/// `phase::main` -- antes vivia dentro del render, donde un scan PCI por fuerza
/// bruta (~65k lecturas de config) bloqueaba el primer frame.
pub fn boot_probe() {
    info("cabina", "observador omnisciente en linea", 0);
    // CENSO COMPLETO, no "el primero". Saber cuantos controladores de
    // almacenamiento hay y DE QUE TIPO es lo que dice donde buscar un disco.
    // Si la BIOS tiene el SATA del chipset en modo RAID, ese controlador
    // aparece con clase RAID y no con clase AHCI -- y un buscador que solo
    // pregunta por AHCI pasa de largo sin enterarse de que existe.
    let mut index = 0usize;
    let mut found = 0u64;
    while let Some(loc) = crate::ring0::dev::pci::storage_at(index) {
        index += 1;
        found += 1;
        let msg = match loc.kind {
            crate::ring0::dev::pci::StorageKind::Nvme => "controlador NVMe (via PCI)",
            crate::ring0::dev::pci::StorageKind::Ahci => "controlador SATA/AHCI (via PCI)",
            crate::ring0::dev::pci::StorageKind::Raid => "controlador en modo RAID (via PCI)",
            crate::ring0::dev::pci::StorageKind::Ide  => "controlador en modo IDE (via PCI)",
            _ => "controlador de almacenamiento (via PCI)",
        };
        // El valor lleva bus:dev.func empaquetado + el MMIO, para poder
        // localizarlo despues sin volver a barrer el bus.
        info("pci", msg, loc.mmio);
        if index >= 8 { break; }
    }
    if found == 0 {
        warn("pci", "sin controlador de almacenamiento visible", 0);
    } else {
        info("pci", "controladores de almacenamiento hallados", found);
    }
}

//! **El hilo de kernel que mantiene vivo el bus USB.**
//!
//! [carril]  ROJO      el hilo que mantiene vivo el bus
//!
//! Salio de `dev/usb/mod.rs` el 2026-08-12 por la regla modular. Se puede sacar
//! solo porque **no toca ni una tecla**: bombea el bus y mira el rescate. Todo
//! lo que sabe de teclado y raton se lo pregunta al modulo padre.

use super::rescate::watch_rescue;
use super::{bombear_interno, PRESENT};

// -- ** THE BUS BELONGS TO THE KERNEL ----------------------------------------
//
// # The bug, told in full
//
// Until now the USB bus **only advanced when somebody asked for a key**. The
// only two callers of `bombear_interno` were `poll_ascii` and `evento_tecla`,
// that is:
//
//   * the Ring 0 shell -- but **only while `input::yielded()` is false**, which
//     is the exact opposite of when it is needed; and
//   * the `INPUT_OP_*` of whichever program holds the input.
//
// Put those together and you get this: **the moment a Ring 3 program takes the
// input, the only thing keeping the keyboard and mouse alive is that same
// program.** If it hangs, if it spins, or if it merely takes its time -- loading
// a 4 MB WAD, compositing a heavy frame -- the bus stops advancing, and from the
// outside that looks like a frozen machine. It wasn't frozen: it was waiting for
// the hijacker to ask for the time.
//
// And the rescue shortcut was built on top of that same pumping, so it fell with
// it.
//
// # What is done about it
//
// A **kernel thread** ([`bus_thread`]) pumps the bus on its own, with its own
// stack and its own scheduler slice. From here on:
//
//   * keyboard and mouse keep beating even when nobody asks;
//   * the rescue is watched by the thread ([`watch_rescue`]), so it **works even
//     when the input owner is hung**, which is the only case where it is truly
//     needed;
//   * the syscall paths can still pump -- that is not taken away, so a failure of
//     the thread does not leave the system mute -- but they are no longer the
//     only ones.
//
// # The guard, and why it is not optional
//
// `bombear_interno` touches dozens of `static mut` (queues, counters, xHCI
// state). With the thread there are, for the first time, **two** callers the
// timer can interleave mid-work. The guard is the same one CABINA uses: a flag,
// not a `SpinLock` -- a lock here would deadlock against itself if the one
// already inside is the one that got interrupted.
//
// [!] This is NOT SMP-safe and does not pretend to be: it holds because only the
// BSP runs. The day an AP touches the bus, this flag is a race. Written down on
// purpose instead of pretending otherwise.
static mut PUMPING: bool = false;

/// How many turns the bus thread has taken. If this stops rising the thread died
/// or never started -- and the keyboard depends on somebody asking again.
static mut BUS_TURNS: u64 = 0;
/// How many times the pump was found already running. A high number is not a
/// failure: it is the thread and a syscall asking at the same time.
static mut PUMP_OVERLAPS: u64 = 0;

/// **TSC del final de la ultima vuelta del hilo**, y cero mientras no haya dado
/// ninguna.
///
/// `BUS_TURNS` dice *cuantas*; esto dice *cuando*, y esa es la diferencia entre
/// un contador y un latido. Un numero de vueltas hay que recordarlo entre dos
/// miradas para saber si sube --lo que obliga a quien mira a tener memoria, y
/// por eso `cabina/watch.rs` guarda dos `static`s para conseguirlo--. Una marca
/// de tiempo **se juzga de un vistazo y sin recordar nada**, que es lo que
/// necesita un estado leido desde Ring 3 por alguien que acaba de arrancar.
///
/// Lo pone el hilo y solo el hilo: bombear desde un syscall NO es un latido. Si
/// esto se queda quieto, E1 esta caida aunque el bus siga avanzando a ratos.
static mut ULTIMO_LATIDO: u64 = 0;

/// `(thread turns, overlapped pumps)`. For the panel.
pub fn bus_stats() -> (u64, u64) {
    unsafe { (BUS_TURNS, PUMP_OVERLAPS) }
}

/// TSC de la ultima vuelta del hilo, o `0` si no ha dado ninguna. Ver
/// [`ULTIMO_LATIDO`]; lo lee `salud.rs` para poner la edad del latido en
/// `INFO_USB_SALUD`.
pub fn ultimo_latido() -> u64 {
    unsafe { ULTIMO_LATIDO }
}

/// Pumps the bus with the kernel CR3 loaded and without letting two in at once.
/// **This is the only place that calls `bombear_interno`.**
pub(super) fn pump_bus() {
    use crate::ring0::mm::vmm;
    unsafe {
        if PUMPING {
            PUMP_OVERLAPS = PUMP_OVERLAPS.wrapping_add(1);
            return;
        }
        PUMPING = true;
    }
    // xHCI MMIO is only mapped in the kernel PML4. See the header of
    // [`poll_ascii`]: if we are already on the kernel one, this costs nothing.
    let kpml4 = vmm::kernel_pml4();
    let previous = vmm::read_cr3();
    let switched = kpml4 != 0 && previous != kpml4;
    if switched {
        vmm::switch_to(kpml4);
    }
    bombear_interno();
    // *** EL AUDIO COME AQUI, y no en su propio hilo.
    //
    // Una trama isocrona dura 1 ms y este latido son 4, asi que se encolan
    // varias de golpe -- ver `audio::latido`. Un hilo aparte a 1 kHz seria un
    // segundo consumidor del mismo anillo de transferencias, y dos productores
    // sobre un anillo sin cerrojo es como se corrompe uno.
    //
    // [!] Y no hace nada si nadie lo armo: abrir el tubo es seguro, empujar
    // tramas es trafico. Ver `audio::armar_silencio`.
    super::audio::latido();
    // ** LA FOTO DE SALUD SE SACA AQUI DENTRO, y ese es su sitio exacto: leer
    // el estado de un endpoint recorre el Device Context y `USBSTS` es MMIO, y
    // las dos cosas solo estan mapeadas en el PML4 que acabamos de cargar.
    // Sacarla desde `OP_INFO` --con el CR3 del que pregunta-- seria un `#PF`.
    super::salud::refrescar();
    if switched {
        vmm::switch_to(previous);
    }
    unsafe { PUMPING = false };
}

/// How often the bus beats, in milliseconds.
///
/// 4 ms = 250 Hz. A USB boot keyboard asks to be polled every 8-10 ms, so this
/// sits comfortably above that without becoming a busy loop. And the thread
/// **sleeps** between turns (`park_until`) instead of yielding hot: yielding in a
/// tight loop would eat everything as soon as there was nothing else to do.
const BUS_PERIOD_MS: u64 = 4;

/// **The kernel thread that keeps the bus alive.** Started once, at boot, and it
/// never returns.
///
/// See the header of [`PUMPING`] for the why. The proof that it is alive is
/// `bus_stats().0` rising.
pub extern "C" fn bus_thread(_arg: u64) -> ! {
    use crate::ring0::task::scheduler;
    loop {
        pump_bus();
        watch_rescue();
        // ** LA PATADA, en el mismo sitio y por la misma razon que el rescate.
        //
        // Este hilo es el unico que despierta solo, cada 4 ms, y **sin ningun
        // cerrojo en la mano**. Quien declara una corrupcion corre con el
        // cerrojo del planificador puesto y no puede hacer el trabajo alli.
        // Ver `core/emergencia.rs`.
        crate::ring0::core::emergencia::atender();
        // Y la purga que haya pedido la tecla. Aqui se puede ceder el CPU:
        // este es un hilo de KERNEL, asi que la limpieza de Ring 3 no se lo
        // lleva por delante. Ver `core/purga.rs`.
        crate::ring0::core::purga::atender();
        // ** Y EL RITMO DEL RADAR, en el mismo turno y por la misma razon.
        //
        // Cerrar la ventana son 40 restas UNA VEZ POR SEGUNDO -- este hilo late
        // 250 veces, asi que 249 de cada 250 vueltas esto es una comparacion y
        // se va. El propio radar decide si toca: aqui solo se le da la hora.
        crate::ring0::cabina::radar::cerrar_ventana(
            scheduler::rdtsc(),
            scheduler::tsc_freq(),
        );
        unsafe {
            BUS_TURNS = BUS_TURNS.wrapping_add(1);
            // El latido se sella DESPUES de la vuelta, no antes: lo que
            // interesa saber es que la vuelta TERMINO. Un hilo que entra en
            // `pump_bus` y se queda dentro esta tan caido como uno que no
            // entro, y sellando al principio se veria vivo.
            ULTIMO_LATIDO = scheduler::rdtsc();
        }
        let hz = scheduler::tsc_freq();
        if hz == 0 {
            // With no measured TSC there is no way to sleep a concrete amount of
            // time, so yielding is the only honest thing. Should not happen: the
            // TSC is measured before this starts.
            scheduler::yield_current();
            continue;
        }
        let wake_at = scheduler::rdtsc() + hz / 1000 * BUS_PERIOD_MS;
        scheduler::park_until(wake_at);
    }
}

/// Starts [`bus_thread`]. Returns its tid, or `None` if there was no slot.
///
/// Priority 2: above idle and below anything doing real work. The thread runs 250
/// times a second and every turn is short, so what matters is not that it runs
/// soon but that it **always** runs.
pub fn start_bus_thread() -> Option<u32> {
    if !unsafe { PRESENT } {
        crate::ring0::cabina::warn("usb", "sin aparatos: el bus no tiene hilo propio", 0);
        return None;
    }
    let tid = crate::ring0::task::scheduler::spawn_kernel(
        bus_thread as *const () as usize as u64,
        0,
        2,
    );
    match tid {
        Some(t) => {
            crate::ring0::cabina::id("usb", "el bus tiene hilo propio, tid", t as u64);
            Some(t)
        }
        None => {
            // Said out loud, not swallowed: with no thread the system behaves
            // exactly as before -- that is, with the freeze bug.
            crate::ring0::cabina::warn("usb", "NO hubo ranura para el hilo del bus", 0);
            None
        }
    }
}
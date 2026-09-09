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

// == *** EL RITMO SE MIDE CONTRA UN RELOJ, NO CONTRA EL TRABAJO (2026-09-07) ==
//
// # Lo que estaba mal, y es una linea
//
// ```text
//    let wake_at = rdtsc() + 4 ms;      <- 4 ms DESPUES DE ACABAR
// ```
//
// Eso no es *"late cada 4 ms"*: es *"duerme 4 ms cuando termine"*. El periodo de
// verdad era **trabajo + 4 ms**, y el trabajo de una vuelta no es constante:
// adoptar un puerto lleva hasta seis reintentos de 50 ms, el barrido cae cada
// 500, el audio encola varias tramas. Una vuelta cara alarga el periodo de todas
// las siguientes, y nadie se enteraba.
//
// ** Y peor: un retraso se ABSORBIA. Si el planificador no le daba turno en 40
// ms, el hilo despertaba, hacia su vuelta y volvia a dormir 4 ms mas. El retraso
// no se recuperaba, no se contaba, y `ULTIMO_LATIDO` solo anotaba una hora mas
// tarde. **Nadie sabia nunca que el latido se habia saltado.**
//
// # Como lo hacen Windows y Linux, que es de donde sale esto
//
// En los dos, quien pregunta al aparato **es el controlador, en hardware**, en
// el `Interval` que se le programo al endpoint. El driver no marca el ritmo: su
// trabajo es que SIEMPRE haya un sitio donde dejar el informe --una URB
// reenviada desde el propio handler en Linux, un lector continuo en Windows--.
//
// Asi que el reparto de BMO-X, dicho entero, es este:
//
// ```text
//    preguntar al aparato        su bInterval      EL xHC, en hardware
//    volver a armar el TRB       al llegar el evento   bmo_uhid
//    VACIAR el anillo            4 ms              este hilo   <- el que se retrasa
//    la red por si se perdio     500 ms            el barrido
// ```
//
// [!] Por eso un latido tarde **no pierde una tecla directamente**: el xHC sigue
// preguntando y dejando informes. Lo que cuesta es LATENCIA --se nota en la
// mano-- y riesgo de desborde del aparcadero, que ya tiene su propio contador
// (`evt_park_stats().perdidos`). Decirlo asi y no *"se pierden teclas"* es la
// diferencia entre un instrumento y un susto.
//
// # Y NO se recupera en rafaga, a proposito
//
// Al llegar tarde se **re-ancla** y se cuenta lo que no se dio. Dar de golpe los
// cinco turnos que se perdieron es empeorar el atasco que los provoco -- y es lo
// mismo que decide Linux con `URB_ISO_ASAP`: saltar al siguiente hueco, no
// repetir los que ya pasaron.

/// **Cuando VENCE el proximo latido**, en TSC absoluto. Cero mientras no se haya
/// anclado el reloj, que ocurre en la primera vuelta.
static mut PROXIMO: u64 = 0;
/// Latidos que llegaron DESPUES de su hora.
static mut LATIDOS_TARDE: u64 = 0;
/// Turnos ENTEROS que cabian en el retraso y no se dieron. Esta es la fila que
/// duele: `LATIDOS_TARDE` dice que hubo retraso, esta dice cuanto bus se perdio.
static mut LATIDOS_PERDIDOS: u64 = 0;
/// El peor retraso visto, en milisegundos. Un maximo y no una media: una media
/// de latencias esconde justo el pico que el dueno nota con la mano.
static mut PEOR_RETRASO_MS: u64 = 0;

/// A partir de aqui un retraso deja de ser ruido y se dice en CABINA.
///
/// Veinte milisegundos son CINCO latidos, y mas del doble de lo que pide un
/// teclado boot (8-10 ms). Por debajo no lo nota una mano; por encima, el
/// aparcadero de eventos empieza a ser lo unico que sostiene el teclado.
const RETRASO_QUE_SE_DICE_MS: u64 = 20;

/// `(latidos tarde, turnos perdidos, peor retraso en ms)`.
pub fn ritmo() -> (u64, u64, u64) {
    unsafe { (LATIDOS_TARDE, LATIDOS_PERDIDOS, PEOR_RETRASO_MS) }
}

// == ** Y QUIEN SE COMIO EL TURNO ===========================================
//
// `ritmo()` dice que el latido llego tarde. No dice POR QUIEN, y sin eso el
// numero manda a auditar los cinco trabajos de la vuelta.
//
// ** Se mide el PEOR de cada uno y no la media, por lo mismo que el retraso: una
// media de 40 us con un pico de 90 ms se lee como "todo bien" y el pico es
// justo lo que se nota en la mano.
//
// [!] Y `purga` va a salir alta SIEMPRE, porque cede el CPU hasta ocho veces
// esperando a `reap`. Eso no es un fallo suyo: es lo que hace. Se anota igual
// --tapar un numero porque se sabe explicar es como se pierden los datos-- pero
// se lee sabiendolo.

/// Los trabajos de una vuelta, EN ORDEN.
///
/// El orden no es de gusto y ya estaba escrito en el bucle: el rescate va justo
/// detras del bombeo porque su tecla acaba de entrar en la cola, y la purga
/// detras de la emergencia porque son dos motivos distintos por el mismo camino.
const NOMBRES: [&str; 5] = ["bombeo", "rescate", "emergencia", "purga", "radar"];

/// Lo peor que ha tardado cada uno, en microsegundos.
static mut PEOR_US: [u64; 5] = [0; 5];

/// `(nombre del que mas tardo alguna vez, sus microsegundos)`.
pub fn peor_trabajo() -> (&'static str, u64) {
    unsafe {
        let p = &*core::ptr::addr_of!(PEOR_US);
        let mut cual = 0usize;
        for i in 1..p.len() {
            if p[i] > p[cual] {
                cual = i;
            }
        }
        (NOMBRES[cual], p[cual])
    }
}

/// **El ritmo del bus y su peor trabajo, en un solo numero para Ring 3.**
///
/// ```text
///    bits  0..15   el periodo del bus, en ms          (`BUS_PERIOD_MS`)
///    bits 16..47   el peor trabajo visto, en us
///    bits 48..55   cual de los cinco (indice en `NOMBRES`)
/// ```
///
/// # *** POR QUE SUBE A RING 3, Y ES LA SEPTIMA VEZ (2026-09-09)
///
/// `ritmo()` y `peor_trabajo()` existen desde hace semanas y **las lee UN solo
/// sitio: `cabina/cockpit.rs`, que es una pantalla de RING 0.** Y de Ring 0 no
/// se vuelve: el dueno vive en el escritorio.
///
/// ** O sea que los dos numeros que deciden si BMO-X puede bajar la latencia de
/// la entrada estaban donde no los ve nadie. Van seis instrumentos con esa misma
/// forma --CABINA, el testigo del USB, el pulso, el volcado, el modo del
/// lienzo, `cuerpo`-- y este es el septimo.
///
/// # Que pregunta contesta, y por que es LA pregunta del tiempo real
///
/// El camino de la mano al pixel empieza aqui: `BUS_PERIOD_MS = 4` son **hasta
/// 4 ms** antes de que el sistema sepa siquiera que el raton se movio. Un raton
/// declara su `bInterval` --muchos piden 1 ms-- y `uhid/enumera.rs` LO LEE, se
/// lo pasa al Endpoint Context y lo escribe en el log. Y despues este hilo drena
/// el anillo a 250 Hz pase lo que pase.
///
/// > El aparato dice cada cuanto quiere hablar, el controlador se entera, y el
/// > hilo que le escucha no se ha enterado.
///
/// [!] Y bajar el periodo NO es gratis, que es justo para lo que sirve el otro
/// campo: la vuelta hace cinco trabajos, y a 1 ms se harian **cuatro veces mas
/// veces**. `peor_us` dice si caben. Con este numero la decision es un dato; sin
/// el, es una opinion -- y LEY 24 dice que el hardware se PERFILA.
pub fn ritmo_y_peor() -> u64 {
    unsafe {
        let p = &*core::ptr::addr_of!(PEOR_US);
        let mut cual = 0usize;
        for i in 1..p.len() {
            if p[i] > p[cual] {
                cual = i;
            }
        }
        // Se satura en vez de envolver: un `peor` que da la vuelta se leeria
        // como un numero pequeno, que es la mentira mas cara que puede decir un
        // instrumento de peor caso.
        let us = if p[cual] > 0xFFFF_FFFF { 0xFFFF_FFFF } else { p[cual] };
        (BUS_PERIOD_MS & 0xFFFF) | (us << 16) | ((cual as u64) << 48)
    }
}

/// Anota lo que tardo el trabajo `i` y devuelve el TSC de ahora, para encadenar.
///
/// `por_us` en cero --sin TSC medido-- solo devuelve la hora: medir contra un
/// reloj sin frecuencia daria un numero con cara de dato.
fn anota(i: usize, desde: u64, por_us: u64) -> u64 {
    let ahora = crate::ring0::task::scheduler::rdtsc();
    if por_us != 0 {
        let us = ahora.wrapping_sub(desde) / por_us;
        unsafe {
            let p = &mut *core::ptr::addr_of_mut!(PEOR_US);
            if us > p[i] {
                p[i] = us;
            }
        }
    }
    ahora
}

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
        // ** El reloj de la vuelta. `por_us` se saca ANTES de trabajar para que
        // los cinco trabajos se midan contra el mismo, y en cero cuando no hay
        // TSC medido: entonces se hace la vuelta igual y no se mide nada.
        let por_us = scheduler::tsc_freq() / 1_000_000;
        let mut t = scheduler::rdtsc();
        pump_bus();
        t = anota(0, t, por_us);
        watch_rescue();
        t = anota(1, t, por_us);
        // ** LA PATADA, en el mismo sitio y por la misma razon que el rescate.
        //
        // Este hilo es el unico que despierta solo, cada 4 ms, y **sin ningun
        // cerrojo en la mano**. Quien declara una corrupcion corre con el
        // cerrojo del planificador puesto y no puede hacer el trabajo alli.
        // Ver `core/emergencia.rs`.
        crate::ring0::core::emergencia::atender();
        t = anota(2, t, por_us);
        // Y la purga que haya pedido la tecla. Aqui se puede ceder el CPU:
        // este es un hilo de KERNEL, asi que la limpieza de Ring 3 no se lo
        // lleva por delante. Ver `core/purga.rs`.
        crate::ring0::core::purga::atender();
        t = anota(3, t, por_us);
        // ** Y EL RITMO DEL RADAR, en el mismo turno y por la misma razon.
        //
        // Cerrar la ventana son 40 restas UNA VEZ POR SEGUNDO -- este hilo late
        // 250 veces, asi que 249 de cada 250 vueltas esto es una comparacion y
        // se va. El propio radar decide si toca: aqui solo se le da la hora.
        crate::ring0::cabina::radar::cerrar_ventana(
            scheduler::rdtsc(),
            scheduler::tsc_freq(),
        );
        anota(4, t, por_us);
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
        // ** LA HORA DEL PROXIMO LATIDO SALE DE LA DEL ANTERIOR, no de ahora.
        // Ver la nota de `PROXIMO`: con esto el periodo es 4 ms de verdad y no
        // "4 ms mas lo que haya costado la vuelta".
        let por_ms = hz / 1000;
        let periodo = por_ms * BUS_PERIOD_MS;
        let ahora = scheduler::rdtsc();
        let mut aviso = 0u64;
        let wake_at = unsafe {
            if PROXIMO == 0 {
                // La primera vuelta ancla el reloj. Sin esto el primer latido
                // saldria "tarde" por todo lo que tardo el arranque.
                PROXIMO = ahora;
            }
            PROXIMO = PROXIMO.saturating_add(periodo);
            if PROXIMO <= ahora {
                // Su hora ya paso mientras trabajabamos o mientras no nos daban
                // turno.
                let retraso = ahora - PROXIMO;
                LATIDOS_TARDE = LATIDOS_TARDE.wrapping_add(1);
                LATIDOS_PERDIDOS = LATIDOS_PERDIDOS.wrapping_add(retraso / periodo);
                let ms = retraso / por_ms;
                if ms > PEOR_RETRASO_MS {
                    PEOR_RETRASO_MS = ms;
                    // Solo en un PEOR NUEVO, y solo pasado el umbral. Un aviso
                    // por cada retraso llenaria CABINA en el primer atasco y
                    // taparia la linea que lo explica.
                    if ms >= RETRASO_QUE_SE_DICE_MS {
                        aviso = ms;
                    }
                }
                // Re-anclar, NO recuperar en rafaga. Ver la nota de arriba.
                PROXIMO = ahora + periodo;
            }
            PROXIMO
        };
        if aviso != 0 {
            crate::ring0::cabina::warn(
                "usb", "el latido del bus llego TARDE (peor caso, en ms)", aviso);
        }
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
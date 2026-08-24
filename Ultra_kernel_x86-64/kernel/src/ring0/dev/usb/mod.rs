//! USB HID bridge: xHCI controller + boot-protocol keyboard/mouse en Ring 0.
//!
//! Motivo: la emulacion USB->PS/2 del firmware MSI muere tras ExitBootServices
//! (el i8042 solo entrega ruido: 0xFE/0x6D), asi que el teclado y el mouse
//! USB reales necesitan un driver xHCI de verdad. Este modulo es el PUENTE
//! entre el kernel y los drivers agnosticos `bmo-xhci`/`bmo-uhid`:
//!
//!   - Implementa `XhciHal` (DMA via el frame allocator, phys->virt via el
//!     physmap, log al panel de kernel coloreado).
//!   - Descubre el controlador xHCI en `ctx.pci_devices` (clase 0x0C serial
//!     bus, subclase 0x03 USB) y le pasa el MMIO del BAR0.
//!   - Traduce los `InputEvent` (scancodes Set 1) a ASCII con la MISMA tabla
//!     que el path PS/2, y los ofrece al shell por `poll_ascii`.
//!
//! v1 vive en Ring 0 (como el PS/2). Migrara a servidor Ring 3 via Endpoint
//! RPC -- el patron DEVICE/DMA/IRQ como capabilities (roadmap F4).

use boot_context::BootContext;

use bmo_input::event::{InputEvent, InputEventKind};
use bmo_input::hal::InputHal;
use bmo_uhid::UsbHidHal;
use bmo_xhci::XhciHal;

use crate::ring0::dev::console::serial_write;
use crate::ring0::mm::{self, phys};

use crate::ring0::dev::pci;
use crate::ring0::dev::keyboard;

// -- ** LOS TRES TRABAJOS QUE VIVIAN AQUI DENTRO (2026-08-12) -----------------
//
// Este fichero era **uno de 1179 lineas con 39 `static mut`** -- el que mas
// tiene de todo el kernel, o sea tambien el peor bloqueante de SMP. Y no era un
// fichero grande: eran cuatro trabajos distintos compartiendo un cajon.
//
// La regla de esta casa es MODULAR, y el dueno la nombro por su nombre:
// *"si el xHCI esta mezclado con el mouse y el teclado y audifono ME VA A ROMPER
// EL HUEVO para modificar luego"*.
//
// [!] Y al medirlo salio que el miedo apuntaba al sitio equivocado, que es justo
// para lo que sirve medir: **`bmo-xhci` esta limpio** --cero dependencias, cero
// identificadores de teclado o raton en su codigo-- y lo mismo `bmo-uaudio`.
// El monolito no estaba en los drivers: estaba **en el pegamento del kernel**,
// que es este fichero.
//
// | modulo | trabajo | por que se puede sacar solo |
// |---|---|---|
// | [`bus`] | el hilo de kernel que mantiene vivo el bus | no toca ni una tecla: bombea y vigila |
// | [`rescate`] | `Ctrl+Alt+Esc` en las DOS puertas | es POLITICA, no driver |
// | [`panel`] | lo que CABINA lee | solo LEE: no cambia un byte de estado |
//
// Lo que queda aqui es lo que de verdad es "el puente al xHC": el HAL, la
// enumeracion, el bombeo, y la traduccion de scancode a caracter con su estado
// (Shift/AltGr/CAPS/typematic). Ese ultimo es el siguiente corte y NO se hace
// hoy: sus banderas las escribe `bombear_interno` y las lee `drain`, asi que
// separarlos es mover estado compartido y no mover funciones. Se dice en vez de
// dejarlo a medias.

/// Preguntarle al aparato de audio como quiere las muestras. Paso 0 de
/// `docs/maestro/AUDIO_MAESTRO.md`: no le escribe un byte.
pub mod audio;
/// El hilo de kernel que bombea el bus. Sin el, el teclado depende de que
/// alguien pregunte -- ver su cabecera.
pub mod bus;
/// Lo que CABINA lee de aqui. **Solo lectura**, a proposito.
pub mod panel;
/// **La salud del bus como ESTADO**, legible desde Ring 3 con `OP_INFO`. Es la
/// sexta exigencia de `docs/componente/EL_TECLADO_EXIGE.md`: un contador que solo se lee
/// en el shell de Ring 0 no existe para quien vive en el escritorio.
pub mod salud;
/// El atajo que le devuelve la maquina al dueno. Politica, no driver.
pub mod rescate;

pub use bus::{bus_stats, bus_thread, start_bus_thread};
use bus::pump_bus;
use rescate::{raw_key_from_owner, tecla_del_dueno};
pub use panel::*;

// Line buffer for the driver's diagnostic stream. The driver logs in
// fragments (`log("[uhid] slot=")` then `log_u64(..)` then `log("\n")`), so
// we accumulate to '\n' and flush the whole line to the on-screen panel --
// otherwise every xHCI/HID diagnostic is invisible on a headless board and
// we debug blind (exactly what "init sin teclado" left us).
const DLOG_MAX: usize = 96;
/// **Encender el bus**: esperar a un puerto y enumerar. Ocurre UNA vez.
mod arranque;
/// **Lo que llega despues**: enchufar, desenchufar, y el barrido de 500 ms que
/// recoge lo que un aviso perdido dejo caer.
mod enchufe;
/// **Que esta pasando en el teclado AHORA**: la cola cruda, modificadores,
/// LEDs, repeticion y el puntero.
mod teclas;

pub use arranque::*;
pub use enchufe::*;
pub use teclas::*;

static mut DLOG: [u8; DLOG_MAX] = [0u8; DLOG_MAX];
static mut DLOG_N: usize = 0;

fn dlog_push(s: &str) {
    serial_write(s); // serial keeps the verbatim stream
    if !crate::info::has_fb() {
        return;
    }
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(DLOG);
        for &b in s.as_bytes() {
            if b == b'\n' {
                let n = DLOG_N;
                if n > 0 {
                    if let Ok(line) = core::str::from_utf8(&buf[..n]) {
                        crate::ring0::core::dashboard::dashboard_log(line);
                    }
                }
                DLOG_N = 0;
            } else if b >= 0x20 && b < 0x7F && DLOG_N < DLOG_MAX {
                buf[DLOG_N] = b;
                DLOG_N += 1;
            }
        }
    }
}

fn dlog_u64(val: u64) {
    const H: &[u8; 16] = b"0123456789ABCDEF";
    // Trim leading zeros to a compact hex, prefixed 0x.
    let mut tmp = [0u8; 18];
    let mut o = 0;
    tmp[o] = b'0'; o += 1;
    tmp[o] = b'x'; o += 1;
    let mut started = false;
    for i in (0..16).rev() {
        let nib = ((val >> (i * 4)) & 0xF) as usize;
        if nib != 0 || started || i == 0 {
            tmp[o] = H[nib];
            o += 1;
            started = true;
        }
    }
    if let Ok(s) = core::str::from_utf8(&tmp[..o]) {
        dlog_push(s);
    }
}

/// El HAL que `bmo-xhci` invoca para DMA / traduccion de direcciones / log.
struct KernelXhciHal;

impl XhciHal for KernelXhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        // Frames FISICAMENTE CONTIGUOS: los anillos TRB y buffers de reporte
        // se direccionan linealmente y el xHC los lee por direccion fisica.
        phys::alloc_frames_contig(count as u64)
    }
    fn phys_to_virt(&self, phys: u64) -> *mut u8 {
        // El physmap (0..PHYSMAP_SIZE) espeja toda la RAM en HIGH_MEM_BASE.
        mm::phys_to_virt(phys) as *mut u8
    }
    fn log(&self, msg: &str) {
        dlog_push(msg);
    }
    fn log_u64(&self, msg: &str, val: u64) {
        dlog_push(msg);
        dlog_u64(val);
    }
    fn delay_ms(&self, ms: u64) {
        delay_ms(ms);
    }
}

static HAL: KernelXhciHal = KernelXhciHal;
static mut HID: UsbHidHal = UsbHidHal::new();
static mut READY: bool = false;
static mut SHIFT: bool = false;
static mut CAPS: bool = false;
/// AltGr mantenido (Alt derecho): abre el tercer nivel del teclado espanol.
static mut ALTGR: bool = false;
/// Ctrl mantenido (cualquiera de los dos).
static mut CTRL: bool = false;
/// Alt IZQUIERDO mantenido. Windows acepta Ctrl+Alt como AltGr, y quien
/// aprendio ahi lo tiene en los dedos: aqui tambien vale.
static mut LALT: bool = false;

// -- Repeticion al mantener (typematic) --------------------------------------
//
// El teclado USB no repite solo: manda un reporte cuando la tecla BAJA y otro
// cuando SUBE, y entre medias silencio. Repetir es trabajo del host. Sin esto,
// mantener el retroceso borra UN caracter y se queda mirando.

/// Ultima tecla que sigue pulsada (0 = ninguna) y su contexto.
static mut HELD_CODE: u8 = 0;
static mut HELD_SHIFT: bool = false;
static mut HELD_ALTGR: bool = false;
static mut HELD_CTRL: bool = false;
/// TSC del momento en que se pulso, y del ultimo disparo automatico.
static mut HELD_SINCE: u64 = 0;
static mut HELD_LAST: u64 = 0;
/// Espera antes de empezar a repetir, y periodo entre repeticiones (ms).
/// Los mismos valores de siempre: medio segundo de gracia, luego ~30 por
/// segundo -- lo bastante rapido para borrar una linea sin pasarse.
const REPEAT_DELAY_MS: u64 = 500;
const REPEAT_RATE_MS: u64 = 33;
static mut PRESENT: bool = false;
// Diagnostico DETALLADO del HID (pedido del usuario: "llamar al mouse, mas
// detallado total"). Estado por dispositivo + telemetria viva del mouse, para
// que la proxima foto diga exactamente que enumero y si el mouse late.
static mut KBD_RDY: bool = false;
static mut MOUSE_RDY: bool = false;
static mut KBD_SLOT: u8 = 0;
static mut MOUSE_SLOT: u8 = 0;
static mut MOUSE_EVENTS: u32 = 0;   // no de reportes de movimiento/boton vistos
static mut MOUSE_X: i32 = 0;        // posicion acumulada (relativa) X
static mut MOUSE_Y: i32 = 0;        // posicion acumulada (relativa) Y
static mut MOUSE_BTN: u8 = 0;       // bitmap de botones actual
static mut KEY_EVENTS: u32 = 0;     // no de teclas imprimibles entregadas
static mut FIRST_KEY: bool = false;   // ya se grabo la primera tecla en CABINA?
static mut FIRST_MOUSE: bool = false; // idem para el primer movimiento de mouse
/// Vueltas de rueda acumuladas desde la ultima lectura. Se vacia al leerlo.
static mut MOUSE_WHEEL: i32 = 0;
static mut HID_EVENTS: u32 = 0;     // no TOTAL de InputEvents de hid.poll (kbd+mouse)


/// Se inicializo un teclado USB?
pub fn is_ready() -> bool {
    unsafe { READY }
}

/// Poll no bloqueante: drena eventos HID y devuelve UN ascii si hubo una
/// tecla imprimible (o Enter/Backspace/Tab). Mantiene el estado de Shift.
/// Alimenta `shell_read_line` igual que `keyboard::poll_ascii`.
///
/// ## Por que esto se envuelve en un cambio de CR3
///
/// Tocar el xHCI es **escribir MMIO**: el `ERDP` del interrupter 0 vive en
/// `base + RTSOFF + 0x38`, que en esta placa cae en `0xFC2004F8`. Ese rango esta
/// mapeado en el PML4 del kernel y **no** en el de una tarea de usuario.
///
/// Mientras el unico que llamaba aqui era el shell de Ring 0 --una tarea de
/// kernel, con el CR3 del kernel cargado-- eso no se notaba. Pero desde que
/// `KIND_INPUT` entrega teclas, este camino se recorre **desde dentro de un
/// SYSCALL**, y en un SYSCALL desde Ring 3 el CR3 sigue siendo el del llamante:
/// el cambio de CR3 solo ocurre en un cambio de contexto, y ahi todavia no ha
/// habido ninguno. El resultado fue un `#PF` de escritura sobre pagina ausente
/// en Ring 0 --`err=0x2`, `cr2=0xFC2004F8`-- a los 144 ticks: en cuanto el
/// compositor pidio su primera tecla.
///
/// Es la misma trampa que ya esta anotada en `fault_dispatch` para el
/// framebuffer ("el CR3 de usuario puede no mapear el rango identidad"). Aqui
/// la respuesta es la misma: ponerse el CR3 del kernel para tocar el hardware y
/// devolverlo al salir.
///
/// * No es gratis: dos escrituras de CR3 son dos vaciados de TLB, y esto se
/// llama una vez por fotograma. La solucion barata de verdad seria mapear el
/// agujero de MMIO en todo espacio de direcciones --es memoria de supervisor,
/// asi que Ring 3 no la veria igualmente-- y eso ahorraria los dos vaciados. Se
/// deja anotado y no hecho: primero que funcione y este aislado en un sitio.
pub fn poll_ascii() -> Option<u8> {
    use crate::ring0::mm::vmm;
    let kpml4 = vmm::kernel_pml4();
    let previo = vmm::read_cr3();
    // `kpml4 == 0` = todavia no hay PML4 de kernel publicado (arranque muy
    // temprano). Entonces el CR3 que hay ES el bueno y no se toca nada.
    let cambiado = kpml4 != 0 && previo != kpml4;
    if cambiado {
        vmm::switch_to(kpml4);
    }
    let r = tecla_del_dueno(poll_ascii_interno());
    // Se devuelve SIEMPRE, por un solo camino. `poll_ascii_interno` tiene
    // varios `return` y dejar el CR3 del kernel puesto al volver a Ring 3 seria
    // mucho peor que el fallo original: la tarea seguiria corriendo con el
    // espacio de direcciones de otro.
    if cambiado {
        vmm::switch_to(previo);
    }
    r
}

fn poll_ascii_interno() -> Option<u8> {
    // Lo que dejo pendiente la pulsacion anterior sale primero: una tecla
    // muerta que no combina produce DOS caracteres (' + q = 'q).
    if let Some(b) = drain() { return Some(b); }
    pump_bus();
    drain()
}


/// Drena el bus HID y actualiza todo el estado, **sin sacar nada de ninguna
/// cola**.
///
/// Estaba dentro de `poll_ascii_interno`, que hacia dos trabajos: bombear el
/// bus y entregar un caracter. Separarlos hace falta desde que hay **dos**
/// consumidores de lo mismo -- el que quiere caracteres y el que quiere teclas
/// crudas (ver [`evento_tecla`]). Si el segundo tuviera que llamar al primero
/// para que el bus avanzara, se comeria un caracter por cada evento que pide.
fn bombear_interno() {
    // Correr si hay CUALQUIER dispositivo enumerado (no solo teclado): asi el
    // mouse late en el diagnostico aunque el teclado no haya enumerado.
    if !unsafe { PRESENT } {
        return;
    }

    atender_avisos();
    barrer_si_toca();

    let mut evs = [InputEvent::empty(); 16];
    let n = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.poll(&mut evs)
    };
    unsafe { HID_EVENTS = HID_EVENTS.wrapping_add(n as u32); }
    repartir_eventos(&evs[..n]);
}


/// El reparto de los eventos de entrada a las colas del kernel.
///
/// Salio de `bombear_interno` al meter ahi los avisos y el barrido: aquel se
/// habia convertido en tres trabajos dentro de una funcion, que es la forma que
/// tiene este kernel de acumular monolitos.
fn repartir_eventos(evs: &[InputEvent]) {
    for ev in evs {
        // ** LA TECLA CRUDA, ANTES DE QUE NADIE LA INTERPRETE.
        //
        // Va aqui arriba y no dentro de las ramas porque las ramas de
        // modificador hacen `continue`: Shift, Ctrl y Alt no producen caracter
        // y por eso salian del bucle antes de tiempo. Para un juego esos tres
        // son teclas como las demas --en DOOM, correr y disparar-- asi que
        // tienen que entrar en la cola cruda igual que una letra.
        if matches!(ev.kind, InputEventKind::KeyDown | InputEventKind::KeyUp) {
            empujar_evento(ev.code, matches!(ev.kind, InputEventKind::KeyDown));
        }
        match ev.kind {
            InputEventKind::KeyDown => {
                // Shift (Set 1 make: 0x2A izq, 0x36 der).
                if ev.code == 0x2A || ev.code == 0x36 {
                    unsafe { SHIFT = true };
                    continue;
                }
                // AltGr: el tercer nivel del teclado espanol. Llega con
                // codigo propio (ver bmo_uhid::SC_ALTGR) para no confundirse
                // con el Alt izquierdo.
                if ev.code == bmo_uhid::SC_ALTGR {
                    unsafe { ALTGR = true };
                    continue;
                }
                if ev.code == 0x38 { unsafe { LALT = true }; continue; }
                if ev.code == 0x1D { unsafe { CTRL = true }; continue; }
                // Caps Lock (0x3A): toggle al presionar, como Windows.
                if ev.code == 0x3A {
                    unsafe { CAPS = !CAPS };
                    continue;
                }
                // La distribucion activa decide que letra es. Lo que produzca
                // (0, 1 o 2 caracteres) queda en la cola del teclado: nada se
                // pierde aunque lleguen varias teclas en el mismo sondeo.
                unsafe {
                    HELD_CODE = ev.code;
                    HELD_SHIFT = SHIFT;
                    HELD_ALTGR = altgr_active();
                    HELD_CTRL = CTRL;
                    HELD_SINCE = crate::ring0::task::scheduler::rdtsc();
                    HELD_LAST = HELD_SINCE;
                    keyboard::feed_full(ev.code, SHIFT, altgr_active(), CAPS, CTRL);
                }
            }
            InputEventKind::KeyUp => {
                if ev.code == 0x2A || ev.code == 0x36 {
                    unsafe { SHIFT = false };
                }
                if ev.code == bmo_uhid::SC_ALTGR { unsafe { ALTGR = false }; }
                if ev.code == 0x38 { unsafe { LALT = false }; }
                if ev.code == 0x1D { unsafe { CTRL = false }; }
                // Soltar la tecla corta la repeticion.
                unsafe { if HELD_CODE == ev.code { HELD_CODE = 0; } }
            }
            // MOUSE: antes se descartaba (esperaba el compositor F5). Ahora lo
            // "llamamos": acumulamos posicion y botones para el diagnostico y,
            // a futuro, el cursor del compositor.
            InputEventKind::MouseMove => unsafe {
                // ** SE RECORTA EL ACUMULADOR, no solo lo que se lee.
                //
                // Antes esto sumaba sin tope y el recorte estaba unicamente en
                // `INPUT_OP_PUNTERO`, al contestar. O sea que empujar el raton
                // contra el borde de arriba dejaba el puntero pegado a `y = 0`
                // --correcto en pantalla-- mientras `MOUSE_Y` seguia bajando a
                // -500, -2000, lo que hiciera falta. Y para volver al centro
                // habia que **deshacer primero todo ese exceso**: el raton se
                // movia y el puntero no, durante un rato largo.
                //
                // Eddi lo describio exacto: *"cuando voy arriba, el puntero se
                // demora en ir al centro"*. Ese retraso es la deuda acumulada
                // contra el borde, cobrandose.
                //
                // El recorte tiene que estar DONDE SE SUMA. Recortar al leer
                // ensena bien el numero y deja el estado mintiendo, que es la
                // forma mas cara de tener razon.
                let (ancho, alto) = (
                    crate::info::FB_WIDTH.max(1) as i32 - 1,
                    crate::info::FB_HEIGHT.max(1) as i32 - 1,
                );
                MOUSE_X = MOUSE_X.saturating_add(ev.mouse_dx() as i32).clamp(0, ancho);
                MOUSE_Y = MOUSE_Y.saturating_add(ev.mouse_dy() as i32).clamp(0, alto);
                MOUSE_EVENTS = MOUSE_EVENTS.wrapping_add(1);
                if !FIRST_MOUSE {
                    FIRST_MOUSE = true;
                    crate::ring0::cabina::info("usb", "primer movimiento de mouse recibido", 0);
                }
            },
            InputEventKind::MouseButton => unsafe {
                MOUSE_BTN = ev.mouse_buttons();
                MOUSE_EVENTS = MOUSE_EVENTS.wrapping_add(1);
            },
            // * El delta de la rueda se TIRABA: solo se contaba el evento.
            // Otro valor que el sistema tenia y no decia. Se acumula y se
            // entrega al leerlo, que es como se consume un evento.
            InputEventKind::MouseWheel => unsafe {
                MOUSE_WHEEL = MOUSE_WHEEL.saturating_add(ev.mouse_wheel_delta() as i32);
                MOUSE_EVENTS = MOUSE_EVENTS.wrapping_add(1);
            },
        }
    }

    // Sincronizar las lucecitas: si el estado de los bloqueos cambio, hay que
    // DECIRSELO al teclado. No se encienden solas.
    sync_leds();
    // Repeticion de la tecla mantenida.
    //
    // [!] Alimenta SOLO la cola de caracteres. La cola cruda no lleva repes a
    // proposito: quien pide teclas crudas quiere saber que esta pulsado, y una
    // repeticion no es otra pulsacion -- entregarla obligaria al llamante a
    // filtrar "pulsada otra vez sin haberse soltado", que es exactamente el
    // trabajo que esta cola le esta quitando.
    repeat_held();
}


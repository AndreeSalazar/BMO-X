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
/// `docs/AUDIO_MAESTRO.md`: no le escribe un byte.
pub mod audio;
/// El hilo de kernel que bombea el bus. Sin el, el teclado depende de que
/// alguien pregunte -- ver su cabecera.
pub mod bus;
/// Lo que CABINA lee de aqui. **Solo lectura**, a proposito.
pub mod panel;
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
                        crate::ring0::core::phase::dashboard_log(line);
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

fn log(msg: &str) {
    serial_write(msg);
    if crate::info::has_fb() {
        crate::ring0::core::phase::dashboard_log(msg);
    }
}

/// Espera real en milisegundos por TSC. El spec USB pide tiempos HUMANOS
/// (100 ms de debounce de conexion, 20+ ms de estabilizacion de power) -- los
/// spin-counts heredados de QEMU duran microsegundos y en hardware real los
/// puertos aun no reportan CCS cuando el driver pregunta.
fn delay_ms(ms: u64) {
    let f = crate::ring0::task::scheduler::tsc_freq();
    if f == 0 {
        for _ in 0..ms * 2_000_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let end = crate::ring0::task::scheduler::rdtsc() + ms * (f / 1000);
    while crate::ring0::task::scheduler::rdtsc() < end {
        core::hint::spin_loop();
    }
}

/// Descubre e inicializa xHCI + HID. Reporta al panel.
///
/// Estrategia hardware-real:
/// 1. Scan PCI propio (dev::pci, detras de bridges, MEM+BME habilitados).
/// 2. Los Ryzen traen VARIOS xHC (CPU + chipset): se prueban en orden.
/// 3. Por controlador: init -> power a TODOS los puertos -> 200 ms de settle
///    (spec: 100 ms debounce) -> censo PORTSC. Si algun puerto tiene CCS=1
///    (dispositivo FISICAMENTE presente), ese controlador gana y el HID
///    enumera ahi. El censo se pinta: dice donde esta el teclado
///    electricamente aunque la enumeracion posterior fallara.
pub fn init(_ctx: &BootContext) {
    bmo_xhci::init_hal(&HAL);

    let mut chosen = false;
    for skip in 0..4usize {
        let loc = match pci::find_xhci(skip) {
            Some(l) => l,
            None => break,
        };
        // MMIO virtual: SIEMPRE por el physmap. La identidad de s2 vive en
        // PML4[0] y un espacio de Ring 3 solo hereda su primer GiB, asi que
        // tocar un BAR de ~4 GiB bajo el CR3 de un proceso es un #PF en Ring 0.
        // Aqui no se notaba porque el sondeo del xHC corre en una tarea de
        // Ring 0; el mismo fallo SI mataba al disco. Ver la nota larga en
        // `dev/disk.rs`.
        let mmio_va = mm::phys_to_virt(loc.mmio);
        dlog_push("[usb] xHC pci ");
        dlog_u64(loc.bus as u64);
        dlog_push(":");
        dlog_u64(loc.dev as u64);
        dlog_push(".");
        dlog_u64(loc.func as u64);
        dlog_push(" mmio=");
        dlog_u64(loc.mmio);
        dlog_push("\n");

        crate::ring0::cabina::info("usb", "controlador xHCI hallado en PCI", loc.mmio);

        bmo_xhci::reset_ctrl();
        bmo_xhci::set_mmio(mmio_va);
        if !unsafe { bmo_xhci::init(mmio_va) } {
            dlog_push("[usb] init fallo en este xHC, probando siguiente\n");
            crate::ring0::cabina::warn("usb", "el xHC no inicializo, probando el siguiente", loc.mmio);
            continue;
        }
        let nports = match bmo_xhci::controller() {
            Some(c) => c.max_ports,
            None => continue,
        };
        // Power a todos los puertos y settle REAL (el uhid hace su propio
        // power+reset despues; para entonces CCS ya estara latcheado).
        // * Encender los ocho y esperar UNA vez, no ocho.
        //
        // La estabilizacion de VBUS es un tiempo fisico del puerto y los
        // puertos se estabilizan en paralelo. Antes cada `port_power_on`
        // esperaba sus 20 ms por su cuenta: ocho puertos por dos controladores
        // eran 320 ms de arranque comprando exactamente nada.
        for p in 0..nports {
            unsafe { bmo_xhci::port_power_solo(p) };
        }
        delay_ms(200);
        // Censo: que puertos tienen un dispositivo fisico (PORTSC.CCS).
        let mut connected = 0u64;
        for p in 0..nports {
            let sc = unsafe { bmo_xhci::port_peek(p) };
            if sc & 1 != 0 {
                connected += 1;
                dlog_push(" p");
                dlog_u64(p as u64);
                dlog_push("=");
                dlog_u64(sc as u64);
            }
        }
        if connected > 0 {
            dlog_push("\n");
        }
        dlog_push("[usb] puertos con dispositivo: ");
        dlog_u64(connected);
        dlog_push("\n");
        if connected > 0 {
            crate::ring0::cabina::info("usb", "puertos con dispositivo fisico (CCS=1)", connected);
            chosen = true;
            break;
        }
        // Nada conectado aqui: probar el siguiente controlador.
    }

    if !chosen {
        log("[usb] ningun xHC ve el teclado (probar otro puerto fisico)\n");
        crate::ring0::cabina::fault("usb", "ningun xHC ve dispositivos (probar otro puerto)", 0);
        return;
    }

    let ok = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        let r = hid.init();
        refrescar_presencia();
        r
    };
    unsafe {
        PRESENT = true;
        READY = ok;
    }
    // Resumen detallado en serial + panel (ademas del status fijo en pantalla).
    unsafe {
        if KBD_RDY {
            log("[usb] teclado USB listo (slot ");
            dlog_u64(KBD_SLOT as u64);
            log(")\n");
            crate::ring0::cabina::info("usb", "teclado enumerado y configurado", KBD_SLOT as u64);
            // Lo que de verdad decide si el teclado hablara: el estado del
            // endpoint segun el xHC y el intervalo que quedo programado.
            let (st, bi, iv, _sp, sts) = kbd_ep_debug();
            crate::ring0::cabina::info("xhci", "kbd bInterval->Interval programado", ((bi as u64) << 8) | iv as u64);
            if st != 1 {
                crate::ring0::cabina::fault("xhci", "endpoint del teclado NO quedo Running", st as u64);
            }
            // HSE (bit 2) o HCE (bit 12): el controlador se cayo, todo lo demas
            // que veamos despues es ruido.
            if sts & ((1 << 2) | (1 << 12)) != 0 {
                crate::ring0::cabina::fault("xhci", "controlador en error (USBSTS HSE/HCE)", sts as u64);
            }
        } else {
            log("[usb] SIN teclado (no enumero interface kbd)\n");
            crate::ring0::cabina::warn("usb", "ninguna interface de teclado enumero", 0);
        }
        if MOUSE_RDY {
            log("[usb] mouse USB listo (slot ");
            dlog_u64(MOUSE_SLOT as u64);
            log(")\n");
            crate::ring0::cabina::info("usb", "mouse enumerado y configurado", MOUSE_SLOT as u64);
        } else {
            log("[usb] SIN mouse (no enumero interface mouse)\n");
            crate::ring0::cabina::warn("usb", "ninguna interface de mouse enumero", 0);
        }
    }
}

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

    // -- Enchufaron algo? Adoptarlo -------------------------------------
    //
    // * La enumeracion del arranque era una carrera de UN SOLO INTENTO. El
    // bucle recorre los puertos una vez y lo que no estuviera listo en ese
    // instante se perdia **hasta el siguiente reinicio** -- y un raton con
    // firmware RGB tarda en engancharse mas que un teclado.
    //
    // De ahi el sintoma que no encajaba con nada: unas veces arrancaba el
    // teclado y otras el raton, nunca los dos, sin cambiar una linea entre
    // arranque y arranque. No era hardware intermitente: era quien llegaba a
    // tiempo. La foto lo dijo entero --`k=OK(s2) m=OK(s2)`, o sea el raton era
    // otra vez la interfaz de medios del teclado, y tres lineas mas arriba
    // `puerto: algo se ENCHUFO (sin re-enumerar aun) =3`: el raton de verdad
    // anunciandose en el puerto 3 **despues** de que el bucle ya hubiera
    // pasado, y nadie recogiendolo.
    //
    // El aviso ya llegaba desde el commit anterior; lo que faltaba era actuar.
    // Y actuar aqui es seguro por dos cosas que ya estan puestas: este camino
    // corre con el CR3 del kernel (ver la cabecera de `poll_ascii`), y los
    // informes del aparato que YA bombea no se pierden mientras se enumera el
    // nuevo porque el aparcadero de `bmo_xhci` los guarda.
    //
    // * Lo que SI cuesta, dicho claro: enumerar lleva esperas (hasta seis
    // reintentos de 50 ms), y esto se recorre desde dentro de un syscall. Un
    // enchufe puede congelar al que pidio la tecla casi un tercio de segundo.
    // Se acepta porque ocurre **una vez por enchufe** --`tomar_cambio_puerto`
    // consume el aviso-- y porque la alternativa era no tener nunca los dos
    // aparatos. Cuando haya un hilo de kernel para el bus, esto se muda ahi.
    if let Some((puerto, conectado)) = bmo_xhci::tomar_cambio_puerto() {
        if conectado {
            let adoptado = unsafe {
                let hid = &mut *core::ptr::addr_of_mut!(HID);
                // `port_reset` y compania trabajan en indice 0-based; el Port
                // ID del evento es 1-based. Restar aqui y no en el driver: el
                // que traduce es el que conoce las dos convenciones.
                hid.adoptar_puerto(puerto.saturating_sub(1))
            };
            if adoptado {
                crate::ring0::cabina::info("usb", "puerto: ENCHUFADO y adoptado", puerto as u64);
                unsafe { refrescar_presencia() };
            } else {
                // * AND SAYING "nothing to adopt" WAS HIDING THE BUG OF 08-12.
                //
                // This line was technically true and told the wrong story. The
                // owner replugged his keyboard, this printed `nada que adoptar`,
                // and the truth was *"I still think I have it"* -- because
                // unplugging freed the port and forgot to forget the device, so
                // `completo()` kept saying everything was present.
                //
                // Now the state is printed next to the verdict, and the two
                // cases stop looking alike: `k1 m1` here means the adopter
                // believes it has both, which after a disconnect is a lie that
                // can be seen. See `bmo_uhid::soltar_puerto`.
                let (k, m) = unsafe {
                    let hid = &*core::ptr::addr_of!(HID);
                    (hid.has_kbd(), hid.has_mouse())
                };
                let estado = ((k as u64) << 8) | m as u64;
                crate::ring0::cabina::info("usb", "puerto: ENCHUFADO, nada que adoptar", puerto as u64);
                crate::ring0::cabina::bits("usb", "  ...y creo tener teclado:raton", estado);
            }
        } else {
            // * Desenchufar LIBERA el puerto y le devuelve los intentos. Sin
            // esto, enchufar y desenchufar tres veces dejaria un puerto
            // inservible hasta el siguiente reinicio: los intentos son para
            // "este aparato tarda", no para "este puerto esta prohibido".
            //
            // ** Y DESDE EL 08-12, SUELTA TAMBIEN EL APARATO. La mitad que
            // faltaba: sin ella el teclado desenchufado seguia contando como
            // presente y no volvia jamas. Ver `bmo_uhid::soltar_puerto`.
            let solto = unsafe {
                let hid = &mut *core::ptr::addr_of_mut!(HID);
                hid.soltar_puerto(puerto.saturating_sub(1))
            };
            crate::ring0::cabina::warn("usb", "puerto: algo se DESENCHUFO", puerto as u64);
            if solto {
                // Two different pieces of news, and they used to be one. A
                // device leaving is what has to be REPAIRED; an empty port
                // changing state is noise.
                crate::ring0::cabina::warn("usb", "  ...y ERA UN APARATO MIO: lo suelto", puerto as u64);
                unsafe { refrescar_presencia() };
            }
        }
    }

    let mut evs = [InputEvent::empty(); 16];
    let n = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.poll(&mut evs)
    };
    unsafe { HID_EVENTS = HID_EVENTS.wrapping_add(n as u32); }
    for ev in &evs[..n] {
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

// -- LA COLA CRUDA DE TECLAS: scancode + pulsada/soltada -----------------
//
// ** El kernel SIEMPRE tuvo esta informacion y la tiraba en la puerta.
//
// `bmo_uhid::teclado` compara cada informe boot con el anterior y produce
// `InputEvent::key(scancode, pulsada)` -- las dos cosas, desde el primer dia.
// Lo que llegaba a Ring 3 era un flujo de CARACTERES: `INPUT_OP_TECLA` entrega
// un byte Latin-1 ya resuelto, que es lo correcto para escribir y **no sirve
// para jugar**. Un juego no pregunta "que letra se escribio", pregunta "esta
// la flecha abajo AHORA". Sin el soltar, quien anda no para nunca.
//
// Por eso esto no es una cola nueva de datos nuevos: es dejar de tirar lo que
// ya se tenia. La de caracteres se queda intacta y las dos se llenan del mismo
// sondeo -- no hay dos lectores del bus.
//
// 64 entradas: un informe boot trae hasta 6 teclas y el sondeo va por
// fotograma. Si se llena, se tira **lo mas VIEJO** y se cuenta. Tirar lo nuevo
// seria peor de una forma concreta: se perderia el `soltar` de una tecla cuyo
// `pulsar` ya se entrego, y el juego se quedaria andando solo.
const EVENTOS_CRUDOS: usize = 64;
static mut CRUDOS: [u16; EVENTOS_CRUDOS] = [0; EVENTOS_CRUDOS];
static mut CRUDOS_LEE: usize = 0;
static mut CRUDOS_ESCRIBE: usize = 0;
/// Cuantos se han tirado por cola llena. Si esto sube, el consumidor no esta
/// drenando lo bastante rapido -- y es un numero, no una sospecha.
static mut CRUDOS_PERDIDOS: u32 = 0;

fn empujar_evento(scancode: u8, pulsada: bool) {
    unsafe {
        let siguiente = (CRUDOS_ESCRIBE + 1) % EVENTOS_CRUDOS;
        if siguiente == CRUDOS_LEE {
            // Llena: se tira la mas vieja para hacer sitio.
            CRUDOS_LEE = (CRUDOS_LEE + 1) % EVENTOS_CRUDOS;
            CRUDOS_PERDIDOS = CRUDOS_PERDIDOS.saturating_add(1);
        }
        CRUDOS[CRUDOS_ESCRIBE] = if pulsada {
            0x100 | scancode as u16
        } else {
            scancode as u16
        };
        CRUDOS_ESCRIBE = siguiente;
    }
}

/// La siguiente tecla cruda: `Some((scancode Set 1, pulsada))`, o `None`.
///
/// **No bloquea** y **bombea el bus** si la cola esta vacia, por el mismo
/// motivo que `poll_ascii`: quien llama tiene un bucle de fotograma y el bus
/// solo avanza cuando alguien lo mira.
///
/// El envoltorio de CR3 es el de `poll_ascii` y por la misma razon -- tocar el
/// xHCI es escribir MMIO que solo esta mapeado en el PML4 del kernel, y esto se
/// recorre desde dentro de un syscall. Ver su cabecera.
pub fn evento_tecla() -> Option<(u8, bool)> {
    // ** El rescate se mira en LAS DOS salidas, y por eso no vale envolver solo
    // la de abajo: la de arriba es el camino rapido --la cola ya tenia algo-- y
    // es justo por donde pasa un juego que va sobrado de eventos. Ver
    // [`rescatar`].
    if let Some(v) = sacar_crudo() {
        return raw_key_from_owner(Some(v));
    }
    // The CR3 wrapper is no longer here: it lives inside [`pump_bus`], the only
    // thing that touches the bus. Having it in every caller was the way for a new
    // caller to forget it.
    pump_bus();
    raw_key_from_owner(sacar_crudo())
}

fn sacar_crudo() -> Option<(u8, bool)> {
    unsafe {
        if CRUDOS_LEE == CRUDOS_ESCRIBE {
            return None;
        }
        let v = CRUDOS[CRUDOS_LEE];
        CRUDOS_LEE = (CRUDOS_LEE + 1) % EVENTOS_CRUDOS;
        Some(((v & 0xFF) as u8, v & 0x100 != 0))
    }
}

/// Eventos crudos tirados por cola llena. Para el panel.
pub fn eventos_crudos_perdidos() -> u32 {
    unsafe { CRUDOS_PERDIDOS }
}

/// Esta activo el tercer nivel? AltGr, o el Ctrl+Alt al que acostumbra
/// Windows (y por tanto los dedos de medio mundo).
/// Mascara de modificadores VIVA, para Ring 3.
///
/// El byte que entrega `INPUT_OP_TECLA` viene ya resuelto --la `n` es `0xF1`--
/// y eso es lo correcto para escribir, pero deja fuera los atajos: un
/// compositor no puede distinguir `Ctrl+Alt` de nada porque `Ctrl+Alt` sin
/// otra tecla no produce caracter. Esto lo abre sin tocar el camino de
/// escritura.
pub const MOD_SHIFT: u8 = 1 << 0;
pub const MOD_CTRL: u8 = 1 << 1;
pub const MOD_ALT: u8 = 1 << 2;
pub const MOD_ALTGR: u8 = 1 << 3;
pub const MOD_CAPS: u8 = 1 << 4;

pub fn modificadores() -> u8 {
    unsafe {
        let mut m = 0;
        if SHIFT { m |= MOD_SHIFT; }
        if CTRL { m |= MOD_CTRL; }
        if LALT { m |= MOD_ALT; }
        if ALTGR { m |= MOD_ALTGR; }
        if CAPS { m |= MOD_CAPS; }
        m
    }
}

/// * OJO al usar esto para atajos: en la distribucion espanola `Ctrl+Alt` ES
/// `AltGr` -- es lo que produce `@`, `#`, `[`, `]`, `\`, `|` y `EUR`. Un atajo
/// que dispare al PULSAR `Ctrl+Alt` rompe escribir todos esos caracteres. Ver
/// como lo resuelve el compositor: dispara al SOLTAR, y solo si no se escribio
/// nada mientras estaban pulsados.
fn altgr_active() -> bool {
    unsafe { ALTGR || (CTRL && LALT) }
}

/// Manda al teclado el estado de sus LEDs cuando cambia. Un SET_REPORT por
/// cambio, no por sondeo: es un control transfer y no hace falta mas.
fn sync_leds() {
    static mut LAST_LEDS: u8 = 0xFF;
    let want = crate::ring0::dev::keyboard::led_mask();
    unsafe {
        if LAST_LEDS == want { return; }
        LAST_LEDS = want;
        let hid = &*core::ptr::addr_of!(HID);
        hid.set_leds(want);
    }
}

/// Repite la tecla mantenida: tras `REPEAT_DELAY_MS` empieza a inyectarla
/// cada `REPEAT_RATE_MS`. El teclado USB solo avisa de bajada y subida --
/// repetir es trabajo del host, y sin esto mantener el retroceso no borra.
fn repeat_held() {
    unsafe {
        if HELD_CODE == 0 { return; }
        let hz = crate::ring0::task::scheduler::tsc_freq();
        if hz == 0 { return; }
        let now = crate::ring0::task::scheduler::rdtsc();
        let delay = hz / 1000 * REPEAT_DELAY_MS;
        let period = hz / 1000 * REPEAT_RATE_MS;
        if now.wrapping_sub(HELD_SINCE) < delay { return; }
        if now.wrapping_sub(HELD_LAST) < period { return; }
        HELD_LAST = now;
        keyboard::feed_full(HELD_CODE, HELD_SHIFT, HELD_ALTGR, CAPS, HELD_CTRL);
    }
}

/// Saca un caracter de la cola del teclado y lleva la cuenta. Aqui se graba
/// la PRIMERA tecla que cruza de verdad -- en el instante exacto, no deducida
/// despues comparando contadores.
fn drain() -> Option<u8> {
    let b = crate::ring0::dev::keyboard::pop_out()?;
    unsafe {
        KEY_EVENTS = KEY_EVENTS.wrapping_add(1);
        if !FIRST_KEY {
            FIRST_KEY = true;
            crate::ring0::cabina::info("usb", "primera tecla recibida: el teclado ESCRIBE", b as u64);
        }
    }
    Some(b)
}

/// Estado DETALLADO del HID para el panel de diagnostico (fila fija, sobrevive
/// al auto-clear). Devuelve: (teclado_listo, mouse_listo, slot_kbd, slot_mouse,
/// eventos_mouse, x_mouse, y_mouse, botones, eventos_tecla).
/// El puntero: `(x, y, botones, eventos)`.
///
/// Lo que `KIND_INPUT` entrega a Ring 3. Son los deltas del HID ya acumulados;
/// el recorte al panel lo hace `input.rs`, que es quien sabe de pantallas.
pub fn puntero() -> (i32, i32, u8, u32) {
    unsafe { (MOUSE_X, MOUSE_Y, MOUSE_BTN, MOUSE_EVENTS) }
}

/// Las vueltas de rueda desde la ultima vez, y las pone a cero.
///
/// Consumir al leer y no dar un acumulado: quien pregunta quiere saber cuanto
/// se ha girado DESDE QUE MIRO, no desde el arranque. Un acumulado obligaria a
/// cada llamante a guardar el anterior y restar, y el primero que lo olvidara
/// tendria un scroll que se va solo.
pub fn rueda() -> i32 {
    unsafe {
        let v = MOUSE_WHEEL;
        MOUSE_WHEEL = 0;
        v
    }
}

/// Vuelve a leer del driver quien hay y en que slot.
///
/// Se llama tras enumerar Y tras cada adopcion en caliente. Antes esto estaba
/// copiado en linea dentro de `init` y por eso no existia la posibilidad de
/// actualizarlo: un raton adoptado mas tarde habria seguido saliendo como
/// ausente en el panel aunque estuviera bombeando, y la fila del diagnostico
/// habria mentido justo cuando por fin decia la verdad.
///
/// # Safety
/// Toca los estaticos del modulo; solo desde el camino de USB.
unsafe fn refrescar_presencia() {
    let hid = &*core::ptr::addr_of!(HID);
    KBD_RDY = hid.has_kbd();
    MOUSE_RDY = hid.has_mouse();
    KBD_SLOT = hid.kbd_slot();
    MOUSE_SLOT = hid.mouse_slot();
}

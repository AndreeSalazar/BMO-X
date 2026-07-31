//! USB HID bridge: xHCI controller + boot-protocol keyboard/mouse en Ring 0.
//!
//! Motivo: la emulación USB→PS/2 del firmware MSI muere tras ExitBootServices
//! (el i8042 solo entrega ruido: 0xFE/0x6D), así que el teclado y el mouse
//! USB reales necesitan un driver xHCI de verdad. Este módulo es el PUENTE
//! entre el kernel y los drivers agnósticos `bmo-xhci`/`bmo-uhid`:
//!
//!   - Implementa `XhciHal` (DMA vía el frame allocator, phys→virt vía el
//!     physmap, log al panel de kernel coloreado).
//!   - Descubre el controlador xHCI en `ctx.pci_devices` (clase 0x0C serial
//!     bus, subclase 0x03 USB) y le pasa el MMIO del BAR0.
//!   - Traduce los `InputEvent` (scancodes Set 1) a ASCII con la MISMA tabla
//!     que el path PS/2, y los ofrece al shell por `poll_ascii`.
//!
//! v1 vive en Ring 0 (como el PS/2). Migrará a servidor Ring 3 vía Endpoint
//! RPC — el patrón DEVICE/DMA/IRQ como capabilities (roadmap F4).

use boot_context::BootContext;

use bmo_input::event::{InputEvent, InputEventKind};
use bmo_input::hal::InputHal;
use bmo_uhid::UsbHidHal;
use bmo_xhci::XhciHal;

use crate::ring0::dev::console::serial_write;
use crate::ring0::mm::{self, phys};

use crate::ring0::dev::pci;
use crate::ring0::dev::keyboard;

// Line buffer for the driver's diagnostic stream. The driver logs in
// fragments (`log("[uhid] slot=")` then `log_u64(..)` then `log("\n")`), so
// we accumulate to '\n' and flush the whole line to the on-screen panel —
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

/// El HAL que `bmo-xhci` invoca para DMA / traducción de direcciones / log.
struct KernelXhciHal;

impl XhciHal for KernelXhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64> {
        // Frames FÍSICAMENTE CONTIGUOS: los anillos TRB y buffers de reporte
        // se direccionan linealmente y el xHC los lee por dirección física.
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
/// AltGr mantenido (Alt derecho): abre el tercer nivel del teclado español.
static mut ALTGR: bool = false;
/// Ctrl mantenido (cualquiera de los dos).
static mut CTRL: bool = false;
/// Alt IZQUIERDO mantenido. Windows acepta Ctrl+Alt como AltGr, y quien
/// aprendió ahí lo tiene en los dedos: aquí también vale.
static mut LALT: bool = false;

// ── Repetición al mantener (typematic) ──────────────────────────────────────
//
// El teclado USB no repite solo: manda un reporte cuando la tecla BAJA y otro
// cuando SUBE, y entre medias silencio. Repetir es trabajo del host. Sin esto,
// mantener el retroceso borra UN carácter y se queda mirando.

/// Última tecla que sigue pulsada (0 = ninguna) y su contexto.
static mut HELD_CODE: u8 = 0;
static mut HELD_SHIFT: bool = false;
static mut HELD_ALTGR: bool = false;
static mut HELD_CTRL: bool = false;
/// TSC del momento en que se pulsó, y del último disparo automático.
static mut HELD_SINCE: u64 = 0;
static mut HELD_LAST: u64 = 0;
/// Espera antes de empezar a repetir, y periodo entre repeticiones (ms).
/// Los mismos valores de siempre: medio segundo de gracia, luego ~30 por
/// segundo — lo bastante rápido para borrar una línea sin pasarse.
const REPEAT_DELAY_MS: u64 = 500;
const REPEAT_RATE_MS: u64 = 33;
static mut PRESENT: bool = false;
// Diagnóstico DETALLADO del HID (pedido del usuario: "llamar al mouse, más
// detallado total"). Estado por dispositivo + telemetría viva del mouse, para
// que la próxima foto diga exactamente qué enumeró y si el mouse late.
static mut KBD_RDY: bool = false;
static mut MOUSE_RDY: bool = false;
static mut KBD_SLOT: u8 = 0;
static mut MOUSE_SLOT: u8 = 0;
static mut MOUSE_EVENTS: u32 = 0;   // nº de reportes de movimiento/botón vistos
static mut MOUSE_X: i32 = 0;        // posición acumulada (relativa) X
static mut MOUSE_Y: i32 = 0;        // posición acumulada (relativa) Y
static mut MOUSE_BTN: u8 = 0;       // bitmap de botones actual
static mut KEY_EVENTS: u32 = 0;     // nº de teclas imprimibles entregadas
static mut FIRST_KEY: bool = false;   // ¿ya se grabó la primera tecla en CABINA?
static mut FIRST_MOUSE: bool = false; // ídem para el primer movimiento de mouse
/// Vueltas de rueda acumuladas desde la ultima lectura. Se vacia al leerlo.
static mut MOUSE_WHEEL: i32 = 0;
static mut HID_EVENTS: u32 = 0;     // nº TOTAL de InputEvents de hid.poll (kbd+mouse)

fn log(msg: &str) {
    serial_write(msg);
    if crate::info::has_fb() {
        crate::ring0::core::phase::dashboard_log(msg);
    }
}

/// Espera real en milisegundos por TSC. El spec USB pide tiempos HUMANOS
/// (100 ms de debounce de conexión, 20+ ms de estabilización de power) — los
/// spin-counts heredados de QEMU duran microsegundos y en hardware real los
/// puertos aún no reportan CCS cuando el driver pregunta.
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
/// 1. Scan PCI propio (dev::pci, detrás de bridges, MEM+BME habilitados).
/// 2. Los Ryzen traen VARIOS xHC (CPU + chipset): se prueban en orden.
/// 3. Por controlador: init → power a TODOS los puertos → 200 ms de settle
///    (spec: 100 ms debounce) → censo PORTSC. Si algún puerto tiene CCS=1
///    (dispositivo FÍSICAMENTE presente), ese controlador gana y el HID
///    enumera ahí. El censo se pinta: dice dónde está el teclado
///    eléctricamente aunque la enumeración posterior fallara.
pub fn init(_ctx: &BootContext) {
    bmo_xhci::init_hal(&HAL);

    let mut chosen = false;
    for skip in 0..4usize {
        let loc = match pci::find_xhci(skip) {
            Some(l) => l,
            None => break,
        };
        // MMIO virtual: SIEMPRE por el physmap. La identidad de s2 vive en
        // PML4[0] y un espacio de Ring 3 sólo hereda su primer GiB, así que
        // tocar un BAR de ~4 GiB bajo el CR3 de un proceso es un #PF en Ring 0.
        // Aquí no se notaba porque el sondeo del xHC corre en una tarea de
        // Ring 0; el mismo fallo SÍ mataba al disco. Ver la nota larga en
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
        // power+reset después; para entonces CCS ya estará latcheado).
        for p in 0..nports {
            unsafe { bmo_xhci::port_power_on(p) };
        }
        delay_ms(200);
        // Censo: qué puertos tienen un dispositivo físico (PORTSC.CCS).
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
        // Nada conectado aquí: probar el siguiente controlador.
    }

    if !chosen {
        log("[usb] ningun xHC ve el teclado (probar otro puerto fisico)\n");
        crate::ring0::cabina::fault("usb", "ningun xHC ve dispositivos (probar otro puerto)", 0);
        return;
    }

    let ok = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        let r = hid.init();
        // Estado por dispositivo (teclado y mouse son interfaces separadas, o
        // dispositivos separados): registramos ambos aunque READY mire al kbd.
        KBD_RDY = hid.has_kbd();
        MOUSE_RDY = hid.has_mouse();
        KBD_SLOT = hid.kbd_slot();
        MOUSE_SLOT = hid.mouse_slot();
        r
    };
    unsafe {
        PRESENT = true;
        READY = ok;
    }
    // Resumen detallado en serial + panel (además del status fijo en pantalla).
    unsafe {
        if KBD_RDY {
            log("[usb] teclado USB listo (slot ");
            dlog_u64(KBD_SLOT as u64);
            log(")\n");
            crate::ring0::cabina::info("usb", "teclado enumerado y configurado", KBD_SLOT as u64);
            // Lo que de verdad decide si el teclado hablará: el estado del
            // endpoint según el xHC y el intervalo que quedó programado.
            let (st, bi, iv, _sp, sts) = kbd_ep_debug();
            crate::ring0::cabina::info("xhci", "kbd bInterval->Interval programado", ((bi as u64) << 8) | iv as u64);
            if st != 1 {
                crate::ring0::cabina::fault("xhci", "endpoint del teclado NO quedo Running", st as u64);
            }
            // HSE (bit 2) o HCE (bit 12): el controlador se cayó, todo lo demás
            // que veamos después es ruido.
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

/// ¿Se inicializó un teclado USB?
pub fn is_ready() -> bool {
    unsafe { READY }
}

/// Poll no bloqueante: drena eventos HID y devuelve UN ascii si hubo una
/// tecla imprimible (o Enter/Backspace/Tab). Mantiene el estado de Shift.
/// Alimenta `shell_read_line` igual que `keyboard::poll_ascii`.
///
/// ## Por qué esto se envuelve en un cambio de CR3
///
/// Tocar el xHCI es **escribir MMIO**: el `ERDP` del interrupter 0 vive en
/// `base + RTSOFF + 0x38`, que en esta placa cae en `0xFC2004F8`. Ese rango está
/// mapeado en el PML4 del kernel y **no** en el de una tarea de usuario.
///
/// Mientras el único que llamaba aquí era el shell de Ring 0 —una tarea de
/// kernel, con el CR3 del kernel cargado— eso no se notaba. Pero desde que
/// `KIND_INPUT` entrega teclas, este camino se recorre **desde dentro de un
/// SYSCALL**, y en un SYSCALL desde Ring 3 el CR3 sigue siendo el del llamante:
/// el cambio de CR3 solo ocurre en un cambio de contexto, y ahí todavía no ha
/// habido ninguno. El resultado fue un `#PF` de escritura sobre página ausente
/// en Ring 0 —`err=0x2`, `cr2=0xFC2004F8`— a los 144 ticks: en cuanto el
/// compositor pidió su primera tecla.
///
/// Es la misma trampa que ya está anotada en `fault_dispatch` para el
/// framebuffer ("el CR3 de usuario puede no mapear el rango identidad"). Aquí
/// la respuesta es la misma: ponerse el CR3 del kernel para tocar el hardware y
/// devolverlo al salir.
///
/// ★ No es gratis: dos escrituras de CR3 son dos vaciados de TLB, y esto se
/// llama una vez por fotograma. La solución barata de verdad sería mapear el
/// agujero de MMIO en todo espacio de direcciones —es memoria de supervisor,
/// así que Ring 3 no la vería igualmente— y eso ahorraría los dos vaciados. Se
/// deja anotado y no hecho: primero que funcione y esté aislado en un sitio.
pub fn poll_ascii() -> Option<u8> {
    use crate::ring0::mm::vmm;
    let kpml4 = vmm::kernel_pml4();
    let previo = vmm::read_cr3();
    // `kpml4 == 0` = todavía no hay PML4 de kernel publicado (arranque muy
    // temprano). Entonces el CR3 que hay ES el bueno y no se toca nada.
    let cambiado = kpml4 != 0 && previo != kpml4;
    if cambiado {
        vmm::switch_to(kpml4);
    }
    let r = poll_ascii_interno();
    // Se devuelve SIEMPRE, por un solo camino. `poll_ascii_interno` tiene
    // varios `return` y dejar el CR3 del kernel puesto al volver a Ring 3 sería
    // mucho peor que el fallo original: la tarea seguiría corriendo con el
    // espacio de direcciones de otro.
    if cambiado {
        vmm::switch_to(previo);
    }
    r
}

fn poll_ascii_interno() -> Option<u8> {
    // Correr si hay CUALQUIER dispositivo enumerado (no solo teclado): así el
    // mouse late en el diagnóstico aunque el teclado no haya enumerado.
    if !unsafe { PRESENT } {
        return None;
    }
    // Lo que dejó pendiente la pulsación anterior sale primero: una tecla
    // muerta que no combina produce DOS caracteres (´ + q = ´q).
    if let Some(b) = drain() { return Some(b); }

    // ¿Enchufaron o desenchufaron algo? El xHC lo avisa con un evento de
    // cambio de puerto, que hasta ahora se descartaba en el driver — por eso
    // desconectar el teclado y volver a conectarlo no revivía nada.
    //
    // Todavía NO se re-enumera: reconstruir el dispositivo es asignar slot,
    // direccionarlo y configurar endpoints, y eso hay que hacerlo bien o deja
    // el controlador a medias. Lo que sí hay ya es la CONSTANCIA — y con ella
    // se puede comprobar en el Ryzen que el aviso llega antes de escribir la
    // parte que actúa sobre él. Primero ver, luego hacer.
    if let Some((puerto, conectado)) = bmo_xhci::tomar_cambio_puerto() {
        if conectado {
            crate::ring0::cabina::info("usb", "puerto: algo se ENCHUFO (sin re-enumerar aun)", puerto as u64);
        } else {
            crate::ring0::cabina::warn("usb", "puerto: algo se DESENCHUFO", puerto as u64);
        }
    }

    let mut evs = [InputEvent::empty(); 16];
    let n = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.poll(&mut evs)
    };
    unsafe { HID_EVENTS = HID_EVENTS.wrapping_add(n as u32); }
    for ev in &evs[..n] {
        match ev.kind {
            InputEventKind::KeyDown => {
                // Shift (Set 1 make: 0x2A izq, 0x36 der).
                if ev.code == 0x2A || ev.code == 0x36 {
                    unsafe { SHIFT = true };
                    continue;
                }
                // AltGr: el tercer nivel del teclado español. Llega con
                // código propio (ver bmo_uhid::SC_ALTGR) para no confundirse
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
                // La distribución activa decide qué letra es. Lo que produzca
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
                // Soltar la tecla corta la repetición.
                unsafe { if HELD_CODE == ev.code { HELD_CODE = 0; } }
            }
            // MOUSE: antes se descartaba (esperaba el compositor F5). Ahora lo
            // "llamamos": acumulamos posición y botones para el diagnóstico y,
            // a futuro, el cursor del compositor.
            InputEventKind::MouseMove => unsafe {
                MOUSE_X = MOUSE_X.saturating_add(ev.mouse_dx() as i32);
                MOUSE_Y = MOUSE_Y.saturating_add(ev.mouse_dy() as i32);
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
            // ★ El delta de la rueda se TIRABA: solo se contaba el evento.
            // Otro valor que el sistema tenia y no decia. Se acumula y se
            // entrega al leerlo, que es como se consume un evento.
            InputEventKind::MouseWheel => unsafe {
                MOUSE_WHEEL = MOUSE_WHEEL.saturating_add(ev.mouse_wheel_delta() as i32);
                MOUSE_EVENTS = MOUSE_EVENTS.wrapping_add(1);
            },
        }
    }

    // Sincronizar las lucecitas: si el estado de los bloqueos cambió, hay que
    // DECÍRSELO al teclado. No se encienden solas.
    sync_leds();
    // Repetición de la tecla mantenida.
    repeat_held();
    drain()
}

/// ¿Está activo el tercer nivel? AltGr, o el Ctrl+Alt al que acostumbra
/// Windows (y por tanto los dedos de medio mundo).
/// Máscara de modificadores VIVA, para Ring 3.
///
/// El byte que entrega `INPUT_OP_TECLA` viene ya resuelto —la `ñ` es `0xF1`—
/// y eso es lo correcto para escribir, pero deja fuera los atajos: un
/// compositor no puede distinguir `Ctrl+Alt` de nada porque `Ctrl+Alt` sin
/// otra tecla no produce carácter. Esto lo abre sin tocar el camino de
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

/// ★ OJO al usar esto para atajos: en la distribucion espanola `Ctrl+Alt` ES
/// `AltGr` — es lo que produce `@`, `#`, `[`, `]`, `\`, `|` y `EUR`. Un atajo
/// que dispare al PULSAR `Ctrl+Alt` rompe escribir todos esos caracteres. Ver
/// como lo resuelve el compositor: dispara al SOLTAR, y solo si no se escribio
/// nada mientras estaban pulsados.
fn altgr_active() -> bool {
    unsafe { ALTGR || (CTRL && LALT) }
}

/// Manda al teclado el estado de sus LEDs cuando cambia. Un SET_REPORT por
/// cambio, no por sondeo: es un control transfer y no hace falta más.
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
/// cada `REPEAT_RATE_MS`. El teclado USB solo avisa de bajada y subida —
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

/// Saca un carácter de la cola del teclado y lleva la cuenta. Aquí se graba
/// la PRIMERA tecla que cruza de verdad — en el instante exacto, no deducida
/// después comparando contadores.
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

/// Estado DETALLADO del HID para el panel de diagnóstico (fila fija, sobrevive
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

pub fn hid_stats() -> (bool, bool, u8, u8, u32, i32, i32, u8, u32) {
    unsafe {
        (KBD_RDY, MOUSE_RDY, KBD_SLOT, MOUSE_SLOT,
         MOUSE_EVENTS, MOUSE_X, MOUSE_Y, MOUSE_BTN, KEY_EVENTS)
    }
}

/// Contadores de bajo nivel del xHC + HID para cazar el corte del teclado:
/// (transfer_events_del_xHC, raw_events_del_xHC, hid_events_totales).
/// Si al teclear TEV no sube → el xHC no completa la interrupción (endpoint/
/// ring/doorbell). Si TEV sube pero HEV no → el evento no matchea al teclado.
/// Si HEV sube pero kev no → mapeo (ya no deberia tras el keypad).
pub fn xfer_stats() -> (u32, u32, u32) {
    (bmo_xhci::xfer_events(), bmo_xhci::raw_events(), unsafe { HID_EVENTS })
}

/// El reparto de informes: `(bombea el teclado, bombea el raton, huerfanos)`.
///
/// Los dos primeros son la pregunta que no se podia hacer: un periferico sin
/// transferencia encolada esta enumerado, con el endpoint en `Running`, y mudo
/// para siempre. El tercero cuenta los Transfer Events que no eran de ningun
/// periferico conocido — antes se descartaban sin dejar rastro.
pub fn reparto_stats() -> (bool, bool, u32) {
    unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        let (k, r) = hid.bombeando();
        (k, r, hid.huerfanos())
    }
}

/// El aparcadero de eventos del xHC: `(aparcados en total, PERDIDOS, ahora)`.
///
/// El anillo de eventos es uno para todo el controlador, así que quien espera
/// una compleción de comando se cruza con los informes de los aparatos que ya
/// están bombeando. Antes los descartaba, y descartar el primer informe de un
/// endpoint lo deja mudo para siempre — nadie vuelve a encolar la
/// transferencia. Ahora se aparcan; `PERDIDOS` es lo que hay que vigilar.
pub fn park_stats() -> (u32, u32, u32) {
    bmo_xhci::evt_park_stats()
}

/// Salud del endpoint de interrupción del teclado leída DEL HARDWARE, no de
/// nuestras suposiciones: `(ep_state, bInterval_del_descriptor,
/// Interval_programado, speed, usbsts)`.
///
/// `ep_state` sale del Device Context que mantiene el xHC: 1=Running es lo
/// único aceptable. 2=Halted, 3=Stopped o 4=Error significan que el endpoint no
/// está agendado y ningún doorbell lo va a revivir. `bi`/`iv` delatan el bug
/// clásico del Interval (ver `bmo_xhci::encode_interval`).
pub fn kbd_ep_debug() -> (u8, u8, u8, u8, u32) {
    let (slot, dci) = unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        (KBD_SLOT, hid.kbd_dci())
    };
    let st = unsafe { bmo_xhci::ep_state(slot, dci) };
    let (bi, iv, sp) = bmo_xhci::last_ep_timing();
    (st, bi, iv, sp, unsafe { bmo_xhci::usbsts() })
}

/// DCI del teclado + último Transfer Event (slot, ep, cc) del xHC. Si el ep del
/// último evento ≠ dci del teclado, el evento no matchea y no se re-encola →
/// tev pegado en 1. Ese es el corte que buscamos.
pub fn kbd_debug() -> (u8, u8, u8, u8) {
    let dci = unsafe {
        let hid = &*core::ptr::addr_of!(HID);
        hid.kbd_dci()
    };
    let (s, e, c) = bmo_xhci::last_event();
    (dci, s, e, c)
}

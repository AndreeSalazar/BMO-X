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
static mut PRESENT: bool = false;

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
    let f = crate::ring0::scheduler::tsc_freq();
    if f == 0 {
        for _ in 0..ms * 2_000_000 {
            core::hint::spin_loop();
        }
        return;
    }
    let end = crate::ring0::scheduler::rdtsc() + ms * (f / 1000);
    while crate::ring0::scheduler::rdtsc() < end {
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
        // MMIO virtual: < 4 GiB cae en la identidad de s2; más arriba lo
        // cubre el physmap.
        let mmio_va = if loc.mmio < 0x1_0000_0000 {
            loc.mmio
        } else {
            mm::phys_to_virt(loc.mmio)
        };
        dlog_push("[usb] xHC pci ");
        dlog_u64(loc.bus as u64);
        dlog_push(":");
        dlog_u64(loc.dev as u64);
        dlog_push(".");
        dlog_u64(loc.func as u64);
        dlog_push(" mmio=");
        dlog_u64(loc.mmio);
        dlog_push("\n");

        bmo_xhci::reset_ctrl();
        bmo_xhci::set_mmio(mmio_va);
        if !unsafe { bmo_xhci::init(mmio_va) } {
            dlog_push("[usb] init fallo en este xHC, probando siguiente\n");
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
            chosen = true;
            break;
        }
        // Nada conectado aquí: probar el siguiente controlador.
    }

    if !chosen {
        log("[usb] ningun xHC ve el teclado (probar otro puerto fisico)\n");
        return;
    }

    let ok = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.init()
    };
    unsafe {
        PRESENT = true;
        READY = ok;
    }
    if ok {
        log("[usb] teclado USB listo\n");
    } else {
        log("[usb] dispositivo visto pero HID no enumero (ver lineas uhid)\n");
    }
}

/// ¿Se inicializó un teclado USB?
pub fn is_ready() -> bool {
    unsafe { READY }
}

/// Poll no bloqueante: drena eventos HID y devuelve UN ascii si hubo una
/// tecla imprimible (o Enter/Backspace/Tab). Mantiene el estado de Shift.
/// Alimenta `shell_read_line` igual que `keyboard::poll_ascii`.
pub fn poll_ascii() -> Option<u8> {
    if !unsafe { READY } {
        return None;
    }
    let mut evs = [InputEvent::empty(); 16];
    let n = unsafe {
        let hid = &mut *core::ptr::addr_of_mut!(HID);
        hid.poll(&mut evs)
    };
    let mut out: Option<u8> = None;
    for ev in &evs[..n] {
        match ev.kind {
            InputEventKind::KeyDown => {
                // Shift (Set 1 make: 0x2A izq, 0x36 der).
                if ev.code == 0x2A || ev.code == 0x36 {
                    unsafe { SHIFT = true };
                    continue;
                }
                if let Some(a) =
                    crate::ring0::dev::keyboard::scancode1_to_ascii(ev.code, unsafe { SHIFT })
                {
                    out = Some(a); // el último gana (typematic ya viene diffeado)
                }
            }
            InputEventKind::KeyUp => {
                if ev.code == 0x2A || ev.code == 0x36 {
                    unsafe { SHIFT = false };
                }
            }
            _ => {} // mouse: se cablea con el compositor (F5)
        }
    }
    out
}

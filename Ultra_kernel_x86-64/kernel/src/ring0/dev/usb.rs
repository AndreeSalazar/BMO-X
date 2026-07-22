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

use crate::ring0::dev::console::{serial_write, serial_write_u64};
use crate::ring0::mm::{self, phys};

/// PCI class/subclass del controlador USB.
const PCI_CLASS_SERIAL_BUS: u8 = 0x0C;
const PCI_SUBCLASS_USB: u8 = 0x03;

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
        serial_write("[usb] ");
        serial_write(msg);
    }
    fn log_u64(&self, msg: &str, val: u64) {
        serial_write(msg);
        serial_write_u64(val, 1);
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

/// Localiza el controlador xHCI y trae su MMIO (BAR0, enmascarando los bits
/// de tipo). Asume MMIO < 4 GiB (identity-mapped por s2) — cierto en esta
/// placa; el caso 64-bit alto se atenderá cuando el BootContext capture BAR1.
fn find_xhci(ctx: &BootContext) -> Option<u64> {
    for i in 0..ctx.pci_count.min(ctx.pci_devices.len() as u32) as usize {
        let d = ctx.pci_devices[i];
        if d.class == PCI_CLASS_SERIAL_BUS && d.subclass == PCI_SUBCLASS_USB {
            // BAR de memoria: bit0=0. Enmascarar los 4 bits bajos de tipo.
            let bar = (d.bar0 & 0xFFFF_FFF0) as u64;
            if bar != 0 {
                log("[usb] xHC en PCI ");
                serial_write_u64(d.bus as u64, 1);
                log(":");
                return Some(bar);
            }
        }
    }
    None
}

/// Descubre e inicializa xHCI + HID. Idempotente. Reporta al panel.
pub fn init(ctx: &BootContext) {
    let mmio = match find_xhci(ctx) {
        Some(m) => m,
        None => {
            log("[usb] no se encontro controlador xHCI\n");
            return;
        }
    };
    bmo_xhci::init_hal(&HAL);
    bmo_xhci::set_mmio(mmio);

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
        log("[usb] xHCI init sin teclado (ver serial)\n");
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

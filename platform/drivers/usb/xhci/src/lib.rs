//! xHCI USB Controller Driver -- full device lifecycle + HID interrupt transfers.
//!
//! Modeled after the proven xhci-nostd crate at github.com/suhteevah/xhci-nostd.

#![no_std]
#![allow(static_mut_refs)]

/// La cola de avisos de cambio de puerto. Vive aparte porque es la unica parte
/// de este driver que se puede probar sin un xHC delante -- y era la que estaba
/// mal: un buzon de una plaza donde el enchufe pisaba al desenchufe.
pub mod avisos;

/// **De un aparato enchufado a un aparato con direccion**: puertos, ranuras y
/// `Address Device`. Aparte porque es UNA pregunta y se dice en una linea (L6b).
mod enumerar;
pub use enumerar::*;

/// **Hablar con un aparato que ya tiene direccion**: transferencias de control,
/// endpoints, y el sondeo que no bloquea. Aqui vive `RESUCITAR UN ENDPOINT
/// PARADO`, que es la leccion mas cara de este driver.
mod transferencia;
pub use transferencia::*;

use avisos::Avisos;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ===================================================================
//  HAL trait
// ===================================================================

pub trait XhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64>;
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    fn log(&self, msg: &str);
    fn log_u64(&self, msg: &str, val: u64);
    /// Espera de tiempo REAL en milisegundos. Los tiempos fisicos del USB
    /// (debounce de conexion, reset de puerto, recovery) son del orden de
    /// decenas de ms -- imposibles de cubrir con spin-counts sin depender de
    /// la frecuencia del CPU. El default es un spin grosero (para QEMU/tests);
    /// el kernel lo sobreescribe con una espera exacta por TSC.
    fn delay_ms(&self, ms: u64) {
        for _ in 0..(ms * 500_000) {
            core::hint::spin_loop();
        }
    }
}

static mut XHCI_HAL: Option<&'static dyn XhciHal> = None;
static INIT: AtomicBool = AtomicBool::new(false);

pub fn init_hal(hal: &'static dyn XhciHal) {
    if INIT.swap(true, Ordering::SeqCst) { return; }
    unsafe { XHCI_HAL = Some(hal); }
}
pub fn hal() -> &'static dyn XhciHal { unsafe { XHCI_HAL.expect("XhciHal not init") } }

static MMIO_BASE: AtomicUsize = AtomicUsize::new(0);

pub fn set_mmio(mmio: u64) { MMIO_BASE.store(mmio as usize, Ordering::Relaxed); }
pub fn get_mmio() -> Option<u64> {
    let v = MMIO_BASE.load(Ordering::Relaxed);
    if v == 0 { None } else { Some(v as u64) }
}
pub fn is_controller_initialized() -> bool { unsafe { CTRL.is_some() } }

// ===================================================================
//  Registers
// ===================================================================

const CAPLENGTH: u32 = 0x00; const HCSPARAMS1: u32 = 0x04;
#[allow(dead_code)] const HCSPARAMS2: u32 = 0x08;
#[allow(dead_code)] const HCSPARAMS3: u32 = 0x0C;
const HCCPARAMS1: u32 = 0x10; const DBOFF: u32 = 0x14; const RTSOFF: u32 = 0x18;
const USBCMD: u32 = 0x00; const USBSTS: u32 = 0x04;
#[allow(dead_code)] const PAGESIZE: u32 = 0x08; const CONFIG: u32 = 0x38;
#[allow(dead_code)] const DBOFF_DB: u32 = 0x00;
const RT_IMAN: u32 = 0x20; #[allow(dead_code)] const RT_IMOD: u32 = 0x24;
const RT_ERSTSZ: u32 = 0x28; const RT_ERSTBA: u32 = 0x30;
const RT_ERDP: u32 = 0x38;
/// `ERDP` bit 3 -- **EHB, Event Handler Busy**. Lo pone el xHC al publicar un
/// evento y es *write-1-to-clear*: el software lo baja escribiendole un 1 al
/// actualizar el ERDP.
///
/// * No bajarlo NO es un detalle cosmetico: el xHC considera que el manejador
/// sigue ocupado, el anillo de eventos se llena, entra en **Event Ring Full**
/// y **deja de publicar eventos para siempre**. El sintoma en esta maquina era
/// exacto: aporrear el teclado mientras arranca (cuando todavia nadie drena el
/// anillo) lo llenaba, y a partir de ahi el teclado estaba muerto hasta
/// reiniciar. Sin aporrear, el anillo nunca se llenaba y no se notaba.
///
/// Como `erdp` va alineado a 16 bytes, el bit 3 salia siempre 0 -- o sea que
/// se escribia "EHB sigue ocupado" en cada vuelta.
const ERDP_EHB: u64 = 1 << 3;
const PORTSC: u32 = 0x00;
const USBCMD_RS: u32 = 1 << 0; const USBCMD_HCRST: u32 = 1 << 1;
const USBSTS_HCH: u32 = 1 << 0; const USBSTS_CNR: u32 = 1 << 11;
const PORTSC_CCS: u32 = 1 << 0; const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR: u32 = 1 << 4; const PORTSC_PP: u32 = 1 << 9;
const PORTSC_CSC: u32 = 1 << 17; const PORTSC_PRC: u32 = 1 << 21;
const IMAN_IE: u32 = 1 << 1; #[allow(dead_code)] const IMAN_IP: u32 = 1 << 0;

const TRB_NORMAL: u32 = 1;  const TRB_SETUP: u32 = 2;
/// **TRB Isocrono (tipo 5).** El hermano del Normal, y el unico que puede llevar
/// muestras a un endpoint de audio. Ver `queue_isoch_out`.
const TRB_ISOCH: u32 = 5;
const TRB_DATA: u32 = 3;    const TRB_STATUS: u32 = 4;
const TRB_LINK: u32 = 6;    const TRB_ENABLE: u32 = 9;
const TRB_DISABLE: u32 = 10;
const TRB_ADDRESS_DEV: u32 = 11; const TRB_CONFIGURE: u32 = 12;
#[allow(dead_code)] const TRB_EVAL_CTX: u32 = 13;
const TRB_RESET_EP: u32 = 14; const TRB_SET_TR_DEQ: u32 = 16;
const TRB_TRANSFER: u32 = 32; const TRB_COMPLETION: u32 = 33;
const TRB_PORT_STATUS: u32 = 34;

const TRB_SIZE: usize = 16;
const RING_SIZE: usize = 256;
const LAST_TRB_IDX: usize = RING_SIZE - 1; // Link TRB lives here

const CC_SUCCESS: u32 = 1;
const CC_SHORT: u32 = 13;

// ===================================================================
//  TRB  (16 bytes, 4 DWORDs)
// ===================================================================

#[derive(Clone, Copy)]
pub struct Trb { dw0: u32, dw1: u32, dw2: u32, dw3: u32 }

impl Trb {
    #[allow(dead_code)]
    fn zeroed() -> Self { Trb { dw0: 0, dw1: 0, dw2: 0, dw3: 0 } }
    #[allow(dead_code)]
    fn with_ptr(ptr: u64) -> Self {
        Trb { dw0: ptr as u32, dw1: (ptr >> 32) as u32, dw2: 0, dw3: 0 }
    }
}

// ===================================================================
//  Transfer Ring
// ===================================================================

pub struct TransferRing {
    dma_virt: *mut u32,
    dma_phys: u64,
    enqueue: usize,
    pcs: bool,
}

impl TransferRing {
    /// Create a new ring. The DMA buffer must be 4K, zeroed.
    /// Places a Link TRB at LAST_TRB_IDX pointing back to `dma_phys`.
    pub unsafe fn new(dma_virt: *mut u32, dma_phys: u64) -> Self {
        let r = Self { dma_virt, dma_phys: dma_phys & !0xF, enqueue: 0, pcs: true };
        // Link TRB at the last slot
        let base = LAST_TRB_IDX * 4;
        r.dma_virt.add(base    ).write_volatile((r.dma_phys & 0xFFFF_FFFF) as u32);
        r.dma_virt.add(base + 1).write_volatile(((r.dma_phys >> 32) & 0xFFFF_FFFF) as u32);
        r.dma_virt.add(base + 2).write_volatile(0);
        r.dma_virt.add(base + 3).write_volatile((TRB_LINK << 10) | 1);
        r
    }

    /// Anillo SIN Link TRB -- para el **Event Ring**, que no lleva uno: el xHC
    /// conoce su tamano por el ERST y da la vuelta solo. Escribirle un Link TRB
    /// (lo que hacia `new`) dejaba basura en la ultima entrada que el consumidor
    /// leia como si fuera un evento real en la primera vuelta.
    pub unsafe fn new_unlinked(dma_virt: *mut u32, dma_phys: u64) -> Self {
        Self { dma_virt, dma_phys: dma_phys & !0xF, enqueue: 0, pcs: true }
    }

    /// Enable Toggle Cycle on the Link TRB: the consumer inverts its cycle
    /// state when it traverses the link, matching the producer's flip.
    pub unsafe fn enable_toggle_cycle(&mut self) {
        let base = LAST_TRB_IDX * 4;
        let dw3 = self.dma_virt.add(base + 3).read_volatile();
        self.dma_virt.add(base + 3).write_volatile(dw3 | (1 << 1));
    }

    pub fn phys_with_dcs(&self) -> u64 { self.dma_phys | if self.pcs { 1 } else { 0 } }

    /// Write a TRB at the current enqueue position, advance.
    pub fn enqueue(&mut self, trb: &Trb) {
        let idx = self.enqueue;
        let b = idx * 4;
        unsafe {
            self.dma_virt.add(b    ).write_volatile(trb.dw0);
            self.dma_virt.add(b + 1).write_volatile(trb.dw1);
            self.dma_virt.add(b + 2).write_volatile(trb.dw2);
            let cycle = if self.pcs { 1u32 } else { 0u32 };
            self.dma_virt.add(b + 3).write_volatile(trb.dw3 | cycle);
        }
        self.advance();
    }

    /// Advance enqueue, wrapping at Link TRB.
    fn advance(&mut self) {
        self.enqueue += 1;
        if self.enqueue >= LAST_TRB_IDX {
            // Update Link TRB cycle bit to match current PCS
            let lb = LAST_TRB_IDX * 4;
            unsafe {
                let link_dw3 = self.dma_virt.add(lb + 3).read_volatile();
                if self.pcs { self.dma_virt.add(lb + 3).write_volatile(link_dw3 | 1); }
                else { self.dma_virt.add(lb + 3).write_volatile(link_dw3 & !1u32); }
            }
            self.enqueue = 0;
            self.pcs = !self.pcs;
        }
    }
}

// ===================================================================
//  USB Descriptor types  (for external consumers)
// ===================================================================

pub const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SET_CONFIG: u8 = 0x09;
pub const USB_DESC_DEVICE: u16 = 1;
pub const USB_DESC_CONFIG: u16 = 2;

// ===================================================================
//  Controller
// ===================================================================

pub struct XhciController {
    pub mmio: u64, pub op_base: u32, pub rt_base: u32, pub db_base: u32,
    pub max_slots: u8, pub max_ports: u8, pub ctx_size: u8,
    pub dcbaa_phys: u64,
    pub cmd_ring: TransferRing,
    pub event_ring: TransferRing,
    pub erst_phys: u64,
    pub initialized: bool,
    pub evt_dequeue: u32, pub evt_cycle: u32,
}

static mut CTRL: Option<XhciController> = None;
pub fn controller() -> Option<&'static XhciController> { unsafe { CTRL.as_ref() } }
pub fn controller_mut() -> Option<&'static mut XhciController> { unsafe { CTRL.as_mut() } }

// ===================================================================
//  MMIO helpers
// ===================================================================

unsafe fn r32(addr: u64) -> u32 { core::ptr::read_volatile(addr as *const u32) }
unsafe fn w32(addr: u64, val: u32) { core::ptr::write_volatile(addr as *mut u32, val); }
unsafe fn op_r(m: u64, o: u32, off: u32) -> u32 { r32(m + o as u64 + off as u64) }
unsafe fn op_w(m: u64, o: u32, off: u32, v: u32) { w32(m + o as u64 + off as u64, v) }
unsafe fn rt_w(m: u64, r: u32, off: u32, v: u32) { w32(m + r as u64 + off as u64, v) }

fn ctx_sz(c: &XhciController) -> usize { if c.ctx_size != 0 { 64 } else { 32 } }

// ===================================================================
//  Event ring consumer
// ===================================================================

/// Que evento espera quien se bloquea.
///
/// **El anillo de eventos es UNO para todo el controlador.** Un teclado, un
/// raton, una complecion de comando y un cambio de puerto salen todos por el
/// mismo sitio, en el orden en que el xHC los postea. Por eso quien espera
/// tiene que decir QUE espera: coger el primero que pase es coger el de otro.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Espera {
    /// Una complecion de comando. El anillo de comandos se usa de uno en uno,
    /// asi que no hace falta distinguir cual.
    Comando,
    /// Un Transfer Event de un endpoint CONCRETO.
    Transferencia { slot: u8, ep: u8 },
}

fn cuadra(ev: &(u32, u32, u32, u32), esp: Espera) -> bool {
    let typ = (ev.3 >> 10) & 0x3F;
    match esp {
        Espera::Comando => typ == TRB_COMPLETION,
        Espera::Transferencia { slot, ep } => {
            typ == TRB_TRANSFER
                && ((ev.3 >> 24) & 0xFF) as u8 == slot
                && ((ev.3 >> 16) & 0x1F) as u8 == ep
        }
    }
}

// -- El aparcadero de eventos ----------------------------------------
//
// * ESTA ES LA PIEZA QUE FALTABA, y costo el teclado entero.
//
// Antes, esperar bloqueando sacaba del anillo **el primer evento de cualquier
// tipo** y lo daba por bueno; los tres bucles que si miraban el tipo
// (`send_cmd`, `control_transfer`) **descartaban** lo que no era suyo, y
// `address_device` y `configure_endpoint` ni siquiera miraban el tipo: leian
// el `cc` de lo que hubiera. Como un Transfer Event correcto tambien trae
// `cc=1`, un informe del raton se leia como "el comando salio bien".
//
// Mientras nada estuviera bombeando durante la enumeracion, no se notaba. En
// cuanto un endpoint de interrupcion quedo vivo **mientras se enumeraba el
// siguiente puerto**, cada control transfer del teclado se comia un informe
// del raton: el evento desaparecia, `uhid::poll` no lo veia nunca, y sin ese
// evento **nadie vuelve a encolar la transferencia**. La bomba no arranca, el
// endpoint se queda en `Running` para siempre y el aparato enmudece sin un
// solo error. Los dos, raton y teclado, por el mismo camino.
//
// La regla ahora: **un evento que no es mio se APARCA, jamas se tira.**
const APARCADOS_MAX: usize = 64;
static mut APARCADOS: [(u32, u32, u32, u32); APARCADOS_MAX] = [(0, 0, 0, 0); APARCADOS_MAX];
static mut APARCADOS_N: usize = 0;
/// Cuantos se han aparcado en total y cuantos se han PERDIDO por aparcadero
/// lleno. Lo segundo tiene que ser cero; si un dia no lo es, hay que subir el
/// tope -- y se vera, que es justo lo que no pasaba antes.
static mut APARCADOS_TOTAL: u32 = 0;
static mut APARCADOS_PERDIDOS: u32 = 0;

/// `(aparcados en total, dropped por lleno, aparcados ahora mismo)`.
pub fn evt_park_stats() -> (u32, u32, u32) {
    unsafe { (APARCADOS_TOTAL, APARCADOS_PERDIDOS, APARCADOS_N as u32) }
}

unsafe fn aparcar(ev: (u32, u32, u32, u32)) {
    if APARCADOS_N >= APARCADOS_MAX {
        // Perder uno aqui vuelve a matar un endpoint. Se cuenta para que se
        // vea en CABINA en vez de repetir el silencio de antes.
        APARCADOS_PERDIDOS = APARCADOS_PERDIDOS.wrapping_add(1);
        return;
    }
    APARCADOS[APARCADOS_N] = ev;
    APARCADOS_N += 1;
    APARCADOS_TOTAL = APARCADOS_TOTAL.wrapping_add(1);
}

/// Saca del aparcadero el primero que cuadre con lo que se espera, si lo hay.
///
/// Hace falta porque una espera anterior pudo aparcar justo lo que ahora se
/// busca: una complecion de comando aparcada mientras un control transfer
/// esperaba su Transfer Event.
unsafe fn desaparcar_que_cuadre(esp: Espera) -> Option<(u32, u32, u32, u32)> {
    let mut i = 0;
    while i < APARCADOS_N {
        if cuadra(&APARCADOS[i], esp) {
            let ev = APARCADOS[i];
            let mut j = i;
            while j + 1 < APARCADOS_N {
                APARCADOS[j] = APARCADOS[j + 1];
                j += 1;
            }
            APARCADOS_N -= 1;
            return Some(ev);
        }
        i += 1;
    }
    None
}

/// El mas viejo del aparcadero, sea de quien sea. Lo drena el bucle de sondeo.
unsafe fn desaparcar_cualquiera() -> Option<(u32, u32, u32, u32)> {
    if APARCADOS_N == 0 {
        return None;
    }
    let ev = APARCADOS[0];
    let mut j = 0;
    while j + 1 < APARCADOS_N {
        APARCADOS[j] = APARCADOS[j + 1];
        j += 1;
    }
    APARCADOS_N -= 1;
    Some(ev)
}

/// Saca un TRB del anillo y avanza el ERDP. No filtra: es la boca del anillo.
unsafe fn evt_ring_pop(ctrl: &mut XhciController) -> Option<(u32, u32, u32, u32)> {
    let base = hal().phys_to_virt(ctrl.erst_phys) as *const u32;
    let dq = ctrl.evt_dequeue;
    let cy = ctrl.evt_cycle;
    let dw3 = base.add((dq as usize) * 4 + 3).read_volatile();
    if (dw3 & 1) != cy {
        return None;
    }
    let dw0 = base.add((dq as usize) * 4).read_volatile();
    let dw1 = base.add((dq as usize) * 4 + 1).read_volatile();
    let dw2 = base.add((dq as usize) * 4 + 2).read_volatile();
    let mut ndq = dq + 1;
    let mut ncy = cy;
    if ndq >= RING_SIZE as u32 {
        ndq = 0;
        ncy ^= 1;
    }
    let erdp = ctrl.erst_phys + (ndq as u64) * (TRB_SIZE as u64);
    w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64, (erdp & 0xFFFF_FFFF) as u32);
    w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4, ((erdp >> 32) & 0xFFFF_FFFF) as u32);
    ctrl.evt_dequeue = ndq;
    ctrl.evt_cycle = ncy;
    Some((dw0, dw1, dw2, dw3))
}

/// Espera **lo que se le pide**, aparcando todo lo demas.
///
/// Devuelve `None` solo si se agota el plazo sin que llegue: eso si es un
/// fallo del controlador, y ahora se distingue de "llego lo de otro".
unsafe fn evt_poll_block(
    ctrl: &mut XhciController,
    esp: Espera,
) -> Option<(u32, u32, u32, u32)> {
    for _ in 0..500000 {
        if let Some(ev) = desaparcar_que_cuadre(esp) {
            return Some(ev);
        }
        match evt_ring_pop(ctrl) {
            Some(ev) => {
                if cuadra(&ev, esp) {
                    return Some(ev);
                }
                aparcar(ev);
            }
            None => core::hint::spin_loop(),
        }
    }
    None
}

unsafe fn evt_poll_nb(ctrl: &mut XhciController) -> Option<(u32, u32, u32, u32)> {
    let base = hal().phys_to_virt(ctrl.erst_phys) as *const u32;
    let dq = ctrl.evt_dequeue;
    let dw3 = base.add((dq as usize) * 4 + 3).read_volatile();
    if (dw3 & 1) == ctrl.evt_cycle {
        let dw0 = base.add((dq as usize) * 4).read_volatile();
        let dw1 = base.add((dq as usize) * 4 + 1).read_volatile();
        let dw2 = base.add((dq as usize) * 4 + 2).read_volatile();
        let ndq = if dq + 1 >= RING_SIZE as u32 { 0 } else { dq + 1 };
        let ncy = if ndq == 0 { ctrl.evt_cycle ^ 1 } else { ctrl.evt_cycle };
        let erdp = ctrl.erst_phys + (ndq as u64) * (TRB_SIZE as u64);
        // * La mitad ALTA primero, la baja despues. La baja es la que lleva el
        // EHB y la que el xHC toma como "ya esta": escribir primero la baja
        // dejaria, durante unos ciclos, una direccion con la mitad nueva y la
        // mitad vieja. Aqui casi nunca cambia la alta, pero el orden correcto
        // no cuesta nada y el incorrecto solo falla el dia que el anillo cruce
        // una frontera de 4 GiB.
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4, ((erdp >> 32) & 0xFFFF_FFFF) as u32);
        // Y con EHB puesto, que es lo que lo BAJA (RW1C). Ver `ERDP_EHB`: sin
        // esto el anillo se llena y el xHC deja de publicar eventos.
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64, ((erdp | ERDP_EHB) & 0xFFFF_FFFF) as u32);
        ctrl.evt_dequeue = ndq; ctrl.evt_cycle = ncy;
        Some((dw0, dw1, dw2, dw3))
    } else { None }
}

// ===================================================================
//  Helpers
// ===================================================================

unsafe fn dcbaa_get(slot: u8) -> Option<u64> {
    let ctrl = CTRL.as_ref()?;
    let a = hal().phys_to_virt(ctrl.dcbaa_phys) as *const u64;
    let v = a.add(slot as usize).read_volatile();
    if v == 0 { None } else { Some(v) }
}

unsafe fn send_cmd(trb: Trb) -> Option<(u32, u32, u32, u32)> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    // El bucle que habia aqui descartaba en silencio todo lo que no fuera una
    // complecion. Ahora la seleccion la hace `evt_poll_block`, que ademas lo
    // aparca en vez de tirarlo.
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    let cc = (ev.2 >> 24) & 0xFF;
    if cc == CC_SUCCESS || cc == CC_SHORT { return Some(ev); }
    None
}

// ===================================================================
//  Init  (unchanged logic, proven)
// ===================================================================

/// Reset the global controller state so init() can be called again
/// (needed when switching between CPU SoC and chipset XHCI controllers).
pub fn reset_ctrl() {
    unsafe { CTRL = None; }
    MMIO_BASE.store(0, core::sync::atomic::Ordering::SeqCst);
}

/// Una espera acotada que **dice si acerto**. Reemplaza al `for` que se agotaba
/// en silencio y seguia como si nada.
fn esperar(mut listo: impl FnMut() -> bool) -> bool {
    for _ in 0..1_000_000 {
        if listo() {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// ID de la capacidad extendida "USB Legacy Support" (xHCI spec, tabla 7-1).
const XECP_ID_LEGACY: u32 = 1;
/// USBLEGSUP: el firmware dice que es suyo.
const LEGSUP_BIOS: u32 = 1 << 16;
/// USBLEGSUP: nosotros decimos que es nuestro.
const LEGSUP_OS: u32 = 1 << 24;
/// USBLEGCTLSTS: todos los "manda un SMI cuando pase X" juntos.
const LEGCTL_SMI_ENABLES: u32 = (0x7 << 0) | (0xFF << 5) | (0x7 << 17);
/// USBLEGCTLSTS: los tres avisos que se limpian escribiendo un 1.
const LEGCTL_SMI_STATUS: u32 = 0x7 << 29;

/// **Busca una capacidad extendida recorriendo la lista.**
///
/// === El fallo que esto corrige, y es de campo ===
///
/// Estaba escrito asi:
///
/// ```text
///    let eecp = ((hcc1 >> 8) & 0xFF) as u32;   // <- bits 15:8
/// ```
///
/// El puntero a capacidades extendidas (**xECP**) vive en `HCCPARAMS1[31:16]`
/// y **cuenta en palabras de 32 bits**, no en bytes. Los bits 15:8 son
/// `MaxPSASize`, `CFC`, `SEC`, `SPC` y `PAE`: no un offset de nada.
///
/// Consecuencias, las tres:
///
///   1. El traspaso del firmware **nunca ocurria**, aunque el codigo pareciera
///      hacerlo.
///   2. Si esos bits daban >= 0x40, se escribia un 1 en `mmio + basura + 4`:
///      una escritura MMIO **en un sitio cualquiera** de la region de
///      capacidades.
///   3. La espera miraba el campo ID esperando que llegara a 0, cosa que no
///      pasa nunca: cincuenta mil lecturas para nada.
unsafe fn xecp_buscar(mmio: u64, hcc1: u32, id: u32) -> Option<u64> {
    let mut off = (((hcc1 >> 16) & 0xFFFF) as u64) * 4;
    if off == 0 {
        return None;
    }
    // Tope de vueltas: una lista enlazada leida de un aparato puede venir
    // circular o con basura, y colgar el arranque leyendo MMIO es peor que no
    // encontrar la capacidad.
    for _ in 0..64 {
        let cap = r32(mmio + off);
        if cap == 0 || cap == 0xFFFF_FFFF {
            return None;
        }
        if cap & 0xFF == id {
            return Some(mmio + off);
        }
        let next = ((cap >> 8) & 0xFF) as u64 * 4;
        if next == 0 {
            return None;
        }
        off += next;
    }
    None
}

/// **QUE EL FIRMWARE SUELTE EL CONTROLADOR, y que se calle.**
///
/// === Por que esto importa justo al REINICIAR desde Windows ===
///
/// En frio el xHC llega virgen. En un arranque en caliente no: el firmware --y
/// antes Windows-- lo han tocado, y el BIOS puede seguir declarandose dueno con
/// `USBLEGSUP.BIOS`. Mientras eso siga puesto, el SMM del firmware atiende
/// eventos del bus por debajo del sistema operativo, o sea que hay **dos
/// drivers** hablandole al mismo aparato.
///
/// Se pide la propiedad (bit 24), se espera a que el firmware suelte (bit 16), y
/// **se apagan sus SMI** -- si no, el firmware sigue entrando por interrupcion
/// de gestion aunque ya no sea el dueno. Los tres bits de estado se limpian
/// escribiendo un 1, que es como se limpian.
unsafe fn traspaso_del_firmware(mmio: u64, hcc1: u32) {
    let h = hal();
    let legsup = match xecp_buscar(mmio, hcc1, XECP_ID_LEGACY) {
        Some(a) => a,
        // No todos los controladores traen la capacidad. No tenerla es
        // correcto y no se avisa como si fuera un fallo.
        None => return,
    };
    let v = r32(legsup);
    w32(legsup, v | LEGSUP_OS);
    if v & LEGSUP_BIOS != 0 {
        if esperar(|| r32(legsup) & LEGSUP_BIOS == 0) {
            h.log("[xhci] el firmware SOLTO el controlador\n");
        } else {
            // No se aborta: hay firmwares que no bajan el bit nunca y aun asi
            // dejan trabajar. Pero se DICE, porque si el bus se comporta raro
            // esta linea es la primera sospechosa.
            h.log("[xhci] AVISO el firmware NO suelta (USBLEGSUP.BIOS sigue)\n");
        }
    }
    // Y que se calle: enables a cero, estados limpiados con un 1.
    let ctl = r32(legsup + 4);
    w32(legsup + 4, (ctl & !LEGCTL_SMI_ENABLES) | LEGCTL_SMI_STATUS);
}

pub unsafe fn init(mmio: u64) -> bool {
    if CTRL.is_some() { return true; }
    let h = hal();
    h.log("[xhci] === INIT ===\n");
    let cap_len = r32(mmio + CAPLENGTH as u64) & 0xFF;
    let hcs1 = r32(mmio + HCSPARAMS1 as u64);
    let hcc1 = r32(mmio + HCCPARAMS1 as u64);
    let op_base = cap_len;
    let rt_off = r32(mmio + RTSOFF as u64) & !0x1F;
    let db_off = r32(mmio + DBOFF as u64) & !0x1F;
    let max_slots = (hcs1 & 0xFF) as u8;
    let max_ports = ((hcs1 >> 24) & 0xFF) as u8;
    let csz = if hcc1 & (1 << 2) != 0 { 1u8 } else { 0u8 };
    h.log_u64(" max_slots=", max_slots as u64);
    h.log_u64(" max_ports=", max_ports as u64);

    traspaso_del_firmware(mmio, hcc1);

    // == EL RESET, Y AHORA CON TESTIGOS ==
    //
    // ** Las tres esperas eran `for _ in 0..N { if listo { break } }` **sin
    // mirar por que salieron**. Un bucle que se agota y uno que acierta
    // terminan en la misma linea, asi que un controlador que no llega a estar
    // listo se programaba igual -- y el xHCI spec dice que mientras `CNR` este
    // puesto **no se puede escribir ningun registro operacional** que no sea
    // USBSTS. O sea que el siguiente `op_w(CONFIG)` era comportamiento
    // indefinido, y el sintoma seria justo el que se ve: arranca bien en frio y
    // en caliente el teclado no aparece.
    let cmd = op_r(mmio, op_base, USBCMD);
    op_w(mmio, op_base, USBCMD, cmd & !USBCMD_RS);
    if !esperar(|| op_r(mmio, op_base, USBSTS) & USBSTS_HCH != 0) {
        h.log("[xhci] FAIL el controlador no PARA (USBSTS.HCH sigue a 0)\n");
        return false;
    }
    op_w(mmio, op_base, USBCMD, USBCMD_HCRST);
    if !esperar(|| op_r(mmio, op_base, USBCMD) & USBCMD_HCRST == 0) {
        h.log("[xhci] FAIL el reset no termina (USBCMD.HCRST sigue puesto)\n");
        return false;
    }
    if !esperar(|| op_r(mmio, op_base, USBSTS) & USBSTS_CNR == 0) {
        h.log("[xhci] FAIL sigue NO LISTO tras el reset (USBSTS.CNR)\n");
        return false;
    }
    op_w(mmio, op_base, CONFIG, (op_r(mmio, op_base, CONFIG) & !0xFF) | max_slots as u32);

    // DCBAA
    let dp = ((max_slots as usize + 1) * 8 + 4095) / 4096;
    let da = match h.alloc_dma_pages(dp) { Some(p) => p, None => { h.log("[xhci] FAIL dcbaa\n"); return false; } };
    core::ptr::write_bytes(h.phys_to_virt(da), 0, dp * 4096);
    let da2 = da & !0x3F;
    op_w(mmio, op_base, 0x30, (da2 & 0xFFFF_FFFF) as u32);
    op_w(mmio, op_base, 0x34, ((da2 >> 32) & 0xFFFF_FFFF) as u32);

    // Cmd ring
    let cp = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let ca = match h.alloc_dma_pages(cp) { Some(p) => p, None => { h.log("[xhci] FAIL cmd\n"); return false; } };
    let cv = h.phys_to_virt(ca) as *mut u32;
    core::ptr::write_bytes(cv as *mut u8, 0, cp * 4096);
    let cr_val = (ca & !0x3F) | 1;
    op_w(mmio, op_base, 0x18, (cr_val & 0xFFFF_FFFF) as u32);
    op_w(mmio, op_base, 0x1C, ((cr_val >> 32) & 0xFFFF_FFFF) as u32);

    // Event ring
    let ep = (RING_SIZE * TRB_SIZE + 4095) / 4096;
    let ea = match h.alloc_dma_pages(ep) { Some(p) => p, None => { h.log("[xhci] FAIL evt\n"); return false; } };
    let ev = h.phys_to_virt(ea) as *mut u32;
    core::ptr::write_bytes(ev as *mut u8, 0, ep * 4096);
    let eea = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] FAIL ERST\n"); return false; } };
    let eev = h.phys_to_virt(eea) as *mut u32;
    core::ptr::write_bytes(eev as *mut u8, 0, 4096);
    eev.add(0).write_volatile((ea & 0xFFFF_FFFF) as u32);
    eev.add(1).write_volatile(((ea >> 32) & 0xFFFF_FFFF) as u32);
    eev.add(2).write_volatile(RING_SIZE as u32);
    rt_w(mmio, rt_off, RT_ERSTSZ, 1);
    rt_w(mmio, rt_off, RT_ERSTBA, (eea & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_ERSTBA + 4, ((eea >> 32) & 0xFFFF_FFFF) as u32);
    let eo = (ea & !0x3F) as u64;
    rt_w(mmio, rt_off, RT_ERDP, (eo & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_ERDP + 4, ((eo >> 32) & 0xFFFF_FFFF) as u32);
    rt_w(mmio, rt_off, RT_IMAN, IMAN_IE);

    // Start
    op_w(mmio, op_base, USBCMD, op_r(mmio, op_base, USBCMD) | USBCMD_RS);
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_HCH == 0 { break; } }

    // Build ring wrappers
    let cmd_ring = TransferRing::new(cv, ca & !0x3F);
    // El event ring NO lleva Link TRB (el xHC lo recorre por tamano del ERST).
    let event_ring = TransferRing::new_unlinked(ev, eo);

    CTRL = Some(XhciController {
        mmio, op_base, rt_base: rt_off, db_base: db_off,
        max_slots, max_ports, ctx_size: csz,
        dcbaa_phys: da2,
        cmd_ring, event_ring,
        erst_phys: eo,
        initialized: true,
        evt_dequeue: 0, evt_cycle: 1,
    });
    h.log("[xhci] INIT DONE\n");
    true
}


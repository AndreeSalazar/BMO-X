//! xHCI USB Controller Driver — full device lifecycle + HID interrupt transfers.
//!
//! Modeled after the proven xhci-nostd crate at github.com/suhteevah/xhci-nostd.

#![no_std]
#![allow(static_mut_refs)]

use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

// ═══════════════════════════════════════════════════════════════════
//  HAL trait
// ═══════════════════════════════════════════════════════════════════

pub trait XhciHal {
    fn alloc_dma_pages(&self, count: usize) -> Option<u64>;
    fn phys_to_virt(&self, phys: u64) -> *mut u8;
    fn log(&self, msg: &str);
    fn log_u64(&self, msg: &str, val: u64);
    /// Espera de tiempo REAL en milisegundos. Los tiempos físicos del USB
    /// (debounce de conexión, reset de puerto, recovery) son del orden de
    /// decenas de ms — imposibles de cubrir con spin-counts sin depender de
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

// ═══════════════════════════════════════════════════════════════════
//  Registers
// ═══════════════════════════════════════════════════════════════════

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
/// `ERDP` bit 3 — **EHB, Event Handler Busy**. Lo pone el xHC al publicar un
/// evento y es *write-1-to-clear*: el software lo baja escribiéndole un 1 al
/// actualizar el ERDP.
///
/// ★ No bajarlo NO es un detalle cosmético: el xHC considera que el manejador
/// sigue ocupado, el anillo de eventos se llena, entra en **Event Ring Full**
/// y **deja de publicar eventos para siempre**. El síntoma en esta máquina era
/// exacto: aporrear el teclado mientras arranca (cuando todavía nadie drena el
/// anillo) lo llenaba, y a partir de ahí el teclado estaba muerto hasta
/// reiniciar. Sin aporrear, el anillo nunca se llenaba y no se notaba.
///
/// Como `erdp` va alineado a 16 bytes, el bit 3 salía siempre 0 — o sea que
/// se escribía "EHB sigue ocupado" en cada vuelta.
const ERDP_EHB: u64 = 1 << 3;
const PORTSC: u32 = 0x00;
const USBCMD_RS: u32 = 1 << 0; const USBCMD_HCRST: u32 = 1 << 1;
const USBSTS_HCH: u32 = 1 << 0; const USBSTS_CNR: u32 = 1 << 11;
const PORTSC_CCS: u32 = 1 << 0; const PORTSC_PED: u32 = 1 << 1;
const PORTSC_PR: u32 = 1 << 4; const PORTSC_PP: u32 = 1 << 9;
const PORTSC_CSC: u32 = 1 << 17; const PORTSC_PRC: u32 = 1 << 21;
const IMAN_IE: u32 = 1 << 1; #[allow(dead_code)] const IMAN_IP: u32 = 1 << 0;

const TRB_NORMAL: u32 = 1;  const TRB_SETUP: u32 = 2;
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

// ═══════════════════════════════════════════════════════════════════
//  TRB  (16 bytes, 4 DWORDs)
// ═══════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════
//  Transfer Ring
// ═══════════════════════════════════════════════════════════════════

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

    /// Anillo SIN Link TRB — para el **Event Ring**, que no lleva uno: el xHC
    /// conoce su tamaño por el ERST y da la vuelta solo. Escribirle un Link TRB
    /// (lo que hacía `new`) dejaba basura en la última entrada que el consumidor
    /// leía como si fuera un evento real en la primera vuelta.
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

// ═══════════════════════════════════════════════════════════════════
//  USB Descriptor types  (for external consumers)
// ═══════════════════════════════════════════════════════════════════

pub const USB_REQ_GET_DESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SET_CONFIG: u8 = 0x09;
pub const USB_DESC_DEVICE: u16 = 1;
pub const USB_DESC_CONFIG: u16 = 2;

// ═══════════════════════════════════════════════════════════════════
//  Controller
// ═══════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════
//  MMIO helpers
// ═══════════════════════════════════════════════════════════════════

unsafe fn r32(addr: u64) -> u32 { core::ptr::read_volatile(addr as *const u32) }
unsafe fn w32(addr: u64, val: u32) { core::ptr::write_volatile(addr as *mut u32, val); }
unsafe fn op_r(m: u64, o: u32, off: u32) -> u32 { r32(m + o as u64 + off as u64) }
unsafe fn op_w(m: u64, o: u32, off: u32, v: u32) { w32(m + o as u64 + off as u64, v) }
unsafe fn rt_w(m: u64, r: u32, off: u32, v: u32) { w32(m + r as u64 + off as u64, v) }

fn ctx_sz(c: &XhciController) -> usize { if c.ctx_size != 0 { 64 } else { 32 } }

// ═══════════════════════════════════════════════════════════════════
//  Event ring consumer
// ═══════════════════════════════════════════════════════════════════

/// Qué evento espera quien se bloquea.
///
/// **El anillo de eventos es UNO para todo el controlador.** Un teclado, un
/// ratón, una compleción de comando y un cambio de puerto salen todos por el
/// mismo sitio, en el orden en que el xHC los postea. Por eso quien espera
/// tiene que decir QUÉ espera: coger el primero que pase es coger el de otro.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Espera {
    /// Una compleción de comando. El anillo de comandos se usa de uno en uno,
    /// así que no hace falta distinguir cuál.
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

// ── El aparcadero de eventos ────────────────────────────────────────
//
// ★ ESTA ES LA PIEZA QUE FALTABA, y costó el teclado entero.
//
// Antes, esperar bloqueando sacaba del anillo **el primer evento de cualquier
// tipo** y lo daba por bueno; los tres bucles que sí miraban el tipo
// (`send_cmd`, `control_transfer`) **descartaban** lo que no era suyo, y
// `address_device` y `configure_endpoint` ni siquiera miraban el tipo: leían
// el `cc` de lo que hubiera. Como un Transfer Event correcto también trae
// `cc=1`, un informe del ratón se leía como "el comando salió bien".
//
// Mientras nada estuviera bombeando durante la enumeración, no se notaba. En
// cuanto un endpoint de interrupción quedó vivo **mientras se enumeraba el
// siguiente puerto**, cada control transfer del teclado se comía un informe
// del ratón: el evento desaparecía, `uhid::poll` no lo veía nunca, y sin ese
// evento **nadie vuelve a encolar la transferencia**. La bomba no arranca, el
// endpoint se queda en `Running` para siempre y el aparato enmudece sin un
// solo error. Los dos, ratón y teclado, por el mismo camino.
//
// La regla ahora: **un evento que no es mío se APARCA, jamás se tira.**
const APARCADOS_MAX: usize = 64;
static mut APARCADOS: [(u32, u32, u32, u32); APARCADOS_MAX] = [(0, 0, 0, 0); APARCADOS_MAX];
static mut APARCADOS_N: usize = 0;
/// Cuántos se han aparcado en total y cuántos se han PERDIDO por aparcadero
/// lleno. Lo segundo tiene que ser cero; si un día no lo es, hay que subir el
/// tope — y se verá, que es justo lo que no pasaba antes.
static mut APARCADOS_TOTAL: u32 = 0;
static mut APARCADOS_PERDIDOS: u32 = 0;

/// `(aparcados en total, perdidos por lleno, aparcados ahora mismo)`.
pub fn evt_park_stats() -> (u32, u32, u32) {
    unsafe { (APARCADOS_TOTAL, APARCADOS_PERDIDOS, APARCADOS_N as u32) }
}

unsafe fn aparcar(ev: (u32, u32, u32, u32)) {
    if APARCADOS_N >= APARCADOS_MAX {
        // Perder uno aquí vuelve a matar un endpoint. Se cuenta para que se
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
/// busca: una compleción de comando aparcada mientras un control transfer
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

/// El más viejo del aparcadero, sea de quien sea. Lo drena el bucle de sondeo.
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

/// Espera **lo que se le pide**, aparcando todo lo demás.
///
/// Devuelve `None` sólo si se agota el plazo sin que llegue: eso sí es un
/// fallo del controlador, y ahora se distingue de "llegó lo de otro".
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
        // ★ La mitad ALTA primero, la baja después. La baja es la que lleva el
        // EHB y la que el xHC toma como "ya está": escribir primero la baja
        // dejaría, durante unos ciclos, una dirección con la mitad nueva y la
        // mitad vieja. Aquí casi nunca cambia la alta, pero el orden correcto
        // no cuesta nada y el incorrecto sólo falla el día que el anillo cruce
        // una frontera de 4 GiB.
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64 + 4, ((erdp >> 32) & 0xFFFF_FFFF) as u32);
        // Y con EHB puesto, que es lo que lo BAJA (RW1C). Ver `ERDP_EHB`: sin
        // esto el anillo se llena y el xHC deja de publicar eventos.
        w32(ctrl.mmio + ctrl.rt_base as u64 + RT_ERDP as u64, ((erdp | ERDP_EHB) & 0xFFFF_FFFF) as u32);
        ctrl.evt_dequeue = ndq; ctrl.evt_cycle = ncy;
        Some((dw0, dw1, dw2, dw3))
    } else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Helpers
// ═══════════════════════════════════════════════════════════════════

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
    // El bucle que había aquí descartaba en silencio todo lo que no fuera una
    // compleción. Ahora la selección la hace `evt_poll_block`, que además lo
    // aparca en vez de tirarlo.
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    let cc = (ev.2 >> 24) & 0xFF;
    if cc == CC_SUCCESS || cc == CC_SHORT { return Some(ev); }
    None
}

// ═══════════════════════════════════════════════════════════════════
//  Init  (unchanged logic, proven)
// ═══════════════════════════════════════════════════════════════════

/// Reset the global controller state so init() can be called again
/// (needed when switching between CPU SoC and chipset XHCI controllers).
pub fn reset_ctrl() {
    unsafe { CTRL = None; }
    MMIO_BASE.store(0, core::sync::atomic::Ordering::SeqCst);
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

    let eecp = ((hcc1 >> 8) & 0xFF) as u32;
    if eecp >= 0x40 && (r32(mmio + eecp as u64) & 1) != 0 {
        w32(mmio + eecp as u64 + 4, 1);
        for _ in 0..50000 { if r32(mmio + eecp as u64) & 1 == 0 { break; } }
    }
    let cmd = op_r(mmio, op_base, USBCMD);
    op_w(mmio, op_base, USBCMD, cmd & !USBCMD_RS);
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_HCH != 0 { break; } }
    op_w(mmio, op_base, USBCMD, USBCMD_HCRST);
    for _ in 0..100000 { if op_r(mmio, op_base, USBCMD) & USBCMD_HCRST == 0 { break; } }
    for _ in 0..50000 { if op_r(mmio, op_base, USBSTS) & USBSTS_CNR == 0 { break; } }
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
    // El event ring NO lleva Link TRB (el xHC lo recorre por tamaño del ERST).
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

// ═══════════════════════════════════════════════════════════════════
//  Port ops
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn port_speed(port: u8) -> u8 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    ((r32(c.mmio + pb + PORTSC as u64) >> 10) & 0x0F) as u8
}

pub unsafe fn port_peek(port: u8) -> u32 {
    let c = match CTRL.as_ref() { Some(c) => c, None => return 0 };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    r32(c.mmio + pb + PORTSC as u64)
}

/// Enciende la corriente del puerto **y espera** la estabilización de VBUS.
/// Para encender UNO. Quien encienda varios debe usar [`port_power_solo`] y
/// esperar una sola vez al final — ver ahí por qué.
pub unsafe fn port_power_on(port: u8) {
    port_power_solo(port);
    // Spec: >=20 ms de estabilización de VBUS antes de confiar en CCS.
    hal().delay_ms(20);
}

/// Enciende la corriente y **no espera**.
///
/// ★ La espera de VBUS es un tiempo FÍSICO del puerto, y los puertos se
/// estabilizan **en paralelo**: encender ocho y esperar 20 ms una vez es tan
/// correcto como esperar 20 ms ocho veces, y tarda 160 ms menos. Con dos
/// controladores en esta placa, eso es un tercio de segundo de arranque que no
/// compraba nada.
pub unsafe fn port_power_solo(port: u8) {
    let c = match CTRL.as_mut() { Some(c) => c, None => return };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    w32(c.mmio + pb + PORTSC as u64, r32(c.mmio + pb + PORTSC as u64) | PORTSC_PP);
}

/// Reset del puerto con TIEMPOS REALES. Un reset USB2 tarda ~10-50 ms; el
/// firmware/PHY latchea PED sólo cuando termina. Poll a 1 ms, hasta 120 ms.
pub unsafe fn port_reset(port: u8) -> bool {
    let c = match CTRL.as_mut() { Some(c) => c, None => return false };
    let pb = c.op_base as u64 + 0x400 + port as u64 * 0x10;
    let sc = r32(c.mmio + pb + PORTSC as u64);
    if sc & PORTSC_CCS == 0 { return false; }
    // Escribir PR preservando bits RW1C (no re-limpiar cambios por error):
    // sólo PP + PR, el resto a 0 (los bits de estado son RO/RW1C).
    w32(c.mmio + pb + PORTSC as u64, (sc & PORTSC_PP) | PORTSC_PR);
    for _ in 0..120 {
        hal().delay_ms(1);
        let s = r32(c.mmio + pb + PORTSC as u64);
        // Reset completo cuando PR se auto-limpia. Reconocer PRC.
        if s & PORTSC_PR == 0 {
            if s & PORTSC_PRC != 0 {
                w32(c.mmio + pb + PORTSC as u64, (s & PORTSC_PP) | PORTSC_PRC);
            }
            // Recovery post-reset (spec: 10 ms) y comprobar habilitación.
            hal().delay_ms(10);
            let e = r32(c.mmio + pb + PORTSC as u64);
            return e & PORTSC_PED != 0;
        }
    }
    false
}

// ═══════════════════════════════════════════════════════════════════
//  Enable Slot
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn enable_slot() -> Option<u8> {
    hal().log("[xhci] enable_slot\n");
    let ev = send_cmd(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: TRB_ENABLE << 10 })?;
    let cc = (ev.2 >> 24) & 0xFF;
    let slot = ((ev.3 >> 24) & 0xFF) as u8;
    hal().log_u64(" cc=", cc as u64);
    hal().log_u64(" slot=", slot as u64);
    if cc != CC_SUCCESS || slot == 0 { None } else { Some(slot) }
}

/// **Devolver un slot al controlador.** La pareja de `enable_slot`, y sin ella
/// el controlador se queda sin slots.
///
/// ★ Esto faltaba, y se vio en la primera foto: los slots subían `0x30`,
/// `0x31`, … `0x40` en el registro de arranque. Cada intento de adopción que
/// no acababa en un aparato instalado se llevaba un slot **para siempre**;
/// al llegar a los 64 que declara este xHC, el `Address Device` empezó a
/// contestar `cc=0x9` — *No Slots Available* — y a partir de ahí no se pudo
/// enumerar nada más en toda la sesión.
///
/// Un recurso que se pide en un camino que puede fallar necesita su
/// devolución **en el mismo sitio**, no en el camino feliz.
///
/// Lo que NO devuelve: las páginas DMA del anillo EP0 y del contexto de
/// dispositivo. El HAL no tiene `free_dma_pages` todavía, así que eso sigue
/// siendo una fuga — acotada, porque ahora los intentos están contados.
pub unsafe fn disable_slot(slot: u8) -> bool {
    if slot == 0 { return false; }
    let ok = send_cmd(Trb {
        dw0: 0, dw1: 0, dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_DISABLE << 10),
    })
    .is_some();
    hal().log_u64("[xhci] disable_slot ", slot as u64);
    hal().log(if ok { " ok\n" } else { " FALLO\n" });
    // El puntero del contexto de dispositivo se retira SIEMPRE, salga bien el
    // comando o no: dejarlo puesto apuntando a un slot que el xHC ya no cree
    // suyo es peor que retirarlo de más.
    if let Some(c) = CTRL.as_ref() {
        let dcbaa = hal().phys_to_virt(c.dcbaa_phys) as *mut u64;
        dcbaa.add(slot as usize).write_volatile(0);
    }
    if (slot as usize) < MAX_SLOTS {
        EP0_RINGS[slot as usize].valid = false;
    }
    ok
}

// ═══════════════════════════════════════════════════════════════════
//  Per-slot EP0 ring storage
// ═══════════════════════════════════════════════════════════════════

const MAX_SLOTS: usize = 255;
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Ep0Info { valid: bool, ring_phys: u64, ring_virt: *mut u32, pcs: bool, enqueue: usize }
static mut EP0_RINGS: [Ep0Info; MAX_SLOTS] = [Ep0Info {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: true, enqueue: 0
}; MAX_SLOTS];
unsafe fn ep0_reg(slot: u8, phys: u64, virt: *mut u32) {
    EP0_RINGS[slot as usize] = Ep0Info { valid: true, ring_phys: phys, ring_virt: virt, pcs: true, enqueue: 0 };
}
fn ep0_mut(slot: u8) -> Option<&'static mut Ep0Info> {
    if (slot as usize) < MAX_SLOTS { unsafe { let p = &mut EP0_RINGS[slot as usize]; if p.valid { Some(p) } else { None } } }
    else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Address Device
// ═══════════════════════════════════════════════════════════════════

/// Pide un slot y direcciona el aparato del puerto.
///
/// ★ **Si algo falla después de tener el slot, el slot se DEVUELVE.** Antes se
/// salía por cinco sitios distintos con un `?` o un `return None` y el slot se
/// quedaba pedido para siempre; el bucle de adopción del arranque los fue
/// gastando de uno en uno hasta agotar los 64 del controlador. La pareja
/// pedir/devolver tiene que estar en la misma función o no está.
pub unsafe fn address_device(port: u8, speed: u8) -> Option<u8> {
    let slot = enable_slot()?;
    match direccionar_en_slot(port, speed, slot) {
        Some(s) => Some(s),
        None => {
            disable_slot(slot);
            None
        }
    }
}

unsafe fn direccionar_en_slot(port: u8, speed: u8, slot: u8) -> Option<u8> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let ep0_phys = h.alloc_dma_pages(1)?;
    let ep0_virt = h.phys_to_virt(ep0_phys) as *mut u32;
    core::ptr::write_bytes(ep0_virt as *mut u8, 0, 4096);
    let mut ring = TransferRing::new(ep0_virt, ep0_phys);
    // El productor (control_transfer) alterna su cycle state al dar la
    // vuelta — el Link TRB necesita Toggle Cycle para que el xHC haga lo
    // mismo, o el anillo se desincroniza tras el primer wrap.
    ring.enable_toggle_cycle();
    ep0_reg(slot, ep0_phys & !0xF, ep0_virt);

    let in_phys = h.alloc_dma_pages(1)?;
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);
    let dev_phys = h.alloc_dma_pages(1)?;
    let dev_virt = h.phys_to_virt(dev_phys) as *mut u8;
    core::ptr::write_bytes(dev_virt, 0, 4096);

    // Input Control Context
    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile(3); // Add Slot+EP0

    // Slot Context
    let sc = in_virt.add(cs) as *mut u32;
    sc.add(0).write_volatile(((speed as u32) & 0xF) << 20 | (1 << 27));
    sc.add(1).write_volatile((port as u32 + 1) << 16);

    let mps: u32 = match speed { 1|2 => 8, 3 => 64, 4|5 => 512, _ => 8 };
    let dq = (ep0_phys & !0xF) | 1;

    // EP0 Context
    let ep0 = in_virt.add(2 * cs) as *mut u32;
    ep0.add(0).write_volatile(0);
    ep0.add(1).write_volatile((mps << 16) | (4 << 3) | (3 << 1));
    ep0.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep0.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    ep0.add(4).write_volatile(8);

    // DCBAA[slot]
    let dcbaa = h.phys_to_virt(ctrl.dcbaa_phys) as *mut u64;
    dcbaa.add(slot as usize).write_volatile(dev_phys & !0x3F);

    // Address Device TRB
    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_ADDRESS_DEV << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    // ★ Esto tomaba el primer evento SIN MIRAR EL TIPO y le leía el `cc`. Un
    // Transfer Event correcto también trae `cc=1`, así que un informe del
    // ratón se leía como "el Address Device salió bien" — y de paso ese
    // informe desaparecía.
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    let cc = (ev.2 >> 24) & 0xFF;
    h.log_u64(" addr_dev cc=", cc as u64);
    if cc != CC_SUCCESS { return None; }

    // Write EP0 dequeue into Device Context EP0 for future doorbell reloads
    let d_ep0 = dev_virt.add(cs) as *mut u32;
    d_ep0.add(2).write_volatile((ep0_phys & !0xF) as u32 | 1);
    d_ep0.add(3).write_volatile(((ep0_phys >> 32) & 0xFFFF_FFFF) as u32);

    Some(slot)
}

// ═══════════════════════════════════════════════════════════════════
//  Control Transfer — uses per-slot EP0 ring
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn control_transfer(slot: u8, bm_req_type: u8, b_request: u8,
    w_value: u16, w_index: u16, buf: &mut [u8], data_in: bool) -> usize
{
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return 0 };
    let h = hal();
    let ep0 = match ep0_mut(slot) { Some(e) => e, None => { h.log("no ep0 ring\n"); return 0; } };

    let has_data = !buf.is_empty();
    let data_page = if has_data {
        // Quedarse sin páginas DMA se trataba igual que "el aparato no mandó
        // nada": devolver 0. Dos causas opuestas —una es memoria del sistema,
        // la otra es el periférico— con la misma cara, y sin una línea. La de
        // arriba (`no ep0 ring`) sí gritaba; ésta no. Ahora las dos.
        let dp = h.alloc_dma_pages(1).unwrap_or(0);
        if dp == 0 {
            h.log("[xhci] control_transfer: SIN PAGINAS DMA (no es el aparato, es la memoria)\n");
            return 0;
        }
        if !data_in {
            let dv = h.phys_to_virt(dp);
            for i in 0..buf.len() { dv.add(i).write_volatile(buf[i]); }
        }
        dp
    } else { 0 };

    let trt = if !has_data { 0u32 } else if data_in { 3u32 } else { 2u32 };

    // Setup Stage. Spec 4.11.2.2: Setup/Data/Status son TDs SEPARADOS —
    // CH=0 en cada stage. Encadenarlos (CH=1) lo tolera QEMU pero el
    // silicio real (AMD) responde con Transaction Error (cc=4).
    let setup = Trb {
        dw0: (bm_req_type as u32) | ((b_request as u32) << 8) | ((w_value as u32) << 16),
        dw1: (w_index as u32) | ((buf.len() as u32) << 16),
        dw2: 8,
        dw3: (TRB_SETUP << 10) | (1 << 6) | (trt << 16), // IDT; sin CH
    };
    let s_idx = ep0.enqueue;
    let sb = s_idx * 4;
    ep0.ring_virt.add(sb).write_volatile(setup.dw0);
    ep0.ring_virt.add(sb + 1).write_volatile(setup.dw1);
    ep0.ring_virt.add(sb + 2).write_volatile(setup.dw2);
    ep0.ring_virt.add(sb + 3).write_volatile(setup.dw3 | if ep0.pcs { 1 } else { 0 });
    ep0.enqueue = s_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Data Stage
    if has_data {
        let d_idx = ep0.enqueue;
        let db = d_idx * 4;
        ep0.ring_virt.add(db).write_volatile((data_page & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 1).write_volatile(((data_page >> 32) & 0xFFFF_FFFF) as u32);
        ep0.ring_virt.add(db + 2).write_volatile(buf.len() as u32 & 0x1FFFF);
        let dir = if data_in { 1u32 << 16 } else { 0 };
        // Data Stage de un solo TRB = TD propio: sin CH (ver nota del Setup).
        ep0.ring_virt.add(db + 3).write_volatile(
            (TRB_DATA << 10) | dir | if ep0.pcs { 1 } else { 0 });
        ep0.enqueue = d_idx + 1;
        if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }
    }

    // Status Stage
    let st_idx = ep0.enqueue;
    let stb = st_idx * 4;
    ep0.ring_virt.add(stb).write_volatile(0);
    ep0.ring_virt.add(stb + 1).write_volatile(0);
    ep0.ring_virt.add(stb + 2).write_volatile(0);
    let dir_in = if has_data { !data_in } else { true };
    ep0.ring_virt.add(stb + 3).write_volatile(
        (TRB_STATUS << 10) | (if dir_in { 1u32 << 16 } else { 0 }) | (1 << 5)
        | if ep0.pcs { 1 } else { 0 }
    );
    ep0.enqueue = st_idx + 1;
    if ep0.enqueue >= LAST_TRB_IDX { ep0.enqueue = 0; ep0.pcs = !ep0.pcs; }

    // Ring EP0 doorbell
    ring_doorbell(slot, 1);

    // Espera el Transfer Event de ESTE EP0 y de nadie más.
    //
    // El bucle de antes descartaba todo lo que no fuera suyo — incluidos los
    // informes de interrupción de un ratón ya enumerado, que es exactamente el
    // camino por el que el teclado y el ratón se quedaban mudos los dos.
    let ev = match evt_poll_block(ctrl, Espera::Transferencia { slot, ep: 1 }) {
        Some(e) => e,
        None => return 0,
    };
    let dw2 = ev.2;
    let cc = (dw2 >> 24) & 0xFF;
    if cc != CC_SUCCESS && cc != CC_SHORT {
        h.log_u64(" ctrl_xfer cc=", cc as u64);
        return 0;
    }
    let rem = dw2 & 0xFFFFFF;
    let xfer = buf.len().saturating_sub(rem as usize);
    if data_in && has_data && data_page != 0 {
        let dv = h.phys_to_virt(data_page);
        for i in 0..xfer.min(buf.len()) { buf[i] = dv.add(i).read_volatile(); }
    }
    xfer
}

// ═══════════════════════════════════════════════════════════════════
//  Descriptor helpers
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn get_device_descriptor(slot: u8, buf: &mut [u8]) -> usize {
    let len = if buf.len() > 18 { 18 } else { buf.len() };
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, USB_DESC_DEVICE << 8, 0, &mut buf[..len], true)
}

pub unsafe fn get_config_descriptor(slot: u8, index: u8, buf: &mut [u8]) -> usize {
    control_transfer(slot, 0x80, USB_REQ_GET_DESCRIPTOR, (USB_DESC_CONFIG << 8) | index as u16, 0, buf, true)
}

// ═══════════════════════════════════════════════════════════════════
//  Per-endpoint transfer ring storage (for non-EP0 endpoints)
// ═══════════════════════════════════════════════════════════════════

const MAX_DCI: usize = 32;
#[derive(Clone, Copy)]
#[allow(dead_code)]
struct EpRing { valid: bool, ring_phys: u64, ring_virt: *mut u32, pcs: bool, enqueue: usize }
static mut EP_RINGS: [[EpRing; MAX_DCI]; MAX_SLOTS] = [[EpRing {
    valid: false, ring_phys: 0, ring_virt: core::ptr::null_mut(), pcs: true, enqueue: 0
}; MAX_DCI]; MAX_SLOTS];
fn ep_ring_mut(slot: u8, dci: u8) -> Option<&'static mut EpRing> {
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        unsafe { let p = &mut EP_RINGS[slot as usize][dci as usize]; if p.valid { Some(p) } else { None } }
    } else { None }
}

// ═══════════════════════════════════════════════════════════════════
//  Configure Endpoint
// ═══════════════════════════════════════════════════════════════════

/// Convierte el `bInterval` del descriptor de endpoint al campo **Interval**
/// del Endpoint Context.
///
/// ★ EL BUG QUE MATABA AL TECLADO: ese campo NO es lineal. El xHC sirve el
/// endpoint cada `2^Interval × 125 µs`, o sea es un EXPONENTE. Escribíamos el
/// `bInterval` crudo, que en Low/Full Speed viene en MILISEGUNDOS (10, 24,
/// 32...). Un teclado que pide 24 ms terminaba programado como 2^24 × 125 µs =
/// **35 minutos** entre sondeos; con 32, 149 horas. El endpoint queda
/// "configurado" y el Configure Endpoint devuelve éxito — el xHC sencillamente
/// no lo consulta jamás. Se ve idéntico a un driver muerto.
///
/// Reglas (xHCI 6.2.3.6):
///   - Low/Full Speed interrupt: `bInterval` en FRAMES de 1 ms (1..255) →
///     `Interval = 3 + floor(log2(bInterval))` (125 µs × 2^3 = 1 ms).
///   - High/Super Speed: `bInterval` YA es un exponente (1..16) →
///     `Interval = bInterval - 1`.
/// El campo tiene 4 bits útiles: se acota a 0..15.
pub fn encode_interval(speed: u8, b_interval: u8) -> u8 {
    match speed {
        1 | 2 => {
            // Full (1) / Low (2): milisegundos → exponente de 125 µs.
            let ms = if b_interval == 0 { 1u32 } else { b_interval as u32 };
            let mut e = 0u32;
            while (1u32 << (e + 1)) <= ms { e += 1; } // floor(log2(ms))
            let v = 3 + e;
            if v > 15 { 15 } else { v as u8 }
        }
        _ => {
            // High (3) / Super (4+): ya viene como exponente.
            let b = if b_interval == 0 { 1u8 } else { b_interval };
            let v = b - 1;
            if v > 15 { 15 } else { v }
        }
    }
}

// Diagnóstico del último endpoint configurado: qué pidió el descriptor y qué
// programamos de verdad. Con esto CABINA puede decir "pediste 24 ms, programé
// exponente 7 (16 ms)" en vez de dejarnos adivinando.
static mut LAST_EP_BINTERVAL: u8 = 0;
static mut LAST_EP_INTERVAL: u8 = 0;
static mut LAST_EP_SPEED: u8 = 0;

/// `(bInterval_del_descriptor, Interval_programado, speed_del_slot)`.
pub fn last_ep_timing() -> (u8, u8, u8) {
    unsafe { (LAST_EP_BINTERVAL, LAST_EP_INTERVAL, LAST_EP_SPEED) }
}

/// Estado del endpoint leído del **Device Context** (el que mantiene el xHC,
/// no el que le mandamos): 0=Disabled 1=Running 2=Halted 3=Stopped 4=Error.
/// Si tras configurar no está en Running, el endpoint no está agendado y
/// ninguna cantidad de doorbells lo va a despertar.
pub unsafe fn ep_state(slot: u8, dci: u8) -> u8 {
    let ctrl = match CTRL.as_ref() { Some(c) => c, None => return 0xFF };
    let cs = ctx_sz(ctrl);
    let dev_phys = match dcbaa_get(slot) { Some(p) => p, None => return 0xFF };
    let dev_virt = hal().phys_to_virt(dev_phys) as *const u32;
    let ep = dev_virt.add((dci as usize) * cs / 4);
    (ep.read_volatile() & 0x7) as u8
}

/// USBSTS crudo. Bit 2 = HSE (Host System Error, típicamente un DMA a memoria
/// que el xHC no puede tocar) y bit 12 = HCE (Host Controller Error). Si
/// alguno está encendido el controlador está muerto y todo lo demás es ruido.
pub unsafe fn usbsts() -> u32 {
    match CTRL.as_ref() { Some(c) => op_r(c.mmio, c.op_base, USBSTS), None => 0 }
}

pub unsafe fn configure_endpoint(slot: u8, dci: u8, ep_type: u8, max_pkt: u16, interval: u8) -> bool {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return false };
    let h = hal();
    let cs = ctx_sz(ctrl);

    let in_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no mem\n"); return false; } };
    let in_virt = h.phys_to_virt(in_phys) as *mut u8;
    core::ptr::write_bytes(in_virt, 0, 4096);

    let in32 = in_virt as *mut u32;
    in32.add(0).write_volatile(0); // Drop
    in32.add(1).write_volatile((1u32 << dci) | 1); // Add Slot(0) + EP(dci)

    // Slot Context: preserve the controller-populated route/speed/root-port
    // fields from the output Device Context; only raise Context Entries.
    let sc = in_virt.add(cs) as *mut u32;
    let dev_phys = match dcbaa_get(slot) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no dev ctx\n"); return false; } };
    let dev_virt = h.phys_to_virt(dev_phys) as *const u32;
    let slot_dwords = cs / 4;
    for i in 0..slot_dwords {
        sc.add(i).write_volatile(dev_virt.add(i).read_volatile());
    }
    let old_dw0 = sc.add(0).read_volatile();
    let old_entries = (old_dw0 >> 27) & 0x1F;
    let new_entries = old_entries.max(dci as u32);
    sc.add(0).write_volatile((old_dw0 & !(0x1F << 27)) | (new_entries << 27));

    // Allocate transfer ring for this endpoint
    let tr_phys = match h.alloc_dma_pages(1) { Some(p) => p, None => { h.log("[xhci] cfg_ep: no ring\n"); return false; } };
    let tr_virt = h.phys_to_virt(tr_phys) as *mut u32;
    core::ptr::write_bytes(tr_virt as *mut u8, 0, 4096);
    // El Link TRB del final del anillo necesita **Toggle Cycle**: sin él, al dar
    // la primera vuelta (255 reportes) el productor invierte su PCS pero el
    // consumidor no, el ciclo deja de coincidir y el endpoint se congela para
    // siempre. Una bomba de tiempo a ~255 pulsaciones de tecla.
    let mut tr = TransferRing::new(tr_virt, tr_phys);
    tr.enable_toggle_cycle();
    if (slot as usize) < MAX_SLOTS && (dci as usize) < MAX_DCI {
        EP_RINGS[slot as usize][dci as usize] = EpRing {
            valid: true, ring_phys: tr_phys & !0xF, ring_virt: tr_virt, pcs: true, enqueue: 0,
        };
    }

    let dq = (tr_phys & !0xF) | 1;
    let ep = in_virt.add((dci as usize + 1) * cs) as *mut u32;
    // La velocidad la sabe el propio Slot Context que acabamos de copiar del
    // Device Context (bits 23:20) — no hace falta que el caller la adivine ni
    // arrastrarla por media pila de llamadas: se la preguntamos al hardware.
    let speed = ((old_dw0 >> 20) & 0xF) as u8;
    let enc = encode_interval(speed, interval);
    LAST_EP_BINTERVAL = interval;
    LAST_EP_INTERVAL = enc;
    LAST_EP_SPEED = speed;
    ep.add(0).write_volatile((enc as u32) << 16); // DW0: Interval in bits 23:16
    ep.add(1).write_volatile(
        ((max_pkt as u32) << 16) | ((ep_type as u32) << 3) | (3 << 1)
    );
    ep.add(2).write_volatile((dq & 0xFFFF_FFFF) as u32);
    ep.add(3).write_volatile(((dq >> 32) & 0xFFFF_FFFF) as u32);
    // DW4: Max ESIT Payload Lo (bits 31:16) | Average TRB Length (bits 15:0).
    // ★ EL BUG DEL TECLADO: sin Max ESIT Payload, el xHC asigna CERO ancho de
    // banda periodico al endpoint de INTERRUPCION -> nunca lo sirve -> las
    // teclas jamas completan (tev pegado, kev=0). Para un teclado boot el
    // payload por intervalo = max_pkt (8 bytes). Con esto el DCI del teclado
    // deberia empezar a postear Transfer Events al presionar teclas.
    let max_esit = max_pkt as u32; // interrupt LS/FS/HS boot: 1 paquete por ESIT
    ep.add(4).write_volatile((max_esit << 16) | 8);

    let trb = Trb {
        dw0: (in_phys & 0xFFFF_FFFF) as u32,
        dw1: ((in_phys >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0,
        dw3: ((slot as u32) << 24) | (TRB_CONFIGURE << 10),
    };
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    // Igual que en `address_device`: esto tomaba el primer evento sin mirar el
    // tipo, así que podía dar por configurado un endpoint leyendo el `cc` del
    // informe de otro aparato.
    let ev = evt_poll_block(ctrl, Espera::Comando);
    match ev {
        Some((_, _, dw2, _)) => {
            let cc = (dw2 >> 24) & 0xFF;
            if cc != CC_SUCCESS {
                // El CÓDIGO, no un "FAIL" mudo. Los que importan aquí:
                // 4=Transaction Error, 8=Bandwidth Error (el intervalo pedido
                // no cabe en la agenda periódica), 11=Trb Error,
                // 17=Parameter Error (algún campo del contexto no vale).
                h.log_u64("[xhci] cfg_ep cc=", cc as u64);
                h.log("\n");
            }
            cc == CC_SUCCESS
        }
        None => {
            h.log("[xhci] cfg_ep sin respuesta del controlador\n");
            false
        }
    }
}

// ═══════════════════════════════════════════════════════════════════
//  RESUCITAR UN ENDPOINT PARADO
// ═══════════════════════════════════════════════════════════════════
//
// ★ EL AGUJERO QUE ESTO TAPA: el driver sabía VER que un endpoint estaba
// Halted (`ep_state` lo documenta desde hace tiempo) y no tenía con qué
// levantarlo. Los dos comandos que hacen falta —Reset Endpoint (14) y Set TR
// Dequeue Pointer (16)— sencillamente no estaban escritos.
//
// El síntoma es el que contó el dueño: **el teclado deja de responder al
// pulsar, sin que nadie lo desenchufe**. Un error de transacción del bus
// —cable, ruido, un paquete que llega mal— deja el endpoint parado, y a partir
// de ahí `rearmar()` encola y toca el timbre para nada: **el xHC ignora el
// doorbell de un endpoint Halted**. Se veía idéntico a un aparato desconectado.
//
// La secuencia es de la spec (xHCI 4.6.8) y el ORDEN no es negociable:
//
//   1. Reset Endpoint      Halted → Stopped. Sin esto, lo demás no vale.
//   2. Set TR Dequeue      decirle POR DÓNDE seguir. El endpoint parado dejó
//                          el puntero a mitad del anillo; si no se recoloca,
//                          al arrancar lee TRBs viejos con el ciclo cambiado.
//   3. ← el llamante encola y toca el timbre (`rearmar`)
//
// El paso 2 es el que se olvida y el que hace que "el reset no sirviera de
// nada": resetear sin recolocar deja el endpoint listo para leer basura.

/// Los `cc` de un Transfer Event que dejan el endpoint **parado**, y por tanto
/// exigen recuperarlo en vez de reintentar.
///
/// `3` Babble (el aparato mandó más de lo que cabía), `4` USB Transaction Error
/// (el bus falló), `6` Stall (el aparato dijo que no). Cualquier otro `cc` malo
/// es informativo: molesta, pero el endpoint sigue agendado.
pub fn cc_halta_endpoint(cc: u8) -> bool { matches!(cc, 3 | 4 | 6) }

static mut RECUPERACIONES: u32 = 0;
static mut RECUPERACIONES_FALLIDAS: u32 = 0;

/// `(endpoints resucitados, intentos que no salieron)`.
///
/// El segundo número es el que hay que mirar: si sube, el aparato no vuelve con
/// un reset y el problema está más abajo (el puerto o el propio cable).
pub fn recuperaciones() -> (u32, u32) {
    unsafe { (RECUPERACIONES, RECUPERACIONES_FALLIDAS) }
}

/// Manda un comando y devuelve su `cc` — a diferencia de `send_cmd`, que
/// convierte cualquier fallo en `None` y se lleva el número por delante. Aquí
/// el número ES el diagnóstico: `19` (Context State Error) significa "el
/// endpoint no estaba en el estado que este comando espera", que es distinto de
/// "el controlador no contestó".
unsafe fn cmd_cc(trb: Trb) -> Option<u32> {
    let ctrl = CTRL.as_mut()?;
    ctrl.cmd_ring.enqueue(&trb);
    ring_doorbell(0, 0);
    let ev = evt_poll_block(ctrl, Espera::Comando)?;
    Some((ev.2 >> 24) & 0xFF)
}

/// Levanta un endpoint parado y lo deja listo para que el llamante encole.
///
/// Devuelve `true` si el endpoint quedó en condiciones de volver a bombear. **No
/// encola ni toca el timbre**: eso es trabajo del dueño del endpoint, que es
/// quien sabe qué buffer y qué largo le tocan.
///
/// ⚠️ Bloquea, porque espera la compleción de dos comandos. Es aceptable por lo
/// mismo que la adopción en caliente: ocurre **sólo cuando algo ya ha fallado**,
/// no en el camino normal. El día que haya un hilo de kernel para el bus, esto
/// se muda ahí con lo demás.
pub unsafe fn recuperar_endpoint(slot: u8, dci: u8) -> bool {
    let h = hal();

    // El estado lo dice el xHC, no nosotros. Si ya está Running no hay nada que
    // resetear y hacerlo daría Context State Error: un `cc=19` en el log que
    // parecería un fallo cuando en realidad no había avería.
    let estado = ep_state(slot, dci);
    if estado == 1 {
        return true;
    }
    if estado == 0 || estado == 0xFF {
        // Disabled: el endpoint no está configurado. Un reset no lo arregla —
        // esto es re-enumerar, y no se decide aquí.
        h.log_u64("[xhci] recuperar: endpoint sin configurar, dci=", dci as u64);
        h.log("\n");
        RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
        return false;
    }

    let campos = ((slot as u32) << 24) | ((dci as u32) << 16);

    // ── 1. Reset Endpoint: Halted → Stopped ──────────────────────────
    //
    // El bit TSP (9) se deja a 0 a propósito: preservar el estado de
    // transferencia es justo lo que NO se quiere aquí. Lo que había en vuelo
    // cuando el endpoint se paró es exactamente lo que falló.
    if estado == 2 {
        match cmd_cc(Trb { dw0: 0, dw1: 0, dw2: 0, dw3: campos | (TRB_RESET_EP << 10) }) {
            Some(CC_SUCCESS) => {}
            Some(cc) => {
                h.log_u64("[xhci] reset_ep cc=", cc as u64);
                h.log("\n");
                RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
                return false;
            }
            None => {
                h.log("[xhci] reset_ep sin respuesta del controlador\n");
                RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
                return false;
            }
        }
    }

    // ── 2. Set TR Dequeue Pointer: volver al principio del anillo ─────
    //
    // Se recoloca al TRB 0 con ciclo 1 y **la contabilidad nuestra se pone a
    // juego** (`enqueue = 0`, `pcs = true`). Las dos mitades tienen que decir lo
    // mismo: el xHC leerá donde le decimos, y nosotros escribiremos ahí con el
    // ciclo que le hemos declarado. Descuadrarlas es congelar el endpoint de la
    // forma más difícil de ver — el mismo fallo que el Toggle Cycle del Link.
    let ring_phys = match ep_ring_mut(slot, dci) {
        Some(r) => {
            r.enqueue = 0;
            r.pcs = true;
            r.ring_phys
        }
        None => {
            h.log("[xhci] recuperar: no hay anillo para ese endpoint\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
    };
    let dq = (ring_phys & !0xFu64) | 1; // DCS = 1, a juego con pcs = true
    let trb = Trb {
        dw0: (dq & 0xFFFF_FFFF) as u32,
        dw1: ((dq >> 32) & 0xFFFF_FFFF) as u32,
        dw2: 0, // Stream ID 0: este endpoint no usa streams
        dw3: campos | (TRB_SET_TR_DEQ << 10),
    };
    match cmd_cc(trb) {
        Some(CC_SUCCESS) => {}
        Some(cc) => {
            h.log_u64("[xhci] set_tr_deq cc=", cc as u64);
            h.log("\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
        None => {
            h.log("[xhci] set_tr_deq sin respuesta del controlador\n");
            RECUPERACIONES_FALLIDAS = RECUPERACIONES_FALLIDAS.wrapping_add(1);
            return false;
        }
    }

    RECUPERACIONES = RECUPERACIONES.wrapping_add(1);
    h.log_u64("[xhci] endpoint RESUCITADO dci=", dci as u64);
    h.log_u64(" slot=", slot as u64);
    h.log("\n");
    true
}

// ═══════════════════════════════════════════════════════════════════
//  Queue interrupt IN transfer
// ═══════════════════════════════════════════════════════════════════

pub unsafe fn queue_interrupt_in(slot: u8, dci: u8, buf_phys: u64, len: u16) -> bool {
    let ring = match ep_ring_mut(slot, dci) { Some(r) => r, None => return false };
    let idx = ring.enqueue;
    let b = idx * 4;
    ring.ring_virt.add(b).write_volatile((buf_phys & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 1).write_volatile(((buf_phys >> 32) & 0xFFFF_FFFF) as u32);
    ring.ring_virt.add(b + 2).write_volatile(len as u32);
    let ctl = (TRB_NORMAL << 10) | (1 << 5); // IOC
    ring.ring_virt.add(b + 3).write_volatile(ctl | if ring.pcs { 1 } else { 0 });
    ring.enqueue = idx + 1;
    if ring.enqueue >= LAST_TRB_IDX {
        // Al dar la vuelta hay que dejar el Link TRB con el ciclo ACTUAL antes
        // de invertir el nuestro; si no, el xHC llega al Link, ve un ciclo que
        // no es el suyo, y se detiene ahí para siempre. (El bit Toggle Cycle lo
        // pusimos al crear el anillo en configure_endpoint.)
        let lb = LAST_TRB_IDX * 4;
        let dw3 = ring.ring_virt.add(lb + 3).read_volatile();
        let dw3 = (dw3 & !1) | if ring.pcs { 1 } else { 0 };
        ring.ring_virt.add(lb + 3).write_volatile(dw3);
        ring.enqueue = 0;
        ring.pcs = !ring.pcs;
    }
    true
}

// ═══════════════════════════════════════════════════════════════════
//  Public non-blocking event poll
// ═══════════════════════════════════════════════════════════════════

/// Returns (slot, endpoint_id, cc) for the next transfer event, or None.
/// Ring doorbell for a slot+endpoint. EP0=1, EP1 OUT=2, EP1 IN=3, etc.
pub unsafe fn ring_doorbell(slot: u8, endpoint_id: u8) {
    if let Some(c) = CTRL.as_ref() {
        let db_addr = c.mmio + c.db_base as u64 + (slot as u64) * 4;
        w32(db_addr, endpoint_id as u32);
    }
}

/// Diagnóstico: cuántos Transfer Events ha posteado el xHC (cualquier slot/ep)
/// y cuántos eventos crudos de cualquier tipo. Si al presionar teclas TEV no
/// sube, el controlador no está completando la transferencia de interrupción
/// (endpoint/ring/doorbell), no el parseo. Ojos en metal desnudo.
static mut XFER_EVENTS: u32 = 0;
static mut RAW_EVENTS: u32 = 0;
// Ultimo Transfer Event: slot, endpoint_id, completion_code. Para comparar con
// el dci del teclado y ver si el evento matchea (si no, no se re-encola).
static mut LAST_SLOT: u8 = 0;
static mut LAST_EP: u8 = 0;
static mut LAST_CC: u8 = 0;

pub fn xfer_events() -> u32 { unsafe { XFER_EVENTS } }
pub fn raw_events() -> u32 { unsafe { RAW_EVENTS } }
pub fn last_event() -> (u8, u8, u8) { unsafe { (LAST_SLOT, LAST_EP, LAST_CC) } }

pub unsafe fn poll_transfer_event() -> Option<(u8, u8, u8)> {
    let ctrl = match CTRL.as_mut() { Some(c) => c, None => return None };
    loop {
        // ★ Lo APARCADO primero, y en orden.
        //
        // Un evento que llegó mientras la enumeración esperaba otra cosa es
        // tan válido como uno recién posteado — y es justamente el primer
        // informe de cada aparato, el que arranca la bomba. Si el aparcadero
        // no se drenara aquí, se habría cambiado tirar eventos por guardarlos
        // donde nadie los mira, que es el mismo silencio con otra cara.
        let ev = match desaparcar_cualquiera() {
            Some(e) => e,
            None => evt_poll_nb(ctrl)?,
        };
        RAW_EVENTS = RAW_EVENTS.wrapping_add(1);
        let typ = (ev.3 >> 10) & 0x3F;
        if typ == TRB_TRANSFER {
            XFER_EVENTS = XFER_EVENTS.wrapping_add(1);
            let slot = ((ev.3 >> 24) & 0xFF) as u8;
            let ep = ((ev.3 >> 16) & 0x1F) as u8;
            let cc = (ev.2 >> 24) as u8;
            LAST_SLOT = slot; LAST_EP = ep; LAST_CC = cc;
            return Some((slot, ep, cc));
        }
        // ── Cambio de puerto: enchufaron o desenchufaron algo ──
        //
        // Esto se estaba DESCARTANDO junto con las compleciones, y por eso no
        // habia hot-plug: el xHC avisa de que un puerto cambio de estado, y
        // nadie escuchaba. Al desenchufar el teclado no se enteraba nadie, y al
        // volver a enchufarlo tampoco.
        //
        // El Port ID viene en los bits 31:24 del primer dword del TRB, y es
        // 1-based (el puerto 1 del xHC es el indice 0 de PORTSC).
        //
        // ★ Hay que limpiar CSC SI O SI. Es write-1-to-clear: mientras siga
        // puesto, el xHC no vuelve a avisar de ese puerto — el segundo
        // enchufe pasaria en silencio. Se escribe preservando PP y poniendo
        // SOLO el bit que se quiere limpiar: los demas bits de estado son
        // RW1C y escribirles un 1 limpiaria cambios que no hemos atendido.
        if typ == TRB_PORT_STATUS {
            let port_id = ((ev.0 >> 24) & 0xFF) as u8;
            if port_id >= 1 {
                let idx = port_id - 1;
                if let Some(c) = CTRL.as_ref() {
                    let pb = c.op_base as u64 + 0x400 + idx as u64 * 0x10;
                    let sc = r32(c.mmio + pb + PORTSC as u64);
                    if sc & PORTSC_CSC != 0 {
                        w32(c.mmio + pb + PORTSC as u64, (sc & PORTSC_PP) | PORTSC_CSC);
                    }
                    PORT_EVENTS = PORT_EVENTS.wrapping_add(1);
                    LAST_PORT = port_id;
                    LAST_PORT_CCS = sc & PORTSC_CCS != 0;
                    PORT_PENDIENTE = true;
                }
            }
            continue;
        }
        if typ == TRB_COMPLETION { continue; }
    }
}

// ── Hot-plug: lo que el driver ve, para que otro decida ──────────────
//
// El driver NO re-enumera solo. Reconstruir un dispositivo es asignar slot,
// direccionarlo y configurar endpoints — decisiones que toma la capa de
// arriba (`uhid` + `dev::usb`), que es la que sabe si lo que se enchufo es un
// teclado, un raton o un disco. Aqui solo se anota el hecho.

static mut PORT_EVENTS: u32 = 0;
static mut LAST_PORT: u8 = 0;
static mut LAST_PORT_CCS: bool = false;
static mut PORT_PENDIENTE: bool = false;

/// `(cuantos cambios de puerto, ultimo puerto, hay dispositivo ahora)`.
/// Para diagnostico: si esto no sube al desenchufar, el xHC no esta avisando.
pub fn port_stats() -> (u32, u8, bool) {
    unsafe { (PORT_EVENTS, LAST_PORT, LAST_PORT_CCS) }
}

/// Consume el aviso: `Some((puerto, conectado))` una sola vez por cambio.
///
/// Devuelve `None` si no hay nada nuevo, para que el llamante pueda sondear en
/// su bucle sin re-enumerar cien veces el mismo enchufe.
pub fn tomar_cambio_puerto() -> Option<(u8, bool)> {
    unsafe {
        if !PORT_PENDIENTE {
            return None;
        }
        PORT_PENDIENTE = false;
        Some((LAST_PORT, LAST_PORT_CCS))
    }
}

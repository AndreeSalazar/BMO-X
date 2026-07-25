//! CABINA — el registrador omnisciente del sistema (lado Ring 0).
//!
//! Le da VIDA a `cabina-core`: mantiene el ANILLO DE EVENTOS del kernel y lo
//! pinta como un cockpit permanente en el framebuffer. La visión del usuario:
//! un observador que "ve todo" entre Ring 0 y Ring 3 — para dejar de debuggear
//! a ciegas.
//!
//! ## Grabadora, no encuestadora
//!
//! CABINA no adivina el estado comparando contadores al repintar: los módulos
//! EMPUJAN su evento en el instante exacto en que ocurre el hecho
//! (`cabina::info/warn/fault` desde usb, proc, faults, phase...). Consecuencia
//! importante: un hecho queda grabado aunque el shell nunca llegue a correr —
//! justo el escenario donde antes quedábamos ciegos.
//!
//! Lo único que sigue siendo POLLING es `watch()`, y a propósito: son
//! vigilancias de una CONDICIÓN que se sostiene en el tiempo (RAM baja, un
//! teclado que enumeró pero lleva rato mudo), no hechos puntuales.
//!
//! ## Reentrancia
//!
//! `record()` se llama desde el shell, desde `init` y desde el manejador de
//! faults. `cli` cubre la preempción por IRQ, pero una EXCEPCIÓN no se
//! enmascara: un #PF a media escritura del anillo re-entraría aquí. Por eso
//! hay un flag `BUSY` en vez de un spinlock — un lock se auto-bloquearía para
//! siempre en ese caso. Los eventos perdidos por reentrancia se CUENTAN y se
//! muestran: preferimos un número honesto a un hueco silencioso.
//!
//! A futuro: volcado del anillo a disco (NVMe+FAT32) = la caja negra forense,
//! y el buffer de shared-memory para que Ring 3 aporte su parte.

use cabina_core::{TelemetrySnapshot, Event, Severity, Layer, Entity};
use crate::ring0::core::splash::splash_dashboard_log_color;

// ── Buffer de EVENTOS: la grabadora ─────────────────────────────────────────
// Ring de eventos con severidad/capa/entidad. `cabina-core::Event` ya trae
// severidad, capa (from_module), módulo, mensaje y valor.

const EVENT_RING: usize = 48;
static mut EVENTS: [Event; EVENT_RING] = [Event::ZERO; EVENT_RING];
static mut EV_WRITE: usize = 0;
static mut EV_SEQ: u64 = 0;
static mut EV_TOTAL: u64 = 0;
static mut EV_LOST: u64 = 0;
static mut BUSY: bool = false;

#[inline]
fn irq_save() -> u64 {
    let f: u64;
    unsafe { core::arch::asm!("pushfq", "pop {}", "cli", out(reg) f); }
    f
}

#[inline]
fn irq_restore(flags: u64) {
    if flags & (1 << 9) != 0 {
        unsafe { core::arch::asm!("sti", options(nomem, nostack)); }
    }
}

/// Graba un evento. Seguro desde IRQ y desde el manejador de faults. La capa
/// se infiere del nombre del módulo (`Layer::from_module`): "usb"→ring0,
/// "lang"→lang, "cap"→sec, etc.
pub fn record(sev: Severity, module: &str, msg: &str, value: u64) {
    let flags = irq_save();
    unsafe {
        // Reentrancia (excepción a media escritura): contar y salir. Nunca
        // dejar el anillo a medio escribir ni girar en un lock imposible.
        if BUSY {
            EV_LOST = EV_LOST.wrapping_add(1);
            irq_restore(flags);
            return;
        }
        BUSY = true;

        let layer = Layer::from_module(module);
        let mut ev = Event::new(sev, layer, Entity::Module, module, 0, msg, value);
        EV_SEQ = EV_SEQ.wrapping_add(1);
        ev.seq = EV_SEQ;
        ev.tick_ns = crate::ring0::timer::ticks();
        let arr = core::ptr::addr_of_mut!(EVENTS) as *mut Event;
        core::ptr::write(arr.add(EV_WRITE), ev);
        EV_WRITE = (EV_WRITE + 1) % EVENT_RING;
        EV_TOTAL = EV_TOTAL.wrapping_add(1);

        BUSY = false;
    }
    irq_restore(flags);
}

/// Atajos por severidad — el vocabulario del narrador.
pub fn info(module: &str, msg: &str, value: u64)  { record(Severity::Info, module, msg, value); }
pub fn warn(module: &str, msg: &str, value: u64)  { record(Severity::Warning, module, msg, value); }
pub fn fault(module: &str, msg: &str, value: u64) { record(Severity::Fault, module, msg, value); }
/// Lo irrecuperable: fault de kernel, doble falta. Última línea de la bitácora
/// antes de que la máquina se detenga.
pub fn panic_ev(module: &str, msg: &str, value: u64) { record(Severity::Panic, module, msg, value); }

/// Evento `n` posiciones antes del más reciente (0 = el último). Para mostrar
/// el HISTORIAL, no solo la última línea.
fn event_back(n: usize) -> Option<Event> {
    unsafe {
        if n as u64 >= EV_TOTAL || n >= EVENT_RING { return None; }
        let idx = (EV_WRITE + EVENT_RING - 1 - n) % EVENT_RING;
        let arr = core::ptr::addr_of!(EVENTS) as *const Event;
        Some(core::ptr::read(arr.add(idx)))
    }
}

/// Total de eventos grabados desde el arranque (puede exceder el anillo).
pub fn event_total() -> u64 { unsafe { EV_TOTAL } }
/// Eventos perdidos por reentrancia. Debería ser 0; si no lo es, algo faltó
/// durante un fault y la bitácora lo dice en vez de callarlo.
pub fn event_lost() -> u64 { unsafe { EV_LOST } }

// Paleta de estado (aviso por color, como pidió el usuario): verde = bien,
// ámbar = atención, rojo = problema, cyan = info/título, gris = neutro.
const C_OK: u32    = 0xFF00E676; // verde neón
const C_WARN: u32  = 0xFFFFB020; // ámbar
const C_FAULT: u32 = 0xFFFF4D4D; // rojo
const C_INFO: u32  = 0xFF00E5FF; // cyan (título)
const C_DIM: u32   = 0xFF94A3B8; // gris azulado
const C_TEXT: u32  = 0xFFF1F5F9; // texto normal
const C_RING3: u32 = 0xFF00E676; // verde — userspace
const C_FS: u32    = 0xFF818CF8; // índigo — almacenamiento
const C_SEC: u32   = 0xFFC084FC; // violeta — capabilities

/// Color de una línea de bitácora. La severidad manda (un fallo es rojo venga
/// de donde venga); si es informativa, el color lo pone la CAPA — así se lee
/// de un vistazo quién habla sin descifrar el prefijo.
fn ev_color(ev: &Event) -> u32 {
    match ev.severity {
        Severity::Panic | Severity::Fault => C_FAULT,
        Severity::Warning => C_WARN,
        Severity::Trace => C_DIM,
        Severity::Info => match ev.layer {
            Layer::Ring3 => C_RING3,
            Layer::Fs => C_FS,
            Layer::Sec => C_SEC,
            Layer::Lang | Layer::BmoGpu => C_INFO,
            _ => C_TEXT,
        },
    }
}

/// Construye un snapshot desde el estado VIVO del kernel. Aquí `cabina-core`
/// deja de ser estructuras muertas y empieza a respirar.
pub fn snapshot() -> TelemetrySnapshot {
    let mut s = TelemetrySnapshot::zero();
    s.cpu.timer_ticks = crate::ring0::timer::ticks();
    let (_total, free) = crate::ring0::mm::phys::stats();
    s.memory.free_pages = free;
    s.scheduler.context_switches = crate::ring0::scheduler::user_switches();
    let (tasks, runnable) = crate::ring0::scheduler::counts();
    s.scheduler.processes = tasks as u64;
    s.scheduler.threads = runnable as u64;
    s.uptime_ns = s.cpu.timer_ticks; // proxy hasta tener ns reales del TSC
    s
}

// ── Censo de arranque ───────────────────────────────────────────────────────

/// Primer paso hacia la CAJA NEGRA: ¿qué controlador de disco hay? Saberlo
/// decide el driver a cablear (AHCI vs NVMe). Se llama UNA vez desde
/// `phase::main` — antes vivía dentro del render, donde un scan PCI por fuerza
/// bruta (~65k lecturas de config) bloqueaba el primer frame.
pub fn boot_probe() {
    info("cabina", "observador omnisciente en linea", 0);
    // CENSO COMPLETO, no "el primero". Saber cuántos controladores de
    // almacenamiento hay y DE QUÉ TIPO es lo que dice dónde buscar un disco.
    // Si la BIOS tiene el SATA del chipset en modo RAID, ese controlador
    // aparece con clase RAID y no con clase AHCI — y un buscador que solo
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
        // localizarlo después sin volver a barrer el bus.
        info("pci", msg, loc.mmio);
        if index >= 8 { break; }
    }
    if found == 0 {
        warn("pci", "sin controlador de almacenamiento visible", 0);
    } else {
        info("pci", "controladores de almacenamiento hallados", found);
    }
}

// ── Vigilancias (esto SÍ es polling, y con razón) ───────────────────────────

/// Condiciones que se sostienen en el tiempo y no tienen un "instante" en que
/// alguien pueda emitirlas: hay que mirarlas cada tanto. Cada una se narra UNA
/// vez (de-dup por flag). Llamado desde `render_hud`.
fn watch(s: &TelemetrySnapshot, mib_free: u64) {
    static mut W_KBD_MUDO: bool = false;
    static mut W_MEMLOW: bool = false;
    unsafe {
        // El teclado enumeró pero lleva un buen rato sin entregar una sola
        // tecla: el endpoint de interrupción no completa. Es EL fallo vivo.
        if !W_KBD_MUDO {
            let (kbd, _m, _ks, _ms, _mev, _x, _y, _b, kev) = crate::ring0::dev::usb::hid_stats();
            if kbd && kev == 0 && s.cpu.timer_ticks > 0x2000 {
                W_KBD_MUDO = true;
                let (kdci, _, _, _) = crate::ring0::dev::usb::kbd_debug();
                fault("usb", "teclado enumero pero no entrega teclas", kdci as u64);
            }
        }
        if mib_free < 256 && !W_MEMLOW {
            W_MEMLOW = true;
            warn("mem", "RAM libre por debajo de 256MiB", mib_free);
        }
    }
}

// ── Formateo sin std (buffer de bytes) ──────────────────────────────────────

struct Buf {
    b: [u8; 96],
    o: usize,
}
impl Buf {
    fn new() -> Self { Self { b: [0u8; 96], o: 0 } }
    fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() { if self.o < self.b.len() { self.b[self.o] = c; self.o += 1; } }
    }
    fn dec(&mut self, mut v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut tmp = [0u8; 20]; let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if self.o < self.b.len() { self.b[self.o] = tmp[i]; self.o += 1; } }
    }
    /// Decimal alineado a la derecha en `width` — mantiene las columnas de la
    /// bitácora quietas aunque el número crezca.
    fn dec_pad(&mut self, v: u64, width: usize) {
        let mut digits = 1; let mut t = v;
        while t >= 10 { t /= 10; digits += 1; }
        for _ in digits..width { self.txt(" "); }
        self.dec(v);
    }
    fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.o < self.b.len() { self.b[self.o] = H[((v >> (i * 4)) & 0xF) as usize]; self.o += 1; }
        }
    }
    /// Hex sin ceros a la izquierda — para el `value` del evento, que puede ser
    /// una dirección MMIO o un contador de 2 dígitos.
    fn hex_min(&mut self, v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut digits = 1;
        let mut t = v >> 4;
        while t > 0 { t >>= 4; digits += 1; }
        self.hex(v, digits);
    }
    /// Texto a ancho fijo (recorta o rellena) — columnas estables.
    fn pad(&mut self, s: &str, width: usize) {
        let n = s.len().min(width);
        self.txt(&s[..n]);
        for _ in n..width { self.txt(" "); }
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}

// ── Cockpit ─────────────────────────────────────────────────────────────────
//
// CABINA vive en la BANDA INFERIOR del panel; el log rodante del kernel/shell
// se queda con la banda de arriba. Antes ambos escribían en las filas 2-13 y
// se borraban mutuamente (el panel estaba fijado en 14 filas aunque en 1080p
// caben ~49). Ahora el reparto se calcula del alto real del framebuffer.

/// Filas que ocupa el cockpit dentro de un panel de `total` filas: cabecera +
/// bitácora + 3 de telemetría. Un tercio del panel, acotado.
pub fn band_rows(total: usize) -> usize {
    // Panel diminuto: dejar SIEMPRE 2 filas al log rodante. Nunca devolver más
    // que `total` — quien llama resta esto y una resta negativa en `usize` da
    // la vuelta (bucle de millones de filas en release, no un panic honesto).
    if total < 12 { return total.saturating_sub(2); }
    (total / 3).clamp(6, 20)
}

/// Pinta el cockpit omnisciente. Llamado always-on desde el loop del shell.
pub fn render_hud() {
    if !crate::info::has_fb() { return; }
    let total = crate::ring0::core::splash::dash_rows();
    if total == 0 { return; }
    let rows = band_rows(total);
    // El cockpit necesita cabecera + al menos 1 línea de bitácora + 4 de
    // telemetría. Menos que eso no es un cockpit, es ruido: mejor no pintar.
    if rows < 6 { return; }
    let top = total - rows;

    let s = snapshot();
    let (kbd, mouse, ks, ms, mev, _mx, _my, _btn, kev) = crate::ring0::dev::usb::hid_stats();
    let (tev, rev, hev) = crate::ring0::dev::usb::xfer_stats();
    let (kdci, es, ee, ec) = crate::ring0::dev::usb::kbd_debug();
    let (kst, kbi, kiv, _ksp, ksts) = crate::ring0::dev::usb::kbd_ep_debug();
    let st = crate::ring0::scheduler::tid_state(2);
    let (rx, ln) = crate::ring0::uconsole::stats();
    let tid = crate::ring0::scheduler::current_tid();
    let mib_free = s.memory.free_pages / 256; // 4096 B/página → /256 = MiB

    watch(&s, mib_free);

    // Firma de cambio: ticks en bucket grueso (>>8) para no parpadear; el
    // resto son eventos reales. + generación de pantalla para repintar tras clear.
    static mut LAST: u64 = u64::MAX;
    static mut LAST_GEN: u32 = u32::MAX;
    let sig = (s.cpu.timer_ticks >> 8)
        ^ (s.scheduler.context_switches << 8)
        ^ ((s.memory.free_pages & 0xFFFF) << 16)
        ^ ((kev as u64) << 24) ^ ((tev as u64) << 28) ^ ((mev as u64) << 32)
        ^ ((st as u64) << 40) ^ ((kbd as u64) << 48) ^ ((mouse as u64) << 49)
        ^ ((s.scheduler.processes) << 50) ^ ((rx as u64) << 54)
        ^ (event_total() << 58)
        // El estado del endpoint puede pasar a Halted en caliente: si cambia,
        // hay que repintar aunque no se mueva ningún contador.
        ^ ((kst as u64) << 20) ^ ((rev as u64) << 36);
    let gen = crate::ring0::core::phase::screen_gen();
    unsafe {
        if LAST == sig && LAST_GEN == gen { return; }
        LAST = sig; LAST_GEN = gen;
    }

    // CABINA se pinta también desde contextos cuya CR3 (la del usuario) no
    // mapea el framebuffer: pintar bajo la CR3 del kernel y restaurar. Mismo
    // patrón que uconsole::flush.
    let saved_cr3 = crate::ring0::mm::vmm::read_cr3();
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(kpml4); }

    // ══ Cabecera de la banda: identidad + salud del propio registrador.
    let mut r = Buf::new();
    r.txt("== CABINA == eventos="); r.dec(event_total());
    r.txt(" perdidos="); r.dec(event_lost());
    r.txt(" tk=0x"); r.hex(s.cpu.timer_ticks, 6);
    let hdr_color = if event_lost() > 0 { C_WARN } else { C_INFO };
    splash_dashboard_log_color(top, r.as_str(), hdr_color);

    // ══ BITÁCORA EN TIEMPO REAL: el historial, el más nuevo abajo, cada línea
    // con seq y tick (orden y distancia entre hechos = la mitad del valor
    // forense) y el color de su severidad/capa. Esto es la caja negra — aún
    // en RAM, pendiente el volcado a disco.
    let log_rows = rows - 5; // cabecera + 4 filas de telemetría (sys/ring3/usb/kbd)
    for slot in 0..log_rows {
        let row = top + 1 + slot;
        let n = log_rows - 1 - slot; // arriba = más viejo; abajo = más nuevo
        match event_back(n) {
            Some(ev) => {
                let mut r = Buf::new();
                r.dec_pad(ev.seq, 4);
                r.txt(" t"); r.hex(ev.tick_ns, 5);
                r.txt(" "); r.pad(ev.severity.name(), 5);
                r.txt(" "); r.txt(ev.module_str()); r.txt(": ");
                r.txt(ev.msg_str());
                // El `value` del evento se guardaba y se TIRABA al pintar. Es
                // justo el dato duro (dirección MMIO, slot, código de estado)
                // que convierte una frase en una pista.
                if ev.value != 0 { r.txt(" ="); r.hex_min(ev.value); }
                splash_dashboard_log_color(row, r.as_str(), ev_color(&ev));
            }
            None => splash_dashboard_log_color(row, "", C_DIM),
        }
    }

    // ══ TELEMETRÍA COMPACTA (3 últimas filas): salud del sistema de un vistazo.
    // Sistema — verde = sano, ámbar = RAM baja.
    let mut r = Buf::new();
    r.txt("sys  mem="); r.dec(mib_free); r.txt("MiB");
    r.txt(" sw="); r.dec(s.scheduler.context_switches);
    r.txt(" task="); r.dec(s.scheduler.processes); r.txt("/"); r.dec(s.scheduler.threads);
    r.txt(" tid="); r.dec(tid as u64);
    let health = if mib_free < 256 { C_WARN } else { C_OK };
    splash_dashboard_log_color(total - 4, r.as_str(), health);

    // Ring 3 — verde = corriendo, gris = terminó, ámbar = bloqueado.
    let mut r = Buf::new();
    r.txt("ring3 st="); r.hex(st as u64, 2);
    r.txt(" rx="); r.dec(rx as u64); r.txt(" ln="); r.dec(ln as u64);
    r.txt("  (01Rdy 02Run 03Blk 04Exit FFdone)");
    let r3_color = match st { 0x02 => C_OK, 0xFF | 0x04 => C_DIM, 0x03 => C_WARN, _ => C_INFO };
    splash_dashboard_log_color(total - 3, r.as_str(), r3_color);

    // USB — EL AVISO: verde si escribe, ROJO si enumeró sin teclas.
    let mut r = Buf::new();
    r.txt("usb  k="); r.txt(if kbd { "OK" } else { "--" });
    r.txt("(s"); r.dec(ks as u64); r.txt(")");
    r.txt(" m="); r.txt(if mouse { "OK" } else { "--" });
    r.txt("(s"); r.dec(ms as u64); r.txt(")");
    r.txt(" kev="); r.dec(kev as u64);
    r.txt(" tev="); r.dec(tev as u64);
    // rev = eventos CRUDOS del xHC (de cualquier tipo). Se calculaba y no se
    // mostraba: es el que distingue "el controlador está mudo" de "habla pero
    // no de este endpoint".
    r.txt(" rev="); r.dec(rev as u64);
    r.txt(" hev="); r.dec(hev as u64);
    r.txt(" dci="); r.dec(kdci as u64);
    r.txt(" lev="); r.dec(es as u64); r.txt(":"); r.dec(ee as u64); r.txt(":"); r.dec(ec as u64);
    let usb_color = if kbd && kev > 0 { C_OK }
                    else if kbd && kev == 0 { C_FAULT }
                    else { C_WARN };
    splash_dashboard_log_color(total - 2, r.as_str(), usb_color);

    // Última fila — el ENDPOINT del teclado según el xHC. `ep` debe ser 1
    // (Running); `bi`→`iv` muestra el bInterval del descriptor y el exponente
    // que programamos de verdad (el bug del Interval se ve aquí de un vistazo).
    let mut r = Buf::new();
    r.txt("kbd  ep=");
    r.txt(match kst { 0 => "Disabled", 1 => "Running", 2 => "Halted", 3 => "Stopped", 4 => "Error", _ => "?" });
    r.txt(" bi="); r.dec(kbi as u64);
    r.txt(" iv="); r.dec(kiv as u64);
    r.txt(" (2^iv x125us)");
    r.txt(" usbsts=0x"); r.hex(ksts as u64, 4);
    let ep_color = if !kbd { C_DIM }
                   else if kst == 1 && ksts & ((1 << 2) | (1 << 12)) == 0 { C_OK }
                   else { C_FAULT };
    splash_dashboard_log_color(total - 1, r.as_str(), ep_color);

    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(saved_cr3); }
}

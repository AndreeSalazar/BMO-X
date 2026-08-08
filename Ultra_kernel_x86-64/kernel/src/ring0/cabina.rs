//! CABINA -- el registrador omnisciente del sistema (lado Ring 0).
//!
//! Le da VIDA a `cabina-core`: mantiene el ANILLO DE EVENTOS del kernel y lo
//! pinta como un cockpit permanente en el framebuffer. La vision del usuario:
//! un observador que "ve todo" entre Ring 0 y Ring 3 -- para dejar de debuggear
//! a ciegas.
//!
//! ## Grabadora, no encuestadora
//!
//! CABINA no adivina el estado comparando contadores al repintar: los modulos
//! EMPUJAN su evento en el instante exacto en que ocurre el hecho
//! (`cabina::info/warn/fault` desde usb, proc, faults, phase...). Consecuencia
//! importante: un hecho queda grabado aunque el shell nunca llegue a correr --
//! justo el escenario donde antes quedabamos ciegos.
//!
//! Lo unico que sigue siendo POLLING es `watch()`, y a proposito: son
//! vigilancias de una CONDICION que se sostiene en el tiempo (RAM baja, un
//! teclado que enumero pero lleva rato mudo), no hechos puntuales.
//!
//! ## Reentrancia
//!
//! `record()` se llama desde el shell, desde `init` y desde el manejador de
//! faults. `cli` cubre la preempcion por IRQ, pero una EXCEPCION no se
//! enmascara: un #PF a media escritura del anillo re-entraria aqui. Por eso
//! hay un flag `BUSY` en vez de un spinlock -- un lock se auto-bloquearia para
//! siempre en ese caso. Los eventos perdidos por reentrancia se CUENTAN y se
//! muestran: preferimos un numero honesto a un hueco silencioso.
//!
//! A futuro: volcado del anillo a disco (NVMe+FAT32) = la caja negra forense,
//! y el buffer de shared-memory para que Ring 3 aporte su parte.

use cabina_core::{TelemetrySnapshot, Event, Severity, Layer, Entity};
use crate::ring0::core::splash::splash_dashboard_log_color;

// -- Buffer de EVENTOS: la grabadora -----------------------------------------
// Ring de eventos con severidad/capa/entidad. `cabina-core::Event` ya trae
// severidad, capa (from_module), modulo, mensaje y valor.

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
/// se infiere del nombre del modulo (`Layer::from_module`): "usb"->ring0,
/// "lang"->lang, "cap"->sec, etc.
pub fn record(sev: Severity, module: &str, msg: &str, value: u64) {
    let flags = irq_save();
    unsafe {
        // Reentrancia (excepcion a media escritura): contar y salir. Nunca
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
        ev.tick_ns = crate::ring0::plat::timer::ticks();
        let arr = core::ptr::addr_of_mut!(EVENTS) as *mut Event;
        core::ptr::write(arr.add(EV_WRITE), ev);
        EV_WRITE = (EV_WRITE + 1) % EVENT_RING;
        EV_TOTAL = EV_TOTAL.wrapping_add(1);

        BUSY = false;
    }
    irq_restore(flags);
}

/// Atajos por severidad -- el vocabulario del narrador.
pub fn info(module: &str, msg: &str, value: u64)  { record(Severity::Info, module, msg, value); }
pub fn warn(module: &str, msg: &str, value: u64)  { record(Severity::Warning, module, msg, value); }
pub fn fault(module: &str, msg: &str, value: u64) { record(Severity::Fault, module, msg, value); }
/// Lo irrecuperable: fault de kernel, doble falta. Ultima linea de la bitacora
/// antes de que la maquina se detenga.
pub fn panic_ev(module: &str, msg: &str, value: u64) { record(Severity::Panic, module, msg, value); }

/// Evento `n` posiciones antes del mas reciente (0 = el ultimo). Para mostrar
/// el HISTORIAL, no solo la ultima linea.
fn event_back(n: usize) -> Option<Event> {
    unsafe {
        if n as u64 >= EV_TOTAL || n >= EVENT_RING { return None; }
        let idx = (EV_WRITE + EVENT_RING - 1 - n) % EVENT_RING;
        let arr = core::ptr::addr_of!(EVENTS) as *const Event;
        Some(core::ptr::read(arr.add(idx)))
    }
}

// -- La caja negra: el anillo, en el disco -----------------------------------
//
// Hasta aqui CABINA lo ve todo y lo olvida al apagar. Un registrador de vuelo
// que solo existe mientras vuela el avion no sirve para investigar la caida.
//
// El buffer es estatico y no una variable local: 48 eventos formateados son
// varios KiB y la pila del kernel no esta para eso.

const DUMP_MAX: usize = 8 * 1024;
static mut DUMP: [u8; DUMP_MAX] = [0u8; DUMP_MAX];

/// Vuelca la bitacora entera a `CABINA.LOG` en el volumen de datos.
///
/// Devuelve los bytes escritos, o 0 con el motivo ya narrado. Se escribe el
/// anillo en orden CRONOLOGICO (el mas viejo primero), al reves de como se
/// pinta en pantalla: un archivo se lee de arriba abajo.
pub fn dump_to_disk() -> usize {
    let mut n = 0usize;
    {
        // Cabecera: sin ella, un archivo de lineas sueltas no dice de que
        // arranque es ni cuanto se perdio.
        let mut h = Buf::new();
        h.txt("BMO-X CABINA -- bitacora de vuelo\n");
        n = append(n, h.as_str());
        let mut h = Buf::new();
        h.txt("eventos="); h.dec(event_total());
        h.txt(" perdidos="); h.dec(event_lost());
        h.txt(" anillo="); h.dec(EVENT_RING as u64);
        h.txt("\n");
        n = append(n, h.as_str());
        let mut h = Buf::new();
        h.txt("disco="); h.txt(crate::ring0::dev::disk::model());
        h.txt(" serie="); h.txt(crate::ring0::dev::disk::serial());
        h.txt("\n\n");
        n = append(n, h.as_str());
    }

    // Del mas viejo al mas nuevo.
    let have = (event_total() as usize).min(EVENT_RING);
    for i in (0..have).rev() {
        let ev = match event_back(i) { Some(e) => e, None => continue };
        let mut r = Buf::new();
        r.dec_pad(ev.seq, 4);
        r.txt(" t"); r.hex(ev.tick_ns, 5);
        r.txt(" "); r.pad(ev.severity.name(), 5);
        r.txt(" "); r.txt(ev.module_str()); r.txt(": ");
        r.txt(ev.msg_str());
        if ev.value != 0 { r.txt(" ="); r.hex_min(ev.value); }
        r.txt("\n");
        n = append(n, r.as_str());
    }

    // Sin autoref sobre el puntero crudo: se pide el slice explicitamente, que
    // es lo mismo que el compilador iba a hacer pero dicho en voz alta.
    let data = unsafe { core::slice::from_raw_parts(core::ptr::addr_of!(DUMP) as *const u8, n) };
    match crate::ring0::fsys::fs::create(b"CABINA  LOG", data) {
        Ok(()) => {
            info("fs", "bitacora volcada a CABINA.LOG", n as u64);
            n
        }
        Err(e) => {
            // El motivo REAL, no un "no se pudo". "Ya existe" y "disco lleno"
            // piden cosas distintas de quien lo lee.
            fault("fs", e.name(), n as u64);
            0
        }
    }
}

/// Anade texto al buffer de volcado sin desbordarlo. Devuelve el nuevo final.
fn append(mut n: usize, s: &str) -> usize {
    unsafe {
        let buf = &mut *core::ptr::addr_of_mut!(DUMP);
        for &b in s.as_bytes() {
            if n >= buf.len() { return n; }
            buf[n] = b;
            n += 1;
        }
    }
    n
}

/// Total de eventos grabados desde el arranque (puede exceder el anillo).
pub fn event_total() -> u64 { unsafe { EV_TOTAL } }
/// Eventos perdidos por reentrancia. Deberia ser 0; si no lo es, algo falto
/// durante un fault y la bitacora lo dice en vez de callarlo.
pub fn event_lost() -> u64 { unsafe { EV_LOST } }

// Paleta de estado (aviso por color, como pidio el usuario): verde = bien,
// ambar = atencion, rojo = problema, cyan = info/titulo, gris = neutro.
// Los valores son los mismos que la paleta del panel (core/splash.rs): CABINA
// y el log rodante comparten pantalla, y dos verdes distintos a diez pixeles
// uno del otro se leen como un error de impresion, no como dos capas.
const C_OK: u32    = 0xFF39FF88; // verde neon
const C_WARN: u32  = 0xFFF6C445; // ambar
const C_FAULT: u32 = 0xFFFF3355; // rojo lacado -- SOLO para lo que va mal
const C_INFO: u32  = 0xFF00F0FF; // cian (titulo)
const C_DIM: u32   = 0xFF55647E; // gris azulado
const C_TEXT: u32  = 0xFFE6EDF7; // texto normal
const C_RING3: u32 = 0xFF39FF88; // verde -- userspace
const C_FS: u32    = 0xFF2DE2C5; // jade -- almacenamiento
const C_SEC: u32   = 0xFFC084FC; // violeta -- capabilities

/// Color de una linea de bitacora. La severidad manda (un fallo es rojo venga
/// de donde venga); si es informativa, el color lo pone la CAPA -- asi se lee
/// de un vistazo quien habla sin descifrar el prefijo.
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

/// Construye un snapshot desde el estado VIVO del kernel. Aqui `cabina-core`
/// deja de ser estructuras muertas y empieza a respirar.
pub fn snapshot() -> TelemetrySnapshot {
    let mut s = TelemetrySnapshot::zero();
    s.cpu.timer_ticks = crate::ring0::plat::timer::ticks();
    let (_total, free) = crate::ring0::mm::phys::stats();
    s.memory.free_pages = free;
    s.scheduler.context_switches = crate::ring0::task::scheduler::user_switches();
    let (tasks, runnable) = crate::ring0::task::scheduler::counts();
    s.scheduler.processes = tasks as u64;
    s.scheduler.threads = runnable as u64;
    s.uptime_ns = s.cpu.timer_ticks; // proxy hasta tener ns reales del TSC
    s
}

// -- Censo de arranque -------------------------------------------------------

/// Primer paso hacia la CAJA NEGRA: que controlador de disco hay? Saberlo
/// decide el driver a cablear (AHCI vs NVMe). Se llama UNA vez desde
/// `phase::main` -- antes vivia dentro del render, donde un scan PCI por fuerza
/// bruta (~65k lecturas de config) bloqueaba el primer frame.
pub fn boot_probe() {
    info("cabina", "observador omnisciente en linea", 0);
    // CENSO COMPLETO, no "el primero". Saber cuantos controladores de
    // almacenamiento hay y DE QUE TIPO es lo que dice donde buscar un disco.
    // Si la BIOS tiene el SATA del chipset en modo RAID, ese controlador
    // aparece con clase RAID y no con clase AHCI -- y un buscador que solo
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
        // localizarlo despues sin volver a barrer el bus.
        info("pci", msg, loc.mmio);
        if index >= 8 { break; }
    }
    if found == 0 {
        warn("pci", "sin controlador de almacenamiento visible", 0);
    } else {
        info("pci", "controladores de almacenamiento hallados", found);
    }
}

// -- Vigilancias (esto SI es polling, y con razon) ---------------------------

/// Condiciones que se sostienen en el tiempo y no tienen un "instante" en que
/// alguien pueda emitirlas: hay que mirarlas cada tanto. Cada una se narra UNA
/// vez (de-dup por flag). Llamado desde `render_hud`.
fn watch(s: &TelemetrySnapshot, mib_free: u64) {
    static mut W_KBD_MUDO: bool = false;
    static mut W_MEMLOW: bool = false;
    unsafe {
        // El teclado enumero pero lleva un buen rato sin entregar una sola
        // tecla: el endpoint de interrupcion no completa. Es EL fallo vivo.
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

// -- Formateo sin std (buffer de bytes) --------------------------------------

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
    /// Decimal alineado a la derecha en `width` -- mantiene las columnas de la
    /// bitacora quietas aunque el numero crezca.
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
    /// Hex sin ceros a la izquierda -- para el `value` del evento, que puede ser
    /// una direccion MMIO o un contador de 2 digitos.
    fn hex_min(&mut self, v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut digits = 1;
        let mut t = v >> 4;
        while t > 0 { t >>= 4; digits += 1; }
        self.hex(v, digits);
    }
    /// Texto a ancho fijo (recorta o rellena) -- columnas estables.
    fn pad(&mut self, s: &str, width: usize) {
        let n = s.len().min(width);
        self.txt(&s[..n]);
        for _ in n..width { self.txt(" "); }
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}

// -- Cockpit -----------------------------------------------------------------
//
// CABINA vive en la BANDA INFERIOR del panel; el log rodante del kernel/shell
// se queda con la banda de arriba. Antes ambos escribian en las filas 2-13 y
// se borraban mutuamente (el panel estaba fijado en 14 filas aunque en 1080p
// caben ~49). Ahora el reparto se calcula del alto real del framebuffer.

/// Filas que ocupa el cockpit dentro de un panel de `total` filas: cabecera +
/// bitacora + 3 de telemetria. Un tercio del panel, acotado.
pub fn band_rows(total: usize) -> usize {
    // Panel diminuto: dejar SIEMPRE 2 filas al log rodante. Nunca devolver mas
    // que `total` -- quien llama resta esto y una resta negativa en `usize` da
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
    // El cockpit necesita cabecera + al menos 1 linea de bitacora + CINCO de
    // telemetria (sys, ring3, usb, kbd y raton). Menos que eso no es un
    // cockpit, es ruido: mejor no pintar.
    if rows < 7 { return; }
    let top = total - rows;

    let s = snapshot();
    // * `mx`, `my` y `btn` llegaban aqui y se tiraban con un guion bajo, igual
    // que el `_info` del panico del compositor. CABINA sabia donde estaba el
    // raton y no lo decia, asi que "el raton no va" no se podia repartir entre
    // tres culpables muy distintos. Ahora se ensenan.
    let (kbd, mouse, ks, ms, mev, mx, my, btn, kev) = crate::ring0::dev::usb::hid_stats();
    let (tev, rev, hev) = crate::ring0::dev::usb::xfer_stats();
    let (kdci, es, ee, ec) = crate::ring0::dev::usb::kbd_debug();
    let (kst, kbi, kiv, _ksp, ksts) = crate::ring0::dev::usb::kbd_ep_debug();
    let st = crate::ring0::task::scheduler::tid_state(2);
    let (rx, ln) = crate::ring0::uconsole::stats();
    let tid = crate::ring0::task::scheduler::current_tid();
    let mib_free = s.memory.free_pages / 256; // 4096 B/pagina -> /256 = MiB

    watch(&s, mib_free);

    // Firma de cambio: ticks en bucket grueso (>>8) para no parpadear; el
    // resto son eventos reales. + generacion de pantalla para repintar tras clear.
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
        // hay que repintar aunque no se mueva ningun contador.
        ^ ((kst as u64) << 20) ^ ((rev as u64) << 36);
    let gen = crate::ring0::core::phase::screen_gen();
    unsafe {
        if LAST == sig && LAST_GEN == gen { return; }
        LAST = sig; LAST_GEN = gen;
    }

    // CABINA se pinta tambien desde contextos cuya CR3 (la del usuario) no
    // mapea el framebuffer: pintar bajo la CR3 del kernel y restaurar. Mismo
    // patron que uconsole::flush.
    let saved_cr3 = crate::ring0::mm::vmm::read_cr3();
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(kpml4); }

    // == Cabecera de la banda: identidad + salud del propio registrador.
    // Va como REGLA, no como una linea mas de texto: es la frontera entre el
    // log rodante y el cockpit, y tiene que verse sin leerla.
    let mut r = Buf::new();
    r.txt("CABINA  eventos="); r.dec(event_total());
    r.txt("  perdidos="); r.dec(event_lost());
    r.txt("  tk=0x"); r.hex(s.cpu.timer_ticks, 6);
    let hdr_color = if event_lost() > 0 { C_WARN } else { C_INFO };
    crate::ring0::core::splash::splash_dash_rule(top, r.as_str(), hdr_color);

    // == BITACORA EN TIEMPO REAL: el historial, el mas nuevo abajo, cada linea
    // con seq y tick (orden y distancia entre hechos = la mitad del valor
    // forense) y el color de su severidad/capa. Esto es la caja negra -- aun
    // en RAM, pendiente el volcado a disco.
    let log_rows = rows - 5; // cabecera + 4 filas de telemetria (sys/ring3/usb/kbd)
    for slot in 0..log_rows {
        let row = top + 1 + slot;
        let n = log_rows - 1 - slot; // arriba = mas viejo; abajo = mas nuevo
        match event_back(n) {
            Some(ev) => {
                let mut r = Buf::new();
                r.dec_pad(ev.seq, 4);
                r.txt(" t"); r.hex(ev.tick_ns, 5);
                r.txt(" "); r.pad(ev.severity.name(), 5);
                r.txt(" "); r.txt(ev.module_str()); r.txt(": ");
                r.txt(ev.msg_str());
                // El `value` del evento se guardaba y se TIRABA al pintar. Es
                // justo el dato duro (direccion MMIO, slot, codigo de estado)
                // que convierte una frase en una pista.
                if ev.value != 0 { r.txt(" ="); r.hex_min(ev.value); }
                splash_dashboard_log_color(row, r.as_str(), ev_color(&ev));
            }
            None => splash_dashboard_log_color(row, "", C_DIM),
        }
    }

    // == TELEMETRIA COMPACTA (3 ultimas filas): salud del sistema de un vistazo.
    // Sistema -- verde = sano, ambar = RAM baja.
    let mut r = Buf::new();
    r.txt("sys  mem="); r.dec(mib_free); r.txt("MiB");
    r.txt(" sw="); r.dec(s.scheduler.context_switches);
    r.txt(" task="); r.dec(s.scheduler.processes); r.txt("/"); r.dec(s.scheduler.threads);
    r.txt(" tid="); r.dec(tid as u64);
    let health = if mib_free < 256 { C_WARN } else { C_OK };
    splash_dashboard_log_color(total - 5, r.as_str(), health);

    // Ring 3 -- verde = corriendo, gris = termino, ambar = bloqueado.
    let mut r = Buf::new();
    r.txt("ring3 st="); r.hex(st as u64, 2);
    r.txt(" rx="); r.dec(rx as u64); r.txt(" ln="); r.dec(ln as u64);
    r.txt("  (01Rdy 02Run 03Blk 04Exit FFdone)");
    let r3_color = match st { 0x02 => C_OK, 0xFF | 0x04 => C_DIM, 0x03 => C_WARN, _ => C_INFO };
    splash_dashboard_log_color(total - 4, r.as_str(), r3_color);

    // USB -- EL AVISO: verde si escribe, ROJO si enumero sin teclas.
    let mut r = Buf::new();
    r.txt("usb  k="); r.txt(if kbd { "OK" } else { "--" });
    r.txt("(s"); r.dec(ks as u64); r.txt(")");
    r.txt(" m="); r.txt(if mouse { "OK" } else { "--" });
    r.txt("(s"); r.dec(ms as u64); r.txt(")");
    r.txt(" kev="); r.dec(kev as u64);
    r.txt(" tev="); r.dec(tev as u64);
    // rev = eventos CRUDOS del xHC (de cualquier tipo). Se calculaba y no se
    // mostraba: es el que distingue "el controlador esta mudo" de "habla pero
    // no de este endpoint".
    r.txt(" rev="); r.dec(rev as u64);
    r.txt(" hev="); r.dec(hev as u64);
    r.txt(" dci="); r.dec(kdci as u64);
    r.txt(" lev="); r.dec(es as u64); r.txt(":"); r.dec(ee as u64); r.txt(":"); r.dec(ec as u64);
    // El APARCADERO de eventos: `total:dropped:ahora`.
    //
    // Un evento que llega mientras la enumeracion espera otra cosa ya no se
    // tira -- se aparca. `dropped` tiene que ser **0**: si sube, el aparcadero
    // se lleno y se perdio un informe, que es como enmudece un endpoint. Es el
    // numero que antes no existia y por el que el teclado se apago sin decir
    // nada.
    let (apk_tot, apk_perd, apk_hoy) = crate::ring0::dev::usb::park_stats();
    r.txt(" apk="); r.dec(apk_tot as u64);
    r.txt(":"); r.dec(apk_perd as u64);
    r.txt(":"); r.dec(apk_hoy as u64);
    let usb_color = if apk_perd > 0 { C_FAULT }
                    else if kbd && kev > 0 { C_OK }
                    else if kbd && kev == 0 { C_FAULT }
                    else { C_WARN };
    splash_dashboard_log_color(total - 3, r.as_str(), usb_color);

    // Ultima fila -- el ENDPOINT del teclado segun el xHC. `ep` debe ser 1
    // (Running); `bi`->`iv` muestra el bInterval del descriptor y el exponente
    // que programamos de verdad (el bug del Interval se ve aqui de un vistazo).
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
    splash_dashboard_log_color(total - 2, r.as_str(), ep_color);

    // -- El RATON, con los tres numeros que reparten la culpa --
    //
    // Con una foto de esta linea se sabe cual de los tres es, y son problemas
    // en sitios muy distintos:
    //
    //   mev = 0                -> el HID no entrega NADA. Es el USB: endpoint,
    //                            ring o timbre. Ni el kernel ni el compositor.
    //   mev sube, x/y quietos  -> llegan informes pero los deltas salen cero:
    //                            el formato del informe no es el que leemos
    //                            (protocolo boot vs report, o un report ID
    //                            delante que corre todos los campos uno).
    //   x/y se mueven          -> el kernel lo tiene y el cursor no se pinta:
    //                            entonces es del compositor, y ahi si es
    //                            dibujo.
    let mut r = Buf::new();
    r.txt("raton ev="); r.dec(mev as u64);
    r.txt(" x="); if mx < 0 { r.txt("-"); } r.dec(mx.unsigned_abs() as u64);
    r.txt(" y="); if my < 0 { r.txt("-"); } r.dec(my.unsigned_abs() as u64);
    r.txt(" bot=0b"); r.hex(btn as u64, 2);
    // El SLOT, que es lo que destapo el bug: si el raton sale en el MISMO
    // slot que el teclado, no es un raton -- es la interfaz de medios del
    // teclado haciendose pasar por uno.
    r.txt(" slot="); r.dec(ms as u64);
    if ms != 0 && ms == ks { r.txt("(=kbd!)"); }
    // -- Las dos cosas que el reparto por fin deja ver --
    //
    // `bmb` = tiene cada uno una transferencia ENCOLADA? Un periferico que
    // deja de bombear queda enumerado, con el endpoint en `Running`, y mudo
    // para siempre -- nadie le vuelve a pedir nada. `k-` o `r-` aqui es
    // exactamente eso, y antes no se podia ver de ninguna forma.
    //
    // `hu` = Transfer Events que no eran de NADIE. Unos pocos al arrancar son
    // normales (restos de la enumeracion); si sube **mientras se teclea**, el
    // informe llega con una direccion distinta de la que creemos y por eso
    // nadie rearma.
    let (bomba_k, bomba_r, huerfanos) = crate::ring0::dev::usb::reparto_stats();
    r.txt(" bmb="); r.txt(if bomba_k { "k+" } else { "k-" });
    r.txt(if bomba_r { "r+" } else { "r-" });
    r.txt(" hu="); r.dec(huerfanos as u64);
    r.txt("  (ev=0 -> USB - ev sube y x/y quietos -> formato del informe)");
    let raton_color = if !mouse { C_DIM } else if mev > 0 { C_OK } else { C_FAULT };
    splash_dashboard_log_color(total - 1, r.as_str(), raton_color);

    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(saved_cr3); }
}

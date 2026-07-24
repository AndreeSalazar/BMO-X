//! CABINA — telemetría OMNISCIENTE del sistema (lado Ring 0).
//!
//! Le da VIDA a `cabina-core`: construye un `TelemetrySnapshot` desde los
//! contadores VIVOS del kernel y lo pinta como un cockpit constante en el
//! framebuffer. La visión del usuario: un observador que "ve todo" entre
//! Ring 0 y Ring 3 — para dejar de debuggear a ciegas.
//!
//! Consolida en una sola vista coherente lo que antes eran paneles sueltos
//! (heartbeat, usb): CPU, memoria, scheduler, Ring 3 y USB, siempre presentes.
//! A futuro: eventos con severidad/capa (cabina-core::Event) + el buffer de
//! shared-memory para que Ring 3 aporte su parte (protocolo SHM ya definido).

use cabina_core::TelemetrySnapshot;
use crate::ring0::core::splash::splash_dashboard_log;

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

// ── Formateo sin std (buffer de bytes) ──────────────────────────────────────

struct Buf {
    b: [u8; 80],
    o: usize,
}
impl Buf {
    fn new() -> Self { Self { b: [0u8; 80], o: 0 } }
    fn txt(&mut self, s: &str) {
        for &c in s.as_bytes() { if self.o < self.b.len() { self.b[self.o] = c; self.o += 1; } }
    }
    fn dec(&mut self, mut v: u64) {
        if v == 0 { self.txt("0"); return; }
        let mut tmp = [0u8; 20]; let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if self.o < self.b.len() { self.b[self.o] = tmp[i]; self.o += 1; } }
    }
    fn hex(&mut self, v: u64, digits: usize) {
        const H: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if self.o < self.b.len() { self.b[self.o] = H[((v >> (i * 4)) & 0xF) as usize]; self.o += 1; }
        }
    }
    fn as_str(&self) -> &str { core::str::from_utf8(&self.b[..self.o]).unwrap_or("") }
}

// ── Cockpit ─────────────────────────────────────────────────────────────────
// Filas fijas 9-12 del panel. Se repinta solo cuando algo cambia o tras un
// clear (anti-ghosting): CABINA es estable, no parpadea.

/// Pinta el cockpit omnisciente. Llamado always-on desde el loop del shell.
pub fn render_hud() {
    if !crate::info::has_fb() { return; }
    let s = snapshot();
    let (kbd, mouse, ks, ms, mev, _mx, _my, _btn, kev) = crate::ring0::dev::usb::hid_stats();
    let (tev, _rev, hev) = crate::ring0::dev::usb::xfer_stats();
    let (kdci, es, ee, ec) = crate::ring0::dev::usb::kbd_debug();
    let st = crate::ring0::scheduler::tid_state(2);
    let (rx, ln) = crate::ring0::uconsole::stats();
    let tid = crate::ring0::scheduler::current_tid();

    // Firma de cambio: ticks en bucket grueso (>>8) para no parpardear; el
    // resto son eventos reales. + generación de pantalla para repintar tras clear.
    static mut LAST: u64 = u64::MAX;
    static mut LAST_GEN: u32 = u32::MAX;
    let sig = (s.cpu.timer_ticks >> 8)
        ^ (s.scheduler.context_switches << 8)
        ^ ((s.memory.free_pages & 0xFFFF) << 16)
        ^ ((kev as u64) << 24) ^ ((tev as u64) << 28) ^ ((mev as u64) << 32)
        ^ ((st as u64) << 40) ^ ((kbd as u64) << 48) ^ ((mouse as u64) << 49)
        ^ ((s.scheduler.processes) << 50) ^ ((rx as u64) << 54);
    let gen = crate::ring0::core::phase::screen_gen();
    unsafe {
        if LAST == sig && LAST_GEN == gen { return; }
        LAST = sig; LAST_GEN = gen;
    }

    let mib_free = s.memory.free_pages / 256; // 4096 B/página → /256 = MiB

    // CABINA se pinta también desde el timer (que puede correr bajo la CR3 del
    // usuario, cuyo espacio no mapea el framebuffer): pintar bajo la CR3 del
    // kernel y restaurar. Mismo patrón que uconsole::flush.
    let saved_cr3 = crate::ring0::mm::vmm::read_cr3();
    let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(kpml4); }

    // Fila 9 — cabecera + CPU
    let mut r = Buf::new();
    r.txt("CABINA omnisciente  up tk=0x"); r.hex(s.cpu.timer_ticks, 6);
    r.txt("  ints="); r.dec(s.cpu.interrupts);
    splash_dashboard_log(9, r.as_str());

    // Fila 10 — memoria + scheduler
    let mut r = Buf::new();
    r.txt("mem free="); r.dec(mib_free); r.txt("MiB");
    r.txt("  sched sw="); r.dec(s.scheduler.context_switches);
    r.txt(" task="); r.dec(s.scheduler.processes);
    r.txt("/"); r.dec(s.scheduler.threads);
    r.txt(" tid="); r.dec(tid as u64);
    splash_dashboard_log(10, r.as_str());

    // Fila 11 — Ring 3
    let mut r = Buf::new();
    r.txt("ring3 st="); r.hex(st as u64, 2);
    r.txt(" rx="); r.dec(rx as u64);
    r.txt(" ln="); r.dec(ln as u64);
    r.txt("   (st: 01Ready 02Run 03Blk 04Exit FFdone)");
    splash_dashboard_log(11, r.as_str());

    // Fila 12 — USB (con el debug del teclado que seguimos cazando)
    let mut r = Buf::new();
    r.txt("usb k="); r.txt(if kbd { "OK" } else { "--" });
    r.txt("(s"); r.dec(ks as u64); r.txt(")");
    r.txt(" m="); r.txt(if mouse { "OK" } else { "--" });
    r.txt("(s"); r.dec(ms as u64); r.txt(")");
    r.txt(" kev="); r.dec(kev as u64);
    r.txt(" tev="); r.dec(tev as u64);
    r.txt(" hev="); r.dec(hev as u64);
    r.txt(" dci="); r.dec(kdci as u64);
    r.txt(" lev="); r.dec(es as u64); r.txt(":"); r.dec(ee as u64); r.txt(":"); r.dec(ec as u64);
    splash_dashboard_log(12, r.as_str());

    if saved_cr3 != kpml4 { crate::ring0::mm::vmm::switch_to(saved_cr3); }
}

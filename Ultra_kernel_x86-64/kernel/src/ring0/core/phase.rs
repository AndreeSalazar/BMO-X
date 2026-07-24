//! Ring 0 boot phases - orchestrator for the kernel entry path.
//!
//! In Ultra_kernel_x86-64's minimal Ring 0 base we keep only what's necessary:
//! the splash animation, the framebuffer init, and a serial shell.
//! All GDT/IDT/CPU/MM/IRQ/SMP subsystems live in the faggin stages
//! (s2_gdt, s3_idt, s4_cpuid, s5_control, s9_paging) and are already
//! configured by the time the kernel runs.
//!
//! Phases:
//!   0. fb    - framebuffer init from BootContext
//!   1. ui    - splash animation (if FB available)
//!
//! After phases: serial shell takes over so the user has a way to
//! interact even without a display.

use boot_context::BootContext;
use super::splash;

fn s_log(msg: &str) {
    crate::ring0::dev::console::serial_write(msg);
    crate::ring0::dev::console::serial_write("\n");
    // Mirror to the on-screen log panel (if framebuffer present).
    if crate::info::has_fb() {
        let row = unsafe { DASH_LOG_ROW };
        unsafe { DASH_LOG_ROW = (row + 1) % 14; }
        splash::splash_dashboard_log(row, msg);
    }
}

fn phase0_fb(ctx: &BootContext) {
    s_log("[phase0] === Framebuffer Init ===");
    let fmt = match ctx.fb_pixel_format {
        0 => crate::ring0::dev::framebuffer::PixelFormat::Bgr,
        1 => crate::ring0::dev::framebuffer::PixelFormat::Rgb,
        _ => crate::ring0::dev::framebuffer::PixelFormat::Unknown,
    };
    crate::ring0::dev::framebuffer::init_gop(
        ctx.fb_addr,
        ctx.fb_width,
        ctx.fb_height,
        ctx.fb_stride,
        fmt,
    );
    s_log("[phase0] done");
}

fn phase1_ui(_ctx: &BootContext) {
    s_log("[phase1] === UI (dashboard) ===");
    if crate::info::has_fb() {
        // Land on the persistent dashboard after the cinematic intro.
        splash::splash_dashboard_init();
    } else {
        s_log("[splash] no framebuffer, skipping");
    }
    s_log("[phase1] done");
}

// ---------------------------------------------------------------------------
// Serial shell (with optional framebuffer echo)
// ---------------------------------------------------------------------------

// Rolling index into the dashboard log. Each `dash_log` call
// advances this and wraps at DASH_LOG_LINES.
static mut DASH_LOG_ROW: usize = 0;

// Mirror the serial output to a line in the dashboard's log
// area, so the user can see what the kernel is doing without a
// serial terminal attached.
fn dash_log(msg: &str) {
    dashboard_log(msg);
}

/// Append one line to the rolling on-screen kernel log. Public so other
/// subsystems (e.g. the Ring 3 bootstrap console in `uconsole`) can surface
/// output in the same panel instead of maintaining a competing row cursor.
/// Framebuffer-only; a no-op on a headless (serial) boot.
pub fn dashboard_log(msg: &str) {
    if !crate::info::has_fb() { return; }
    let row = unsafe { DASH_LOG_ROW };
    unsafe { DASH_LOG_ROW = (row + 1) % 14; }
    splash::splash_dashboard_log(row, msg);
}

// Mirror the current in-progress shell line to the framebuffer's prompt area,
// con CURSOR PARPADEANTE. Antes se repintaba en CADA iteración del loop del
// shell (limpiar+dibujar sin cambio) → ese era el ghosting ocasional del
// prompt. Ahora solo repinta cuando: cambia la línea, parpadea el cursor, o
// hubo un clear. Pantalla estable + cursor vivo.
fn dash_prompt(line: &str) {
    if !crate::info::has_fb() { return; }
    let ticks = crate::ring0::timer::ticks();
    let blink = ((ticks >> 6) & 1) == 0; // visible ~mitad del tiempo
    let n = line.len();
    static mut LAST_N: usize = usize::MAX;
    static mut LAST_BLINK: bool = false;
    static mut LAST_GEN: u32 = u32::MAX;
    unsafe {
        let gen = SCREEN_GEN;
        if LAST_N == n && LAST_BLINK == blink && LAST_GEN == gen { return; }
        LAST_N = n; LAST_BLINK = blink; LAST_GEN = gen;
    }
    splash::splash_dashboard_prompt(line, blink);
}

fn shell_prompt() {
    crate::ring0::dev::console::serial_write("> ");
    dash_prompt("");
}

/// Generación de pantalla: se incrementa en cada limpieza. Los paneles de fila
/// FIJA (heartbeat, usb) la comparan para FORZAR un repintado tras un clear,
/// aunque sus valores no hayan cambiado — si no, la detección de cambios los
/// dejaría en blanco para siempre después de limpiar (bug real observado).
static mut SCREEN_GEN: u32 = 0;

/// Limpia la pantalla y re-dibuja el dashboard vacío (comando `cls` y
/// auto-limpieza al terminar un proceso). Reinicia el cursor rodante del log
/// para que el panel arranque de cero, como una terminal recién abierta.
pub(crate) fn clear_screen() {
    if !crate::info::has_fb() { return; }
    splash::splash_clear();
    splash::splash_dashboard_init();
    unsafe {
        DASH_LOG_ROW = 0;
        SCREEN_GEN = SCREEN_GEN.wrapping_add(1); // fuerza repintado de paneles fijos
    }
}

/// Live Ring 3 heartbeat on FIXED row 10, repainted by the shell's poll loop
/// whenever a value changes. This is the always-on view the post-mortem
/// fault reporter cannot give when nothing faults:
///   tk = timer ticks (counting ⇒ timer alive)
///   sw = switches into CPL3 (nonzero ⇒ scheduler entered the user task)
///   st = init task (tid 2) state: 01 Ready, 02 Running, 03 Blocked,
///        04 Exited, FF reaped/absent (FF after sw>0 ⇒ ran and finished)
///   rx/ln = CONSOLE_WRITE words / lines received from Ring 3
pub(crate) fn dash_heartbeat() {
    if !crate::info::has_fb() {
        return;
    }
    let ticks = crate::ring0::timer::ticks();
    let sw = crate::ring0::scheduler::user_switches();
    let st = crate::ring0::scheduler::tid_state(2);
    let (rx, ln) = crate::ring0::uconsole::stats();
    static mut LAST: [u64; 4] = [u64::MAX; 4];
    static mut LAST_GEN: u32 = u32::MAX;
    // ANTI-GHOSTING (monitor 74 Hz): el disparo de repintado usaba `ticks`
    // crudo → se repintaba en CADA pulso del timer, y como el clear+draw no
    // está sincronizado con el refresco, se veía parpadeo/estela. Ahora `tk`
    // solo dispara cada 256 ticks (bucket) → la pantalla queda ESTABLE y solo
    // se repinta cuando pasa algo REAL (sw/st/rx/ln) o de tanto en tanto.
    // El VALOR mostrado sigue siendo el tk completo (abajo).
    const TK_BUCKET_SHIFT: u32 = 8;
    let cur = [ticks >> TK_BUCKET_SHIFT, sw, st as u64, (ln << 32) | (rx & 0xFFFF_FFFF)];
    unsafe {
        let gen = SCREEN_GEN;
        let last = &mut *core::ptr::addr_of_mut!(LAST);
        // Repintar si algo cambió O si hubo un clear (gen distinta).
        if *last == cur && LAST_GEN == gen {
            return;
        }
        *last = cur;
        LAST_GEN = gen;
    }
    fn hx(b: &mut [u8; 56], o: &mut usize, v: u64, digits: usize) {
        const HEX: &[u8; 16] = b"0123456789ABCDEF";
        for i in (0..digits).rev() {
            if *o < b.len() {
                b[*o] = HEX[((v >> (i * 4)) & 0xF) as usize];
                *o += 1;
            }
        }
    }
    fn txt(b: &mut [u8; 56], o: &mut usize, s: &str) {
        for &c in s.as_bytes() {
            if *o < b.len() {
                b[*o] = c;
                *o += 1;
            }
        }
    }
    let mut b = [0u8; 56];
    let mut o = 0;
    txt(&mut b, &mut o, "r3hb tk=");
    hx(&mut b, &mut o, ticks, 6);
    txt(&mut b, &mut o, " sw=");
    hx(&mut b, &mut o, sw, 4);
    txt(&mut b, &mut o, " st=");
    hx(&mut b, &mut o, st as u64, 2);
    txt(&mut b, &mut o, " rx=");
    hx(&mut b, &mut o, rx, 4);
    txt(&mut b, &mut o, " ln=");
    hx(&mut b, &mut o, ln, 2);
    if let Ok(s) = core::str::from_utf8(&b[..o]) {
        // The timer may call this while the USER CR3 is loaded (its address
        // space does not map the framebuffer's identity range) — paint under
        // the kernel CR3 and restore. See uconsole::flush for the full why.
        let cur = crate::ring0::mm::vmm::read_cr3();
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(kpml4);
        }
        splash::splash_dashboard_log(10, s);
        if cur != kpml4 {
            crate::ring0::mm::vmm::switch_to(cur);
        }
    }
}

/// Panel USB DETALLADO en fila fija 12 (sobrevive al auto-clear porque se
/// repinta en cada iteración del shell). Muestra teclado/mouse por separado y
/// telemetría VIVA del mouse — mueve el mouse y verás mev/x/y/b cambiar, aunque
/// el teclado aún no escriba. Pedido del usuario: "llamar al mouse, más
/// detallado total". Throttled por change-detection para no parpadear.
pub(crate) fn dash_usb_status() {
    if !crate::info::has_fb() { return; }
    let (kbd, mouse, ks, ms, mev, mx, my, btn, kev) = crate::ring0::dev::usb::hid_stats();
    let (tev, rev, hev) = crate::ring0::dev::usb::xfer_stats();
    static mut LAST: u64 = u64::MAX;
    static mut LAST_GEN: u32 = u32::MAX;
    // Firma de cambio: agrupa lo relevante. mx/my se truncan a 12 bits para que
    // micro-jitter del mouse no dispare repintados (anti-ghosting).
    let sig = (kbd as u64) | ((mouse as u64) << 1)
        | ((mev as u64 & 0xFF) << 8)
        | ((kev as u64 & 0xFF) << 16)
        | ((tev as u64 & 0xFFFF) << 24)
        | ((hev as u64 & 0xFF) << 40)
        | ((btn as u64) << 48)
        | (((mx as u64) & 0x3F) << 56);
    unsafe {
        let gen = SCREEN_GEN;
        // Repintar si algo cambió O si hubo un clear (gen distinta) que borró la fila.
        if LAST == sig && LAST_GEN == gen { return; }
        LAST = sig;
        LAST_GEN = gen;
    }
    let _ = rev;

    fn txt(b: &mut [u8; 80], o: &mut usize, s: &str) {
        for &c in s.as_bytes() { if *o < b.len() { b[*o] = c; *o += 1; } }
    }
    fn dec(b: &mut [u8; 80], o: &mut usize, mut v: u32) {
        if v == 0 { if *o < b.len() { b[*o] = b'0'; *o += 1; } return; }
        let mut tmp = [0u8; 10]; let mut i = 0;
        while v > 0 { tmp[i] = b'0' + (v % 10) as u8; v /= 10; i += 1; }
        while i > 0 { i -= 1; if *o < b.len() { b[*o] = tmp[i]; *o += 1; } }
    }
    fn sdec(b: &mut [u8; 80], o: &mut usize, v: i32) {
        if v < 0 { if *o < b.len() { b[*o] = b'-'; *o += 1; } dec(b, o, (-v) as u32); }
        else { if *o < b.len() { b[*o] = b'+'; *o += 1; } dec(b, o, v as u32); }
    }

    let _ = (mx, my, btn); // (x/y/botones del mouse: cuando enumere, línea aparte)
    let mut b = [0u8; 80];
    let mut o = 0;
    txt(&mut b, &mut o, "usb k=");
    txt(&mut b, &mut o, if kbd { "OK" } else { "--" });
    txt(&mut b, &mut o, "(s"); dec(&mut b, &mut o, ks as u32); txt(&mut b, &mut o, ")");
    txt(&mut b, &mut o, " m=");
    txt(&mut b, &mut o, if mouse { "OK" } else { "--" });
    txt(&mut b, &mut o, "(s"); dec(&mut b, &mut o, ms as u32); txt(&mut b, &mut o, ")");
    txt(&mut b, &mut o, " mev="); dec(&mut b, &mut o, mev);
    txt(&mut b, &mut o, " kev="); dec(&mut b, &mut o, kev);
    // El corte del teclado se lee aqui:
    txt(&mut b, &mut o, " tev="); dec(&mut b, &mut o, tev);
    txt(&mut b, &mut o, " hev="); dec(&mut b, &mut o, hev);
    // dci del kbd vs (slot:ep:cc) del ultimo Transfer Event. Si ep != dci,
    // el evento no matchea al teclado y no se re-encola => tev pegado.
    let (kdci, es, ee, ec) = crate::ring0::dev::usb::kbd_debug();
    txt(&mut b, &mut o, " dci="); dec(&mut b, &mut o, kdci as u32);
    txt(&mut b, &mut o, " lev="); dec(&mut b, &mut o, es as u32);
    txt(&mut b, &mut o, ":"); dec(&mut b, &mut o, ee as u32);
    txt(&mut b, &mut o, ":"); dec(&mut b, &mut o, ec as u32);

    if let Ok(s) = core::str::from_utf8(&b[..o]) {
        let cur = crate::ring0::mm::vmm::read_cr3();
        let kpml4 = crate::ring0::mm::vmm::kernel_pml4();
        if cur != kpml4 { crate::ring0::mm::vmm::switch_to(kpml4); }
        splash::splash_dashboard_log(12, s);
        if cur != kpml4 { crate::ring0::mm::vmm::switch_to(cur); }
    }
}

/// Último total de tareas visto por el shell, para detectar cuándo un proceso
/// TERMINÓ (el total baja) y limpiar la pantalla automáticamente.
static mut LAST_TASK_TOTAL: usize = 0;

fn shell_read_line(buf: &mut [u8]) -> usize {
    let mut n = 0;
    loop {
        // Auto-limpieza: si un proceso terminó (el total de tareas bajó) y NO
        // estás escribiendo (línea vacía), limpia la pantalla — como una
        // terminal que se refresca al acabar el programa. Nunca borra a media
        // escritura (solo con n==0).
        let (total, _) = crate::ring0::scheduler::counts();
        unsafe {
            if n == 0 && total < LAST_TASK_TOTAL {
                clear_screen();
                dash_log("== proceso terminado : pantalla limpia ==");
            }
            LAST_TASK_TOTAL = total;
        }
        // Update the framebuffer's prompt with the current line
        // (so the screen shows what the user is typing).
        dash_prompt(core::str::from_utf8(&buf[..n]).unwrap_or(""));
        // Live Ring 3 heartbeat (row 10): timer/scheduler/console telemetry.
        dash_heartbeat();
        // Panel USB detallado (row 12): teclado/mouse + telemetría viva del mouse.
        dash_usb_status();
        // Accept input from EITHER the serial line (COM1) or the physical
        // PS/2 keyboard, whichever has a byte ready. Lets the user type on
        // the real keyboard even with no serial cable attached.
        let mut byte = crate::ring0::dev::console::serial_read_byte();
        // USB HID keyboard (xHCI) — the real input path on this board.
        if byte.is_none() {
            byte = crate::ring0::dev::usb::poll_ascii();
        }
        if byte.is_none() {
            if let Some((raw, ascii)) = crate::ring0::dev::keyboard::poll_event() {
                // Raw-scancode monitor: every keyboard byte is surfaced in
                // the on-screen log ("kbd 0xXX"). Bytes appearing at all
                // proves the legacy i8042 stream is alive post-EBS; the
                // values reveal which scancode set the firmware delivers.
                let mut m = *b"kbd 0x00";
                const HEX: &[u8; 16] = b"0123456789ABCDEF";
                m[6] = HEX[(raw >> 4) as usize];
                m[7] = HEX[(raw & 0xF) as usize];
                dash_log(core::str::from_utf8(&m).unwrap_or("kbd ??"));
                byte = ascii;
            }
        }
        match byte {
            Some(b'\r') | Some(b'\n') => {
                crate::ring0::dev::console::serial_write("\n");
                return n;
            }
            Some(0x7f) | Some(0x08) => {
                if n > 0 {
                    n -= 1;
                    crate::ring0::dev::console::serial_write("\x08 \x08");
                    // Reflect the edit on screen immediately.
                    dash_prompt(core::str::from_utf8(&buf[..n]).unwrap_or(""));
                }
            }
            Some(c) if c >= 0x20 && c < 0x7f => {
                if n < buf.len() {
                    buf[n] = c;
                    n += 1;
                    crate::ring0::dev::console::serial_write_byte(c);
                }
            }
            _ => {}
        }
    }
}

fn shell_help() {
    // Compacto por categorías: cabe entero en el panel de 14 filas sin
    // barrer el resto del log, y se lee de un vistazo.
    s_log("== BMO-X shell ==");
    s_log(" sistema : info  mem  tasks");
    s_log(" video   : fb  splash  cls");
    s_log(" ring3   : bex  ktest");
    s_log(" poder   : reboot  halt  panic");
    s_log(" ayuda   : help");
}

fn shell_info(ctx: &BootContext) {
    s_log("--- BootContext ---");
    s_log("magic          = FOSCBOOT");
    crate::ring0::dev::console::serial_write("version         = ");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("fb_addr         = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_addr, 16);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("fb_w x fb_h     = ");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_width as u64, 10);
    crate::ring0::dev::console::serial_write(" x ");
    crate::ring0::dev::console::serial_write_u64(ctx.fb_height as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("mem map entries = ");
    crate::ring0::dev::console::serial_write_u64(ctx.memory_map_count as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("pml4            = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.pml4, 16);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::dev::console::serial_write("tsc_freq        = ");
    crate::ring0::dev::console::serial_write_u64_dec(ctx.tsc_freq);
    crate::ring0::dev::console::serial_write(" Hz\n");
    crate::ring0::dev::console::serial_write("rsdp            = 0x");
    crate::ring0::dev::console::serial_write_u64(ctx.rsdp, 16);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_fb() {
    if !crate::info::has_fb() {
        s_log("[fb] no framebuffer (headless boot)");
        return;
    }
    crate::ring0::dev::console::serial_write("[fb] base=0x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_ADDR }, 16);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_WIDTH } as u64, 10);
    crate::ring0::dev::console::serial_write("x");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_HEIGHT } as u64, 10);
    crate::ring0::dev::console::serial_write("x32 stride=");
    crate::ring0::dev::console::serial_write_u64(unsafe { crate::info::FB_STRIDE } as u64, 10);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_tasks() {
    let (total, runnable) = crate::ring0::scheduler::counts();
    crate::ring0::dev::console::serial_write("[tasks] total=");
    crate::ring0::dev::console::serial_write_u64(total as u64, 10);
    crate::ring0::dev::console::serial_write(" runnable=");
    crate::ring0::dev::console::serial_write_u64(runnable as u64, 10);
    crate::ring0::dev::console::serial_write(" current_tid=");
    crate::ring0::dev::console::serial_write_u64(
        crate::ring0::scheduler::current_tid() as u64,
        10,
    );
    crate::ring0::dev::console::serial_write(" ticks=");
    crate::ring0::dev::console::serial_write_u64(crate::ring0::timer::ticks(), 10);
    crate::ring0::dev::console::serial_write("\n");
}

fn shell_splash() {
    if !crate::info::has_fb() {
        s_log("[splash] no framebuffer");
        return;
    }
    splash::splash_init();
    splash::splash_progress(50, "Shell re-triggered splash");
    // Return to the persistent dashboard instead of clearing to black.
    splash::splash_dashboard_init();
    s_log("[splash] done");
}

fn shell_bex() {
    s_log("[bex] BEX v1 / BEF1 x86-64 admission is enabled");
    s_log("[bex] next: storage input, Ring 3 pages, then iretq entry");
}

/// F1 demo task: runs preempted by the timer, parks on a WAIT deadline,
/// wakes, and exits through the reaper. Watch the interleaving with the
/// shell prompt on serial — that interleaving IS the context switch.
extern "C" fn ktest_main(arg: u64) -> ! {
    use crate::ring0::dev::console::{serial_write, serial_write_u64};
    serial_write("[ktest] start tid=");
    serial_write_u64(crate::ring0::scheduler::current_tid() as u64, 10);
    serial_write(" arg=");
    serial_write_u64(arg, 10);
    serial_write("\n");
    for i in 0..3u64 {
        serial_write("[ktest] window ");
        serial_write_u64(i, 10);
        serial_write("\n");
        // Busy window ~250 ms so the timer preempts us several times and
        // the shell task runs in between (look for the '>' echoes).
        let start = crate::ring0::scheduler::rdtsc();
        let span = crate::ring0::scheduler::tsc_freq() / 4;
        while crate::ring0::scheduler::rdtsc().wrapping_sub(start) < span {
            core::hint::spin_loop();
        }
    }
    serial_write("[ktest] park 2000 ms (WAIT deadline)\n");
    let deadline = crate::ring0::scheduler::rdtsc()
        + crate::ring0::scheduler::ns_to_tsc(2_000_000_000);
    crate::ring0::scheduler::park_until(deadline);
    serial_write("[ktest] woke; exit via reaper\n");
    crate::ring0::scheduler::exit_and_park();
}

fn shell_ktest() {
    match crate::ring0::scheduler::spawn_kernel(ktest_main as usize as u64, 0xB0, 1) {
        Some(tid) => {
            crate::ring0::dev::console::serial_write("[ktest] spawned tid=");
            crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
            crate::ring0::dev::console::serial_write("\n");
        }
        None => s_log("[ktest] spawn failed (no frames or task slots)"),
    }
}

fn shell_mem() {
    let (total, free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[mem] frames free=");
    crate::ring0::dev::console::serial_write_u64(free, 10);
    crate::ring0::dev::console::serial_write(" total=");
    crate::ring0::dev::console::serial_write_u64(total, 10);
    crate::ring0::dev::console::serial_write(" (");
    crate::ring0::dev::console::serial_write_u64(free * 4096 / (1024 * 1024), 10);
    crate::ring0::dev::console::serial_write(" MiB free)\n");
    dash_log("[mem] stats printed on serial");
    if crate::ring0::mm::vmm::self_test() {
        s_log("[mem] vmm selftest OK (alloc/map/translate/unmap/destroy)");
    } else {
        s_log("[mem] vmm selftest FAILED");
    }
}

fn shell_panic() -> ! {
    s_log("[shell] triggering test panic...");
    panic!("intentional panic from serial shell");
}

fn shell_reboot() -> ! {
    s_log("[shell] reboot (keyboard reset pulse)");
    unsafe { core::arch::asm!("out 0x64, al", in("al") 0xFEu8); }
    loop { unsafe { core::arch::asm!("hlt"); } }
}

fn shell_halt() -> ! {
    s_log("[shell] halting");
    loop { unsafe { core::arch::asm!("sti; hlt"); } }
}

fn run_shell(ctx: &BootContext) -> ! {
    // Normalize the i8042 (translation → Set 1, re-enable scanning) so the
    // physical keyboard reaches shell_read_line. No-op if the controller is
    // dead/absent (bounded timeouts inside). El stack USB real (xHCI+HID)
    // ya despertó en el Acto I de main().
    crate::ring0::dev::keyboard::init();
    dash_log("== BMO-X operativo : escribe help ==");
    // Serial-only banner: keep the rolling dashboard rows untouched so the
    // fixed-row Ring 3 diagnostics painted just before timer::enable survive.
    crate::ring0::dev::console::serial_write("\n=== BMO-X Ring 0 shell (type 'help') ===\n");
    shell_prompt();

    let mut buf = [0u8; 64];
    loop {
        let n = shell_read_line(&mut buf);
        if n == 0 { shell_prompt(); continue; }

        let cmd = &buf[..n];

        if cmd == b"help" {
            shell_help();
        } else if cmd == b"cls" || cmd == b"clear" {
            clear_screen();
        } else if cmd == b"info" {
            shell_info(ctx);
        } else if cmd == b"tasks" {
            shell_tasks();
        } else if cmd == b"mem" {
            shell_mem();
        } else if cmd == b"ktest" {
            shell_ktest();
        } else if cmd == b"fb" {
            shell_fb();
        } else if cmd == b"splash" {
            shell_splash();
        } else if cmd == b"bex" {
            shell_bex();
        } else if cmd == b"panic" {
            shell_panic();
        } else if cmd == b"reboot" {
            shell_reboot();
        } else if cmd == b"halt" {
            shell_halt();
        } else {
            s_log("unknown command (try 'help')");
        }
        shell_prompt();
    }
}

/// Public entry: called from `entry::kernel_main_real` after the
/// naked `_start` BSS zero.
pub fn main(ctx: &mut BootContext) {
    // Boot bisected cleanly on real hardware; the visual progress markers
    // are retired. `kbar!` is now a no-op so the call sites can stay as
    // documentation of the init order without painting over the UI. (Their
    // spirit lives on in the planned per-module status/version registry.)
    macro_rules! kbar { ($y:expr, $c:expr) => {{ let _ = ($y, $c); }}; }

    kbar!(90, 0xFF00_FF00u32); // green @90: past the magenta paint, before s_log
    s_log("[ring0] validating BootContext");
    if !ctx.is_valid() {
        // Make an invalid BootContext VISIBLE (red @90) instead of a silent
        // halt — otherwise a magic mismatch looks identical to a hang.
        kbar!(90, 0xFFFF_0000u32);
        s_log("[ring0] FATAL: BootContext magic mismatch");
        loop { unsafe { core::arch::asm!("hlt"); } }
    }
    kbar!(110, 0xFFFF_FFFFu32); // white @110: BootContext valid, before percpu

    crate::ring0::dev::console::serial_write("[ring0] BootContext OK, version=");
    crate::ring0::dev::console::serial_write_u64(ctx.version as u64, 10);
    crate::ring0::dev::console::serial_write("\n");

    // Kernel-init checkpoints live in the empty band starting at row 140
    // (well below the boot bars that end near row 120), so any new bar is
    // unmistakably kernel progress — not a repeat of an s1/s2 color.
    crate::ring0::percpu::init_bsp();
    kbar!(140, 0xFF00_FF00u32); // green: percpu OK
    crate::ring0::scheduler::init(ctx.tsc_freq);
    kbar!(152, 0xFFFF_FFFFu32); // white: scheduler OK
    crate::ring0::mm::phys::init(ctx);
    kbar!(164, 0xFFFF_0000u32); // red: phys::init OK
    crate::ring0::mm::vmm::init();
    kbar!(176, 0xFF00_FFFFu32); // aqua: vmm::init OK
    let (frames_total, frames_free) = crate::ring0::mm::phys::stats();
    crate::ring0::dev::console::serial_write("[ring0] mm ready: frames free=");
    crate::ring0::dev::console::serial_write_u64(frames_free, 10);
    crate::ring0::dev::console::serial_write("/");
    crate::ring0::dev::console::serial_write_u64(frames_total, 10);
    crate::ring0::dev::console::serial_write("\n");
    crate::ring0::channel::init(ctx);
    crate::ring0::svc::register_all();
    crate::ring0::syscall::init();
    // Arm the on-screen fault reporter before anything can enter Ring 3, so a
    // CPL3 crash paints its vector/RIP/CR2 instead of the silent serial-halt
    // the boot stage installs.
    crate::ring0::faults::init(ctx);
    let timer_ready = crate::ring0::timer::init(ctx);
    if timer_ready {
        s_log("[ring0] scheduler + BMO Channel + SYSCALL + LAPIC tick ready (BSP)");
    } else {
        s_log("[ring0] WARNING: LAPIC tick unavailable; scheduler remains cooperative");
    }
    kbar!(188, 0xFFFF_FF00u32); // yellow: channel/svc/syscall/timer OK

    // Populate the active BMO CPU profile (today: Ryzen 5 5600X).
    // Identity, SMT/CCX topology, cache hierarchy, TSC calibration and
    // errata/speculation mitigations all live behind the profile —
    // changing CPU or vendor is a profile swap, never a kernel edit.
    let cpu_profile = crate::ring0::cpu_vendor::active();
    (cpu_profile.init)();
    kbar!(200, 0xFFFF_8800u32); // orange: CPU profile + errata (MSR) OK
    crate::ring0::dev::console::serial_write("[cpu] profile: ");
    crate::ring0::dev::console::serial_write(cpu_profile.vendor);
    crate::ring0::dev::console::serial_write(" ");
    crate::ring0::dev::console::serial_write(cpu_profile.name);
    crate::ring0::dev::console::serial_write("\n");

    // BEX is the only native executable contract admitted by this kernel.
    // The parser is allocation-free so it is safe before the process allocator.
    crate::ring0::bex::announce();
    kbar!(212, 0xFF00_FFFFu32); // aqua: bex announce OK, before proc::spawn_init

    // F2: if the boot chain reserved a Ring 3 payload, admit it as the
    // init process. With no payload this is a no-op and the boot flow is
    // exactly the Ring 0 shell as before.
    crate::ring0::proc::init(ctx);
    let ring3_tid = crate::ring0::proc::spawn_init(ctx);
    if let Some(tid) = ring3_tid {
        crate::ring0::dev::console::serial_write("[ring0] Ring 3 init task ready, tid=");
        crate::ring0::dev::console::serial_write_u64(tid as u64, 10);
        crate::ring0::dev::console::serial_write("\n");
    }
    kbar!(224, 0xFFFF_FFFFu32); // white: proc init + Ring 3 spawn OK, before splash

    // CPU identity detection (CPUID leaf 0, 1, 0x80000002-04)
    let cpu = crate::ring0::cpu::detect_cpu();
    let cpu_line = match cpu.vendor {
        crate::ring0::cpu::CpuVendor::Amd => "AMD",
        crate::ring0::cpu::CpuVendor::Intel => "Intel",
        crate::ring0::cpu::CpuVendor::Unknown => "Unknown",
    };
    let brand = cpu.brand.as_str();
    // Use a stack buffer to build the log line, then emit to both
    // serial and the framebuffer dashboard.
    let mut line = [0u8; 96];
    let prefix = b"[cpu] ";
    let mid1   = b" | ";
    let mid2   = b" | cores=";
    let mut off = 0;
    for &b in prefix { line[off] = b; off += 1; }
    for &b in brand.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid1 { if off < line.len() { line[off] = b; off += 1; } }
    for &b in cpu_line.as_bytes() { if off < line.len() { line[off] = b; off += 1; } }
    for &b in mid2 { if off < line.len() { line[off] = b; off += 1; } }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 / 10); off += 1; }
    if off < line.len() { line[off] = b'0' + (cpu.logical_cores as u8 % 10); off += 1; }
    if let Ok(s) = core::str::from_utf8(&line[..off]) {
        s_log(s);
    }

    // Populate FB globals from the context, then bring up the fb driver.
    crate::info::init_from(ctx);
    phase0_fb(ctx);

    // ── Intro cinemática (logo → preparando → RING 0 → RING 3) ──────────
    // Escenas centradas con fundido y transición, al estilo de un arranque
    // moderno. Al terminar aterrizamos en el dashboard, donde el trabajo
    // REAL de cada etapa fluye como log (igual que Windows: la animación
    // juega, luego apareces en el escritorio).
    if crate::info::has_fb() {
        splash::boot_intro();
    } else {
        s_log("[splash] no framebuffer, skipping splash");
    }

    // Aterrizar en el dashboard persistente.
    phase1_ui(ctx);

    // ── Acto I: RING 0 despierta el hardware (log real) ─────────────────
    // Los encabezados "==" se pintan en cyan (dash_line_color).
    dash_log("== RING 0 : despertando hardware ==");
    s_log("[ring0] cpu Zen 3 perfilado + GDT/IDT propias");
    {
        let (total, free) = crate::ring0::mm::phys::stats();
        let mut b = [0u8; 48];
        let mut o = 0;
        for &c in b"[ring0] mem ".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let gib = (total * 4096) >> 30;
        let mut tmp = [0u8; 4];
        let mut t = 0;
        let mut v = gib.max(1);
        while v > 0 && t < 4 { tmp[t] = b'0' + (v % 10) as u8; v /= 10; t += 1; }
        while t > 0 { t -= 1; if o < b.len() { b[o] = tmp[t]; o += 1; } }
        for &c in b" GiB physmap listos".iter() { if o < b.len() { b[o] = c; o += 1; } }
        let _ = free;
        if let Ok(s) = core::str::from_utf8(&b[..o]) { s_log(s); }
    }
    s_log("[ring0] scheduler preemptivo + capabilities armados");
    // USB en su lugar narrativo: el kernel despierta teclado y mouse AQUI.
    crate::ring0::dev::usb::init(ctx);
    dash_log("== RING 0 : hardware al mando ==");

    // ── Acto II: RING 3 — el userspace nace ─────────────────────────────
    dash_log("== RING 3 : userspace ==");
    // Surface the Ring 3 init outcome on the (now cleared) dashboard so the
    // demo's state is visible without serial. If a tid was admitted, the next
    // timer tick will enter CPL3 and its 'ring3>' lines should follow below.
    {
        let mut summary = [0u8; 64];
        let head = b"[ring3] ";
        let status = crate::ring0::proc::init_status();
        let mut off = 0;
        for &b in head { if off < summary.len() { summary[off] = b; off += 1; } }
        for &b in status.as_bytes() { if off < summary.len() { summary[off] = b; off += 1; } }
        if let Some(tid) = ring3_tid {
            for &b in b" tid=" { if off < summary.len() { summary[off] = b; off += 1; } }
            if off < summary.len() { summary[off] = b'0' + (tid as u8 % 10); off += 1; }
        }
        if let Ok(s) = core::str::from_utf8(&summary[..off]) { s_log(s); }
    }

    // FINAL checkpoint: bright green at row 236 = kernel finished ALL of
    // phase::main and is entering the shell. If this shows, Ring 0 fully
    // booted on real hardware.
    kbar!(236, 0xFF00_FF00u32);

    if timer_ready {
        crate::ring0::timer::enable();
    }

    run_shell(ctx);
}

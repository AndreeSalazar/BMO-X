//! Panic handler — minimal, no-allocation, no-dependency.
//!
//! v1.8.9: rewrite defensivo. El handler anterior llamaba a
//! `cabina::panic_msg` y `cabina::paint_overlay`, los cuales
//! internamente alocan `String` en el heap. Si el panic ocurre en
//! un estado donde el heap está corrupto o no inicializado, el
//! handler mismo cuelga sin output.
//!
//! v1.8.9: el handler es ahora **100% serial-direct**. Sin
//! allocations, sin globals de cabina, sin framebuffer. Solo escribe
//! "!!! KERNEL PANIC !!!" a COM1 (0x3F8) y hace `cli; hlt` en loop.
//! Funciona en cualquier estado del kernel — incluso con heap roto,
//! incluso antes de init_heap().

use core::arch::asm;
use core::fmt::{self, Display, Write as FmtWrite};
use core::panic::PanicInfo;

// ── COM1 (16550 UART) register ports ───────────────────────────────

const COM1_DATA: u16 = 0x3F8;
const COM1_LSR: u16 = 0x3FD;
const LSR_THRE: u8 = 0x20;

// ── fmt::Write adapter ────────────────────────────────────────────

/// Adapter no-alocante que escribe a un slice de bytes. Implementa
/// `core::fmt::Write` para que pueda usarse con `write!` /
/// `format_args!`.
struct SliceWriter<'a> {
    buf: &'a mut [u8],
    pos: usize,
}

impl SliceWriter<'_> {
    fn written(&self) -> usize { self.pos }
}

impl fmt::Write for SliceWriter<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let bytes = s.as_bytes();
        let remaining = self.buf.len() - self.pos;
        let to_copy = bytes.len().min(remaining);
        self.buf[self.pos..self.pos + to_copy].copy_from_slice(&bytes[..to_copy]);
        self.pos += to_copy;
        Ok(())
    }
}

// ── Display wrapper for PanicMessage ──────────────────────────────

/// Wrapper que implementa `Display` para `core::panic::PanicMessage<'_>`.
/// Permite usar `write!(w, "{}", PanicMsgWrap(m))` con la API estable
/// de Rust (sin `Arguments::new_v1` que solo existe inestable).
///
/// `PanicMessage<'_>` no es `Display` directamente (en versiones estables
/// solo se puede pasar a `set_hook`), así que lo envolvemos para que
/// `write!` lo formatee via `Display`.
struct PanicMsgWrap<'a>(core::panic::PanicMessage<'a>);

impl Display for PanicMsgWrap<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

// ── Serial helpers (locales, no dependen de cabina) ───────────────

#[inline]
fn write_byte(b: u8) {
    unsafe {
        // Espera a que el transmisor esté listo (THRE=1).
        loop {
            let lsr: u8;
            asm!("in al, dx", out("al") lsr, in("dx") COM1_LSR, options(nostack));
            if lsr & LSR_THRE != 0 { break; }
        }
        asm!("out dx, al", in("dx") COM1_DATA, in("al") b, options(nostack));
    }
}

fn write_bytes(s: &[u8]) {
    for &b in s { write_byte(b); }
}

fn write_dec(mut v: u32) {
    if v == 0 {
        write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = 10;
    while v > 0 {
        i -= 1;
        buf[i] = b'0' + (v % 10) as u8;
        v /= 10;
    }
    while i < 10 { write_byte(buf[i]); i += 1; }
}

// ── Panic handler ──────────────────────────────────────────────────

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // 1. Direct serial output — always works, even before cabina_0 init
    write_bytes(b"\r\n!!! KERNEL PANIC !!!\r\n");

    // 2. Format location into static buffer (no heap)
    if let Some(loc) = info.location() {
        write_bytes(b"  at ");
        write_bytes(loc.file().as_bytes());
        write_bytes(b":");
        write_dec(loc.line() as u32);
        write_bytes(b"\r\n");
    }

    // 3. Format message into static buffer (no heap)
    {
        let mut buf = [0u8; 128];
        let mut writer = SliceWriter { buf: &mut buf, pos: 0 };
        let _ = write!(&mut writer, "{}", PanicMsgWrap(info.message()));
        let len = writer.written();
        write_bytes(b"  msg: ");
        write_bytes(&buf[..len]);
        write_bytes(b"\r\n");
    }

    // 4. Record in cabina-daemon ring buffer (survives for dump)
    let ev = cabina_daemon::fault("panic", "KERNEL PANIC — see serial above");

    // 5. Dump cabina-daemon events to serial — gives full boot trace
    write_bytes(b"\r\n=== CABINA DUMP ===\r\n");
    {
        let cur = cabina_daemon::ring_buffer::next_seq();
        let start = if cur > 64 { cur - 64 } else { 1 };
        for seq in start..cur {
            if let Some(ev) = cabina_daemon::ring_buffer::event_by_seq(seq) {
                write_bytes(b"#");
                write_dec(seq as u32);
                write_bytes(b" ");
                write_bytes(ev.severity.name().as_bytes());
                write_bytes(b" ");
                write_bytes(ev.module_str().as_bytes());
                write_bytes(b": ");
                write_bytes(ev.msg_str().as_bytes());
                write_bytes(b"\r\n");
            }
        }
    }
    write_bytes(b"=== END CABINA DUMP ===\r\n");

    loop {
        unsafe { asm!("cli; hlt") }
    }
}

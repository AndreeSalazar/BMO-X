use cabina_core::Event;
use core::sync::atomic::AtomicBool;

static SERIAL_READY: AtomicBool = AtomicBool::new(false);

/// Function pointer type for serial output. Takes a formatted string.
type WriteFn = fn(&str);

static mut WRITE_FN: Option<WriteFn> = None;

pub fn init() {
    SERIAL_READY.store(true, core::sync::atomic::Ordering::SeqCst);
}

/// Register the global serial write function.
pub fn register(f: WriteFn) {
    unsafe { WRITE_FN = Some(f); }
}

/// Write an event to the registered global write function (if any).
pub fn write_event(ev: &Event) {
    if !SERIAL_READY.load(core::sync::atomic::Ordering::Relaxed) {
        return;
    }
    let wf = unsafe {
        match WRITE_FN {
            Some(f) => f,
            None => return,
        }
    };
    let mut buf = [0u8; 256];
    let mut len = 0usize;
    let sev = ev.severity.name();
    let module = ev.module_str();
    let msg = ev.msg_str();
    for b in sev.bytes() {
        if len < buf.len() { buf[len] = b; len += 1; }
    }
    if len < buf.len() { buf[len] = b' '; len += 1; }
    for b in module.bytes() {
        if len < buf.len() { buf[len] = b; len += 1; }
    }
    if len < buf.len() { buf[len] = b':'; len += 1; }
    if len < buf.len() { buf[len] = b' '; len += 1; }
    for b in msg.bytes() {
        if len < buf.len() { buf[len] = b; len += 1; }
    }
    if len < buf.len() { buf[len] = b'\n'; len += 1; }
    let out = core::str::from_utf8(&buf[..len.min(buf.len())]).unwrap_or("");
    wf(out);
}

use cabina_core::{Event, SerialSink};

static SERIAL_READY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);

pub fn init() {
    SERIAL_READY.store(true, core::sync::atomic::Ordering::SeqCst);
}

pub fn write_event<S: SerialSink>(sink: &mut S, ev: &Event) {
    if !SERIAL_READY.load(core::sync::atomic::Ordering::Relaxed) { return; }
    let mut buf = [0u8; 256];
    let mut len = 0usize;
    let sev = ev.severity.name();
    let module = ev.module_str();
    let msg = ev.msg_str();
    for b in sev.bytes() { if len < buf.len() { buf[len] = b; len += 1; } }
    if len < buf.len() { buf[len] = b' '; len += 1; }
    for b in module.bytes() { if len < buf.len() { buf[len] = b; len += 1; } }
    if len < buf.len() { buf[len] = b':'; len += 1; }
    if len < buf.len() { buf[len] = b' '; len += 1; }
    for b in msg.bytes() { if len < buf.len() { buf[len] = b; len += 1; } }
    if len < buf.len() { buf[len] = b'\n'; len += 1; }
    let out = core::str::from_utf8(&buf[..len.min(buf.len())]).unwrap_or("");
    sink.write_str(out);
}

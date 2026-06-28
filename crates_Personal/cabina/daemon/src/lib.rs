#![cfg_attr(not(test), no_std)]
extern crate alloc;

pub mod ring_buffer;
pub mod telemetry;
pub mod serial;
pub mod persistent;

use cabina_core::{
    Event, Severity, Layer, Entity,
    SystemSnapshot, SNAPSHOT_EVENTS_MAX,
};
use core::sync::atomic::{AtomicBool, Ordering};

static INITIALIZED: AtomicBool = AtomicBool::new(false);

/// Initialize all daemon subsystems.
pub fn init() {
    ring_buffer::init();
    serial::init();
    persistent::init();
    INITIALIZED.store(true, Ordering::SeqCst);
}

/// Emit a fully-specified event.
pub fn emit_full(
    severity: Severity,
    layer: Layer,
    entity: Entity,
    module: &str,
    entity_id: u32,
    msg: &str,
    value: u64,
) -> Event {
    let mut ev = Event::new(severity, layer, entity, module, entity_id, msg, value);
    if !INITIALIZED.load(Ordering::Relaxed) { return ev; }
    ev.seq = ring_buffer::push(&ev);
    serial::write_event(&ev);
    ev
}

/// Emit with severity + module + msg (layer auto-inferred).
pub fn emit(severity: Severity, module: &str, msg: &str) -> Event {
    emit_full(severity, Layer::from_module(module), Entity::Module, module, 0, msg, 0)
}

/// Emit with layer override.
pub fn emit_layer(severity: Severity, layer: Layer, module: &str, msg: &str) -> Event {
    emit_full(severity, layer, Entity::Module, module, 0, msg, 0)
}

/// Convenience severity wrappers.
pub fn info(module: &str, msg: &str) -> Event { emit(Severity::Info, module, msg) }
pub fn warn(module: &str, msg: &str) -> Event { emit(Severity::Warning, module, msg) }
pub fn fault(module: &str, msg: &str) -> Event { emit(Severity::Fault, module, msg) }
pub fn trace(module: &str, msg: &str) -> Event { emit(Severity::Trace, module, msg) }
pub fn panic_msg(module: &str, msg: &str) -> Event { emit(Severity::Panic, module, msg) }

/// Build a SystemSnapshot (telemetry + recent events).
pub fn take_snapshot() -> SystemSnapshot {
    let telemetry = {
        let mut t = telemetry::snapshot();
        t.uptime_ns = 0;
        t
    };
    let recent = ring_buffer::last(SNAPSHOT_EVENTS_MAX);
    let mut snapshot = SystemSnapshot::zero();
    snapshot.telemetry = telemetry;
    snapshot.event_count = recent.len() as u32;
    for (i, ev) in recent.iter().enumerate().take(SNAPSHOT_EVENTS_MAX) {
        snapshot.events[i] = *ev;
    }
    snapshot
}

#[cfg(test)]
mod tests {
    use super::*;
    use cabina_core::Layer;

    #[test]
    fn init_and_emit() {
        init();
        let ev = info("test_mod", "hello cabina-daemon");
        assert!(ev.seq > 0);
        assert_eq!(ev.module_str(), "test_mod");
        assert_eq!(ev.msg_str(), "hello cabina-daemon");
    }

    #[test]
    fn emit_all_severities() {
        init();
        let evs = [
            info("test", "info"),
            warn("test", "warn"),
            fault("test", "fault"),
            trace("test", "trace"),
            panic_msg("test", "panic"),
        ];
        for ev in &evs {
            assert!(ev.seq > 0);
        }
    }

    #[test]
    fn emit_layer_override() {
        init();
        let ev = emit_layer(Severity::Info, Layer::BmoGpu, "gpu_drv", "layer test");
        assert_eq!(ev.layer, Layer::BmoGpu);
    }

    #[test]
    fn emit_full_event() {
        init();
        let ev = emit_full(
            Severity::Fault,
            Layer::Sec,
            Entity::Process,
            "sec_mon", 1234,
            "security violation", 0xBADC0DE,
        );
        assert_eq!(ev.severity, Severity::Fault);
        assert_eq!(ev.layer, Layer::Sec);
        assert_eq!(ev.entity, Entity::Process);
        assert_eq!(ev.entity_id, 1234);
        assert_eq!(ev.value, 0xBADC0DE);
    }

    #[test]
    fn snapshot_contains_events() {
        init();
        info("snap", "event1");
        warn("snap", "event2");
        let snap = take_snapshot();
        assert!(snap.event_count >= 2);
    }

    #[test]
    fn snapshot_telemetry_fields() {
        init();
        telemetry::cpu::inc_interrupts();
        let snap = take_snapshot();
        assert!(snap.telemetry.cpu.interrupts >= 1);
    }
}

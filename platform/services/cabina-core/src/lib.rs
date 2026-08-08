#![cfg_attr(not(feature = "std"), no_std)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod event;
pub mod telemetry;
pub mod traits;

pub use event::{Event, Severity, SeverityMask, Layer, LayerMask, Entity, EntityMask};
pub use event::{MODULE_MAX, MSG_MAX, str_to_fixed, fixed_to_str};
pub use telemetry::{
    TelemetrySnapshot, SystemSnapshot,
    CpuCounters, MemoryCounters, SchedulerCounters, IoCounters,
    SNAPSHOT_EVENTS_MAX,
};
pub use traits::{SerialSink, Clock, EventSink, NullSink};

// --- Version --------------------------------------------------------------

pub const CABINA_VERSION: (u8, u8) = (1, 0);
pub const CABINA_CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const CABINA_MAGIC: u32 = 0x434142_31; // "CAB1" as u32

// --- Shared memory buffer protocol constants ------------------------------

/// Maximum events in a shared-memory ring buffer.
pub const SHM_BUFFER_CAPACITY: u32 = 256;
/// Magic value for a shared-memory buffer header.
pub const SHM_MAGIC: u32 = 0x434142_32; // "CAB2" as u32
/// Current shared memory protocol version.
pub const SHM_VERSION: u32 = 1;

// --- Tests ----------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_constants() {
        assert_eq!(CABINA_VERSION, (1, 0));
        assert!(CABINA_CORE_VERSION.len() > 0);
    }

    #[test]
    fn magic_constants() {
        assert_eq!(CABINA_MAGIC, 0x434142_31);
        assert_eq!(SHM_MAGIC, 0x434142_32);
    }

    #[test]
    fn event_import_works() {
        let ev = Event::new(
            Severity::Info,
            Layer::Ring0,
            Entity::Module,
            "test", 0, "import check", 0,
        );
        assert_eq!(ev.severity, Severity::Info);
    }

    #[test]
    fn telemetry_import_works() {
        let snap = TelemetrySnapshot::zero();
        assert_eq!(snap.cpu.interrupts, 0);
    }

    #[test]
    fn trait_imports_work() {
        let mut sink = NullSink;
        sink.write_str("ok");
        // Does not panic
    }
}

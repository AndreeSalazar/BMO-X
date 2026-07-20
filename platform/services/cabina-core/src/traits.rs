use crate::event::Event;

/// A sink that can consume diagnostic output (serial, framebuffer, log file).
pub trait SerialSink {
    fn write_str(&mut self, s: &str);
    fn write_bytes(&mut self, bytes: &[u8]);
}

/// A monotonic clock for event timestamps.
pub trait Clock {
    fn now_ns(&self) -> u64;
}

/// A sink that can receive CABINA events.
pub trait EventSink {
    fn emit(&mut self, event: &Event);
}

/// A sink that discards all output (useful for benchmarks or tests).
pub struct NullSink;

impl SerialSink for NullSink {
    fn write_str(&mut self, _s: &str) {}
    fn write_bytes(&mut self, _bytes: &[u8]) {}
}

impl Clock for NullSink {
    fn now_ns(&self) -> u64 { 0 }
}

impl EventSink for NullSink {
    fn emit(&mut self, _event: &Event) {}
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{Severity, Layer, Entity};

    #[test]
    fn null_sink_accepts_events() {
        let mut sink = NullSink;
        sink.write_str("hello");
        sink.write_bytes(b"world");
    }

    #[test]
    fn null_sink_emit() {
        let mut sink = NullSink;
        let ev = Event::new(Severity::Info, Layer::Ring0, Entity::Module, "test", 0, "null", 0);
        sink.emit(&ev);
    }

    #[test]
    fn null_sink_now_ns() {
        let sink = NullSink;
        assert_eq!(sink.now_ns(), 0);
    }
}

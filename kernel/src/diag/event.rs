//! Tipos centrales de diag/.

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warn,
    Fault,
    Panic,
    Trace,
}

#[derive(Clone, Copy)]
pub(crate) struct Event {
    pub seq: u64,
    pub severity: Severity,
    pub module: &'static str,
    pub message: &'static str,
    pub value: u64,
    pub has_value: bool,
}

impl Event {
    pub const fn empty() -> Self {
        Self {
            seq: 0,
            severity: Severity::Info,
            module: "",
            message: "",
            value: 0,
            has_value: false,
        }
    }

    pub const fn new(
        severity: Severity,
        module: &'static str,
        message: &'static str,
    ) -> Self {
        Self {
            seq: 0,
            severity,
            module,
            message,
            value: 0,
            has_value: false,
        }
    }

    pub const fn new_u64(
        severity: Severity,
        module: &'static str,
        message: &'static str,
        value: u64,
    ) -> Self {
        Self {
            seq: 0,
            severity,
            module,
            message,
            value,
            has_value: true,
        }
    }
}

pub fn severity_name(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "INFO",
        Severity::Warn => "WARN",
        Severity::Fault => "FAULT",
        Severity::Panic => "PANIC",
        Severity::Trace => "TRACE",
    }
}

pub(crate) fn severity_tag(severity: Severity) -> &'static [u8] {
    match severity {
        Severity::Info => b"INFO ",
        Severity::Warn => b"WARN ",
        Severity::Fault => b"FAULT",
        Severity::Panic => b"PANIC",
        Severity::Trace => b"TRACE",
    }
}

pub(crate) fn severity_color(severity: Severity) -> u32 {
    match severity {
        Severity::Info => 0xFF58A6FF,
        Severity::Warn => 0xFFFFBD2E,
        Severity::Fault => 0xFFFF7B72,
        Severity::Panic => 0xFFFF2A2A,
        Severity::Trace => 0xFF76B900,
    }
}

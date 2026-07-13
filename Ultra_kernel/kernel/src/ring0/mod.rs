//! Ring 0 — Hardware Abstraction Layer.
//!
//! Minimal base: arch, mem, dev, cpu, proc, irq, boot helpers, core.

pub mod core {
    pub mod entry;
    pub mod phase;
    pub mod splash;
}

pub mod boot {
    pub mod serial;
    pub mod log;
}

pub mod arch;
pub mod cpu;
pub mod dev;
pub mod mm;
pub mod proc;
pub mod irq;

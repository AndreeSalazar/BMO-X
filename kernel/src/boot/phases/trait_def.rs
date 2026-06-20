//! Phase trait — every boot phase has the same interface.
//!
//! Two modes of use:
//!
//!   `run(prev) -> PhaseOutput`
//!     Normal boot flow. Mutates global state, advances boot. Cannot be
//!     called twice safely.
//!
//!   `self_test() -> SelfTestReport`
//!     Isolated check. Does NOT mutate boot state. Can be called from
//!     welcome screen, QEMU pre-flight, or post-mortem diagnostics.
//!     Failure of self_test never halts the system; the report is shown
//!     to the user.
//!
//! The trait keeps `main.rs` as a pure dispatcher: it does not need to
//! know what each phase does internally.

/// TSC tick at which a phase finished. Used to compute per-phase timings.
pub type Timestamp = u64;

/// Result of a phase's normal boot run.
pub struct PhaseOutput {
    pub prev_end: Timestamp,
}

/// Self-test report. Multiple checks per phase, each pass/fail.
#[derive(Clone, Copy)]
pub struct CheckResult {
    pub name: &'static str,
    pub passed: bool,
    pub detail: u64,
}

impl CheckResult {
    pub const fn pass(name: &'static str) -> Self {
        Self { name, passed: true, detail: 0 }
    }
    #[allow(dead_code)] // reserved for future self-test failures
    pub const fn fail(name: &'static str, detail: u64) -> Self {
        Self { name, passed: false, detail }
    }
}

#[derive(Clone, Copy)]
pub struct SelfTestReport {
    pub phase: &'static str,
    pub checks: &'static [CheckResult],
}

impl SelfTestReport {
    #[allow(dead_code)] // public API for future use
    pub fn all_passed(&self) -> bool {
        self.checks.iter().all(|c| c.passed)
    }
    pub fn failed_count(&self) -> usize {
        self.checks.iter().filter(|c| !c.passed).count()
    }
}

/// Pretty-print a self-test report to serial + visual.
pub fn report(r: &SelfTestReport) {
    use crate::drivers::serial;
    use crate::boot::visual;

    serial::serial_write("[selftest] ");
    serial::serial_write(r.phase);
    serial::serial_write(": ");
    let total = r.checks.len();
    let failed = r.failed_count();
    if failed == 0 {
        serial::serial_write("OK (");
        crate::boot::serial::u32_dec(total as u32);
        serial::serial_write(" checks)\n");
    } else {
        serial::serial_write("FAIL ");
        crate::boot::serial::u32_dec(failed as u32);
        serial::serial_write("/");
        crate::boot::serial::u32_dec(total as u32);
        serial::serial_write("\n");
        for c in r.checks {
            if !c.passed {
                serial::serial_write("  - ");
                serial::serial_write(c.name);
                serial::serial_write(" detail=0x");
                crate::boot::serial::hex(c.detail);
                serial::serial_write("\n");
            }
        }
    }

    let color = if failed == 0 { visual::color::OK } else { visual::color::FAULT };
    let mut msg = [0u8; 64];
    let mut len = 0;
    let s = r.phase.as_bytes();
    for &b in s { if len < msg.len() { msg[len] = b; len += 1; } }
    let tag: &[u8] = if failed == 0 { b" selftest OK" } else { b" selftest FAIL" };
    for &b in tag { if len < msg.len() { msg[len] = b; len += 1; } }
    let s = core::str::from_utf8(&msg[..len]).unwrap_or("?");
    visual::log(r.phase, s, color);
}

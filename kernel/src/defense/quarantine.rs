//! `defense::quarantine` — Apps en cuarentena.

use super::report::SecurityReport;

const MAX_QUARANTINE: usize = 16;

static mut COUNT: usize = 0;
static mut HEAD: usize = 0;
static mut ITEMS: [Option<SecurityReport>; MAX_QUARANTINE] = [None, None, None, None, None, None, None, None, None, None, None, None, None, None, None, None];

/// API pública del módulo quarantine (re-exportada como `defense::QuarantineList`).
pub struct QuarantineList;

pub fn init() {}

/// Pone un reporte en cuarentena.
pub fn put(report: SecurityReport) {
    unsafe {
        let slot = HEAD;
        ITEMS[slot] = Some(report);
        HEAD = (HEAD + 1) % MAX_QUARANTINE;
        if COUNT < MAX_QUARANTINE { COUNT += 1; }
    }
}

/// # de items en cuarentena.
pub fn count() -> usize { unsafe { COUNT } }

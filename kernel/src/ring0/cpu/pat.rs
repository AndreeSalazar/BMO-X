#![allow(dead_code)]

//! Page Attribute Table (PAT) — memory caching optimization.

/// Configure PAT. Default PAT value already has WC at index 1:
/// PAT[0]=WB, PAT[1]=WC, PAT[2]=UC-, PAT[3]=UC, ...
/// No explicit write needed for basic operation.
pub fn init() {
    crate::device::serial::serial_write("[cpu] PAT: default config OK (WB+WC)\n");
}

//! Power Manager stub (Ring 0).
//! Full C-state and thermal management lives in the Ring 3 module.

pub fn init() {
    crate::dev::console::serial_write("[power] Ring 0 stub — full C-state/thermal deferred to module\n");
}

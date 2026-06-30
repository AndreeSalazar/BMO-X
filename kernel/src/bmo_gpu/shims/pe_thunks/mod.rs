#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThunkTarget {
    SilentStub,
    LogStub,
    Real(u64),
}

pub fn resolve_fn(_dll: &str, _fn_name: &str) -> (ThunkTarget, u64) {
    (ThunkTarget::SilentStub, silent_stub as *const () as u64)
}

pub extern "C" fn silent_stub() {
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub extern "C" fn log_stub() {
    loop { unsafe { core::arch::asm!("hlt"); } }
}

pub static THUNK_TABLE: &[(&str, &[(&str, u64)])] = &[];

//! `calling` — convención de llamada del BMO ABI.

pub mod registers;

pub use registers::{
    STACK_ALIGNMENT, SHADOW_SPACE, RED_ZONE_SIZE,
    ARG_GPRS, ARG_XMMS, RET_GPRS, RET_XMMS,
    CALLER_SAVED_GPRS, CALLEE_SAVED_GPRS,
};

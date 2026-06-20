//! `time` — tipos de tiempo del BMO ABI.
//!
//! - [`BmoInstant`] — punto en el tiempo, monotónico, ns desde boot.
//! - [`BmoDuration`] — intervalo de tiempo en nanosegundos.
//!
//! ## Inicialización
//!
//! Tras el boot, llamar `bmo_abi::time::init(tsc_at_boot, tsc_freq_hz)` para
//! activar el backend real. Antes de eso, `BmoInstant::now()` retorna ZERO.

pub mod instant;
pub mod duration;

pub use instant::init as init_clock;

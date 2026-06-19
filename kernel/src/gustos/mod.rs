//! `gustos` — Sistema de audio procedural de FastOS.
//!
//! v1.5.0: reproduce sonidos icónicos de Windows + música procedural
//! usando FM synthesis puro. No usa samples.
//!
//! ## Uso
//!
//! ```rust
//! use crate::gustos::tracks::windows;
//!
//! windows::startup();  // Suena el boot de Windows-inspired
//! ```
//!
//! v1.6.15: Allow dead_code — many track functions are exposed as a
//! public API (so user code can call `gustos::tracks::windows::error()`
//! etc.) but only the `logon` track is currently triggered by the
//! welcome flow. The other 6 tracks + 3 procedural helpers will be
//! wired in v1.7.x once the audio event bus is in place.

#![allow(dead_code)]

pub mod synth;
pub mod tracks;


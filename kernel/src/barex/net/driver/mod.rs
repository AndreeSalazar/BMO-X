//! Bridge a NIC. Driver real vive en `crate::drivers::*`; este módulo
//! es la fachada limpia que `bx_net` usa, abstrayendo el chip concreto
//! (Realtek RTL8125B 2.5 GbE, Intel I225-V 2.5 GbE, etc.).

pub mod nic;
pub mod caps;

pub use nic::NicDriver;
pub use caps::NicCapabilities;

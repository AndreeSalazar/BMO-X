//! `cpu_vendor/` -- CPU-specific knowledge for the Ryzen 5 5600X (Vermeer, Zen 3).
//!
//! Everything in this directory is **specific to the AMD Ryzen 5 5600X**
//! (Family 19h, Model 01h). On a different CPU, the corresponding
//! `vendor/<arch>/<cpu>/` directory would be selected at compile time
//! or by the boot profile.
//!
//! ## What's here
//!
//! - `ryzen_5_5600x` -- the canonical profile for our test bench.
//!   Sub-modules cover CPUID topology, cache detection, TSC
//!   calibration, MSR init, errata workarounds, and the consolidated
//!   `BmoCpu` API.
//!
//! ## How the kernel uses it
//!
//! The minimal Ring 0 base (this kernel) only calls `init_bmo_cpu()`
//! from `phase::main` to populate the CPU identity/topology globals.
//! Future work (SMP bring-up, PAT/MTRR setup, etc.) is wired in
//! here too.
//!
//! References:
//! - AMD64 Architecture Programmer's Manual Vol. 3
//! - AMD Zen 3 Family 19h BKDG (PUB, revision 0.91+)
//! - AMD Whitepaper "Software Techniques for Managing Speculation" (rev 4.10)

pub mod profile;
pub mod ryzen_5_5600x;
/// Estado extendido (XSAVE): se lo pregunta al silicio y contrasta con el
/// perfil. NO habilita nada -- ver la nota de cabecera del modulo.
pub mod xsave;

pub use profile::{active, CpuProfile};

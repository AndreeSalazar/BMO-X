//! **Intrínsecos** — keywords semánticos que mapean a **bytes precisos**.
//!
//! Esta es la diferencia clave contra ASM clásico: escribes `pausa` y el
//! emisor produce `F3 90` (PAUSE instruction). Escribes `atomico { x = 1 }`
//! y el emisor prefija con `F0` (LOCK). Escribes `cpuid` y obtienes `0F A2`.
//!
//! **Semántico viviente.** El keyword expresa _intención_; los bytes son
//! producto del intríínseco — no tienes que recordar el opcode.

pub mod intrinsics;
pub mod flags;

pub use intrinsics::{IntrinsicId, emit_intrinsic};

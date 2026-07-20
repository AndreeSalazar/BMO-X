//! `runtime` — el agregador de runtime del BMO ABI.
//!
//! Gestiona el registro de tipos, las tablas virtuales (vtables) y los
//! bridges entre lenguajes. Es el "orquestador" que permite que diferentes
//! lenguajes BEF cooperen en el mismo espacio de direcciones.
//!
//! # Arquitectura
//!
//! ```text
//! BmoRuntime
//!   ├── types:      TypeRegistry  (256 slots para tipos BEF)
//!   ├── vtables:    VTableStore   (64 slots para vtables de interfaz)
//!   └── bridges:    LangBridge    (8 slots para bridges de lenguaje)
//! ```

pub mod lang_bridge;
pub mod types;
pub mod vtable;

use lang_bridge::LangBridge;
use types::TypeRegistry;
use vtable::VTableStore;

use crate::bmo_abi::error_code;
use crate::bmo_abi::fundamentals::primitives::bx_u32;
use crate::bmo_abi::fundamentals::status::BmoStatus;

/// El agregador único de runtime del BMO ABI.
///
/// Se construye una vez en el boot y se pasa a todo `bmo_core::start`.
/// Cada campo tiene capacidad fija (sin heap).
pub struct BmoRuntime {
    pub types: TypeRegistry,
    pub vtables: VTableStore,
    pub bridges: [Option<LangBridge>; 8],
}

impl BmoRuntime {
    pub const fn new() -> Self {
        Self {
            types: TypeRegistry::new(),
            vtables: VTableStore::new(),
            bridges: [None, None, None, None, None, None, None, None],
        }
    }

    /// Register a language bridge in the first free slot.
    pub fn register_bridge(&mut self, bridge: LangBridge) -> Result<bx_u32, BmoStatus> {
        for (i, slot) in self.bridges.iter_mut().enumerate() {
            if slot.is_none() {
                *slot = Some(bridge);
                return Ok(i as bx_u32);
            }
        }
        Err(BmoStatus::err(error_code::OUT_OF_MEMORY))
    }

    /// Validate the entire runtime: types, vtables, and bridges.
    pub fn validate(&self) -> BmoStatus {
        if !self.types.is_valid() {
            return BmoStatus::err(error_code::INVALID_STATE);
        }
        BmoStatus::OK
    }
}

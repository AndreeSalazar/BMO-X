//! Enruta engines a endpoints físicos. Reemplaza MMDevice (Win32 COM)
//! y `pa_context_get_sink_info_*` (PulseAudio).

use crate::barex::{BxError, BxResult};
use super::endpoint::Endpoint;

pub struct Router {
    /// Última vez que se enumeraron endpoints (epoch ns).
    pub last_enum_ns: u64,
}

impl Router {
    pub const fn new() -> Self {
        Self { last_enum_ns: 0 }
    }

    /// Enumera todos los endpoints disponibles. Stub.
    pub fn enumerate(&mut self, _out: &mut [Endpoint]) -> BxResult<usize> {
        Err(BxError::NotImplemented)
    }

    /// Default playback endpoint según política del usuario.
    pub fn default_playback(&self) -> BxResult<Endpoint> {
        Err(BxError::NotImplemented)
    }
}

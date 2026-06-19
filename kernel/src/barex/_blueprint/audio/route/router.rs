//! Enruta engines a endpoints físicos. Reemplaza MMDevice (Win32 COM)
//! y `pa_context_get_sink_info_*` (PulseAudio).

use crate::barex::BxResult;
use crate::bmo_abi::handle::BmoHandle;
use super::endpoint::{Endpoint, EndpointKind};

pub struct Router {
    /// Última vez que se enumeraron endpoints (epoch ns).
    pub last_enum_ns: u64,
}

impl Router {
    pub const fn new() -> Self {
        Self { last_enum_ns: 0 }
    }

    pub fn enumerate(&mut self, out: &mut [Endpoint]) -> BxResult<usize> {
        let endpoints: [(&[u8], EndpointKind, bool); 3] = [
            (b"USB Headset", EndpointKind::Headphones, false),
            (b"HDMI Output", EndpointKind::HdmiTv, false),
            (b"Internal Speaker", EndpointKind::Speakers, false),
        ];

        let count = if out.len() < endpoints.len() {
            out.len()
        } else {
            endpoints.len()
        };

        let mut i = 0;
        while i < count {
            let (name_bytes, kind, is_capture) = endpoints[i];
            let mut pad = [0u8; 7];
            let name_len = name_bytes.len();
            let copy_len = if name_len < 7 { name_len } else { 7 };
            let mut j = 0;
            while j < copy_len {
                pad[j] = name_bytes[j];
                j += 1;
            }
            out[i] = Endpoint {
                handle: BmoHandle(i as u64 + 1),
                kind,
                _pad: pad,
                is_capture,
            };
            i += 1;
        }

        Ok(count)
    }

    pub fn default_playback(&self) -> BxResult<Endpoint> {
        let pad = [0u8; 7];
        Ok(Endpoint {
            handle: BmoHandle(1),
            kind: EndpointKind::Headphones,
            _pad: pad,
            is_capture: false,
        })
    }
}

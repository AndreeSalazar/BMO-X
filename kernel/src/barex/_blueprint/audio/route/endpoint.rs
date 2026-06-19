use crate::bmo_abi::handle::BmoHandle;
use crate::bmo_abi::primitives::bx_u8;

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointKind {
    Headphones  = 0,
    Speakers    = 1,
    Microphone  = 2,
    LineIn      = 3,
    LineOut     = 4,
    HdmiTv      = 5,
    HdmiMonitor = 6,
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Endpoint {
    pub handle: BmoHandle,
    pub kind: EndpointKind,
    pub _pad: [bx_u8; 7],
    /// Indica si soporta entrada (mic, line-in).
    pub is_capture: bool,
}

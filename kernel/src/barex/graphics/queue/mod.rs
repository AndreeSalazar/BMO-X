//! (2) `BxQueue` — cola de envío de comandos.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueKind {
    Graphics,
    Compute,
    Copy,
    VideoDecode,
    VideoEncode,
}

pub struct BxQueue {
    pub kind: QueueKind,
}

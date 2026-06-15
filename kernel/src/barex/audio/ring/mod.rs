//! SQ/CQ io_uring-style para submit PCM low-latency.
//! Reemplaza event_callbacks de WASAPI / portaudio.

pub mod sqe;
pub mod cqe;
pub mod queue;


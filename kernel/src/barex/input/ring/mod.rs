//! SQ/CQ io_uring-style para entrega de input events con latencia <0.5 ms.
//!
//! Reemplaza:
//!   - Win32 message loop (`GetMessage`/`PeekMessage` polled at 60 Hz)
//!   - X11 / Wayland event queues
//!   - GLFW callbacks
//!   - SDL2 polling

pub mod sqe;
pub mod cqe;
pub mod queue;

pub use sqe::InputSqe;
pub use cqe::InputCqe;
pub use queue::{InputSubmissionQueue, InputCompletionQueue};

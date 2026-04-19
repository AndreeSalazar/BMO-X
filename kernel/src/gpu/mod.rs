//! GPU Command-Level Driver — Level 2: Command Submission
//!
//! Phase 1: DMA pushbuffer + FIFO channel
//! Phase 2: NV method constants + command builders
//! Phase 3: Copy Engine DMA + framebuffer fill (visible proof)

pub mod dma;
pub mod fifo;
pub mod methods;
pub mod commands;
pub mod engine;

//! # nv_cmd — FIFO Command Submission
//!
//! GPU command channels, pushbuffers, and fence synchronization.
//! Maps to NVIDIA's PFIFO engine (PAGE_K section in nvlddmkm.sys).
//!
//! SigDead-BIB found: NV_ERR_FIFO_BAD_ACCESS, NV_ERR_INVALID_CHANNEL,
//! NV_ERR_DMA_IN_USE — these define the FIFO error conditions.
//!
//! NVIDIA uses a runlist-based channel scheduler on Ampere.
//! Each channel has a pushbuffer (ring buffer) in system memory.
//! The GPU reads commands from the pushbuffer via DMA.
//!
//! `#![no_std]` compatible.

#![no_std]

use nv_error::{NvError, NvResult};
use nv_regs::pfifo;
use nv_hal::{MmioRegion, DmaBuffer};

/// Pushbuffer size — 1 MB per channel (generous default).
pub const PUSHBUF_SIZE: usize = 1024 * 1024;

/// Maximum commands per pushbuffer before wrap.
pub const PUSHBUF_MAX_ENTRIES: usize = PUSHBUF_SIZE / 8; // 8 bytes per command

/// A GPU channel — one command stream.
/// NVIDIA supports up to 512 channels on GA106.
pub struct Channel {
    pub id: u32,
    pub pushbuf: DmaBuffer,
    put: usize,     // Write offset (bytes)
    get: usize,     // Last known read offset
    fence_seq: u64, // Monotonic fence sequence
}

/// A GPU command: method + data pair.
/// NVIDIA encodes commands as: [header (4B)] [data (4B)]
/// Header format: [count:13][subchannel:3][method:13][type:3]
#[derive(Debug, Clone, Copy)]
pub struct GpuMethod {
    pub subchannel: u8,  // 0-7 engine binding
    pub method: u16,     // Register offset (>> 2)
    pub data: u32,       // Value to write
}

/// Subchannel assignments (engine bindings on the channel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SubChannel {
    Graphics = 0,    // 2D/3D engine (PGRAPH)
    Compute  = 1,    // Compute dispatch
    Copy0    = 2,    // Copy Engine 0
    Copy1    = 3,    // Copy Engine 1
    Inline   = 4,    // Inline-to-memory
}

/// Fence — tracks GPU completion.
pub struct Fence {
    pub sequence: u64,
    pub semaphore_va: u64, // GPU virtual address of semaphore
}

impl Channel {
    /// Create a new channel with the given ID and pre-allocated pushbuffer.
    pub fn new(id: u32, pushbuf: DmaBuffer) -> NvResult<Self> {
        if pushbuf.size < PUSHBUF_SIZE {
            return Err(NvError::BufferTooSmall);
        }
        if id >= pfifo::MAX_CHANNELS {
            return Err(NvError::InvalidChannel);
        }
        Ok(Self {
            id,
            pushbuf,
            put: 0,
            get: 0,
            fence_seq: 0,
        })
    }

    /// Push a single method+data command.
    pub fn push(&mut self, cmd: GpuMethod) -> NvResult<()> {
        if self.put + 8 > self.pushbuf.size {
            return Err(NvError::BufferTooSmall);
        }

        // Encode NV method header (non-incrementing, count=1)
        let header: u32 = (1 << 28)                       // type = non-incrementing
            | ((cmd.subchannel as u32 & 0x7) << 13)       // subchannel
            | ((cmd.method as u32) & 0x1FFF);             // method >> 2

        self.pushbuf.write32(self.put, header);
        self.pushbuf.write32(self.put + 4, cmd.data);
        self.put += 8;

        Ok(())
    }

    /// Push multiple data values to consecutive methods (incrementing).
    pub fn push_inc(&mut self, subchannel: u8, start_method: u16, data: &[u32]) -> NvResult<()> {
        let total_bytes = 4 + data.len() * 4; // header + N data words
        if self.put + total_bytes > self.pushbuf.size {
            return Err(NvError::BufferTooSmall);
        }

        // Incrementing method header
        let header: u32 = (2 << 28)                       // type = incrementing
            | ((data.len() as u32) << 16)                  // count
            | ((subchannel as u32 & 0x7) << 13)
            | ((start_method as u32) & 0x1FFF);

        self.pushbuf.write32(self.put, header);
        self.put += 4;

        for &val in data {
            self.pushbuf.write32(self.put, val);
            self.put += 4;
        }

        Ok(())
    }

    /// Kick — tell GPU there are new commands by updating PUT pointer.
    pub fn kick(&self, bar0: &MmioRegion) {
        bar0.write32(pfifo::CHAN_PUT(self.id), self.put as u32);
    }

    /// Read GPU's GET pointer to see how far it has processed.
    pub fn poll_get(&mut self, bar0: &MmioRegion) -> u32 {
        let get = bar0.read32(pfifo::CHAN_GET(self.id));
        self.get = get as usize;
        get
    }

    /// Check if GPU has consumed all commands.
    pub fn is_idle(&self, bar0: &MmioRegion) -> bool {
        let get = bar0.read32(pfifo::CHAN_GET(self.id)) as usize;
        get == self.put
    }

    /// Reset pushbuffer to start (after GPU has drained).
    pub fn reset(&mut self) {
        self.put = 0;
        self.get = 0;
    }

    /// How many bytes of commands are pending.
    pub fn pending_bytes(&self) -> usize {
        self.put.saturating_sub(self.get)
    }

    /// Insert a fence into the pushbuffer and return a sequence number.
    pub fn emit_fence(&mut self) -> NvResult<u64> {
        self.fence_seq += 1;
        // TODO: Push actual semaphore release method to GPU
        // For now, return sequence as tracking token
        Ok(self.fence_seq)
    }
}

/// Initialize PFIFO engine.
pub fn fifo_init(bar0: &MmioRegion) -> NvResult<()> {
    // Clear FIFO interrupts
    bar0.write32(pfifo::INTR_0, 0xFFFF_FFFF);
    // Enable FIFO interrupts
    bar0.write32(pfifo::INTR_EN_0, 0xFFFF_FFFF);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subchannel_values() {
        assert_eq!(SubChannel::Graphics as u8, 0);
        assert_eq!(SubChannel::Compute as u8, 1);
        assert_eq!(SubChannel::Copy0 as u8, 2);
    }

    #[test]
    fn pushbuf_constants() {
        assert_eq!(PUSHBUF_SIZE, 1024 * 1024);
        assert!(PUSHBUF_MAX_ENTRIES > 100_000);
    }
}

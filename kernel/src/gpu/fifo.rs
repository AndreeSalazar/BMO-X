//! FIFO Channel & Pushbuffer — GPU Command Submission
//!
//! On Ampere (GA106), commands flow: CPU → Pushbuffer → GPFIFO → PBDMA → Engine.
//!
//! The pushbuffer is a ring of GPU methods (NV class methods).
//! GPFIFO entries point to segments of the pushbuffer.
//! PBDMA reads GPFIFO entries and feeds them to the target engine.
//!
//! Since GSP-RM controls FIFO on Ampere, we use a lightweight
//! "direct register" approach for basic operations and a
//! software-managed pushbuffer for demonstration.

use super::dma::{GpuDmaBuffer, alloc_gpu_dma};

/// Pushbuffer size (16KB = 4096 u32 entries).
pub const PUSHBUF_SIZE: usize = 16 * 1024;
/// GPFIFO entry count (256 entries × 8 bytes = 2KB).
pub const GPFIFO_ENTRIES: usize = 256;
pub const GPFIFO_SIZE: usize = GPFIFO_ENTRIES * 8;

/// A GPU FIFO channel with pushbuffer and GPFIFO.
pub struct GpuChannel {
    /// Channel ID.
    pub id: u32,
    /// Pushbuffer DMA memory (commands go here).
    pub pushbuf: GpuDmaBuffer,
    /// Current write offset in pushbuffer (in u32 units).
    pub put: u32,
    /// GPFIFO DMA memory (pointers to pushbuf segments).
    pub gpfifo: GpuDmaBuffer,
    /// Current GPFIFO write index.
    pub gp_put: u32,
    /// Whether the channel is initialized.
    pub active: bool,
    /// Total commands pushed.
    pub cmd_count: u32,
}

impl GpuChannel {
    /// Push a single u32 method/data to the pushbuffer.
    #[inline]
    pub fn push(&mut self, val: u32) {
        let max_entries = (self.pushbuf.size / 4) as u32;
        let offset = (self.put % max_entries) as usize * 4;
        self.pushbuf.write_u32(offset, val);
        self.put = (self.put + 1) % max_entries;
    }

    /// Push a NV method header + data.
    /// Format: [31:29]=type [28:16]=method>>2 [15:0]=subchannel+count
    /// Simplified: push method header then data words.
    pub fn push_method(&mut self, subchannel: u32, method: u32, data: u32) {
        // Ampere method header format (increasing method):
        // [31:29] = 0b001 (increasing)
        // [28:16] = method >> 2
        // [15:13] = subchannel
        // [12:0]  = count (1)
        let header = (1 << 29)
            | (((method >> 2) & 0x1FFF) << 16)
            | ((subchannel & 0x7) << 13)
            | 1; // count = 1
        self.push(header);
        self.push(data);
        self.cmd_count += 1;
    }

    /// Push multiple data words for one method (non-increasing).
    pub fn push_method_multi(&mut self, subchannel: u32, method: u32, data: &[u32]) {
        let count = data.len() as u32;
        // [31:29] = 0b011 (non-increasing)
        let header = (3 << 29)
            | (((method >> 2) & 0x1FFF) << 16)
            | ((subchannel & 0x7) << 13)
            | (count & 0x1FFF);
        self.push(header);
        for &d in data {
            self.push(d);
        }
        self.cmd_count += 1;
    }

    /// Submit current pushbuffer segment to GPFIFO.
    /// Creates a GPFIFO entry pointing to the pushbuffer data.
    pub fn submit_gpfifo(&mut self, start_offset: u32, word_count: u32) {
        let pb_phys = self.pushbuf.phys + (start_offset as u64 * 4);
        let gp_idx = (self.gp_put % GPFIFO_ENTRIES as u32) as usize;

        // GPFIFO entry format (8 bytes):
        // [63:2]  = pushbuffer segment address >> 2
        // [1]     = 0 (no interrupt after)
        // [0]     = 0 (not a NOP)
        // Second dword: [31:10] = length in dwords, [9:8] = flags
        let lo = (pb_phys & 0xFFFF_FFFC) as u32; // address aligned
        let hi = ((word_count & 0x3F_FFFF) << 10) | ((pb_phys >> 32) as u32 & 0xFF);

        self.gpfifo.write_u32(gp_idx * 8, lo);
        self.gpfifo.write_u32(gp_idx * 8 + 4, hi);

        self.gp_put += 1;
    }

    /// Get number of commands in the pushbuffer.
    pub fn pending_words(&self) -> u32 {
        self.put
    }

    /// Reset pushbuffer position to start.
    pub fn reset(&mut self) {
        self.put = 0;
        self.gp_put = 0;
        self.pushbuf.zero();
        self.gpfifo.zero();
    }

    /// Dump pushbuffer contents (first N words) for debug.
    pub fn dump_pushbuf(&self, count: usize) -> &[u32] {
        let max = core::cmp::min(count, self.pushbuf.size / 4);
        unsafe {
            core::slice::from_raw_parts(self.pushbuf.virt as *const u32, max)
        }
    }
}

/// Allocate and initialize a new GPU channel.
pub fn create_channel(id: u32) -> Option<GpuChannel> {
    let pushbuf = alloc_gpu_dma(PUSHBUF_SIZE)?;
    let gpfifo = alloc_gpu_dma(GPFIFO_SIZE)?;

    Some(GpuChannel {
        id,
        pushbuf,
        put: 0,
        gpfifo,
        gp_put: 0,
        active: true,
        cmd_count: 0,
    })
}

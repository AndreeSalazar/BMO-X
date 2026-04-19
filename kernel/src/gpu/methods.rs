//! NV GPU Method Constants — Ampere (GA106) Class Methods
//!
//! These are the hardware method IDs pushed to the pushbuffer.
//! Methods are 32-bit register-like offsets within a GPU object class.
//! Sources: nouveau, envytools, open-gpu-kernel-modules, SigDead-BIB.

// ── NV Object Classes (Ampere GA10x) ────────────────────────────────────────

/// Copy Engine class (CE / DMA copy).
pub const AMPERE_DMA_COPY_A: u32 = 0xC6B5;
/// 2D Engine class (blits, fills).
pub const AMPERE_2D_A: u32 = 0xC697;
/// 3D Engine class (Ampere A).
pub const AMPERE_3D_A: u32 = 0xC797;
/// Compute class (Ampere A).
pub const AMPERE_COMPUTE_A: u32 = 0xC6C0;
/// Channel GPFIFO class.
pub const AMPERE_CHANNEL_GPFIFO_A: u32 = 0xC46F;

// ── Common Methods (all classes) ─────────────────────────────────────────────

/// NOP — does nothing, useful for padding/sync.
pub const NV_NOP: u32 = 0x0000;
/// Set object class on subchannel.
pub const NV_SET_OBJECT: u32 = 0x0000;
/// Semaphore address high.
pub const NV_SEMAPHORE_ADDR_HI: u32 = 0x0010;
/// Semaphore address low.
pub const NV_SEMAPHORE_ADDR_LO: u32 = 0x0014;
/// Semaphore payload.
pub const NV_SEMAPHORE_PAYLOAD: u32 = 0x0018;
/// Semaphore operation (release/acquire).
pub const NV_SEMAPHORE_OP: u32 = 0x001C;

// ── Copy Engine (CE) Methods — class C6B5 ────────────────────────────────────
// Used for DMA memory copies between system RAM and VRAM.

/// Source address high 32 bits.
pub const CE_SRC_ADDR_HI: u32 = 0x0400;
/// Source address low 32 bits.
pub const CE_SRC_ADDR_LO: u32 = 0x0404;
/// Destination address high 32 bits.
pub const CE_DST_ADDR_HI: u32 = 0x0408;
/// Destination address low 32 bits.
pub const CE_DST_ADDR_LO: u32 = 0x040C;
/// Source pitch (bytes per row).
pub const CE_SRC_PITCH: u32 = 0x0410;
/// Destination pitch.
pub const CE_DST_PITCH: u32 = 0x0414;
/// Copy width in bytes.
pub const CE_X_COUNT: u32 = 0x0418;
/// Copy height (rows).
pub const CE_Y_COUNT: u32 = 0x041C;
/// Launch the copy.
pub const CE_LAUNCH_DMA: u32 = 0x0300;

// CE_LAUNCH_DMA flags
/// Transfer type: pipelined.
pub const CE_LAUNCH_PIPELINED: u32 = 1 << 0;
/// Source memory type: physical.
pub const CE_SRC_TYPE_PHYS: u32 = 0 << 4;
/// Destination memory type: physical.
pub const CE_DST_TYPE_PHYS: u32 = 0 << 8;
/// Copy type: non-pipelined (safe, synchronous).
pub const CE_LAUNCH_NON_PIPELINED: u32 = 0;

// ── 2D Engine Methods — class C697 ──────────────────────────────────────────
// Used for rectangle fills, blits, format conversion.

/// Set 2D operation (copy, fill, etc).
pub const M2D_OPERATION: u32 = 0x02AC;
/// Set destination format.
pub const M2D_DST_FORMAT: u32 = 0x0200;
/// Set destination pitch.
pub const M2D_DST_PITCH: u32 = 0x0214;
/// Set destination width.
pub const M2D_DST_WIDTH: u32 = 0x0218;
/// Set destination height.
pub const M2D_DST_HEIGHT: u32 = 0x021C;
/// Destination address high.
pub const M2D_DST_ADDR_HI: u32 = 0x0220;
/// Destination address low.
pub const M2D_DST_ADDR_LO: u32 = 0x0224;
/// Set fill color (solid).
pub const M2D_SOLID_COLOR: u32 = 0x0580;
/// Render solid rectangle X (start, end packed).
pub const M2D_RENDER_SOLID_PRIM_X: u32 = 0x0600;
/// Render solid rectangle Y.
pub const M2D_RENDER_SOLID_PRIM_Y: u32 = 0x0604;

// 2D operation types
pub const M2D_OP_SRCCOPY: u32 = 0x03;
pub const M2D_OP_SOLID_FILL: u32 = 0x05;

// 2D pixel formats
pub const M2D_FORMAT_A8R8G8B8: u32 = 0xCF;
pub const M2D_FORMAT_X8R8G8B8: u32 = 0xE6;

// ── Subchannel Assignments ───────────────────────────────────────────────────
// By convention, NVIDIA drivers use these subchannel assignments:

/// Subchannel 0: 2D engine.
pub const SUBCHAN_2D: u32 = 0;
/// Subchannel 1: 3D engine.
pub const SUBCHAN_3D: u32 = 1;
/// Subchannel 2: Compute.
pub const SUBCHAN_COMPUTE: u32 = 2;
/// Subchannel 3: Copy Engine.
pub const SUBCHAN_CE: u32 = 3;

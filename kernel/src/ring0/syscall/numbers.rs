//! `syscall/numbers.rs` — Syscall number allocations.
//!
//! v1.8.8: central catalog of all syscall numbers used by FastOS.
//! Each "layer" (Ring 0 services, BMO Core, BMO GPU) has its own range
//! to avoid collisions as the system grows.
//!
//! ## Layout
//!
//! | Range       | Owner       | Purpose                       |
//! |-------------|-------------|-------------------------------|
//! | 0x00..=0x0F | Ring 0      | Process / thread management   |
//! | 0x10..=0x1F | Ring 0      | VFS / ramdisk                 |
//! | 0x20..=0x2F | Ring 0      | Framebuffer / VESA            |
//! | 0x30..=0x3F | Ring 0      | Input (keyboard, mouse)       |
//! | 0x40..=0x4F | Ring 0      | Time (clock, sleep)           |
//! | 0x50..=0x5F | Ring 0      | System info / debug           |
//! | 0x60..=0x6F | BMO Core    | Windowing API v1 (legacy)     |
//! | 0x70..=0x7F | BMO Core    | Windowing API v1.5 (partial)   |
//! | 0x80..=0xDF | (reserved)  | Future expansion              |
//! | 0xE0..=0xEF | Ring 0      | Power management              |
//! | 0xF0..=0xFF | Ring 0      | DebugPrint / panic             |
//! | 0x100..=0x1CF| BMO API v2  | Windowing 256 syscalls         |
//! | 0x1D0..=0x1DF| BMO API v2  | Reserved                      |
//! | 0x1E0..=0x1FF| BMO GPU    | GPU operations (reserved)     |
//!
//! v1.8.8: this is documentation. The actual syscall dispatch in
//! `arch::system_call_dispatcher` is keyed by the same numbers but
//! lives in the old location for backwards compatibility.

#![allow(dead_code)]

// ── Ring 0 process / thread (0x00..=0x0F) ─────────────────────────
pub const NR_PROCESS_EXIT: u32 = 0x00;
pub const NR_PROCESS_CREATE: u32 = 0x01;
pub const NR_PROCESS_WAIT: u32 = 0x02;
pub const NR_THREAD_YIELD: u32 = 0x03;
pub const NR_THREAD_CREATE: u32 = 0x04;
pub const NR_THREAD_EXIT: u32 = 0x05;
pub const NR_THREAD_JOIN: u32 = 0x06;
pub const NR_THREAD_SET_AFFINITY: u32 = 0x07;
pub const NR_THREAD_GET_ID: u32 = 0x08;
pub const NR_PROCESS_GET_ID: u32 = 0x09;

// ── Ring 0 VFS / ramdisk (0x10..=0x1F) ──────────────────────────
pub const NR_FS_OPEN: u32 = 0x10;
pub const NR_FS_CLOSE: u32 = 0x11;
pub const NR_FS_READ: u32 = 0x12;
pub const NR_FS_WRITE: u32 = 0x13;
pub const NR_FS_SEEK: u32 = 0x14;
pub const NR_FS_STAT: u32 = 0x15;
pub const NR_FS_READDIR: u32 = 0x16;
pub const NR_FS_MKDIR: u32 = 0x17;

// ── Ring 0 framebuffer / VESA (0x20..=0x2F) ─────────────────────
pub const NR_FB_INFO: u32 = 0x20;
pub const NR_FB_FILL: u32 = 0x21;
pub const NR_FB_BLIT: u32 = 0x22;
pub const NR_FB_PRESENT: u32 = 0x23;

// ── Ring 0 input (0x30..=0x3F) ──────────────────────────────────
pub const NR_INPUT_POLL_KEY: u32 = 0x30;
pub const NR_INPUT_POLL_MOUSE: u32 = 0x31;
pub const NR_INPUT_READ_KEY: u32 = 0x32;
pub const NR_INPUT_READ_MOUSE: u32 = 0x33;

// ── Ring 0 time (0x40..=0x4F) ───────────────────────────────────
pub const NR_CLOCK_GET_TIME: u32 = 0x40;
pub const NR_CLOCK_NANO_SLEEP: u32 = 0x41;
pub const NR_CLOCK_GET_TICKS: u32 = 0x42;

// ── Ring 0 system info / debug (0x50..=0x5F) ───────────────────
pub const NR_SYS_INFO: u32 = 0x50;
pub const NR_SYS_UPTIME: u32 = 0x51;
pub const NR_SYS_CPU_INFO: u32 = 0x52;
pub const NR_SYS_MEM_INFO: u32 = 0x53;
pub const NR_SYS_BEEP: u32 = 0x54;

// ── Power management (0xE0..=0xEF) ──────────────────────────────
pub const NR_POWEROFF: u32 = 0xE0;
pub const NR_REBOOT: u32 = 0xE1;
pub const NR_SUSPEND: u32 = 0xE2;
pub const NR_HIBERNATE: u32 = 0xE3;

// ── Debug (0xF0..=0xFF) ─────────────────────────────────────────
pub const NR_DEBUG_PRINT: u32 = 0xF0;
pub const NR_PANIC: u32 = 0xF1;

// ── BMO API v2 (0x100..=0x1CF) ──────────────────────────────────
pub const BMO_API_V2_BASE: u32 = 0x100;
pub const BMO_API_V2_END: u32 = 0x1CF;
pub const BMO_API_V2_COUNT: u32 = BMO_API_V2_END - BMO_API_V2_BASE + 1;

// ── BMO GPU (0x1E0..=0x1FF) — reserved for Phase 4 ─────────────
pub const BMO_GPU_BASE: u32 = 0x1E0;
pub const BMO_GPU_END: u32 = 0x1FF;

/// Returns true if `nr` belongs to the BMO API v2 range.
pub const fn is_bmo_api_v2(nr: u32) -> bool {
    nr >= BMO_API_V2_BASE && nr <= BMO_API_V2_END
}

/// Returns true if `nr` belongs to the BMO GPU range.
pub const fn is_bmo_gpu(nr: u32) -> bool {
    nr >= BMO_GPU_BASE && nr <= BMO_GPU_END
}

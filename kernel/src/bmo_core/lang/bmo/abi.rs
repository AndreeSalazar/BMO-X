//! BMO ABI — Syscall number constants for the AOT compiler.
//!
//! When BMO source code calls a BMO ABI function (windowing, FS, etc.),
//! the AOT compiler emits a `syscall` instruction with the corresponding
//! number from this table. This file is THE source of truth for the
//! mapping from semantic BMO ABI function to syscall number.
//!
//! Every function name listed here has a corresponding handler in
//! `crate::bmo_core::bmo_api::dispatch_syscall`.
//!
//! ## Calling convention
//!
//! Syscalls 0x100..=0x1FF use SysV AMD64:
//!   - RAX  = syscall number
//!   - RDI  = arg0
//!   - RSI  = arg1
//!   - RDX  = arg2
//!   - R10  = arg3
//!   - R8   = arg4
//!   - R9   = arg5
//!   - RAX  = return value (or 0xFFFF_FFFF_FFFF_FFFF on error)
//!
//! See `crate::bmo_core::bmo_abi::SPEC.md` for the full ABI spec.

#![allow(dead_code)]

// ─── Window manager (0x100..=0x10F) ───────────────────────────────────
pub const WIN_CREATE:        u16 = 0x100;
pub const WIN_DESTROY:       u16 = 0x101;
pub const WIN_SHOW:          u16 = 0x102;
pub const WIN_HIDE:          u16 = 0x103;
pub const WIN_SET_TITLE:     u16 = 0x104;
pub const WIN_SET_BOUNDS:    u16 = 0x105;
pub const WIN_GET_BOUNDS:    u16 = 0x106;
pub const WIN_INVALIDATE:    u16 = 0x107;
pub const WIN_BEGIN_PAINT:   u16 = 0x108;
pub const WIN_END_PAINT:     u16 = 0x109;
pub const WIN_PUSH_CLIP:     u16 = 0x10A;
pub const WIN_POP_CLIP:      u16 = 0x10B;
pub const WIN_SET_FOCUS:     u16 = 0x10C;
pub const WIN_GET_FOCUS:     u16 = 0x10D;
pub const WIN_REGISTER_CLASS: u16 = 0x10E;
pub const WIN_PUMP_EVENTS:   u16 = 0x10F;

// ─── Drawing primitives (0x110..=0x11F) ──────────────────────────────
pub const DRAW_CLEAR:        u16 = 0x110;
pub const DRAW_PIXEL:        u16 = 0x111;
pub const DRAW_LINE:         u16 = 0x112;
pub const DRAW_RECT:         u16 = 0x113;
pub const DRAW_CIRCLE:       u16 = 0x114;
pub const DRAW_TEXT:         u16 = 0x115;
pub const DRAW_BLIT:         u16 = 0x116;
pub const DRAW_GRADIENT_V:   u16 = 0x117;
pub const DRAW_GRADIENT_H:   u16 = 0x118;
pub const DRAW_ROUNDED_RECT: u16 = 0x119;

// ─── Window painting (0x120..=0x12F) ─────────────────────────────────
pub const WIN_FILL_RECT:     u16 = 0x120;
pub const WIN_DRAW_TEXT:     u16 = 0x121;
pub const WIN_DRAW_PIXEL:    u16 = 0x122;
pub const WIN_DRAW_LINE:     u16 = 0x123;
pub const WIN_DRAW_BLIT:     u16 = 0x124;
pub const WIN_DRAW_CIRCLE:   u16 = 0x125;

// ─── Compositor (0x130..=0x13F) ─────────────────────────────────────
pub const COMP_BEGIN_FRAME:  u16 = 0x130;
pub const COMP_END_FRAME:    u16 = 0x131;
pub const COMP_PRESENT:      u16 = 0x132;
pub const COMP_SET_TARGET:   u16 = 0x133;
pub const COMP_FLUSH:        u16 = 0x134;

// ─── Filesystem (0x140..=0x14F) ─────────────────────────────────────
pub const FS_OPEN:           u16 = 0x140;
pub const FS_CLOSE:          u16 = 0x141;
pub const FS_READ:           u16 = 0x142;
pub const FS_WRITE:          u16 = 0x143;
pub const FS_SEEK:           u16 = 0x144;
pub const FS_STAT:           u16 = 0x145;
pub const FS_MKDIR:          u16 = 0x146;
pub const FS_READDIR:        u16 = 0x147;
pub const FS_DELETE:         u16 = 0x148;
pub const FS_MOUNT:          u16 = 0x149;

// ─── Time (0x150..=0x15F) ───────────────────────────────────────────
pub const TIME_NOW_NS:       u16 = 0x150;
pub const TIME_NOW_US:       u16 = 0x151;
pub const TIME_SLEEP_NS:     u16 = 0x152;
pub const TIME_SLEEP_MS:     u16 = 0x153;

// ─── Input (0x160..=0x16F) ──────────────────────────────────────────
pub const INPUT_POLL_KEY:    u16 = 0x160;
pub const INPUT_POLL_MOUSE:  u16 = 0x161;
pub const INPUT_POLL_EVENT:  u16 = 0x162;

// ─── Audio (0x170..=0x17F) ──────────────────────────────────────────
pub const AUDIO_PLAY:        u16 = 0x170;
pub const AUDIO_STOP:        u16 = 0x171;
pub const AUDIO_BEEP:        u16 = 0x172;
pub const AUDIO_LOAD_WAVE:   u16 = 0x173;

// ─── Process / thread (0x180..=0x18F) ───────────────────────────────
pub const PROC_SPAWN:        u16 = 0x180;
pub const PROC_EXIT:         u16 = 0x181;
pub const PROC_GET_PID:      u16 = 0x182;
pub const PROC_GET_TID:      u16 = 0x183;
pub const PROC_YIELD:        u16 = 0x184;
pub const THREAD_CREATE:     u16 = 0x185;
pub const THREAD_EXIT:       u16 = 0x186;
pub const THREAD_JOIN:       u16 = 0x187;
pub const THREAD_SELF:       u16 = 0x188;

// ─── Memory (0x190..=0x19F) ─────────────────────────────────────────
pub const MEM_ALLOC:         u16 = 0x190;
pub const MEM_FREE:          u16 = 0x191;
pub const MEM_MAP:           u16 = 0x192;
pub const MEM_UNMAP:         u16 = 0x193;

// ─── IPC (0x1A0..=0x1AF) ────────────────────────────────────────────
pub const IPC_PORT_CREATE:   u16 = 0x1A0;
pub const IPC_PORT_SEND:     u16 = 0x1A1;
pub const IPC_PORT_RECV:     u16 = 0x1A2;
pub const IPC_PORT_CLOSE:    u16 = 0x1A3;

// ─── Diagnostics (0x1F0..=0x1FF) ────────────────────────────────────
pub const DIAG_PRINT:        u16 = 0x1F0;
pub const DIAG_TRACE:        u16 = 0x1F1;
pub const DIAG_ASSERT:       u16 = 0x1F2;
pub const DIAG_PANIC:        u16 = 0x1F3;

// ─── Name → syscall number resolution ──────────────────────────────
/// Look up a syscall number by its BMO ABI name (e.g. "win_create").
/// Returns None if the name is not a known BMO ABI function.
pub fn resolve(name: &str) -> Option<u16> {
    match name {
        // Window manager
        "win_create"         => Some(WIN_CREATE),
        "win_destroy"        => Some(WIN_DESTROY),
        "win_show"           => Some(WIN_SHOW),
        "win_hide"           => Some(WIN_HIDE),
        "win_set_title"      => Some(WIN_SET_TITLE),
        "win_set_bounds"     => Some(WIN_SET_BOUNDS),
        "win_get_bounds"     => Some(WIN_GET_BOUNDS),
        "win_invalidate"     => Some(WIN_INVALIDATE),
        "win_begin_paint"    => Some(WIN_BEGIN_PAINT),
        "win_end_paint"      => Some(WIN_END_PAINT),
        "win_push_clip"      => Some(WIN_PUSH_CLIP),
        "win_pop_clip"       => Some(WIN_POP_CLIP),
        "win_set_focus"      => Some(WIN_SET_FOCUS),
        "win_get_focus"      => Some(WIN_GET_FOCUS),
        "win_register_class" => Some(WIN_REGISTER_CLASS),
        "win_pump_events"    => Some(WIN_PUMP_EVENTS),

        // Drawing
        "draw_clear"         => Some(DRAW_CLEAR),
        "draw_pixel"         => Some(DRAW_PIXEL),
        "draw_line"          => Some(DRAW_LINE),
        "draw_rect"          => Some(DRAW_RECT),
        "draw_circle"        => Some(DRAW_CIRCLE),
        "draw_text"          => Some(DRAW_TEXT),
        "draw_blit"          => Some(DRAW_BLIT),
        "draw_gradient_v"    => Some(DRAW_GRADIENT_V),
        "draw_gradient_h"    => Some(DRAW_GRADIENT_H),
        "draw_rounded_rect"  => Some(DRAW_ROUNDED_RECT),

        // Window painting
        "win_fill_rect"      => Some(WIN_FILL_RECT),
        "win_draw_text"      => Some(WIN_DRAW_TEXT),
        "win_draw_pixel"     => Some(WIN_DRAW_PIXEL),
        "win_draw_line"      => Some(WIN_DRAW_LINE),
        "win_draw_blit"      => Some(WIN_DRAW_BLIT),
        "win_draw_circle"    => Some(WIN_DRAW_CIRCLE),

        // Compositor
        "comp_begin_frame"   => Some(COMP_BEGIN_FRAME),
        "comp_end_frame"     => Some(COMP_END_FRAME),
        "comp_present"       => Some(COMP_PRESENT),
        "comp_set_target"    => Some(COMP_SET_TARGET),
        "comp_flush"         => Some(COMP_FLUSH),

        // Filesystem
        "fs_open"            => Some(FS_OPEN),
        "fs_close"           => Some(FS_CLOSE),
        "fs_read"            => Some(FS_READ),
        "fs_write"           => Some(FS_WRITE),
        "fs_seek"            => Some(FS_SEEK),
        "fs_stat"            => Some(FS_STAT),
        "fs_mkdir"           => Some(FS_MKDIR),
        "fs_readdir"         => Some(FS_READDIR),
        "fs_delete"          => Some(FS_DELETE),
        "fs_mount"           => Some(FS_MOUNT),

        // Time
        "time_now_ns"        => Some(TIME_NOW_NS),
        "time_now_us"        => Some(TIME_NOW_US),
        "time_sleep_ns"      => Some(TIME_SLEEP_NS),
        "time_sleep_ms"      => Some(TIME_SLEEP_MS),

        // Input
        "input_poll_key"     => Some(INPUT_POLL_KEY),
        "input_poll_mouse"   => Some(INPUT_POLL_MOUSE),
        "input_poll_event"   => Some(INPUT_POLL_EVENT),

        // Audio
        "audio_play"         => Some(AUDIO_PLAY),
        "audio_stop"         => Some(AUDIO_STOP),
        "audio_beep"         => Some(AUDIO_BEEP),
        "audio_load_wave"    => Some(AUDIO_LOAD_WAVE),

        // Process
        "proc_spawn"         => Some(PROC_SPAWN),
        "proc_exit"          => Some(PROC_EXIT),
        "proc_get_pid"       => Some(PROC_GET_PID),
        "proc_get_tid"       => Some(PROC_GET_TID),
        "proc_yield"         => Some(PROC_YIELD),
        "thread_create"      => Some(THREAD_CREATE),
        "thread_exit"        => Some(THREAD_EXIT),
        "thread_join"        => Some(THREAD_JOIN),
        "thread_self"        => Some(THREAD_SELF),

        // Memory
        "mem_alloc"          => Some(MEM_ALLOC),
        "mem_free"           => Some(MEM_FREE),
        "mem_map"            => Some(MEM_MAP),
        "mem_unmap"          => Some(MEM_UNMAP),

        // IPC
        "ipc_port_create"    => Some(IPC_PORT_CREATE),
        "ipc_port_send"      => Some(IPC_PORT_SEND),
        "ipc_port_recv"      => Some(IPC_PORT_RECV),
        "ipc_port_close"     => Some(IPC_PORT_CLOSE),

        // Diagnostics
        "diag_print"         => Some(DIAG_PRINT),
        "diag_trace"         => Some(DIAG_TRACE),
        "diag_assert"        => Some(DIAG_ASSERT),
        "diag_panic"         => Some(DIAG_PANIC),

        _ => None,
    }
}

/// Returns true if the given name is a known BMO ABI function.
#[inline(always)]
pub fn is_abi(name: &str) -> bool { resolve(name).is_some() }

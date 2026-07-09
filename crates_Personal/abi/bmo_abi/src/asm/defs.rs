//! Embedded syscall, type, and ABI definitions — no filesystem needed.
//!
//! These constants are the compiled-in source of truth, replacing the
//! Semantic_ASM/*.toml files. Frontends call `bmo_abi::asm::syscalls()`
//! instead of reading `bmo/proc.toml` from disk.

use alloc::vec::Vec;
use alloc::string::String;
use alloc::collections::BTreeMap;
use super::{SyscallDef, TypeAlias, AbiDataModel};

/// ── All syscall definitions (14 categories, ~100 entries) ─────────

pub const SYSCALLS: &[(&str, u32, u8)] = &[

    // ── wm (0x100..0x10F): Window Manager ─────────────────────────
    ("bmo_wm_create_window",     0x100, 4),
    ("bmo_wm_destroy_window",    0x101, 1),
    ("bmo_wm_show_window",       0x102, 1),
    ("bmo_wm_hide_window",       0x103, 1),
    ("bmo_wm_set_title",         0x104, 2),
    ("bmo_wm_get_bounds",        0x105, 2),
    ("bmo_wm_set_bounds",        0x106, 5),
    ("bmo_wm_begin_paint",       0x107, 1),
    ("bmo_wm_end_paint",         0x108, 1),
    ("bmo_wm_push_clip",         0x109, 5),
    ("bmo_wm_pop_clip",          0x10A, 1),
    ("bmo_wm_set_focus",         0x10B, 1),
    ("bmo_wm_get_focus",         0x10C, 0),
    ("bmo_wm_register_class",    0x10D, 2),
    ("bmo_wm_pump_events",       0x10E, 2),
    ("bmo_wm_translate_message", 0x10F, 2),

    // ── draw (0x110..0x119) ────────────────────────────────────────
    ("bmo_draw_clear",           0x110, 1),
    ("bmo_draw_pixel",           0x111, 3),
    ("bmo_draw_line",            0x112, 5),
    ("bmo_draw_rect",            0x113, 5),
    ("bmo_draw_circle",          0x114, 4),
    ("bmo_draw_text",            0x115, 5),
    ("bmo_draw_blit",            0x116, 6),
    ("bmo_draw_gradient_v",      0x117, 6),
    ("bmo_draw_gradient_h",      0x118, 6),
    ("bmo_draw_rounded_rect",    0x119, 6),

    // ── winpaint (0x120..0x125) ────────────────────────────────────
    ("bmo_winpaint_fill_rect",   0x120, 5),
    ("bmo_winpaint_draw_text",   0x121, 5),
    ("bmo_winpaint_draw_pixel",  0x122, 3),
    ("bmo_winpaint_draw_line",   0x123, 5),
    ("bmo_winpaint_draw_blit",   0x124, 6),
    ("bmo_winpaint_draw_circle", 0x125, 4),

    // ── compositor (0x130..0x134) ──────────────────────────────────
    ("bmo_compositor_begin_frame", 0x130, 0),
    ("bmo_compositor_end_frame",   0x131, 0),
    ("bmo_compositor_present",     0x132, 1),
    ("bmo_compositor_set_target",  0x133, 1),
    ("bmo_compositor_flush",       0x134, 0),

    // ── fs / io (0x140..0x149) ─────────────────────────────────────
    ("bmo_open",                 0x140, 2),
    ("bmo_fs_open",              0x140, 2),
    ("bmo_close",                0x141, 1),
    ("bmo_fs_close",             0x141, 1),
    ("bmo_read",                 0x142, 3),
    ("bmo_fs_read",              0x142, 3),
    ("bmo_write",                0x143, 3),
    ("bmo_fs_write",             0x143, 3),
    ("bmo_seek",                 0x144, 3),
    ("bmo_fs_seek",              0x144, 3),
    ("bmo_stat",                 0x145, 2),
    ("bmo_fs_stat",              0x145, 2),
    ("bmo_mkdir",                0x146, 1),
    ("bmo_fs_mkdir",             0x146, 1),
    ("bmo_readdir",              0x147, 2),
    ("bmo_fs_readdir",           0x147, 2),
    ("bmo_delete",               0x148, 1),
    ("bmo_fs_delete",            0x148, 1),
    ("bmo_mount",                0x149, 2),
    ("bmo_fs_mount",             0x149, 2),

    // ── time (0x150..0x153) ────────────────────────────────────────
    ("bmo_time_now_ns",          0x150, 0),
    ("bmo_time_now_us",          0x151, 0),
    ("bmo_time_sleep_ns",        0x152, 1),
    ("bmo_time_sleep_ms",        0x153, 1),

    // ── input (0x160..0x162) ───────────────────────────────────────
    ("bmo_poll_key",             0x160, 0),
    ("bmo_input_poll_key",       0x160, 0),
    ("bmo_poll_mouse",           0x161, 0),
    ("bmo_input_poll_mouse",     0x161, 0),
    ("bmo_poll_event",           0x162, 1),
    ("bmo_input_poll_event",     0x162, 1),

    // ── audio (0x170..0x173) ───────────────────────────────────────
    ("bmo_audio_play",           0x170, 2),
    ("bmo_audio_stop",           0x171, 0),
    ("bmo_beep",                 0x172, 2),
    ("bmo_audio_beep",           0x172, 2),
    ("bmo_audio_load_wave",      0x173, 2),

    // ── proc (0x180..0x188) ────────────────────────────────────────
    ("bmo_spawn",                0x180, 1),
    ("bmo_proc_spawn",           0x180, 1),
    ("bmo_exit",                 0x181, 1),
    ("bmo_proc_exit",            0x181, 1),
    ("bmo_getpid",               0x182, 0),
    ("bmo_proc_get_pid",         0x182, 0),
    ("bmo_gettid",               0x183, 0),
    ("bmo_proc_get_tid",         0x183, 0),
    ("bmo_yield",                0x184, 0),
    ("bmo_proc_yield",           0x184, 0),
    ("bmo_thread_create",        0x185, 3),
    ("bmo_thread_exit",          0x186, 1),
    ("bmo_thread_join",          0x187, 1),
    ("bmo_thread_self",          0x188, 0),

    // ── mem (0x190..0x197) ─────────────────────────────────────────
    ("bmo_mem_alloc",            0x190, 1),
    ("bmo_mem_free",             0x191, 2),
    ("bmo_mem_map",              0x192, 2),
    ("bmo_mem_unmap",            0x193, 2),
    ("bmo_befcore_send",         0x194, 3),
    ("bmo_befcore_recv",         0x195, 2),
    ("bmo_befcore_poll",         0x196, 0),
    ("bmo_befcore_register",     0x197, 2),

    // ── ipc (0x1A0..0x1A3) ─────────────────────────────────────────
    ("bmo_ipc_port_create",      0x1A0, 0),
    ("bmo_ipc_port_send",        0x1A1, 3),
    ("bmo_ipc_port_recv",        0x1A2, 3),
    ("bmo_ipc_port_close",       0x1A3, 1),

    // ── surface (0x1C0..0x1CF) ─────────────────────────────────────
    ("bmo_surface_map",          0x1C0, 2),
    ("bmo_surface_unmap",        0x1C1, 1),
    ("bmo_surface_present",      0x1C2, 1),

    // ── diag (0x1F0..0x1F3) ────────────────────────────────────────
    ("bmo_debug_print",          0x1F0, 2),
    ("bmo_diag_print",           0x1F0, 2),
    ("bmo_debug_trace",          0x1F1, 1),
    ("bmo_diag_trace",           0x1F1, 1),
    ("bmo_debug_assert",         0x1F2, 1),
    ("bmo_diag_assert",          0x1F2, 1),
    ("bmo_debug_panic",          0x1F3, 1),
    ("bmo_diag_panic",           0x1F3, 1),
];

/// ── Type aliases (from types.toml) ────────────────────────────────

pub const TYPE_ALIASES: &[(&str, &str, Option<i64>)] = &[
    ("size_t",    "u64", None),
    ("ssize_t",   "i64", None),
    ("intptr_t",  "i64", None),
    ("uintptr_t", "u64", None),
    ("uint8_t",   "u8",  None),
    ("uint16_t",  "u16", None),
    ("uint32_t",  "u32", None),
    ("uint64_t",  "u64", None),
    ("int8_t",    "i8",  None),
    ("int16_t",   "i16", None),
    ("int32_t",   "i32", None),
    ("int64_t",   "i64", None),
    ("pid_t",     "i32", None),
    ("uid_t",     "u32", None),
    ("gid_t",     "u32", None),
    ("mode_t",    "u32", None),
    ("off_t",     "i64", None),
    ("dev_t",     "u64", None),
    ("ino_t",     "u64", None),
    ("nlink_t",   "u64", None),
    ("blksize_t", "i64", None),
    ("blkcnt_t",  "i64", None),
    ("time_t",    "i64", None),
    ("BmoHandle", "u64", None),
    ("BmoStatus", "u64", None),
    ("BmoSlice",  "u64", None),
    ("BmoStr",    "u64", None),
    ("NULL",      "",    Some(0)),
    ("BMO_API_BASE", "", Some(0x100)),
    ("BMO_API_END",  "", Some(0x1FF)),
];

/// ── ABI data model (from abi.toml) ────────────────────────────────

pub const POINTER_SIZE: u8 = 8;
pub const ENDIANNESS: &str = "little";
pub const CHAR_IS_SIGNED: bool = true;
pub const TYPE_SIZES: &[(&str, u8)] = &[
    ("void", 0), ("char", 1), ("short", 2), ("int", 4),
    ("long", 8), ("long_long", 8), ("float", 4), ("double", 8),
    ("pointer", 8),
];

// ── Helper functions ───────────────────────────────────────────────

/// All embedded syscall definitions (no filesystem needed).
pub fn syscalls() -> Vec<SyscallDef> {
    SYSCALLS.iter().map(|(n, nr, c)| SyscallDef {
        name: String::from(*n), nr: *nr, arg_count: *c,
    }).collect()
}

/// All embedded type aliases.
pub fn type_aliases() -> Vec<TypeAlias> {
    TYPE_ALIASES.iter().map(|(n, u, v)| TypeAlias {
        name: String::from(*n), underlying: String::from(*u), value: *v,
    }).collect()
}

/// Embedded ABI data model.
pub fn abi_model() -> AbiDataModel {
    AbiDataModel {
        pointer_size: POINTER_SIZE,
        endianness: String::from(ENDIANNESS),
        type_sizes: TYPE_SIZES.iter().map(|(k, v)| (String::from(*k), *v)).collect(),
    }
}

// Generated from bmo_abi::asm::defs — matches old build.rs naming
// Generated 2026-07-07

// ═══════════════════════════════════════════════
//  WM — Window Manager (0x100..0x10F)
// ═══════════════════════════════════════════════
pub const NR_WM_CREATE_WINDOW: u32 = 0x100;
pub const NR_WM_DESTROY_WINDOW: u32 = 0x101;
pub const NR_WM_SHOW_WINDOW: u32 = 0x102;
pub const NR_WM_HIDE_WINDOW: u32 = 0x103;
pub const NR_WM_SET_TITLE: u32 = 0x104;
pub const NR_WM_GET_BOUNDS: u32 = 0x105;
pub const NR_WM_SET_BOUNDS: u32 = 0x106;
pub const NR_WM_BEGIN_PAINT: u32 = 0x107;
pub const NR_WM_END_PAINT: u32 = 0x108;
pub const NR_WM_PUSH_CLIP: u32 = 0x109;
pub const NR_WM_POP_CLIP: u32 = 0x10A;
pub const NR_WM_SET_FOCUS: u32 = 0x10B;
pub const NR_WM_GET_FOCUS: u32 = 0x10C;
pub const NR_WM_REGISTER_CLASS: u32 = 0x10D;
pub const NR_WM_PUMP_EVENTS: u32 = 0x10E;
pub const NR_WM_TRANSLATE_MESSAGE: u32 = 0x10F;

// ═══════════════════════════════════════════════
//  DRAW — Draw (0x110..0x119)
// ═══════════════════════════════════════════════
pub const NR_DRAW_CLEAR: u32 = 0x110;
pub const NR_DRAW_PIXEL: u32 = 0x111;
pub const NR_DRAW_LINE: u32 = 0x112;
pub const NR_DRAW_RECT: u32 = 0x113;
pub const NR_DRAW_CIRCLE: u32 = 0x114;
pub const NR_DRAW_TEXT: u32 = 0x115;
pub const NR_DRAW_BLIT: u32 = 0x116;
pub const NR_DRAW_GRADIENT_V: u32 = 0x117;
pub const NR_DRAW_GRADIENT_H: u32 = 0x118;
pub const NR_DRAW_ROUNDED_RECT: u32 = 0x119;

// ═══════════════════════════════════════════════
//  WINPAINT — Window Painting (0x120..0x125)
// ═══════════════════════════════════════════════
pub const NR_WINPAINT_FILL_RECT: u32 = 0x120;
pub const NR_WINPAINT_DRAW_TEXT: u32 = 0x121;
pub const NR_WINPAINT_DRAW_PIXEL: u32 = 0x122;
pub const NR_WINPAINT_DRAW_LINE: u32 = 0x123;
pub const NR_WINPAINT_DRAW_BLIT: u32 = 0x124;
pub const NR_WINPAINT_DRAW_CIRCLE: u32 = 0x125;

// ═══════════════════════════════════════════════
//  COMPOSITOR — Compositor (0x130..0x134)
// ═══════════════════════════════════════════════
pub const NR_COMPOSITOR_BEGIN_FRAME: u32 = 0x130;
pub const NR_COMPOSITOR_END_FRAME: u32 = 0x131;
pub const NR_COMPOSITOR_PRESENT: u32 = 0x132;
pub const NR_COMPOSITOR_SET_TARGET: u32 = 0x133;
pub const NR_COMPOSITOR_FLUSH: u32 = 0x134;

// ═══════════════════════════════════════════════
//  FS — Filesystem (0x140..0x149)
// ═══════════════════════════════════════════════
pub const NR_FS_OPEN: u32 = 0x140;
pub const NR_FS_CLOSE: u32 = 0x141;
pub const NR_FS_READ: u32 = 0x142;
pub const NR_FS_WRITE: u32 = 0x143;
pub const NR_FS_SEEK: u32 = 0x144;
pub const NR_FS_STAT: u32 = 0x145;
pub const NR_FS_MKDIR: u32 = 0x146;
pub const NR_FS_READDIR: u32 = 0x147;
pub const NR_FS_DELETE: u32 = 0x148;
pub const NR_FS_MOUNT: u32 = 0x149;

// ═══════════════════════════════════════════════
//  TIME — Time (0x150..0x153)
// ═══════════════════════════════════════════════
pub const NR_TIME_NOW_NS: u32 = 0x150;
pub const NR_TIME_NOW_US: u32 = 0x151;
pub const NR_TIME_SLEEP_NS: u32 = 0x152;
pub const NR_TIME_SLEEP_MS: u32 = 0x153;

// ═══════════════════════════════════════════════
//  INPUT — Input (0x160..0x162)
// ═══════════════════════════════════════════════
pub const NR_POLL_KEY: u32 = 0x160;
pub const NR_INPUT_POLL_KEY: u32 = 0x160;
pub const NR_POLL_MOUSE: u32 = 0x161;
pub const NR_INPUT_POLL_MOUSE: u32 = 0x161;
pub const NR_POLL_EVENT: u32 = 0x162;
pub const NR_INPUT_POLL_EVENT: u32 = 0x162;

// ═══════════════════════════════════════════════
//  AUDIO — Audio (0x170..0x173)
// ═══════════════════════════════════════════════
pub const NR_AUDIO_PLAY: u32 = 0x170;
pub const NR_AUDIO_STOP: u32 = 0x171;
pub const NR_AUDIO_BEEP: u32 = 0x172;
pub const NR_AUDIO_LOAD_WAVE: u32 = 0x173;

// ═══════════════════════════════════════════════
//  PROC — Process / Thread (0x180..0x188)
// ═══════════════════════════════════════════════
pub const NR_PROC_SPAWN: u32 = 0x180;
pub const NR_PROC_EXIT: u32 = 0x181;
pub const NR_PROC_GET_PID: u32 = 0x182;
pub const NR_PROC_GET_TID: u32 = 0x183;
pub const NR_PROC_YIELD: u32 = 0x184;
pub const NR_THREAD_CREATE: u32 = 0x185;
pub const NR_THREAD_EXIT: u32 = 0x186;
pub const NR_THREAD_JOIN: u32 = 0x187;
pub const NR_THREAD_SELF: u32 = 0x188;

// ═══════════════════════════════════════════════
//  MEM — Memory + BEFCore (0x190..0x197)
// ═══════════════════════════════════════════════
pub const NR_MEM_ALLOC: u32 = 0x190;
pub const NR_MEM_FREE: u32 = 0x191;
pub const NR_MEM_MAP: u32 = 0x192;
pub const NR_MEM_UNMAP: u32 = 0x193;
pub const NR_BEFCORE_SEND: u32 = 0x194;
pub const NR_BEFCORE_RECV: u32 = 0x195;
pub const NR_BEFCORE_POLL: u32 = 0x196;
pub const NR_BEFCORE_REGISTER: u32 = 0x197;

// ═══════════════════════════════════════════════
//  IPC — IPC (0x1A0..0x1A3)
// ═══════════════════════════════════════════════
pub const NR_IPC_PORT_CREATE: u32 = 0x1A0;
pub const NR_IPC_PORT_SEND: u32 = 0x1A1;
pub const NR_IPC_PORT_RECV: u32 = 0x1A2;
pub const NR_IPC_PORT_CLOSE: u32 = 0x1A3;

// ═══════════════════════════════════════════════
//  SURFACE — Surface mapping (0x1C0..0x1CF)
// ═══════════════════════════════════════════════
pub const NR_SURFACE_MAP: u32 = 0x1C0;
pub const NR_SURFACE_UNMAP: u32 = 0x1C1;
pub const NR_SURFACE_PRESENT: u32 = 0x1C2;

// ═══════════════════════════════════════════════
//  DIAG — Diagnostics (0x1F0..0x1F3)
// ═══════════════════════════════════════════════
pub const NR_DEBUG_PRINT: u32 = 0x1F0;
pub const NR_DEBUG_TRACE: u32 = 0x1F1;
pub const NR_DEBUG_ASSERT: u32 = 0x1F2;
pub const NR_DEBUG_PANIC: u32 = 0x1F3;

// ── Helpers ──

pub const fn is_bmo_api(nr: u32) -> bool {
    nr >= 0x100 && nr <= 0x1FF
}

pub const fn is_befcore(nr: u32) -> bool {
    nr >= 0x194 && nr <= 0x197
}

pub fn name(nr: u32) -> &'static str {
    match nr {
        0x100 => "wm_create_window",
        0x101 => "wm_destroy_window",
        0x102 => "wm_show_window",
        0x103 => "wm_hide_window",
        0x104 => "wm_set_title",
        0x105 => "wm_get_bounds",
        0x106 => "wm_set_bounds",
        0x107 => "wm_begin_paint",
        0x108 => "wm_end_paint",
        0x109 => "wm_push_clip",
        0x10A => "wm_pop_clip",
        0x10B => "wm_set_focus",
        0x10C => "wm_get_focus",
        0x10D => "wm_register_class",
        0x10E => "wm_pump_events",
        0x10F => "wm_translate_message",
        0x110 => "draw_clear",
        0x111 => "draw_pixel",
        0x112 => "draw_line",
        0x113 => "draw_rect",
        0x114 => "draw_circle",
        0x115 => "draw_text",
        0x116 => "draw_blit",
        0x117 => "draw_gradient_v",
        0x118 => "draw_gradient_h",
        0x119 => "draw_rounded_rect",
        0x120 => "winpaint_fill_rect",
        0x121 => "winpaint_draw_text",
        0x122 => "winpaint_draw_pixel",
        0x123 => "winpaint_draw_line",
        0x124 => "winpaint_draw_blit",
        0x125 => "winpaint_draw_circle",
        0x130 => "compositor_begin_frame",
        0x131 => "compositor_end_frame",
        0x132 => "compositor_present",
        0x133 => "compositor_set_target",
        0x134 => "compositor_flush",
        0x140 => "fs_open",
        0x141 => "fs_close",
        0x142 => "fs_read",
        0x143 => "fs_write",
        0x144 => "fs_seek",
        0x145 => "fs_stat",
        0x146 => "fs_mkdir",
        0x147 => "fs_readdir",
        0x148 => "fs_delete",
        0x149 => "fs_mount",
        0x150 => "time_now_ns",
        0x151 => "time_now_us",
        0x152 => "time_sleep_ns",
        0x153 => "time_sleep_ms",
        0x160 => "poll_key",
        0x161 => "poll_mouse",
        0x162 => "poll_event",
        0x170 => "audio_play",
        0x171 => "audio_stop",
        0x172 => "audio_beep",
        0x173 => "audio_load_wave",
        0x180 => "proc_spawn",
        0x181 => "proc_exit",
        0x182 => "proc_get_pid",
        0x183 => "proc_get_tid",
        0x184 => "proc_yield",
        0x185 => "thread_create",
        0x186 => "thread_exit",
        0x187 => "thread_join",
        0x188 => "thread_self",
        0x190 => "mem_alloc",
        0x191 => "mem_free",
        0x192 => "mem_map",
        0x193 => "mem_unmap",
        0x194 => "befcore_send",
        0x195 => "befcore_recv",
        0x196 => "befcore_poll",
        0x197 => "befcore_register",
        0x1A0 => "ipc_port_create",
        0x1A1 => "ipc_port_send",
        0x1A2 => "ipc_port_recv",
        0x1A3 => "ipc_port_close",
        0x1C0 => "surface_map",
        0x1C1 => "surface_unmap",
        0x1C2 => "surface_present",
        0x1F0 => "debug_print",
        0x1F1 => "debug_trace",
        0x1F2 => "debug_assert",
        0x1F3 => "debug_panic",
        _ => "<unknown bmo_api syscall>",
    }
}

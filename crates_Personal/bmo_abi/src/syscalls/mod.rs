//! `bmo_abi::syscalls` — Tabla única de syscall numbers 0x100..0x1FF.
//!
//! **Resuelve el conflicto 0x180** (PROC_SPAWN vs MAP_SURFACE) que
//! detectó el análisis: en este módulo hay UNA sola fuente de verdad
//! para los números de syscall. `lang/bmo/abi.rs` y `bmo_api/syscall.rs`
//! referencian esta tabla en vez de tener sus propias definiciones.
//!
//! ## Layout
//!
//! | Rango | Owner | Notas |
//! |-------|-------|-------|
//! | 0x100..0x10F | WM | Window create/destroy/show/hide/title/bounds/clip |
//! | 0x110..0x119 | Draw | Clear/pixel/line/rect/circle/text/blit/gradient/round |
//! | 0x120..0x125 | WinPaint | fill_rect/draw_text/draw_pixel/line/blit/circle |
//! | 0x130..0x134 | Compositor | begin_frame/end_frame/present/set_target/flush |
//! | 0x140..0x149 | FS | open/close/read/write/seek/stat/mkdir/readdir/delete/mount |
//! | 0x150..0x153 | Time | now_ns/now_us/sleep_ns/sleep_ms |
//! | 0x160..0x162 | Input | poll_key/poll_mouse/poll_event |
//! | 0x170..0x173 | Audio | play/stop/beep/load_wave |
//! | 0x180..0x188 | Process | spawn/exit/get_pid/get_tid/yield/thread_create/thread_exit/thread_join/thread_self |
//! | 0x190..0x197 | Memory | alloc/free/map/unmap + BEFCore send/recv/poll |
//! | 0x1A0..0x1A3 | IPC | port_create/port_send/port_recv/port_close |
//! | 0x1F0..0x1F3 | Diagnostics | print/trace/assert/panic |
//!
//! Status: ✅ COMPLETO — tabla única v1.8.8

#![allow(dead_code)]

// ═══════════════════════════════════════════════════════════════════════
//  Window Manager (0x100..0x10F)
// ═══════════════════════════════════════════════════════════════════════
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

// ═══════════════════════════════════════════════════════════════════════
//  Draw (0x110..0x119)
// ═══════════════════════════════════════════════════════════════════════
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

// ═══════════════════════════════════════════════════════════════════════
//  Window Painting (0x120..0x125)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_WINPAINT_FILL_RECT: u32 = 0x120;
pub const NR_WINPAINT_DRAW_TEXT: u32 = 0x121;
pub const NR_WINPAINT_DRAW_PIXEL: u32 = 0x122;
pub const NR_WINPAINT_DRAW_LINE: u32 = 0x123;
pub const NR_WINPAINT_DRAW_BLIT: u32 = 0x124;
pub const NR_WINPAINT_DRAW_CIRCLE: u32 = 0x125;

// ═══════════════════════════════════════════════════════════════════════
//  Compositor (0x130..0x134)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_COMPOSITOR_BEGIN_FRAME: u32 = 0x130;
pub const NR_COMPOSITOR_END_FRAME: u32 = 0x131;
pub const NR_COMPOSITOR_PRESENT: u32 = 0x132;
pub const NR_COMPOSITOR_SET_TARGET: u32 = 0x133;
pub const NR_COMPOSITOR_FLUSH: u32 = 0x134;

// ═══════════════════════════════════════════════════════════════════════
//  Filesystem (0x140..0x149)
// ═══════════════════════════════════════════════════════════════════════
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

// ═══════════════════════════════════════════════════════════════════════
//  Time (0x150..0x153)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_TIME_NOW_NS: u32 = 0x150;
pub const NR_TIME_NOW_US: u32 = 0x151;
pub const NR_TIME_SLEEP_NS: u32 = 0x152;
pub const NR_TIME_SLEEP_MS: u32 = 0x153;

// ═══════════════════════════════════════════════════════════════════════
//  Input (0x160..0x162)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_INPUT_POLL_KEY: u32 = 0x160;
pub const NR_INPUT_POLL_MOUSE: u32 = 0x161;
pub const NR_INPUT_POLL_EVENT: u32 = 0x162;

// ═══════════════════════════════════════════════════════════════════════
//  Audio (0x170..0x173)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_AUDIO_PLAY: u32 = 0x170;
pub const NR_AUDIO_STOP: u32 = 0x171;
pub const NR_AUDIO_BEEP: u32 = 0x172;
pub const NR_AUDIO_LOAD_WAVE: u32 = 0x173;

// ═══════════════════════════════════════════════════════════════════════
//  Process / Thread (0x180..0x188) — RESUELTO: PROC gana este rango
// ═══════════════════════════════════════════════════════════════════════
pub const NR_PROC_SPAWN: u32 = 0x180;
pub const NR_PROC_EXIT: u32 = 0x181;
pub const NR_PROC_GET_PID: u32 = 0x182;
pub const NR_PROC_GET_TID: u32 = 0x183;
pub const NR_PROC_YIELD: u32 = 0x184;
pub const NR_THREAD_CREATE: u32 = 0x185;
pub const NR_THREAD_EXIT: u32 = 0x186;
pub const NR_THREAD_JOIN: u32 = 0x187;
pub const NR_THREAD_SELF: u32 = 0x188;

// ═══════════════════════════════════════════════════════════════════════
//  Memory + BEFCore (0x190..0x197) — Mueve MAP_SURFACE a 0x1C0
// ═══════════════════════════════════════════════════════════════════════
pub const NR_MEM_ALLOC: u32 = 0x190;
pub const NR_MEM_FREE: u32 = 0x191;
pub const NR_MEM_MAP: u32 = 0x192;
pub const NR_MEM_UNMAP: u32 = 0x193;
pub const NR_BEFCORE_SEND: u32 = 0x194;
pub const NR_BEFCORE_RECV: u32 = 0x195;
pub const NR_BEFCORE_POLL: u32 = 0x196;
pub const NR_BEFCORE_REGISTER: u32 = 0x197;

// ═══════════════════════════════════════════════════════════════════════
//  IPC (0x1A0..0x1A3)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_IPC_PORT_CREATE: u32 = 0x1A0;
pub const NR_IPC_PORT_SEND: u32 = 0x1A1;
pub const NR_IPC_PORT_RECV: u32 = 0x1A2;
pub const NR_IPC_PORT_CLOSE: u32 = 0x1A3;

// ═══════════════════════════════════════════════════════════════════════
//  Surface mapping (0x1C0..0x1CF) — RESUELTO: movido aquí
// ═══════════════════════════════════════════════════════════════════════
pub const NR_SURFACE_MAP: u32 = 0x1C0;
pub const NR_SURFACE_UNMAP: u32 = 0x1C1;
pub const NR_SURFACE_PRESENT: u32 = 0x1C2;

// ═══════════════════════════════════════════════════════════════════════
//  Diagnostics (0x1F0..0x1F3)
// ═══════════════════════════════════════════════════════════════════════
pub const NR_DEBUG_PRINT: u32 = 0x1F0;
pub const NR_DEBUG_TRACE: u32 = 0x1F1;
pub const NR_DEBUG_ASSERT: u32 = 0x1F2;
pub const NR_DEBUG_PANIC: u32 = 0x1F3;

// ── Helpers ────────────────────────────────────────────────────────────

/// Retorna true si `nr` está en el rango BMO API (0x100..0x1FF).
pub const fn is_bmo_api(nr: u32) -> bool {
    nr >= 0x100 && nr <= 0x1FF
}

/// Retorna true si `nr` es un BEFCore message syscall.
pub const fn is_befcore(nr: u32) -> bool {
    nr >= NR_BEFCORE_SEND && nr <= NR_BEFCORE_REGISTER
}

// ═══════════════════════════════════════════════════════════════════════
//  Syscall wrappers (x86_64)
// ═══════════════════════════════════════════════════════════════════════

/// Resultado de un syscall: (code, value) = (RAX, RDX).
///
/// El kernel siempre devuelve `BmoStatus` en RAX:RDX, donde:
/// - RAX bits [31:0] = código de estado (0 = OK)
/// - RDX = valor adicional (handle, contador, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallResult(pub u64, pub u64);

impl SyscallResult {
    pub fn code(&self) -> u32 { self.0 as u32 }
    pub fn value(&self) -> u64 { self.1 }
    pub fn is_ok(&self) -> bool { self.0 == 0 }
}

/// Syscall con 0 argumentos.
#[inline(always)]
pub unsafe fn syscall0(nr: u32) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 1 argumento.
#[inline(always)]
pub unsafe fn syscall1(nr: u32, a1: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 2 argumentos.
#[inline(always)]
pub unsafe fn syscall2(nr: u32, a1: u64, a2: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1, in("rsi") a2,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 3 argumentos.
#[inline(always)]
pub unsafe fn syscall3(nr: u32, a1: u64, a2: u64, a3: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1, in("rsi") a2, in("rdx") a3,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 4 argumentos.
#[inline(always)]
pub unsafe fn syscall4(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 5 argumentos.
#[inline(always)]
pub unsafe fn syscall5(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4, in("r8") a5,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 6 argumentos.
#[inline(always)]
pub unsafe fn syscall6(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64, a6: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1, in("rsi") a2, in("rdx") a3, in("r10") a4, in("r8") a5, in("r9") a6,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Retorna el nombre simbólico de un syscall (para diagnostics).
pub fn name(nr: u32) -> &'static str {
    match nr {
        NR_WM_CREATE_WINDOW => "wm_create_window",
        NR_WM_DESTROY_WINDOW => "wm_destroy_window",
        NR_WM_SHOW_WINDOW => "wm_show_window",
        NR_WM_HIDE_WINDOW => "wm_hide_window",
        NR_WM_SET_TITLE => "wm_set_title",
        NR_WM_GET_BOUNDS => "wm_get_bounds",
        NR_WM_SET_BOUNDS => "wm_set_bounds",
        NR_WM_BEGIN_PAINT => "wm_begin_paint",
        NR_WM_END_PAINT => "wm_end_paint",
        NR_WM_PUSH_CLIP => "wm_push_clip",
        NR_WM_POP_CLIP => "wm_pop_clip",
        NR_WM_SET_FOCUS => "wm_set_focus",
        NR_WM_GET_FOCUS => "wm_get_focus",
        NR_WM_REGISTER_CLASS => "wm_register_class",
        NR_WM_PUMP_EVENTS => "wm_pump_events",
        NR_WM_TRANSLATE_MESSAGE => "wm_translate_message",
        NR_DRAW_CLEAR => "draw_clear",
        NR_DRAW_PIXEL => "draw_pixel",
        NR_DRAW_LINE => "draw_line",
        NR_DRAW_RECT => "draw_rect",
        NR_DRAW_CIRCLE => "draw_circle",
        NR_DRAW_TEXT => "draw_text",
        NR_DRAW_BLIT => "draw_blit",
        NR_DRAW_GRADIENT_V => "draw_gradient_v",
        NR_DRAW_GRADIENT_H => "draw_gradient_h",
        NR_DRAW_ROUNDED_RECT => "draw_rounded_rect",
        NR_WINPAINT_FILL_RECT => "winpaint_fill_rect",
        NR_WINPAINT_DRAW_TEXT => "winpaint_draw_text",
        NR_WINPAINT_DRAW_PIXEL => "winpaint_draw_pixel",
        NR_WINPAINT_DRAW_LINE => "winpaint_draw_line",
        NR_WINPAINT_DRAW_BLIT => "winpaint_draw_blit",
        NR_WINPAINT_DRAW_CIRCLE => "winpaint_draw_circle",
        NR_COMPOSITOR_BEGIN_FRAME => "compositor_begin_frame",
        NR_COMPOSITOR_END_FRAME => "compositor_end_frame",
        NR_COMPOSITOR_PRESENT => "compositor_present",
        NR_COMPOSITOR_SET_TARGET => "compositor_set_target",
        NR_COMPOSITOR_FLUSH => "compositor_flush",
        NR_FS_OPEN => "fs_open",
        NR_FS_CLOSE => "fs_close",
        NR_FS_READ => "fs_read",
        NR_FS_WRITE => "fs_write",
        NR_FS_SEEK => "fs_seek",
        NR_FS_STAT => "fs_stat",
        NR_FS_MKDIR => "fs_mkdir",
        NR_FS_READDIR => "fs_readdir",
        NR_FS_DELETE => "fs_delete",
        NR_FS_MOUNT => "fs_mount",
        NR_TIME_NOW_NS => "time_now_ns",
        NR_TIME_NOW_US => "time_now_us",
        NR_TIME_SLEEP_NS => "time_sleep_ns",
        NR_TIME_SLEEP_MS => "time_sleep_ms",
        NR_INPUT_POLL_KEY => "input_poll_key",
        NR_INPUT_POLL_MOUSE => "input_poll_mouse",
        NR_INPUT_POLL_EVENT => "input_poll_event",
        NR_AUDIO_PLAY => "audio_play",
        NR_AUDIO_STOP => "audio_stop",
        NR_AUDIO_BEEP => "audio_beep",
        NR_AUDIO_LOAD_WAVE => "audio_load_wave",
        NR_PROC_SPAWN => "proc_spawn",
        NR_PROC_EXIT => "proc_exit",
        NR_PROC_GET_PID => "proc_get_pid",
        NR_PROC_GET_TID => "proc_get_tid",
        NR_PROC_YIELD => "proc_yield",
        NR_THREAD_CREATE => "thread_create",
        NR_THREAD_EXIT => "thread_exit",
        NR_THREAD_JOIN => "thread_join",
        NR_THREAD_SELF => "thread_self",
        NR_MEM_ALLOC => "mem_alloc",
        NR_MEM_FREE => "mem_free",
        NR_MEM_MAP => "mem_map",
        NR_MEM_UNMAP => "mem_unmap",
        NR_BEFCORE_SEND => "befcore_send",
        NR_BEFCORE_RECV => "befcore_recv",
        NR_BEFCORE_POLL => "befcore_poll",
        NR_BEFCORE_REGISTER => "befcore_register",
        NR_IPC_PORT_CREATE => "ipc_port_create",
        NR_IPC_PORT_SEND => "ipc_port_send",
        NR_IPC_PORT_RECV => "ipc_port_recv",
        NR_IPC_PORT_CLOSE => "ipc_port_close",
        NR_SURFACE_MAP => "surface_map",
        NR_SURFACE_UNMAP => "surface_unmap",
        NR_SURFACE_PRESENT => "surface_present",
        NR_DEBUG_PRINT => "debug_print",
        NR_DEBUG_TRACE => "debug_trace",
        NR_DEBUG_ASSERT => "debug_assert",
        NR_DEBUG_PANIC => "debug_panic",
        _ => "<unknown bmo_api syscall>",
    }
}

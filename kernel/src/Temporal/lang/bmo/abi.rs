//! BMO ABI → syscall number mapping for the AOT compiler.
//!
//! v1.8.8: este módulo ya NO define los números de syscall (eso vive
//! en `crate::bmo_abi::syscalls`). Solo provee el mapping **name → nr**
//! que el AOT necesita para resolver llamadas BMO ABI en `mov rax, <nr>; syscall`.
//!
//! El AOT emite bytes x86-64 reales. Cuando el código BMO o C generado
//! llama a `win_create("Hello", 0, 0, 100, 100)`, el AOT traduce eso
//! a `mov rax, 0x100; mov rdi, ...; syscall`.
//!
//! Los números vienen de `crate::bmo_abi::syscalls` (fuente única).
//! Ver `kernel/src/bmo_abi/syscalls/mod.rs`.

#![allow(dead_code)]

// Re-export the syscall numbers from the canonical table.
// El AOT los usa directamente: `crate::bmo_abi::syscalls::NR_WM_CREATE_WINDOW`.
pub use crate::bmo_abi::syscalls::*;

// ─── Mapeo de nombre simbólico a número de syscall ───────────────────
//
// El AOT busca el nombre de la función BMO ABI que el programa está
// llamando y lo convierte a un `mov rax, <número>; syscall`.
//
// Tabla centralizada: el nombre que el programador escribe en BMO o C
// debe coincidir con uno de estos strings.
pub fn resolve(name: &str) -> Option<u32> {
    let nr = match name {
        // Window manager (0x100..=0x10F)
        "win_create"         => NR_WM_CREATE_WINDOW,
        "win_destroy"        => NR_WM_DESTROY_WINDOW,
        "win_show"           => NR_WM_SHOW_WINDOW,
        "win_hide"           => NR_WM_HIDE_WINDOW,
        "win_set_title"      => NR_WM_SET_TITLE,
        "win_get_bounds"     => NR_WM_GET_BOUNDS,
        "win_set_bounds"     => NR_WM_SET_BOUNDS,
        "win_begin_paint"    => NR_WM_BEGIN_PAINT,
        "win_end_paint"      => NR_WM_END_PAINT,
        "win_push_clip"      => NR_WM_PUSH_CLIP,
        "win_pop_clip"       => NR_WM_POP_CLIP,
        "win_set_focus"      => NR_WM_SET_FOCUS,
        "win_get_focus"      => NR_WM_GET_FOCUS,
        "win_register_class" => NR_WM_REGISTER_CLASS,
        "win_pump_events"    => NR_WM_PUMP_EVENTS,

        // Drawing (0x110..=0x119)
        "draw_clear"         => NR_DRAW_CLEAR,
        "draw_pixel"         => NR_DRAW_PIXEL,
        "draw_line"          => NR_DRAW_LINE,
        "draw_rect"          => NR_DRAW_RECT,
        "draw_circle"        => NR_DRAW_CIRCLE,
        "draw_text"          => NR_DRAW_TEXT,
        "draw_blit"          => NR_DRAW_BLIT,
        "draw_gradient_v"    => NR_DRAW_GRADIENT_V,
        "draw_gradient_h"    => NR_DRAW_GRADIENT_H,
        "draw_rounded_rect"  => NR_DRAW_ROUNDED_RECT,

        // Window painting (0x120..=0x125)
        "win_fill_rect"      => NR_WINPAINT_FILL_RECT,
        "win_draw_text"      => NR_WINPAINT_DRAW_TEXT,
        "win_draw_pixel"     => NR_WINPAINT_DRAW_PIXEL,
        "win_draw_line"      => NR_WINPAINT_DRAW_LINE,
        "win_draw_blit"      => NR_WINPAINT_DRAW_BLIT,
        "win_draw_circle"    => NR_WINPAINT_DRAW_CIRCLE,

        // Compositor (0x130..=0x134)
        "comp_begin_frame"   => NR_COMPOSITOR_BEGIN_FRAME,
        "comp_end_frame"     => NR_COMPOSITOR_END_FRAME,
        "comp_present"       => NR_COMPOSITOR_PRESENT,
        "comp_set_target"    => NR_COMPOSITOR_SET_TARGET,
        "comp_flush"         => NR_COMPOSITOR_FLUSH,

        // Filesystem (0x140..=0x149)
        "fs_open"            => NR_FS_OPEN,
        "fs_close"           => NR_FS_CLOSE,
        "fs_read"            => NR_FS_READ,
        "fs_write"           => NR_FS_WRITE,
        "fs_seek"            => NR_FS_SEEK,
        "fs_stat"            => NR_FS_STAT,
        "fs_mkdir"           => NR_FS_MKDIR,
        "fs_readdir"         => NR_FS_READDIR,
        "fs_delete"          => NR_FS_DELETE,
        "fs_mount"           => NR_FS_MOUNT,

        // Time (0x150..=0x153)
        "time_now_ns"        => NR_TIME_NOW_NS,
        "time_now_us"        => NR_TIME_NOW_US,
        "time_sleep_ns"      => NR_TIME_SLEEP_NS,
        "time_sleep_ms"      => NR_TIME_SLEEP_MS,

        // Input (0x160..=0x162)
        "input_poll_key"     => NR_INPUT_POLL_KEY,
        "input_poll_mouse"   => NR_INPUT_POLL_MOUSE,
        "input_poll_event"   => NR_INPUT_POLL_EVENT,

        // Audio (0x170..=0x173)
        "audio_play"         => NR_AUDIO_PLAY,
        "audio_stop"         => NR_AUDIO_STOP,
        "audio_beep"         => NR_AUDIO_BEEP,
        "audio_load_wave"    => NR_AUDIO_LOAD_WAVE,

        // Process (0x180..=0x188) — confirmado, este rango es PROC
        "proc_spawn"         => NR_PROC_SPAWN,
        "proc_exit"          => NR_PROC_EXIT,
        "proc_get_pid"       => NR_PROC_GET_PID,
        "proc_get_tid"       => NR_PROC_GET_TID,
        "proc_yield"         => NR_PROC_YIELD,
        "thread_create"      => NR_THREAD_CREATE,
        "thread_exit"        => NR_THREAD_EXIT,
        "thread_join"        => NR_THREAD_JOIN,
        "thread_self"        => NR_THREAD_SELF,

        // Memory (0x190..=0x193) + BEFCore (0x194..=0x197)
        "mem_alloc"          => NR_MEM_ALLOC,
        "mem_free"           => NR_MEM_FREE,
        "mem_map"            => NR_MEM_MAP,
        "mem_unmap"          => NR_MEM_UNMAP,
        "befcore_send"       => NR_BEFCORE_SEND,
        "befcore_recv"       => NR_BEFCORE_RECV,
        "befcore_poll"       => NR_BEFCORE_POLL,
        "befcore_register"   => NR_BEFCORE_REGISTER,

        // IPC (0x1A0..=0x1A3)
        "ipc_port_create"    => NR_IPC_PORT_CREATE,
        "ipc_port_send"      => NR_IPC_PORT_SEND,
        "ipc_port_recv"      => NR_IPC_PORT_RECV,
        "ipc_port_close"     => NR_IPC_PORT_CLOSE,

        // Surface mapping (0x1C0..=0x1C2) — movido aquí por el conflicto 0x180
        "surface_map"        => NR_SURFACE_MAP,
        "surface_unmap"      => NR_SURFACE_UNMAP,
        "surface_present"    => NR_SURFACE_PRESENT,

        // Diagnostics (0x1F0..=0x1F3)
        "diag_print"         => NR_DEBUG_PRINT,
        "diag_trace"         => NR_DEBUG_TRACE,
        "diag_assert"        => NR_DEBUG_ASSERT,
        "diag_panic"         => NR_DEBUG_PANIC,

        _ => return None,
    };
    Some(nr)
}

/// Returns true if the given name is a known BMO ABI function.
#[inline(always)]
pub fn is_abi(name: &str) -> bool { resolve(name).is_some() }

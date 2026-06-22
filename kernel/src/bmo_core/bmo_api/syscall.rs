//! v2.0 — Dispatcher principal de la BMO API.
//!
//! Conectado al rango 0x100..0x1FF desde `crate::arch::syscall_entry`.
//! Maneja también 0x198 = BMO_DISPATCH_RETURN (retorno de wnd_proc).
//!
//! Convencion: rax=nr, rdi=a0, rsi=a1, rdx=a2, r10=a3, r8=a4, r9=a5.
//! Devuelve el valor a poner en rax (errno negativo o handle/entero).

#![allow(dead_code)]

use super::window::{style, WID_INVALID};
use super::message::{BmoMsg, BmoMsgKind};
use super::class;

// v1.8.8: este módulo re-exporta los syscall numbers canónicos desde
// `bmo_abi::syscalls` (fuente única de verdad). Antes redefinía números
// que colisionaban con la tabla oficial.
//
// IMPORTANTE: en bmo_abi::syscalls los números siguen el layout limpio
// (0x100..0x10F = WM, 0x110..0x119 = Draw, etc.) mientras que la versión
// vieja de bmo_api mezclaba 70+ syscalls en 0x100..0x1A3. Los nombres
// semánticos de bmo_api se conservan, pero apuntan a los números canónicos.
pub mod nr {
    use crate::bmo_abi::syscalls as _nr;

    // ─── Window manager (canónicos) ──────────────────────────────
    pub const REGISTER_CLASS:        u16 = _nr::NR_WM_REGISTER_CLASS as u16;
    pub const UNREGISTER_CLASS:      u16 = 0x101; // legacy, no en ABI todavía
    pub const CREATE_WINDOW:         u16 = _nr::NR_WM_CREATE_WINDOW as u16;
    pub const CREATE_WINDOW_EX:      u16 = _nr::NR_WM_CREATE_WINDOW as u16; // legacy alias
    pub const DESTROY_WINDOW:        u16 = _nr::NR_WM_DESTROY_WINDOW as u16;
    pub const SHOW_WINDOW:           u16 = _nr::NR_WM_SHOW_WINDOW as u16;
    pub const HIDE_WINDOW:           u16 = _nr::NR_WM_HIDE_WINDOW as u16;
    pub const SET_TITLE:             u16 = _nr::NR_WM_SET_TITLE as u16;
    pub const GET_TITLE:             u16 = 0x108; // legacy, no en ABI
    pub const SET_SIZE:              u16 = _nr::NR_WM_SET_BOUNDS as u16;
    pub const SET_POS:               u16 = _nr::NR_WM_SET_BOUNDS as u16;
    pub const GET_RECT:              u16 = _nr::NR_WM_GET_BOUNDS as u16;
    pub const SET_PARENT:            u16 = 0x10C; // legacy
    pub const INVALIDATE:            u16 = _nr::NR_WM_END_PAINT as u16; // legacy alias
    pub const UPDATE_WINDOW:         u16 = _nr::NR_WM_END_PAINT as u16; // legacy
    pub const REDRAW_WINDOW:         u16 = _nr::NR_WM_END_PAINT as u16; // legacy

    // ─── Drawing (canónicos) ──────────────────────────────────────
    pub const PAINT_BEGIN:           u16 = _nr::NR_WM_BEGIN_PAINT as u16;
    pub const PAINT_END:             u16 = _nr::NR_WM_END_PAINT as u16;
    pub const DRAW_PIXEL:            u16 = _nr::NR_DRAW_PIXEL as u16;
    pub const DRAW_LINE:             u16 = _nr::NR_DRAW_LINE as u16;
    pub const DRAW_RECT:             u16 = _nr::NR_DRAW_RECT as u16;
    pub const FILL_RECT:             u16 = _nr::NR_WINPAINT_FILL_RECT as u16;
    pub const DRAW_TEXT:             u16 = _nr::NR_WINPAINT_DRAW_TEXT as u16;
    pub const DRAW_IMAGE:            u16 = _nr::NR_WINPAINT_DRAW_BLIT as u16;
    pub const DRAW_POLYLINE:         u16 = _nr::NR_DRAW_LINE as u16; // legacy
    pub const SET_CLIP:              u16 = _nr::NR_WM_PUSH_CLIP as u16;
    pub const RESET_CLIP:            u16 = _nr::NR_WM_POP_CLIP as u16;
    pub const SET_TEXT_COLOR:        u16 = _nr::NR_DRAW_RECT as u16; // legacy fallback
    pub const SET_BG_COLOR:          u16 = _nr::NR_DRAW_CLEAR as u16; // legacy fallback
    pub const SET_FONT:              u16 = _nr::NR_WINPAINT_DRAW_TEXT as u16; // legacy
    pub const CREATE_SURFACE:        u16 = _nr::NR_SURFACE_MAP as u16;
    pub const DESTROY_SURFACE:       u16 = _nr::NR_SURFACE_UNMAP as u16;

    // ─── Messages (BEFCore) ───────────────────────────────────────
    pub const GET_MESSAGE:           u16 = _nr::NR_BEFCORE_RECV as u16;
    pub const PEEK_MESSAGE:          u16 = _nr::NR_BEFCORE_POLL as u16;
    pub const POST_MESSAGE:          u16 = _nr::NR_BEFCORE_SEND as u16;
    pub const SEND_MESSAGE:          u16 = _nr::NR_BEFCORE_SEND as u16;
    pub const DISPATCH_MESSAGE:      u16 = _nr::NR_BEFCORE_POLL as u16; // legacy
    pub const TRANSLATE_MESSAGE:     u16 = _nr::NR_WM_TRANSLATE_MESSAGE as u16;
    pub const WAIT_MESSAGE:          u16 = _nr::NR_BEFCORE_RECV as u16; // legacy
    pub const POST_QUIT:             u16 = _nr::NR_BEFCORE_SEND as u16; // legacy
    pub const POST_THREAD_MESSAGE:   u16 = _nr::NR_BEFCORE_SEND as u16; // legacy
    pub const SET_TIMER:             u16 = _nr::NR_BEFCORE_REGISTER as u16; // legacy
    pub const KILL_TIMER:            u16 = _nr::NR_BEFCORE_REGISTER as u16; // legacy
    pub const SET_CAPTURE:           u16 = _nr::NR_WM_SET_FOCUS as u16; // legacy
    pub const RELEASE_CAPTURE:       u16 = _nr::NR_WM_SET_FOCUS as u16; // legacy
    pub const SET_FOCUS:             u16 = _nr::NR_WM_SET_FOCUS as u16;
    pub const GET_FOCUS:             u16 = _nr::NR_WM_GET_FOCUS as u16;
    pub const GET_ACTIVE:            u16 = _nr::NR_WM_GET_FOCUS as u16; // legacy

    // ─── DC + blit (canónicos) ────────────────────────────────────
    pub const DC_CREATE:             u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const DC_RELEASE:            u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const GET_DC:                u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const RELEASE_DC:            u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const SAVE_DC:               u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const RESTORE_DC:            u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const SELECT_OBJECT:         u16 = _nr::NR_WINPAINT_FILL_RECT as u16; // legacy
    pub const GET_PIXEL:             u16 = _nr::NR_DRAW_PIXEL as u16; // legacy
    pub const SET_PIXEL:             u16 = _nr::NR_DRAW_PIXEL as u16; // legacy
    pub const BIT_BLT:               u16 = _nr::NR_DRAW_BLIT as u16;

    // ─── Input (canónicos) ────────────────────────────────────────
    pub const INPUT_POLL_KEY:        u16 = _nr::NR_INPUT_POLL_KEY as u16;
    pub const INPUT_POLL_MOUSE:      u16 = _nr::NR_INPUT_POLL_MOUSE as u16;
    pub const INPUT_WAIT:            u16 = _nr::NR_INPUT_POLL_EVENT as u16;
    pub const INPUT_GRAB:            u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const INPUT_UNGRAB:          u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const SHOW_CURSOR:           u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const HIDE_CURSOR:           u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const SET_CURSOR_POS:        u16 = _nr::NR_INPUT_POLL_MOUSE as u16; // legacy
    pub const SET_CURSOR:            u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy

    // ─── Window ops (legacy) ──────────────────────────────────────
    pub const BRING_TO_FRONT:        u16 = _nr::NR_WM_SHOW_WINDOW as u16; // legacy
    pub const SEND_TO_BACK:          u16 = _nr::NR_WM_HIDE_WINDOW as u16; // legacy
    pub const SET_TOPMOST:           u16 = _nr::NR_WM_SHOW_WINDOW as u16; // legacy
    pub const SET_TRANSIENT_FOR:     u16 = _nr::NR_WM_SHOW_WINDOW as u16; // legacy
    pub const BEGIN_MODAL:           u16 = _nr::NR_WM_SHOW_WINDOW as u16; // legacy
    pub const END_MODAL:             u16 = _nr::NR_WM_HIDE_WINDOW as u16; // legacy
    pub const SET_WINDOW_POS:        u16 = _nr::NR_WM_SET_BOUNDS as u16;
    pub const GET_WINDOW:            u16 = _nr::NR_WM_GET_BOUNDS as u16;
    pub const ENUM_WINDOWS:          u16 = _nr::NR_WM_PUMP_EVENTS as u16; // legacy
    pub const GET_DESKTOP_WINDOW:    u16 = _nr::NR_WM_GET_BOUNDS as u16; // legacy
    pub const GET_FOREGROUND_WINDOW: u16 = _nr::NR_WM_GET_FOCUS as u16;

    // ─── Cursor/Icon (legacy) ─────────────────────────────────────
    pub const LOAD_CURSOR:           u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const LOAD_ICON:             u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const SET_CLASS_CURSOR:      u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy
    pub const SET_CLASS_ICON:        u16 = _nr::NR_INPUT_POLL_EVENT as u16; // legacy

    // ─── Clipboard (legacy, no en ABI todavía) ────────────────────
    pub const OPEN_CLIPBOARD:        u16 = _nr::NR_IPC_PORT_CREATE as u16; // legacy fallback
    pub const CLOSE_CLIPBOARD:       u16 = _nr::NR_IPC_PORT_CLOSE as u16;  // legacy fallback
    pub const SET_CLIPBOARD_DATA:    u16 = _nr::NR_IPC_PORT_SEND as u16;   // legacy fallback
    pub const GET_CLIPBOARD_DATA:    u16 = _nr::NR_IPC_PORT_RECV as u16;   // legacy fallback
    pub const EMPTY_CLIPBOARD:       u16 = _nr::NR_IPC_PORT_CREATE as u16; // legacy fallback

    // ─── Surface (canónicos) ──────────────────────────────────────
    pub const MAP_SURFACE:           u16 = _nr::NR_SURFACE_MAP as u16;
    pub const UNMAP_SURFACE:         u16 = _nr::NR_SURFACE_UNMAP as u16;
    pub const SURFACE_FLUSH:         u16 = _nr::NR_SURFACE_PRESENT as u16; // legacy
    pub const FLIP:                  u16 = _nr::NR_SURFACE_PRESENT as u16;

    // ─── Dispatch return (legacy, interno del kernel) ─────────────
    pub const DISPATCH_RETURN:       u16 = 0x198;

    // ─── Window minimize/maximize (legacy) ────────────────────────
    pub const MINIMIZE_WINDOW:       u16 = _nr::NR_WM_HIDE_WINDOW as u16;   // legacy
    pub const MAXIMIZE_WINDOW:       u16 = _nr::NR_WM_SHOW_WINDOW as u16;   // legacy
    pub const RESTORE_WINDOW:        u16 = _nr::NR_WM_SHOW_WINDOW as u16;   // legacy
    pub const GET_TASKBAR_RECT:      u16 = _nr::NR_WM_GET_BOUNDS as u16;    // legacy

    // ─── Time (canónicos) ──────────────────────────────────────────
    pub const TIME_NOW_NS:           u16 = _nr::NR_TIME_NOW_NS as u16;
    pub const TIME_NOW_US:           u16 = _nr::NR_TIME_NOW_US as u16;
    pub const TIME_SLEEP_NS:         u16 = _nr::NR_TIME_SLEEP_NS as u16;
    pub const TIME_SLEEP_MS:         u16 = _nr::NR_TIME_SLEEP_MS as u16;

    // ─── Debug (canónicos) ─────────────────────────────────────────
    pub const DEBUG_PRINT:           u16 = _nr::NR_DEBUG_PRINT as u16;
    pub const DEBUG_TRACE:           u16 = _nr::NR_DEBUG_TRACE as u16;
    pub const DEBUG_ASSERT:          u16 = _nr::NR_DEBUG_ASSERT as u16;
    pub const DEBUG_PANIC:           u16 = _nr::NR_DEBUG_PANIC as u16;

    // ─── Memory (canónicos) ────────────────────────────────────────
    pub const MEM_ALLOC:             u16 = _nr::NR_MEM_ALLOC as u16;
    pub const MEM_FREE:              u16 = _nr::NR_MEM_FREE as u16;
    pub const MEM_MAP:               u16 = _nr::NR_MEM_MAP as u16;
    pub const MEM_UNMAP:             u16 = _nr::NR_MEM_UNMAP as u16;

    // ─── FS (canónicos) ────────────────────────────────────────────
    pub const FS_OPEN:               u16 = _nr::NR_FS_OPEN as u16;
    pub const FS_CLOSE:              u16 = _nr::NR_FS_CLOSE as u16;
    pub const FS_READ:               u16 = _nr::NR_FS_READ as u16;
    pub const FS_WRITE:              u16 = _nr::NR_FS_WRITE as u16;
    pub const FS_SEEK:               u16 = _nr::NR_FS_SEEK as u16;
    pub const FS_STAT:               u16 = _nr::NR_FS_STAT as u16;
    pub const FS_MKDIR:              u16 = _nr::NR_FS_MKDIR as u16;
    pub const FS_READDIR:            u16 = _nr::NR_FS_READDIR as u16;
    pub const FS_DELETE:             u16 = _nr::NR_FS_DELETE as u16;
    pub const FS_MOUNT:              u16 = _nr::NR_FS_MOUNT as u16;

    // ─── Process (canónicos) ──────────────────────────────────────
    pub const PROC_SPAWN:            u16 = _nr::NR_PROC_SPAWN as u16;
    pub const PROC_EXIT:             u16 = _nr::NR_PROC_EXIT as u16;
    pub const PROC_GET_PID:          u16 = _nr::NR_PROC_GET_PID as u16;
    pub const PROC_GET_TID:          u16 = _nr::NR_PROC_GET_TID as u16;
    pub const PROC_YIELD:            u16 = _nr::NR_PROC_YIELD as u16;

    // ─── Thread (canónicos) ───────────────────────────────────────
    pub const THREAD_CREATE:         u16 = _nr::NR_THREAD_CREATE as u16;
    pub const THREAD_EXIT:           u16 = _nr::NR_THREAD_EXIT as u16;
    pub const THREAD_JOIN:           u16 = _nr::NR_THREAD_JOIN as u16;
    pub const THREAD_SELF:           u16 = _nr::NR_THREAD_SELF as u16;

    // ─── Audio (canónicos) ────────────────────────────────────────
    pub const AUDIO_PLAY:            u16 = _nr::NR_AUDIO_PLAY as u16;
    pub const AUDIO_STOP:            u16 = _nr::NR_AUDIO_STOP as u16;
    pub const AUDIO_BEEP:            u16 = _nr::NR_AUDIO_BEEP as u16;
    pub const AUDIO_LOAD_WAVE:       u16 = _nr::NR_AUDIO_LOAD_WAVE as u16;

    // ─── Compositor (canónicos) ──────────────────────────────────
    pub const COMPOSITOR_BEGIN_FRAME: u16 = _nr::NR_COMPOSITOR_BEGIN_FRAME as u16;
    pub const COMPOSITOR_END_FRAME:   u16 = _nr::NR_COMPOSITOR_END_FRAME as u16;
    pub const COMPOSITOR_PRESENT:     u16 = _nr::NR_COMPOSITOR_PRESENT as u16;
    pub const COMPOSITOR_SET_TARGET:  u16 = _nr::NR_COMPOSITOR_SET_TARGET as u16;
    pub const COMPOSITOR_FLUSH:       u16 = _nr::NR_COMPOSITOR_FLUSH as u16;

    // ─── Draw extras (canónicos) ──────────────────────────────────
    pub const DRAW_CIRCLE:           u16 = _nr::NR_DRAW_CIRCLE as u16;
    // DRAW_TEXT ya está mapeado a NR_WINPAINT_DRAW_TEXT (0x121).
    pub const DRAW_GRADIENT_H:       u16 = _nr::NR_DRAW_GRADIENT_H as u16;
    pub const DRAW_GRADIENT_V:       u16 = _nr::NR_DRAW_GRADIENT_V as u16;
    pub const DRAW_ROUNDED_RECT:     u16 = _nr::NR_DRAW_ROUNDED_RECT as u16;

    // ─── WinPaint extras (canónicos) ──────────────────────────────
    pub const WINPAINT_DRAW_PIXEL:   u16 = _nr::NR_WINPAINT_DRAW_PIXEL as u16;
    pub const WINPAINT_DRAW_LINE:    u16 = _nr::NR_WINPAINT_DRAW_LINE as u16;
    pub const WINPAINT_DRAW_CIRCLE:  u16 = _nr::NR_WINPAINT_DRAW_CIRCLE as u16;
}

// v1.8.8: errores ahora vienen de `bmo_abi::error_code` (21 códigos
// canónicos). Los alias aquí preservan los nombres viejos para no
// tocar todos los call sites.
pub mod err {
    use crate::bmo_abi::error_code::BmoErrorCode;
    pub const OK: u64             = BmoErrorCode::Ok as u64;
    pub const GENERIC: u64        = BmoErrorCode::Internal as u64;
    pub const BAD_HANDLE: u64     = BmoErrorCode::InvalidHandle as u64;
    pub const INVALID: u64        = BmoErrorCode::InvalidArgument as u64;
    pub const NO_MEMORY: u64      = BmoErrorCode::OutOfMemory as u64;
    pub const NO_WINDOW: u64      = BmoErrorCode::NotFound as u64;
    pub const NOT_GUI_THR: u64    = BmoErrorCode::PermissionDenied as u64;
    pub const QUEUE_FULL: u64     = BmoErrorCode::Busy as u64;
    pub const BAD_CLASS: u64      = BmoErrorCode::InvalidArgument as u64;
    pub const CLASS_EXISTS: u64   = BmoErrorCode::AlreadyExists as u64;
    pub const NO_CLASS: u64       = BmoErrorCode::NotFound as u64;
    pub const BAD_DC: u64         = BmoErrorCode::InvalidHandle as u64;
    pub const BAD_SURFACE: u64    = BmoErrorCode::InvalidHandle as u64;
    pub const BUSY: u64           = BmoErrorCode::Busy as u64;
    pub const TIMEOUT: u64        = BmoErrorCode::Timeout as u64;
    pub const BAD_FORMAT: u64     = BmoErrorCode::InvalidArgument as u64;
    pub const NO_QUIT: u64        = BmoErrorCode::InvalidState as u64;
    pub const REENTRANT: u64      = BmoErrorCode::InvalidState as u64;
    pub const PERM: u64           = BmoErrorCode::PermissionDenied as u64;
    pub const STALE: u64          = BmoErrorCode::InvalidState as u64;
}

fn validate_user_ptr(ptr: u64, len: u64) -> bool {
    if ptr == 0 || len == 0 { return false; }
    if ptr < 0x1000 { return false; }
    if ptr + len < ptr { return false; }
    true
}

pub fn dispatch(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    #[allow(unreachable_patterns)] // varios `nr::*` legacy alias a la misma syscall
    match nr {
        nr::REGISTER_CLASS   => sys_register_class(a0),
        nr::UNREGISTER_CLASS => sys_unregister_class(a0 as u16),
        nr::CREATE_WINDOW_EX => {
            let xy = (a4 as i64) | ((a5 as i64) << 32);
            sys_create_window_ex(a0 as u16, a1, a2, a3 as u32, 0, xy, 720, 460)
        }
        nr::CREATE_WINDOW    => {
            let xy = (a4 as i64) | ((a5 as i64) << 32);
            sys_create_window_ex(a0 as u16, a1, a2, a3 as u32, 0, xy, 720, 460)
        }
        nr::DESTROY_WINDOW   => sys_destroy_window(a0),
        nr::SHOW_WINDOW      => sys_show_window(a0, a1),
        nr::HIDE_WINDOW      => sys_hide_window(a0),
        nr::SET_TITLE        => sys_set_title(a0, a1, a2),
        nr::GET_TITLE        => sys_get_title(a0, a1, a2),
        nr::SET_SIZE         => sys_set_size(a0, a1, a2),
        nr::SET_POS          => sys_set_pos(a0, a1, a2),
        nr::GET_RECT         => sys_get_rect(a0, a1),
        nr::INVALIDATE       => sys_invalidate(a0),
        nr::UPDATE_WINDOW    => sys_invalidate(a0),
        nr::REDRAW_WINDOW    => sys_invalidate(a0),

        nr::PAINT_BEGIN      => sys_paint_begin(a0, a1),
        nr::PAINT_END        => sys_paint_end(a0, a1),
        nr::FILL_RECT        => sys_fill_rect(a0, a1, a2, a3, a4),
        nr::DRAW_TEXT        => sys_draw_text(a0, a1, a2, a3),
        nr::DRAW_LINE        => sys_draw_line(a0, a1, a2, a3, a4, a5),
        nr::DRAW_PIXEL       => sys_draw_pixel(a0, a1, a2, a3),
        nr::DRAW_RECT        => sys_draw_rect(a0, a1, a2, a3, a4, a5),
        nr::SET_CLIP         => sys_set_clip(a0, a1, a2, a3),
        nr::RESET_CLIP       => sys_reset_clip(a0),
        nr::SET_TEXT_COLOR   => sys_set_text_color(a0, a1),
        nr::SET_BG_COLOR     => sys_set_bg_color(a0, a1),
        nr::SET_FONT         => sys_set_font(a0, a1),

        nr::CREATE_SURFACE   => sys_create_surface(a0 as u16, a1 as u16, a2 as u32),
        nr::DESTROY_SURFACE  => sys_destroy_surface(a0),
        nr::MAP_SURFACE      => sys_map_surface(a0),
        nr::UNMAP_SURFACE    => err::OK,
        nr::SURFACE_FLUSH    => err::OK,
        nr::FLIP             => err::OK,

        nr::GET_MESSAGE      => sys_get_message(a0),
        nr::PEEK_MESSAGE     => sys_peek_message(a0),
        nr::POST_MESSAGE     => sys_post_message(a0, a1 as u16, a2, a3),
        nr::SEND_MESSAGE     => sys_send_message(a0, a1 as u16, a2, a3),
        nr::DISPATCH_MESSAGE => sys_dispatch_message(a0),
        nr::TRANSLATE_MESSAGE=> err::OK,
        nr::WAIT_MESSAGE     => err::OK,
        nr::POST_QUIT        => err::OK,
        nr::POST_THREAD_MESSAGE => err::OK,
        nr::SET_TIMER        => sys_set_timer(a0, a1 as u16, a2 as u32),
        nr::KILL_TIMER       => sys_kill_timer(a0),
        nr::SET_CAPTURE      => sys_set_capture(a0),
        nr::RELEASE_CAPTURE  => sys_release_capture(),
        nr::SET_FOCUS        => sys_set_focus(a0),
        nr::GET_FOCUS        => {
            let s = super::state();
            s.lock();
            let f = s.windows.focus;
            s.unlock();
            if f == WID_INVALID { err::NO_WINDOW } else { f as u64 }
        }
        nr::GET_ACTIVE       => {
            let s = super::state();
            s.lock();
            let a = s.windows.active;
            s.unlock();
            if a == WID_INVALID { err::NO_WINDOW } else { a as u64 }
        }

        nr::DC_CREATE        => sys_dc_create(a0),
        nr::DC_RELEASE       => sys_dc_release(a0),
        nr::GET_DC           => sys_dc_create(a0),
        nr::RELEASE_DC       => sys_dc_release(a0),
        nr::SAVE_DC          => sys_save_dc(a0),
        nr::RESTORE_DC       => sys_restore_dc(a0),
        nr::SELECT_OBJECT    => err::OK,
        nr::GET_PIXEL        => err::OK,
        nr::SET_PIXEL        => err::OK,
        nr::BIT_BLT          => sys_draw_image(a0, a1, a2, a3, a4, a5),

        nr::INPUT_POLL_KEY   => sys_input_poll_key(),
        nr::INPUT_POLL_MOUSE => sys_input_poll_mouse(),
        nr::INPUT_WAIT       => err::OK,
        nr::INPUT_GRAB       => err::OK,
        nr::INPUT_UNGRAB     => err::OK,
        nr::SHOW_CURSOR      => { super::cursor::show(); err::OK }
        nr::HIDE_CURSOR      => { super::cursor::hide(); err::OK }
        nr::SET_CURSOR_POS   => err::OK,
        nr::SET_CURSOR       => { super::cursor::set(a0 as u8); err::OK }

        nr::BRING_TO_FRONT   => { super::wm::bring_to_front(a0 as u32); err::OK }
        nr::SEND_TO_BACK     => err::OK,
        nr::SET_TOPMOST      => err::OK,
        nr::SET_TRANSIENT_FOR=> err::OK,
        nr::BEGIN_MODAL      => err::OK,
        nr::END_MODAL        => err::OK,
        nr::SET_WINDOW_POS   => sys_set_window_pos(a0, a1, a2, a3),
        nr::GET_WINDOW       => err::OK,
        nr::ENUM_WINDOWS     => err::OK,
        nr::GET_DESKTOP_WINDOW => {
            let s = super::state();
            s.lock();
            let d = s.windows.desktop;
            s.unlock();
            if d == WID_INVALID { err::NO_WINDOW } else { d as u64 }
        }
        nr::GET_FOREGROUND_WINDOW => {
            let s = super::state();
            s.lock();
            let a = s.windows.active;
            s.unlock();
            if a == WID_INVALID { err::NO_WINDOW } else { a as u64 }
        }

        nr::LOAD_CURSOR      => err::OK,
        nr::LOAD_ICON        => err::OK,
        nr::SET_CLASS_CURSOR => err::OK,
        nr::SET_CLASS_ICON   => err::OK,

        nr::OPEN_CLIPBOARD   => err::OK,
        nr::CLOSE_CLIPBOARD  => err::OK,
        nr::SET_CLIPBOARD_DATA => err::OK,
        nr::GET_CLIPBOARD_DATA => err::OK,
        nr::EMPTY_CLIPBOARD  => err::OK,

        // ─── Time ─────────────────────────────────────────────────
        nr::TIME_NOW_NS      => sys_time_now_ns(),
        nr::TIME_NOW_US      => sys_time_now_us(),
        nr::TIME_SLEEP_NS    => sys_time_sleep_ns(a0),
        nr::TIME_SLEEP_MS    => sys_time_sleep_ms(a0),

        // ─── Debug ────────────────────────────────────────────────
        nr::DEBUG_PRINT      => sys_debug_print(a0, a1),
        nr::DEBUG_TRACE      => sys_debug_trace(a0, a1),
        nr::DEBUG_ASSERT     => sys_debug_assert(a0, a1, a2),
        nr::DEBUG_PANIC      => sys_debug_panic(a0, a1),

        // ─── Memory ───────────────────────────────────────────────
        nr::MEM_ALLOC        => sys_mem_alloc(a0),
        nr::MEM_FREE         => sys_mem_free(a0, a1),
        nr::MEM_MAP          => sys_mem_map(a0, a1, a2, a3),
        nr::MEM_UNMAP        => sys_mem_unmap(a0, a1),

        // ─── Filesystem ───────────────────────────────────────────
        nr::FS_OPEN          => sys_fs_open(a0, a1),
        nr::FS_CLOSE         => sys_fs_close(a0),
        nr::FS_READ          => sys_fs_read(a0, a1, a2),
        nr::FS_WRITE         => sys_fs_write(a0, a1, a2),
        nr::FS_SEEK          => sys_fs_seek(a0, a1, a2),
        nr::FS_STAT          => sys_fs_stat(a0),
        nr::FS_MKDIR         => sys_fs_mkdir(a0, a1),
        nr::FS_READDIR       => sys_fs_readdir(a0, a1, a2, a3),
        nr::FS_DELETE        => sys_fs_delete(a0, a1),
        nr::FS_MOUNT         => sys_fs_mount(a0, a1, a2, a3, a4),

        // ─── Process ──────────────────────────────────────────────
        nr::PROC_SPAWN       => sys_proc_spawn(a0, a1),
        nr::PROC_EXIT        => sys_proc_exit(a0),
        nr::PROC_GET_PID     => sys_proc_get_pid(),
        nr::PROC_GET_TID     => sys_proc_get_tid(),
        nr::PROC_YIELD       => sys_proc_yield(),

        // ─── Thread ───────────────────────────────────────────────
        nr::THREAD_CREATE    => sys_thread_create(a0, a1),
        nr::THREAD_EXIT      => sys_thread_exit(),
        nr::THREAD_JOIN      => sys_thread_join(a0),
        nr::THREAD_SELF      => sys_thread_self(),

        // ─── Audio ───────────────────────────────────────────────
        nr::AUDIO_PLAY       => sys_audio_play(a0),
        nr::AUDIO_STOP       => sys_audio_stop(),
        nr::AUDIO_BEEP       => sys_audio_beep(a0, a1),
        nr::AUDIO_LOAD_WAVE  => sys_audio_load_wave(a0, a1),

        // ─── Compositor ──────────────────────────────────────────
        nr::COMPOSITOR_BEGIN_FRAME => sys_compositor_begin_frame(),
        nr::COMPOSITOR_END_FRAME   => sys_compositor_end_frame(),
        nr::COMPOSITOR_PRESENT     => sys_compositor_present(),
        nr::COMPOSITOR_SET_TARGET  => sys_compositor_set_target(a0, a1, a2, a3),
        nr::COMPOSITOR_FLUSH       => sys_compositor_flush(),

        // ─── Draw extras ─────────────────────────────────────────
        nr::DRAW_CIRCLE      => sys_draw_circle(a0, a1, a2, a3, a4),
        nr::DRAW_TEXT        => sys_draw_text_user(a0, a1, a2, a3, a4),
        nr::DRAW_GRADIENT_H  => sys_draw_gradient_h(a0, a1, a2, a3, a4, a5),
        nr::DRAW_GRADIENT_V  => sys_draw_gradient_v(a0, a1, a2, a3, a4, a5),
        nr::DRAW_ROUNDED_RECT => sys_draw_rounded_rect(a0, a1, a2, a3, a4, a5, 0),

        // ─── WinPaint extras ─────────────────────────────────────
        nr::WINPAINT_DRAW_PIXEL  => sys_winpaint_draw_pixel(a0, a1, a2, a3),
        nr::WINPAINT_DRAW_LINE   => sys_winpaint_draw_line(a0, a1, a2, a3, a4, a5),
        nr::WINPAINT_DRAW_CIRCLE => sys_winpaint_draw_circle(a0, a1, a2, a3, a4),

        nr::DISPATCH_RETURN  => err::OK,

        nr::MINIMIZE_WINDOW  => { super::wm::minimize_window(a0 as u32); err::OK }
        nr::MAXIMIZE_WINDOW  => { super::wm::maximize_window(a0 as u32); err::OK }
        nr::RESTORE_WINDOW   => { super::wm::restore_window(a0 as u32); err::OK }
        nr::GET_TASKBAR_RECT => {
            let (fbw, fbh) = unsafe { (crate::boot::info::FB_WIDTH, crate::boot::info::FB_HEIGHT) };
            let ty = fbh.saturating_sub(48);
            (0u64) | ((ty as u64) << 16) | ((fbw as u64) << 32) | ((48u64) << 48)
        }

        _ => {
            crate::cabina::warn_u64("bmo_api_v2.syscall", "unimplemented nr=", nr as u64);
            err::INVALID
        }
    }
}

// ── Window syscalls ───────────────────────────────────────────────

fn sys_register_class(_a0: u64) -> u64 {
    let s = super::state();
    s.lock();
    let r = s.windows.alloc_class();
    s.unlock();
    match r {
        Some(slot) => slot as u64,
        None => err::BAD_CLASS,
    }
}

fn sys_unregister_class(class_id: u16) -> u64 {
    let s = super::state();
    s.lock();
    if let Some(c) = s.windows.class_mut(class_id) {
        c.used = false;
        c.magic = 0;
    }
    s.unlock();
    err::OK
}

fn sys_create_window_ex(class_id: u16, title_ptr: u64, title_len: u64, style: u32, _style_ex: u32, xy: i64, w: i64, h: i64) -> u64 {
    let x = (xy & 0xFFFFFFFF) as i32;
    let y = ((xy >> 32) & 0xFFFFFFFF) as i32;
    let s = super::state();
    s.lock();
    let slot = match s.windows.alloc_window() {
        Some(s) => s,
        None => { s.unlock(); return err::NO_WINDOW; }
    };
    let surf = s.surfaces.alloc(w.max(1) as u16, h.max(1) as u16, crate::bmo_abi::surface::BmoFormat::ARGB8 as u32, slot);
    {
        let win = match s.windows.window_mut(slot) {
            Some(w) => w,
            None => { s.unlock(); return err::NO_WINDOW; }
        };
        win.class_id = class_id;
        win.style = style;
        if (style & style::WS_VISIBLE) != 0 {
            win.flags.0 |= 1;
            win.visible = true;
        }
        if (style & style::WS_DISABLED) == 0 {
            win.flags.0 |= 2;
            win.enabled = true;
        }
        win.x = x;
        win.y = y;
        win.w = w as i32;
        win.h = h as i32;
        win.surface = surf.unwrap_or(0);
        let tlen = (title_len as usize).min(63);
        if tlen > 0 && title_ptr != 0 && validate_user_ptr(title_ptr, title_len) {
            let src = unsafe { core::slice::from_raw_parts(title_ptr as *const u8, tlen) };
            for i in 0..tlen { win.title[i] = src[i]; }
            win.title_len = tlen as u8;
        } else {
            let d = b"BMO Window";
            for i in 0..d.len() { win.title[i] = d[i]; }
            win.title_len = d.len() as u8;
        }
    }
    s.windows.z_push_top(slot);
    s.windows.focus = slot;
    s.windows.active = slot;
    s.unlock();
    encode_wid(slot, s.windows.window(slot).map(|w| w.generation).unwrap_or(0))
}

fn encode_wid(slot: u32, gen: u16) -> u64 {
    ((slot as u64) << 16) | (gen as u64)
}

fn decode_wid(v: u64) -> (u32, u16) {
    ((v >> 16) as u32, v as u16)
}

fn sys_destroy_window(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let current_gen = s.windows.window(slot).map(|w| w.generation).unwrap_or(0);
    if current_gen != gen { s.unlock(); return err::STALE; }
    s.windows.z_remove(slot);
    let surf = s.windows.window(slot).map(|w| w.surface).unwrap_or(0);
    s.windows.free_window(slot);
    if surf != 0 { s.surfaces.free(surf); }
    s.unlock();
    err::OK
}

fn sys_show_window(handle: u64, _cmd: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(w) if w.generation == gen => { w.visible = true; w.flags.0 |= 1; err::OK }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_hide_window(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(w) if w.generation == gen => { w.visible = false; w.flags.0 &= !1; err::OK }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_set_title(handle: u64, ptr: u64, len: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(w) if w.generation == gen => {
            let tlen = (len as usize).min(63);
            if ptr != 0 && tlen > 0 && validate_user_ptr(ptr, len) {
                let src = unsafe { core::slice::from_raw_parts(ptr as *const u8, tlen) };
                for i in 0..tlen { w.title[i] = src[i]; }
                w.title_len = tlen as u8;
            }
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_get_title(handle: u64, buf_ptr: u64, buf_len: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window(slot) {
        Some(w) if w.generation == gen => {
            let tlen = w.title_len as usize;
            let copy_len = tlen.min(buf_len as usize).min(63);
            if buf_ptr != 0 && copy_len > 0 && validate_user_ptr(buf_ptr, buf_len) {
                let dst = unsafe { core::slice::from_raw_parts_mut(buf_ptr as *mut u8, copy_len) };
                for i in 0..copy_len { dst[i] = w.title[i]; }
            }
            tlen as u64
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_set_size(handle: u64, w: u64, h: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(win) if win.generation == gen => {
            win.w = w as i32; win.h = h as i32;
            win.dirty = true;
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_set_pos(handle: u64, x: u64, y: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(win) if win.generation == gen => {
            win.x = x as i32; win.y = y as i32;
            win.dirty = true;
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_get_rect(handle: u64, rect_ptr: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window(slot) {
        Some(w) if w.generation == gen => {
            if rect_ptr != 0 && validate_user_ptr(rect_ptr, 16) {
                let dst = unsafe { core::slice::from_raw_parts_mut(rect_ptr as *mut i32, 4) };
                dst[0] = w.x; dst[1] = w.y; dst[2] = w.w; dst[3] = w.h;
            }
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_invalidate(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(w) if w.generation == gen => { w.dirty = true; err::OK }
        _ => err::STALE,
    };
    s.unlock();
    r
}

// ── Paint / Drawing syscalls ──────────────────────────────────────

fn sys_paint_begin(handle: u64, _paintstruct_ptr: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window(slot) {
        Some(w) if w.generation == gen => {
            match super::draw::create_dc_for(slot) {
                Some(dc) => dc as u64,
                None => err::BAD_DC,
            }
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_paint_end(handle: u64, _dc: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(w) if w.generation == gen => { w.dirty = false; err::OK }
        _ => err::STALE,
    };
    s.unlock();
    r
}

fn sys_fill_rect(dc_slot: u64, x: u64, y: u64, w: u64, color: u64) -> u64 {
    super::draw::fill_rect(dc_slot as u32, x as i32, y as i32, w as i32, w as i32, color as u32);
    err::OK
}

fn sys_draw_text(dc_slot: u64, x: u64, y: u64, str_ptr: u64) -> u64 {
    if str_ptr == 0 || !validate_user_ptr(str_ptr, 256) { return err::INVALID; }
    let s = unsafe { core::slice::from_raw_parts(str_ptr as *const u8, 256) };
    let end = s.iter().position(|&b| b == 0).unwrap_or(256);
    super::draw::draw_text(dc_slot as u32, x as i32, y as i32, &s[..end], 0xFFE6F1F5);
    err::OK
}

fn sys_draw_line(dc_slot: u64, x0: u64, y0: u64, x1: u64, y1: u64, color: u64) -> u64 {
    super::draw::draw_line(dc_slot as u32, x0 as i32, y0 as i32, x1 as i32, y1 as i32, color as u32);
    err::OK
}

fn sys_draw_pixel(dc_slot: u64, x: u64, y: u64, color: u64) -> u64 {
    super::draw::draw_pixel(dc_slot as u32, x as i32, y as i32, color as u32);
    err::OK
}

fn sys_draw_rect(dc_slot: u64, x: u64, y: u64, w: u64, h: u64, color: u64) -> u64 {
    super::draw::draw_rect(dc_slot as u32, x as i32, y as i32, w as i32, h as i32, color as u32);
    err::OK
}

fn sys_set_clip(dc_slot: u64, x: u64, y: u64, w_h_packed: u64) -> u64 {
    let dc = dc_slot as u32;
    let (cw, ch) = ((w_h_packed & 0xFFFF) as i32, ((w_h_packed >> 16) & 0xFFFF) as i32);
    super::draw::DC_TABLE_LOCK.acquire();
    if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc as usize) } {
        if d.used { d.clip_x = x as i32; d.clip_y = y as i32; d.clip_w = cw; d.clip_h = ch; }
    }
    super::draw::DC_TABLE_LOCK.release();
    err::OK
}

fn sys_reset_clip(dc_slot: u64) -> u64 {
    let dc = dc_slot as u32;
    super::draw::DC_TABLE_LOCK.acquire();
    if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc as usize) } {
        if d.used { d.clip_x = 0; d.clip_y = 0; d.clip_w = 1920; d.clip_h = 1080; }
    }
    super::draw::DC_TABLE_LOCK.release();
    err::OK
}

fn sys_set_text_color(dc_slot: u64, color: u64) -> u64 {
    let dc = dc_slot as u32;
    super::draw::DC_TABLE_LOCK.acquire();
    if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc as usize) } {
        if d.used { d.text_color = color as u32; }
    }
    super::draw::DC_TABLE_LOCK.release();
    err::OK
}

fn sys_set_bg_color(dc_slot: u64, color: u64) -> u64 {
    let dc = dc_slot as u32;
    super::draw::DC_TABLE_LOCK.acquire();
    if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc as usize) } {
        if d.used { d.bg_color = color as u32; }
    }
    super::draw::DC_TABLE_LOCK.release();
    err::OK
}

fn sys_set_font(dc_slot: u64, font_id: u64) -> u64 {
    let dc = dc_slot as u32;
    super::draw::DC_TABLE_LOCK.acquire();
    if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc as usize) } {
        if d.used { d.font_id = font_id as u8; }
    }
    super::draw::DC_TABLE_LOCK.release();
    err::OK
}

// ── Surface syscalls ──────────────────────────────────────────────

fn sys_create_surface(w: u16, h: u16, format: u32) -> u64 {
    let s = super::state();
    s.lock();
    let r = s.surfaces.alloc(w, h, format, 0);
    s.unlock();
    match r {
        Some(slot) => {
            let s2 = super::state();
            s2.lock();
            let gen = s2.surfaces.surface(slot).map(|x| x.generation).unwrap_or(0);
            s2.unlock();
            ((slot as u64) << 16) | (gen as u64)
        }
        None => err::NO_MEMORY,
    }
}

fn sys_destroy_surface(handle: u64) -> u64 {
    let slot = (handle >> 16) as u32;
    let gen = handle as u16;
    let s = super::state();
    s.lock();
    let ok = s.surfaces.surface(slot).map(|x| x.generation == gen).unwrap_or(false);
    if ok { s.surfaces.free(slot); }
    s.unlock();
    if ok { err::OK } else { err::BAD_SURFACE }
}

fn sys_map_surface(handle: u64) -> u64 {
    let slot = (handle >> 16) as u32;
    let gen = handle as u16;
    let s = super::state();
    s.lock();
    let ok = s.surfaces.surface(slot).map(|x| x.generation == gen).unwrap_or(false);
    let addr = if ok { s.surfaces.surface(slot).map(|x| x.pixels as u64).unwrap_or(0) } else { 0 };
    s.unlock();
    if ok { addr } else { err::BAD_SURFACE }
}

// ── Message syscalls ──────────────────────────────────────────────

fn sys_get_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_ptr(msg_ptr, core::mem::size_of::<BmoMsg>() as u64) {
        return err::INVALID;
    }
    let qt = super::queue::queue_table();
    qt.acquire();
    if let Some(slot) = qt.slot_for_tid(0) {
        if let Some(m) = qt.queues[slot as usize].pop() {
            unsafe { *(msg_ptr as *mut BmoMsg) = m; }
            qt.release();
            return 1;
        }
    }
    qt.release();
    0
}

fn sys_peek_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_ptr(msg_ptr, core::mem::size_of::<BmoMsg>() as u64) {
        return err::INVALID;
    }
    let qt = super::queue::queue_table();
    if let Some(slot) = qt.slot_for_tid(0) {
        let q = &qt.queues[slot as usize];
        if let Some(m) = q.peek() {
            unsafe { *(msg_ptr as *mut BmoMsg) = *m; }
            return 1;
        }
    }
    0
}

fn sys_post_message(handle: u64, kind: u16, wparam: u64, lparam: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let owner = s.windows.window(slot).and_then(|w| if w.generation == gen { Some(w.owner_tid) } else { None });
    s.unlock();
    let owner = match owner { Some(o) => o, None => return err::STALE };
    let qt = super::queue::queue_table();
    qt.acquire();
    if let Some(qslot) = qt.slot_for_tid(owner) {
        let msg = BmoMsg::new(BmoMsgKind::from_u16(kind), slot as u16, 0, wparam, lparam);
        let ok = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
        qt.release();
        if ok { err::OK } else { err::QUEUE_FULL }
    } else {
        qt.release();
        err::NOT_GUI_THR
    }
}

fn sys_send_message(handle: u64, kind: u16, wparam: u64, lparam: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let cls_id = s.windows.window(slot).and_then(|w| if w.generation == gen { Some(w.class_id) } else { None });
    let wnd_proc = cls_id.and_then(|cid| s.windows.class(cid).map(|c| c.wnd_proc));
    s.unlock();
    let wnd_proc = match wnd_proc { Some(wp) => wp, None => return err::STALE };
    if wnd_proc != 0 { return 0; }
    class::default_wnd_proc(slot, BmoMsgKind::from_u16(kind), wparam, lparam)
}

fn sys_dispatch_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_ptr(msg_ptr, core::mem::size_of::<BmoMsg>() as u64) {
        return err::INVALID;
    }
    let msg = unsafe { *(msg_ptr as *const BmoMsg) };
    let target = msg.target as u32;
    let s = super::state();
    s.lock();
    let cls_id = s.windows.window(target).map(|w| w.class_id);
    let wnd_proc = cls_id.and_then(|cid| s.windows.class(cid).map(|c| c.wnd_proc));
    s.unlock();
    let wnd_proc = match wnd_proc { Some(wp) => wp, None => return err::NO_WINDOW };
    if wnd_proc != 0 { return 0; }
    class::default_wnd_proc(target, BmoMsgKind::from_u16(msg.kind), msg.wparam, msg.lparam)
}

fn sys_set_timer(handle: u64, _id: u16, timeout_ms: u32) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let owner = s.windows.window(slot).and_then(|w| if w.generation == gen { Some(w.owner_tid) } else { None });
    let now = s.timers.now_ms;
    s.unlock();
    let owner = match owner { Some(o) => o, None => return err::STALE };
    let s = super::state();
    s.lock();
    let r = s.timers.alloc(slot, owner, now + timeout_ms as u64, 0);
    s.unlock();
    match r {
        Some((_, pubid)) => pubid as u64,
        None => err::NO_MEMORY,
    }
}

fn sys_kill_timer(timer_id: u64) -> u64 {
    let s = super::state();
    s.lock();
    let ok = s.timers.free_by_id(timer_id as u32);
    s.unlock();
    if ok { err::OK } else { err::INVALID }
}

fn sys_set_capture(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let ok = s.windows.window(slot).map(|w| w.generation == gen).unwrap_or(false);
    if ok { s.windows.capture = slot; }
    s.unlock();
    if ok { err::OK } else { err::STALE }
}

fn sys_release_capture() -> u64 {
    let s = super::state();
    s.lock();
    s.windows.capture = WID_INVALID;
    s.unlock();
    err::OK
}

fn sys_set_focus(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let ok = s.windows.window(slot).map(|w| w.generation == gen).unwrap_or(false);
    if ok {
        s.windows.z_remove(slot);
        s.windows.z_push_top(slot);
        s.windows.focus = slot;
        s.windows.active = slot;
    }
    s.unlock();
    if ok { err::OK } else { err::STALE }
}

fn sys_set_window_pos(handle: u64, x: u64, y: u64, w_h_packed: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let w = (w_h_packed & 0xFFFF) as i32;
    let h = ((w_h_packed >> 16) & 0xFFFF) as i32;
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(win) if win.generation == gen => {
            win.x = x as i32; win.y = y as i32;
            win.w = w; win.h = h;
            win.dirty = true;
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

// ── DC syscalls ───────────────────────────────────────────────────

fn sys_dc_create(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let ok = s.windows.window(slot).map(|w| w.generation == gen).unwrap_or(false);
    s.unlock();
    if !ok { return err::STALE; }
    match super::draw::create_dc_for(slot) {
        Some(dc) => dc as u64,
        None => err::BAD_DC,
    }
}

fn sys_dc_release(dc_slot: u64) -> u64 {
    super::draw::DC_TABLE_LOCK.acquire();
    let r = if let Some(d) = unsafe { super::draw::dc_table().dcs.get_mut(dc_slot as usize) } {
        if d.used { d.used = false; err::OK } else { err::BAD_DC }
    } else { err::BAD_DC };
    super::draw::DC_TABLE_LOCK.release();
    r
}

// ── Input syscalls ────────────────────────────────────────────────

fn sys_input_poll_key() -> u64 {
    let sc = crate::bmo_core::desktop::poll_key();
    sc as u64
}

fn sys_input_poll_mouse() -> u64 {
    let packed = crate::bmo_core::desktop::poll_mouse();
    packed
}

fn sys_save_dc(dc_slot: u64) -> u64 {
    if super::draw::save_dc(dc_slot as u32) { 1 } else { err::BAD_DC }
}

fn sys_restore_dc(dc_slot: u64) -> u64 {
    if super::draw::restore_dc(dc_slot as u32) { 1 } else { err::BAD_DC }
}

fn sys_draw_image(dc_slot: u64, dst_x: u64, dst_y: u64, pixels_ptr: u64, src_w: u64, src_h: u64) -> u64 {
    if pixels_ptr == 0 || !validate_user_ptr(pixels_ptr, src_w * src_h * 4) { return err::INVALID; }
    let pitch = (src_w * 4).next_multiple_of(16) as u32;
    super::draw::draw_image(dc_slot as u32, dst_x as i32, dst_y as i32,
        pixels_ptr as *const u32, src_w as u32, src_h as u32, pitch);
    err::OK
}

// ── Time syscalls (NR_TIME_*) ──────────────────────────────────────

/// NR_TIME_NOW_NS: devuelve los nanosegundos actuales (TSC-based).
fn sys_time_now_ns() -> u64 {
    let tsc = crate::cpu::rdtsc();
    let hz = crate::cpu::tsc::calibrate();
    if hz == 0 { return 0; }
    tsc.wrapping_mul(1_000_000_000) / hz
}

/// NR_TIME_NOW_US: microsegundos.
fn sys_time_now_us() -> u64 {
    sys_time_now_ns() / 1_000
}

/// NR_TIME_SLEEP_NS: busy-wait N nanosegundos.
fn sys_time_sleep_ns(ns: u64) -> u64 {
    let t0 = sys_time_now_ns();
    loop {
        if sys_time_now_ns().wrapping_sub(t0) >= ns { break; }
        core::hint::spin_loop();
    }
    err::OK
}

/// NR_TIME_SLEEP_MS: busy-wait N milisegundos.
fn sys_time_sleep_ms(ms: u64) -> u64 {
    sys_time_sleep_ns(ms * 1_000_000)
}

// ── Debug syscalls (NR_DEBUG_*) ────────────────────────────────────

/// NR_DEBUG_PRINT: emite un mensaje a la cabina desde Ring 3.
fn sys_debug_print(msg_ptr: u64, len: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_str(msg_ptr, len) { return err::INVALID; }
    let s = unsafe { core::str::from_utf8_unchecked(
        core::slice::from_raw_parts(msg_ptr as *const u8, len as usize)) };
    crate::cabina::info("ring3", s);
    err::OK
}

/// NR_DEBUG_TRACE: igual que print pero con severidad Trace.
fn sys_debug_trace(msg_ptr: u64, len: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_str(msg_ptr, len) { return err::INVALID; }
    let s = unsafe { core::str::from_utf8_unchecked(
        core::slice::from_raw_parts(msg_ptr as *const u8, len as usize)) };
    crate::cabina::trace("ring3", s);
    err::OK
}

/// NR_DEBUG_ASSERT: si cond == 0, emite Fault con msg.
fn sys_debug_assert(cond: u64, msg_ptr: u64, len: u64) -> u64 {
    if cond != 0 { return err::OK; }
    if msg_ptr == 0 || !validate_user_str(msg_ptr, len) { return err::INVALID; }
    let s = unsafe { core::str::from_utf8_unchecked(
        core::slice::from_raw_parts(msg_ptr as *const u8, len as usize)) };
    crate::cabina::fault("ring3.assert", s);
    err::OK
}

/// NR_DEBUG_PANIC: emite Panic y mata el proceso actual.
fn sys_debug_panic(msg_ptr: u64, len: u64) -> u64 {
    if msg_ptr == 0 || !validate_user_str(msg_ptr, len) { return err::INVALID; }
    let s = unsafe { core::str::from_utf8_unchecked(
        core::slice::from_raw_parts(msg_ptr as *const u8, len as usize)) };
    crate::cabina::panic_msg("ring3.panic", s);
    // Ring 3 verá el error y el scheduler lo matará en la próxima salida.
    err::GENERIC
}

// ── Validadores para syscalls desde Ring 3 ────────────────────────

// ── Memory syscalls (NR_MEM_*) ────────────────────────────────────

/// NR_MEM_ALLOC: asigna `size` bytes y devuelve el puntero (kernel heap).
/// v1.8.8: asigna desde el heap del kernel. En v1.9 se mapeará a
/// memoria de usuario por proceso.
fn sys_mem_alloc(size: u64) -> u64 {
    if size == 0 || size > 16 * 1024 * 1024 { return err::INVALID; }
    let p = unsafe { crate::mem::heap::heap_alloc(size as usize, 8) };
    if p.is_null() { err::NO_MEMORY } else { p as u64 }
}

/// NR_MEM_FREE: libera un bloque previamente asignado.
fn sys_mem_free(ptr: u64, size: u64) -> u64 {
    if ptr < 0x1000 { return err::INVALID; }
    if size == 0 { return err::INVALID; }
    unsafe { crate::mem::heap::heap_free(ptr as *mut u8, size as usize, 8); }
    err::OK
}

/// NR_MEM_MAP: mmap estilo POSIX. v1.8.8: stub.
fn sys_mem_map(addr: u64, len: u64, _prot: u64, _flags: u64) -> u64 {
    if len == 0 || len > 16 * 1024 * 1024 { return err::INVALID; }
    // v1.9: implementar con map_user_range.
    let _ = addr;
    err::OK
}

/// NR_MEM_UNMAP: libera un mmap. v1.8.8: stub.
fn sys_mem_unmap(addr: u64, len: u64) -> u64 {
    if addr < 0x1000 || len == 0 { return err::INVALID; }
    // v1.9: implementar con free_user_page_tables (granular).
    err::OK
}

// ── Validador para syscalls desde Ring 3 ──────────────────────────

/// Valida un puntero de usuario que contiene una string UTF-8.
fn validate_user_str(ptr: u64, len: u64) -> bool {
    validate_user_ptr(ptr, len)
}

// ── Filesystem syscalls (NR_FS_*) ─────────────────────────────────

/// NR_FS_OPEN: abre un archivo. Devuelve fd >= 0 o negativo.
fn sys_fs_open(name_ptr: u64, name_len: u64) -> u64 {
    if !validate_user_str(name_ptr, name_len) { return err::INVALID; }
    crate::bmo_core::fs::ramdisk::open(name_ptr, name_len)
}

/// NR_FS_CLOSE: cierra un fd.
fn sys_fs_close(fd: u64) -> u64 {
    crate::bmo_core::fs::ramdisk::close(fd)
}

/// NR_FS_READ: lee `len` bytes de `fd` a `ptr`.
fn sys_fs_read(fd: u64, ptr: u64, len: u64) -> u64 {
    if !validate_user_ptr(ptr, len) { return err::INVALID; }
    crate::bmo_core::fs::ramdisk::read(fd, ptr, len)
}

/// NR_FS_WRITE: escribe `len` bytes a `fd` desde `ptr`.
fn sys_fs_write(fd: u64, ptr: u64, len: u64) -> u64 {
    if !validate_user_ptr(ptr, len) { return err::INVALID; }
    crate::bmo_core::fs::ramdisk::write(fd, ptr, len)
}

/// NR_FS_SEEK: reposiciona el cursor del fd.
fn sys_fs_seek(fd: u64, offset: u64, whence: u64) -> u64 {
    crate::bmo_core::fs::ramdisk::seek(fd, offset, whence)
}

/// NR_FS_STAT: tamaño del archivo. v1.8.8: usa ramdisk::size.
fn sys_fs_stat(fd: u64) -> u64 {
    crate::bmo_core::fs::ramdisk::size(fd)
}

/// NR_FS_MKDIR: crea un directorio. v1.8.8: stub.
fn sys_fs_mkdir(_name_ptr: u64, _name_len: u64) -> u64 {
    // v1.9: implementar en fs::manager.
    err::OK
}

/// NR_FS_READDIR: lista directorio. v1.8.8: stub.
fn sys_fs_readdir(_name_ptr: u64, _name_len: u64, _buf_ptr: u64, _buf_len: u64) -> u64 {
    // v1.9: implementar en fs::manager.
    err::OK
}

/// NR_FS_DELETE: borra archivo. v1.8.8: stub.
fn sys_fs_delete(_name_ptr: u64, _name_len: u64) -> u64 {
    err::OK
}

/// NR_FS_MOUNT: monta filesystem. v1.8.8: stub.
fn sys_fs_mount(_src_ptr: u64, _src_len: u64, _dst_ptr: u64, _dst_len: u64, _fs: u64) -> u64 {
    err::OK
}

// ── Process/Thread syscalls (NR_PROC_*, NR_THREAD_*) ─────────────

/// NR_PROC_SPAWN: crea un nuevo proceso desde un BEF (stub v1.8.8).
fn sys_proc_spawn(_bef_ptr: u64, _bef_len: u64) -> u64 {
    // v1.9: implementar carga dinámica de BEF.
    err::OK
}

/// NR_PROC_EXIT: termina el proceso actual.
fn sys_proc_exit(code: u64) -> u64 {
    // Llamamos a kill_current_process con vector 0xFF (synthetic).
    // Esta función es `-> !` así que no retorna, pero la marcamos
    // como tal para que el compilador no se queje.
    crate::proc::process::kill_current_process(0xFF, code, 0);
}

/// NR_PROC_GET_PID: devuelve el PID del proceso actual.
fn sys_proc_get_pid() -> u64 {
    use crate::proc::task;
    match task::current() {
        Some(t) => t.pid.0 as u64,
        None => 0,
    }
}

/// NR_PROC_GET_TID: devuelve el TID del thread actual.
fn sys_proc_get_tid() -> u64 {
    use crate::proc::task;
    match task::current() {
        Some(t) => t.tid.0 as u64,
        None => 0,
    }
}

/// NR_PROC_YIELD: cede el CPU al scheduler.
fn sys_proc_yield() -> u64 {
    crate::proc::yield_now();
    err::OK
}

/// NR_THREAD_CREATE: crea un thread en el proceso actual. v1.8.8: stub.
fn sys_thread_create(_entry: u64, _arg: u64) -> u64 {
    // v1.9: implementar alloc + start.
    err::OK
}

/// NR_THREAD_EXIT: termina el thread actual.
fn sys_thread_exit() -> u64 {
    // v1.9: implementar exit del thread (sin matar el proceso).
    err::OK
}

/// NR_THREAD_JOIN: espera a un thread. v1.8.8: stub.
fn sys_thread_join(_tid: u64) -> u64 {
    err::OK
}

/// NR_THREAD_SELF: devuelve el TID actual (alias de GET_TID).
fn sys_thread_self() -> u64 {
    sys_proc_get_tid()
}

// ── Audio syscalls (NR_AUDIO_*) ──────────────────────────────────

/// NR_AUDIO_PLAY: inicia reproducción de un track (v1.8.8: stub).
fn sys_audio_play(_track_id: u64) -> u64 {
    // v1.9: reproducir desde RAM.
    err::OK
}

/// NR_AUDIO_STOP: detiene reproducción.
fn sys_audio_stop() -> u64 {
    // v1.8.8: el audio se gestiona por eventos (logon, error, etc).
    // En v1.9 se tendrá un canal que se puede detener.
    err::OK
}

/// NR_AUDIO_BEEP: beep del PC speaker.
fn sys_audio_beep(freq: u64, ms: u64) -> u64 {
    if freq == 0 { return err::OK; }
    crate::bmo_core::desktop::beep(freq as u32, ms as u32);
    err::OK
}

/// NR_AUDIO_LOAD_WAVE: carga un WAVE desde RAM (v1.8.8: stub).
fn sys_audio_load_wave(_ptr: u64, _len: u64) -> u64 {
    err::OK
}

// ── Compositor syscalls (NR_COMPOSITOR_*) ────────────────────────

/// NR_COMPOSITOR_BEGIN_FRAME: inicia un frame de composición.
fn sys_compositor_begin_frame() -> u64 {
    // v1.8.8: el compositor se actualiza por tick (timer ISR).
    err::OK
}

/// NR_COMPOSITOR_END_FRAME: termina el frame.
fn sys_compositor_end_frame() -> u64 {
    err::OK
}

/// NR_COMPOSITOR_PRESENT: presenta al FB.
fn sys_compositor_present() -> u64 {
    crate::bmo_core::bmo_api::paint_compositor::tick();
    err::OK
}

/// NR_COMPOSITOR_SET_TARGET: cambia el destino (v1.8.8: stub).
fn sys_compositor_set_target(_ptr: u64, _w: u64, _h: u64, _stride: u64) -> u64 {
    err::OK
}

/// NR_COMPOSITOR_FLUSH: vacía la cola (v1.8.8: stub).
fn sys_compositor_flush() -> u64 {
    err::OK
}

// ── Draw extras (NR_DRAW_CIRCLE etc.) ────────────────────────────

/// NR_DRAW_CIRCLE: dibuja un círculo. v1.8.8: stub con fill_rect.
fn sys_draw_circle(_dc: u64, _cx: u64, _cy: u64, _r: u64, color: u64) -> u64 {
    // v1.9: implementar draw_circle real (Bresenham).
    let _ = color;
    err::OK
}

/// NR_DRAW_TEXT (alias WINPAINT_DRAW_TEXT): dibuja texto en un DC.
fn sys_draw_text_user(dc: u64, x: u64, y: u64, text_ptr: u64, len: u64) -> u64 {
    if !validate_user_str(text_ptr, len) { return err::INVALID; }
    let s = unsafe { core::slice::from_raw_parts(text_ptr as *const u8, len as usize) };
    super::draw::draw_text(dc as u32, x as i32, y as i32, s, 0xFFFFFFFF);
    err::OK
}

/// NR_DRAW_GRADIENT_H: gradiente horizontal. v1.8.8: stub.
fn sys_draw_gradient_h(dc: u64, x: u64, y: u64, w: u64, h: u64, c0: u64) -> u64 {
    // v1.9: implementar real. v1.8.8 fallback: fill_rect.
    super::draw::fill_rect(dc as u32, x as i32, y as i32, w as i32, h as i32, c0 as u32);
    err::OK
}

/// NR_DRAW_GRADIENT_V: gradiente vertical. v1.8.8: stub.
fn sys_draw_gradient_v(dc: u64, x: u64, y: u64, w: u64, h: u64, c0: u64) -> u64 {
    super::draw::fill_rect(dc as u32, x as i32, y as i32, w as i32, h as i32, c0 as u32);
    err::OK
}

/// NR_DRAW_ROUNDED_RECT: rectángulo redondeado. v1.8.8: stub.
fn sys_draw_rounded_rect(dc: u64, x: u64, y: u64, w: u64, h: u64, _r: u64, color: u64) -> u64 {
    super::draw::fill_rect(dc as u32, x as i32, y as i32, w as i32, h as i32, color as u32);
    err::OK
}

// ── WinPaint extras (NR_WINPAINT_DRAW_*) ─────────────────────────

/// NR_WINPAINT_DRAW_PIXEL: pixel individual en paint DC.
fn sys_winpaint_draw_pixel(dc: u64, x: u64, y: u64, color: u64) -> u64 {
    super::draw::draw_pixel(dc as u32, x as i32, y as i32, color as u32);
    err::OK
}

/// NR_WINPAINT_DRAW_LINE: línea en paint DC.
fn sys_winpaint_draw_line(dc: u64, x0: u64, y0: u64, x1: u64, y1: u64, color: u64) -> u64 {
    super::draw::draw_line(dc as u32, x0 as i32, y0 as i32, x1 as i32, y1 as i32, color as u32);
    err::OK
}

/// NR_WINPAINT_DRAW_CIRCLE: círculo en paint DC. v1.8.8: stub.
fn sys_winpaint_draw_circle(_dc: u64, _cx: u64, _cy: u64, _r: u64, _color: u64) -> u64 {
    err::OK
}




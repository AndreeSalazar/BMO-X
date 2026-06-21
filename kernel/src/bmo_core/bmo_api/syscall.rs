//! v2.0 — Dispatcher principal de la BMO API.
//!
//! Conectado al rango 0x100..0x1FF desde `crate::arch::syscall_entry`.
//! Maneja también 0x198 = BMO_DISPATCH_RETURN (retorno de wnd_proc).
//!
//! Convencion: rax=nr, rdi=a0, rsi=a1, rdx=a2, r10=a3, r8=a4, r9=a5.
//! Devuelve el valor a poner en rax (errno negativo o handle/entero).

#![allow(dead_code)]

use super::window::{style, WID_INVALID};
use super::surface;
use super::message::{BmoMsg, BmoMsgKind};
use super::handle::BmoHandle;
use super::class;

// ── Syscall numbers (deben coincidir con docs/BMO_API_SPEC.md §3.3) ──
pub mod nr {
    pub const REGISTER_CLASS:        u16 = 0x100;
    pub const UNREGISTER_CLASS:      u16 = 0x101;
    pub const CREATE_WINDOW_EX:      u16 = 0x102;
    pub const CREATE_WINDOW:         u16 = 0x103;
    pub const DESTROY_WINDOW:        u16 = 0x104;
    pub const SHOW_WINDOW:           u16 = 0x105;
    pub const HIDE_WINDOW:           u16 = 0x106;
    pub const SET_TITLE:             u16 = 0x107;
    pub const GET_TITLE:             u16 = 0x108;
    pub const SET_SIZE:              u16 = 0x109;
    pub const SET_POS:               u16 = 0x10A;
    pub const GET_RECT:              u16 = 0x10B;
    pub const SET_PARENT:            u16 = 0x10C;
    pub const INVALIDATE:            u16 = 0x10D;
    pub const UPDATE_WINDOW:         u16 = 0x10E;
    pub const REDRAW_WINDOW:         u16 = 0x10F;

    pub const PAINT_BEGIN:           u16 = 0x110;
    pub const PAINT_END:             u16 = 0x111;
    pub const DRAW_PIXEL:            u16 = 0x112;
    pub const DRAW_LINE:             u16 = 0x113;
    pub const DRAW_RECT:             u16 = 0x114;
    pub const FILL_RECT:             u16 = 0x115;
    pub const DRAW_TEXT:             u16 = 0x116;
    pub const DRAW_IMAGE:            u16 = 0x117;
    pub const DRAW_POLYLINE:         u16 = 0x118;
    pub const SET_CLIP:              u16 = 0x119;
    pub const RESET_CLIP:            u16 = 0x11A;
    pub const SET_TEXT_COLOR:        u16 = 0x11B;
    pub const SET_BG_COLOR:          u16 = 0x11C;
    pub const SET_FONT:              u16 = 0x11D;
    pub const CREATE_SURFACE:        u16 = 0x11E;
    pub const DESTROY_SURFACE:       u16 = 0x11F;

    pub const GET_MESSAGE:           u16 = 0x120;
    pub const PEEK_MESSAGE:          u16 = 0x121;
    pub const POST_MESSAGE:          u16 = 0x122;
    pub const SEND_MESSAGE:          u16 = 0x123;
    pub const DISPATCH_MESSAGE:      u16 = 0x124;
    pub const TRANSLATE_MESSAGE:     u16 = 0x125;
    pub const WAIT_MESSAGE:          u16 = 0x126;
    pub const POST_QUIT:             u16 = 0x127;
    pub const POST_THREAD_MESSAGE:   u16 = 0x128;
    pub const SET_TIMER:             u16 = 0x129;
    pub const KILL_TIMER:            u16 = 0x12A;
    pub const SET_CAPTURE:           u16 = 0x12B;
    pub const RELEASE_CAPTURE:       u16 = 0x12C;
    pub const SET_FOCUS:             u16 = 0x12D;
    pub const GET_FOCUS:             u16 = 0x12E;
    pub const GET_ACTIVE:            u16 = 0x12F;

    pub const DC_CREATE:             u16 = 0x130;
    pub const DC_RELEASE:            u16 = 0x131;
    pub const GET_DC:                u16 = 0x132;
    pub const RELEASE_DC:            u16 = 0x133;
    pub const SAVE_DC:               u16 = 0x134;
    pub const RESTORE_DC:            u16 = 0x135;
    pub const SELECT_OBJECT:         u16 = 0x136;
    pub const GET_PIXEL:             u16 = 0x137;
    pub const SET_PIXEL:             u16 = 0x138;
    pub const BIT_BLT:               u16 = 0x139;

    pub const INPUT_POLL_KEY:        u16 = 0x140;
    pub const INPUT_POLL_MOUSE:      u16 = 0x141;
    pub const INPUT_WAIT:            u16 = 0x142;
    pub const INPUT_GRAB:            u16 = 0x143;
    pub const INPUT_UNGRAB:          u16 = 0x144;
    pub const SHOW_CURSOR:           u16 = 0x145;
    pub const HIDE_CURSOR:           u16 = 0x146;
    pub const SET_CURSOR_POS:        u16 = 0x147;
    pub const SET_CURSOR:            u16 = 0x148;

    pub const BRING_TO_FRONT:        u16 = 0x150;
    pub const SEND_TO_BACK:          u16 = 0x151;
    pub const SET_TOPMOST:           u16 = 0x152;
    pub const SET_TRANSIENT_FOR:     u16 = 0x153;
    pub const BEGIN_MODAL:           u16 = 0x154;
    pub const END_MODAL:             u16 = 0x155;
    pub const SET_WINDOW_POS:        u16 = 0x156;
    pub const GET_WINDOW:            u16 = 0x157;
    pub const ENUM_WINDOWS:          u16 = 0x158;
    pub const GET_DESKTOP_WINDOW:    u16 = 0x159;
    pub const GET_FOREGROUND_WINDOW: u16 = 0x15A;

    pub const LOAD_CURSOR:           u16 = 0x160;
    pub const LOAD_ICON:             u16 = 0x161;
    pub const SET_CLASS_CURSOR:      u16 = 0x162;
    pub const SET_CLASS_ICON:        u16 = 0x163;

    pub const OPEN_CLIPBOARD:        u16 = 0x170;
    pub const CLOSE_CLIPBOARD:       u16 = 0x171;
    pub const SET_CLIPBOARD_DATA:    u16 = 0x172;
    pub const GET_CLIPBOARD_DATA:    u16 = 0x173;
    pub const EMPTY_CLIPBOARD:       u16 = 0x174;

    pub const MAP_SURFACE:           u16 = 0x180;
    pub const UNMAP_SURFACE:         u16 = 0x181;
    pub const SURFACE_FLUSH:         u16 = 0x182;
    pub const FLIP:                  u16 = 0x183;

    /// Retorno desde un wnd_proc. El kernel lo intercepta y restaura
    /// el contexto de la llamada original.
    pub const DISPATCH_RETURN:       u16 = 0x198;
}

// ── Error codes (negativos en rax) ──
pub mod err {
    pub const OK: u64             = 0;
    pub const GENERIC: u64        = u64::MAX;     // -1
    pub const BAD_HANDLE: u64     = u64::MAX - 1; // -2
    pub const INVALID: u64        = u64::MAX - 2;
    pub const NO_MEMORY: u64      = u64::MAX - 3;
    pub const NO_WINDOW: u64      = u64::MAX - 4;
    pub const NOT_GUI_THR: u64    = u64::MAX - 5;
    pub const QUEUE_FULL: u64     = u64::MAX - 6;
    pub const BAD_CLASS: u64      = u64::MAX - 7;
    pub const CLASS_EXISTS: u64   = u64::MAX - 8;
    pub const NO_CLASS: u64       = u64::MAX - 9;
    pub const BAD_DC: u64         = u64::MAX - 10;
    pub const BAD_SURFACE: u64    = u64::MAX - 11;
    pub const BUSY: u64           = u64::MAX - 12;
    pub const TIMEOUT: u64        = u64::MAX - 13;
    pub const BAD_FORMAT: u64     = u64::MAX - 14;
    pub const NO_QUIT: u64        = u64::MAX - 15;
    pub const REENTRANT: u64      = u64::MAX - 16;
    pub const PERM: u64           = u64::MAX - 17;
    pub const STALE: u64          = u64::MAX - 18;
}

/// Dispatcher principal. Devuelve el valor para rax.
pub fn dispatch(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    match nr {
        // ── Window lifecycle ─────────────────────────────────────
        nr::REGISTER_CLASS   => sys_register_class(a0),
        nr::UNREGISTER_CLASS => sys_unregister_class(a0 as u16),
        // CREATE_WINDOW_EX(class_id, title_ptr, title_len, style, style_ex, x_y_packed, w, h)
        // a4 = low32:x, a5 = high32:y. En v2.0 simplificado w/h no se
        // pasan por registro; se usan valores por defecto.
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
        nr::SET_SIZE         => sys_set_size(a0, a1, a2),
        nr::SET_POS          => sys_set_pos(a0, a1, a2),
        nr::INVALIDATE       => sys_invalidate(a0),
        nr::UPDATE_WINDOW    => sys_invalidate(a0),

        // ── Paint ────────────────────────────────────────────────
        nr::PAINT_BEGIN      => sys_paint_begin(a0, a1),
        nr::PAINT_END        => sys_paint_end(a0, a1),
        nr::FILL_RECT        => err::OK,
        nr::DRAW_TEXT        => err::OK,
        nr::DRAW_LINE        => err::OK,
        nr::DRAW_PIXEL       => err::OK,
        nr::DRAW_RECT        => err::OK,
        nr::DRAW_IMAGE       => err::OK,
        nr::DRAW_POLYLINE    => err::OK,
        nr::SET_CLIP         => err::OK,
        nr::RESET_CLIP       => err::OK,
        nr::SET_TEXT_COLOR   => err::OK,
        nr::SET_BG_COLOR     => err::OK,
        nr::SET_FONT         => err::OK,

        // ── Surfaces ─────────────────────────────────────────────
        nr::CREATE_SURFACE   => sys_create_surface(a0 as u16, a1 as u16, a2 as u32),
        nr::DESTROY_SURFACE  => err::OK,
        nr::MAP_SURFACE      => surface::surface_storage_addr(),
        nr::UNMAP_SURFACE    => err::OK,
        nr::SURFACE_FLUSH    => err::OK,
        nr::FLIP             => err::OK,

        // ── Messages ─────────────────────────────────────────────
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
        nr::KILL_TIMER       => err::OK,
        nr::SET_CAPTURE      => sys_set_capture(a0),
        nr::RELEASE_CAPTURE  => err::OK,
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

        // ── DC ───────────────────────────────────────────────────
        nr::DC_CREATE        => sys_dc_create(a0),
        nr::DC_RELEASE       => err::OK,
        nr::GET_DC           => sys_dc_create(a0),
        nr::RELEASE_DC       => err::OK,
        nr::SAVE_DC          => err::OK,
        nr::RESTORE_DC       => err::OK,
        nr::SELECT_OBJECT    => err::OK,
        nr::GET_PIXEL        => 0,
        nr::SET_PIXEL        => err::OK,
        nr::BIT_BLT          => err::OK,

        // ── Input ────────────────────────────────────────────────
        nr::INPUT_POLL_KEY   => err::OK,
        nr::INPUT_POLL_MOUSE => err::OK,
        nr::INPUT_WAIT       => err::OK,
        nr::INPUT_GRAB       => err::OK,
        nr::INPUT_UNGRAB     => err::OK,
        nr::SHOW_CURSOR      => { super::cursor::show(); err::OK }
        nr::HIDE_CURSOR      => { super::cursor::hide(); err::OK }
        nr::SET_CURSOR_POS   => err::OK,
        nr::SET_CURSOR       => { super::cursor::set(a0 as u8); err::OK }

        // ── WM ───────────────────────────────────────────────────
        nr::BRING_TO_FRONT   => { super::wm::bring_to_front(a0 as u32); err::OK }
        nr::SEND_TO_BACK     => err::OK,
        nr::SET_TOPMOST      => err::OK,
        nr::SET_TRANSIENT_FOR=> err::OK,
        nr::BEGIN_MODAL      => err::OK,
        nr::END_MODAL        => err::OK,
        nr::SET_WINDOW_POS   => err::OK,
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

        // ── Cursor / Icon ────────────────────────────────────────
        nr::LOAD_CURSOR      => err::OK,
        nr::LOAD_ICON        => err::OK,
        nr::SET_CLASS_CURSOR => err::OK,
        nr::SET_CLASS_ICON   => err::OK,

        // ── Clipboard ────────────────────────────────────────────
        nr::OPEN_CLIPBOARD   => err::OK,
        nr::CLOSE_CLIPBOARD  => err::OK,
        nr::SET_CLIPBOARD_DATA => err::OK,
        nr::GET_CLIPBOARD_DATA => err::OK,
        nr::EMPTY_CLIPBOARD  => err::OK,

        // ── Special: BMO_DISPATCH_RETURN ─────────────────────────
        nr::DISPATCH_RETURN  => {
            // Llamada desde el trampoline de Ring 3 al volver del wnd_proc.
            // v2.0: simplificamos — el wnd_proc se ejecuta en Ring 0 vía
            // default_wnd_proc cuando wnd_proc = 0, así que este caso
            // no llega en el flujo actual. Lo dejamos como OK.
            err::OK
        }

        _ => {
            crate::bmo_core::diag::warn_u64("bmo_api_v2.syscall", "unimplemented nr=", nr as u64);
            err::INVALID
        }
    }
}

// ── Implementaciones individuales ─────────────────────────────────

fn sys_register_class(_a0: u64) -> u64 {
    // a0 = user pointer to struct BmoClass. En v2.0 simplificado,
    // devolvemos una clase built-in (siguiente slot libre).
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
    // Empaquetamos x|y en `xy`: bajo 32 = x, alto 32 = y.
    let x = (xy & 0xFFFFFFFF) as i32;
    let y = ((xy >> 32) & 0xFFFFFFFF) as i32;
    let s = super::state();
    s.lock();
    let slot = match s.windows.alloc_window() {
        Some(s) => s,
        None => { s.unlock(); return err::NO_WINDOW; }
    };
    let surf = s.surfaces.alloc(w.max(1) as u16, h.max(1) as u16, surface::format::XRGB32, slot);
    {
        let win = match s.windows.window_mut(slot) {
            Some(w) => w,
            None => { s.unlock(); return err::NO_WINDOW; }
        };
        win.class_id = class_id;
        win.style = style;
        if (style & style::WS_VISIBLE) != 0 {
            win.flags.0 |= 1; // WF_VISIBLE
            win.visible = true;
        }
        if (style & style::WS_DISABLED) == 0 {
            win.flags.0 |= 2; // WF_ENABLED
            win.enabled = true;
        }
        win.x = x;
        win.y = y;
        win.w = w as i32;
        win.h = h as i32;
        win.surface = surf.unwrap_or(0);
        // Título: copiamos desde user space (sin validar en v2.0).
        let tlen = (title_len as usize).min(63);
        if tlen > 0 && title_ptr != 0 {
            let src = unsafe { core::slice::from_raw_parts(title_ptr as *const u8, tlen) };
            for i in 0..tlen { win.title[i] = src[i]; }
            win.title_len = tlen as u8;
        } else {
            // Título por defecto.
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
    if current_gen != gen {
        s.unlock();
        return err::STALE;
    }
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
        Some(w) if w.generation == gen => {
            w.visible = true;
            w.flags.0 |= 1;
            err::OK
        }
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
        Some(w) if w.generation == gen => {
            w.visible = false;
            w.flags.0 &= !1;
            err::OK
        }
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
            if ptr != 0 && tlen > 0 {
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

fn sys_set_size(handle: u64, w: u64, h: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let r = match s.windows.window_mut(slot) {
        Some(win) if win.generation == gen => {
            win.w = w as i32; win.h = h as i32;
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
        Some(w) if w.generation == gen => {
            w.dirty = true;
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

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
        Some(w) if w.generation == gen => {
            w.dirty = false;
            err::OK
        }
        _ => err::STALE,
    };
    s.unlock();
    r
}

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

fn sys_get_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 { return err::INVALID; }
    // En v2.0 simplificado: pop del thread por tid=0 (kernel thread).
    let qt = super::queue::queue_table();
    qt.lock();
    if let Some(slot) = qt.slot_for_tid(0) {
        if let Some(m) = qt.queues[slot as usize].pop() {
            unsafe {
                *(msg_ptr as *mut BmoMsg) = m;
            }
            qt.unlock();
            return 1;
        }
    }
    qt.unlock();
    0
}

fn sys_peek_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 { return err::INVALID; }
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
    if let Some(qslot) = qt.slot_for_tid(owner) {
        let msg = BmoMsg::new(BmoMsgKind::from_u16(kind), slot as u16, 0, wparam, lparam);
        let ok = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
        if ok { err::OK } else { err::QUEUE_FULL }
    } else { err::NOT_GUI_THR }
}

fn sys_send_message(handle: u64, kind: u16, wparam: u64, lparam: u64) -> u64 {
    // En v2.0 send_message cae en el default_wnd_proc (no hay Ring 3
    // wnd_proc real todavía); devolvemos 0 como retval.
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let cls_id = s.windows.window(slot).and_then(|w| if w.generation == gen { Some(w.class_id) } else { None });
    s.unlock();
    let cls_id = match cls_id { Some(c) => c, None => return err::STALE };
    let s = super::state();
    s.lock();
    let wnd_proc = s.windows.class(cls_id).map(|c| c.wnd_proc).unwrap_or(0);
    s.unlock();
    if wnd_proc != 0 {
        // Ring 3 wnd_proc call path — ver syscall §6.2 del spec.
        // v2.0: en stub. En v2.1 implementamos iretq a user RIP.
        return 0;
    }
    class::default_wnd_proc(slot, BmoMsgKind::from_u16(kind), wparam, lparam)
}

fn sys_dispatch_message(msg_ptr: u64) -> u64 {
    if msg_ptr == 0 { return err::INVALID; }
    let msg = unsafe { *(msg_ptr as *const BmoMsg) };
    let s = super::state();
    s.lock();
    let (target, gen) = (msg.target as u32, 0u16);
    let cls_id = s.windows.window(target).map(|w| w.class_id);
    let owner = s.windows.window(target).map(|w| w.owner_tid);
    s.unlock();
    let cls_id = match cls_id { Some(c) => c, None => return err::NO_WINDOW };
    let _ = (target, gen, owner);
    let s = super::state();
    s.lock();
    let wnd_proc = s.windows.class(cls_id).map(|c| c.wnd_proc).unwrap_or(0);
    s.unlock();
    if wnd_proc != 0 {
        // Ring 3 call — v2.0 stub.
        return 0;
    }
    class::default_wnd_proc(msg.target as u32, BmoMsgKind::from_u16(msg.kind), msg.wparam, msg.lparam)
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

fn sys_set_capture(handle: u64) -> u64 {
    let (slot, gen) = decode_wid(handle);
    let s = super::state();
    s.lock();
    let ok = s.windows.window(slot).map(|w| w.generation == gen).unwrap_or(false);
    if ok { s.windows.capture = slot; }
    s.unlock();
    if ok { err::OK } else { err::STALE }
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

// Necesario para BmoHandle::encode/decode usage (no se usa por ahora).
#[allow(dead_code)]
fn _bh_unused() { let _ = BmoHandle::INVALID; }

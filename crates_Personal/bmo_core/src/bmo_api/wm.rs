//! v3.0 — Window manager: Z-order, focus, drag/resize, snap, modal, alt-tab.

#![allow(dead_code)]

use super::window::{style, wf, WID_INVALID};
use super::message::{BmoMsg, BmoMsgKind};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

static DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
static DRAG_SLOT: AtomicU32 = AtomicU32::new(0);
static DRAG_OFFSET_X: AtomicI32 = AtomicI32::new(0);
static DRAG_OFFSET_Y: AtomicI32 = AtomicI32::new(0);

static RESIZE_ACTIVE: AtomicBool = AtomicBool::new(false);
static RESIZE_SLOT: AtomicU32 = AtomicU32::new(0);
static RESIZE_EDGE: AtomicU32 = AtomicU32::new(0);
static RESIZE_ORIG_X: AtomicI32 = AtomicI32::new(0);
static RESIZE_ORIG_Y: AtomicI32 = AtomicI32::new(0);
static RESIZE_ORIG_W: AtomicI32 = AtomicI32::new(0);
static RESIZE_ORIG_H: AtomicI32 = AtomicI32::new(0);
static RESIZE_MOUSE_X0: AtomicI32 = AtomicI32::new(0);
static RESIZE_MOUSE_Y0: AtomicI32 = AtomicI32::new(0);

const EDGE_NONE: u32 = 0;
const EDGE_LEFT: u32 = 1;
const EDGE_RIGHT: u32 = 2;
const EDGE_TOP: u32 = 4;
const EDGE_BOTTOM: u32 = 8;

const EDGE_MARGIN: i32 = 6;

pub fn create_desktop_window() -> u32 {
    let (fbw, fbh) = unsafe { (crate::info::FB_WIDTH as i32, crate::info::FB_HEIGHT as i32) };
    let s = super::state();
    s.lock();
    let slot = s.windows.alloc_window().expect("no free window slot");
    if let Some(w) = s.windows.window_mut(slot) {
        w.x = 0; w.y = 0;
        w.w = fbw; w.h = fbh;
        w.style = 0;
        w.flags.0 = wf::VISIBLE | wf::ENABLED;
        w.visible = true;
        let d = b"Desktop";
        for i in 0..d.len() { w.title[i] = d[i]; }
        w.title_len = 7;
        let surf = s.surfaces.alloc(fbw as u16, fbh as u16, crate::bmo_abi::surface::BmoFormat::ARGB8 as u32, slot);
        w.surface = surf.unwrap_or(0);
    }
    s.windows.desktop = slot;
    s.windows.focus = slot;
    s.windows.active = slot;
    s.windows.z_push_top(slot);
    s.unlock();
    slot
}

pub fn bring_to_front(slot: u32) {
    let s = super::state();
    s.lock();
    s.windows.z_remove(slot);
    s.windows.z_push_top(slot);
    s.windows.focus = slot;
    s.windows.active = slot;
    s.unlock();
}

pub fn minimize_window(slot: u32) {
    let s = super::state();
    s.lock();
    let (visible, is_desktop) = match s.windows.window(slot) {
        Some(w) => (w.visible, slot == s.windows.desktop),
        None => { s.unlock(); return; }
    };
    if !visible || is_desktop { s.unlock(); return; }
    if let Some(w) = s.windows.window_mut(slot) {
        w.minimized = true;
        w.visible = false;
    }
    s.windows.z_remove(slot);
    let mut new_focus = s.windows.desktop;
    s.windows.z_foreach_top_down(|candidate| {
        if candidate == slot || candidate == s.windows.desktop { return; }
        if let Some(w) = s.windows.window(candidate) {
            if w.visible && w.used {
                new_focus = candidate;
            }
        }
    });
    s.windows.focus = new_focus;
    s.windows.active = new_focus;
    s.unlock();
}

pub fn maximize_window(slot: u32) {
    let (fbw, fbh) = unsafe { (crate::info::FB_WIDTH as i32, crate::info::FB_HEIGHT as i32) };
    let taskbar_h: i32 = 48;
    let s = super::state();
    s.lock();
    let (visible, is_desktop) = match s.windows.window(slot) {
        Some(w) => (w.visible, slot == s.windows.desktop),
        None => { s.unlock(); return; }
    };
    if !visible || is_desktop { s.unlock(); return; }
    let was_maximized = s.windows.window(slot).map(|w| w.maximized).unwrap_or(false);
    let (sx, sy, sw, sh) = s.windows.window(slot).map(|w| (w.x, w.y, w.w, w.h)).unwrap_or((0,0,0,0));
    if was_maximized {
        if let Some(w) = s.windows.window_mut(slot) {
            w.x = w.saved_x; w.y = w.saved_y; w.w = w.saved_w; w.h = w.saved_h;
            w.maximized = false;
            w.dirty = true;
            w.has_dirty_rect = true; w.dirty_x = 0; w.dirty_y = 0; w.dirty_w = w.w; w.dirty_h = w.h;
        }
    } else {
        if let Some(w) = s.windows.window_mut(slot) {
            w.saved_x = sx; w.saved_y = sy; w.saved_w = sw; w.saved_h = sh;
            w.x = 0; w.y = 0; w.w = fbw; w.h = fbh - taskbar_h;
            w.maximized = true;
            w.dirty = true;
            w.has_dirty_rect = true; w.dirty_x = 0; w.dirty_y = 0; w.dirty_w = w.w; w.dirty_h = w.h;
        }
    }
    s.unlock();
}

pub fn restore_window(slot: u32) {
    let s = super::state();
    s.lock();
    let (is_minimized, is_maximized) = match s.windows.window(slot) {
        Some(w) => (w.minimized, w.maximized),
        None => { s.unlock(); return; }
    };
    if !is_minimized && !is_maximized { s.unlock(); return; }
    if is_minimized {
        if let Some(w) = s.windows.window_mut(slot) {
            w.minimized = false;
            w.visible = true;
            w.dirty = true;
        }
        s.windows.z_remove(slot);
        s.windows.z_push_top(slot);
        s.windows.focus = slot;
        s.windows.active = slot;
    }
    if is_maximized {
        if let Some(w) = s.windows.window_mut(slot) {
            w.x = w.saved_x; w.y = w.saved_y; w.w = w.saved_w; w.h = w.saved_h;
            w.maximized = false;
            w.dirty = true;
            w.has_dirty_rect = true; w.dirty_x = 0; w.dirty_y = 0; w.dirty_w = w.w; w.dirty_h = w.h;
        }
    }
    s.unlock();
}

pub fn hit_test(px: i32, py: i32) -> u32 {
    let s = super::state();
    s.lock();
    let mut found = WID_INVALID;
    s.windows.z_foreach_top_down(|slot| {
        if found != WID_INVALID { return; }
        if let Some(w) = s.windows.window(slot) {
            if w.visible && px >= w.x && py >= w.y
                && px < w.x + w.w && py < w.y + w.h {
                found = slot;
            }
        }
    });
    s.unlock();
    found
}

pub fn edge_hit_test(px: i32, py: i32) -> u32 {
    let s = super::state();
    s.lock();
    let mut edge = EDGE_NONE;
    s.windows.z_foreach_top_down(|slot| {
        if edge != EDGE_NONE { return; }
        if let Some(w) = s.windows.window(slot) {
            if !w.visible || slot == s.windows.desktop { return; }
            if (w.style & style::WS_THICKFRAME) == 0 { return; }
            if px >= w.x && py >= w.y && px < w.x + w.w && py < w.y + w.h {
                if px < w.x + EDGE_MARGIN { edge |= EDGE_LEFT; }
                if px >= w.x + w.w - EDGE_MARGIN { edge |= EDGE_RIGHT; }
                if py < w.y + EDGE_MARGIN { edge |= EDGE_TOP; }
                if py >= w.y + w.h - EDGE_MARGIN { edge |= EDGE_BOTTOM; }
            }
        }
    });
    s.unlock();
    edge
}

pub fn edge_cursor(edge: u32) -> u8 {
    let l = edge & EDGE_LEFT != 0;
    let r = edge & EDGE_RIGHT != 0;
    let t = edge & EDGE_TOP != 0;
    let b = edge & EDGE_BOTTOM != 0;
    if l && !r && !t && !b || r && !l && !t && !b {
        super::cursor::id::SIZEWE
    } else if t && !l && !r && !b || b && !l && !r && !t {
        super::cursor::id::SIZENS
    } else if (l || r) && (t || b) {
        let nw_se = (l && t) || (r && b);
        if nw_se { super::cursor::id::SIZENWSE } else { super::cursor::id::SIZENESW }
    } else {
        super::cursor::id::ARROW
    }
}

pub fn raise_and_focus(slot: u32) {
    let s = super::state();
    s.lock();
    let prev = s.windows.focus;
    if prev != WID_INVALID && prev != slot {
        let owner = s.windows.window(prev).map(|w| w.owner_tid).unwrap_or(0);
        s.unlock();
        if owner != 0 {
            let qt = super::queue::queue_table();
            qt.acquire();
            if let Some(qslot) = qt.slot_for_tid(owner) {
                let msg = BmoMsg::new(BmoMsgKind::KillFocus, prev as u16, 0, 0, 0);
                let _ = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
            }
            qt.release();
        }
        bring_to_front(slot);
        let s2 = super::state();
        s2.lock();
        let owner2 = s2.windows.window(slot).map(|w| w.owner_tid).unwrap_or(0);
        s2.unlock();
        if owner2 != 0 {
            let qt = super::queue::queue_table();
            qt.acquire();
            if let Some(qslot) = qt.slot_for_tid(owner2) {
                let msg = BmoMsg::new(BmoMsgKind::SetFocus, slot as u16, 0, 0, 0);
                let _ = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
            }
            qt.release();
        }
        return;
    }
    s.unlock();
    bring_to_front(slot);
}

pub fn start_drag(slot: u32, mx: i32, my: i32) {
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.in_sizemove = true;
        w.flags.set(wf::SIZEMOVE);
        DRAG_OFFSET_X.store(mx - w.x, Ordering::Relaxed);
        DRAG_OFFSET_Y.store(my - w.y, Ordering::Relaxed);
    }
    s.unlock();
    DRAG_SLOT.store(slot, Ordering::Relaxed);
    DRAG_ACTIVE.store(true, Ordering::Relaxed);
    let owner = {
        let s = super::state();
        s.lock();
        let o = s.windows.window(slot).map(|w| w.owner_tid).unwrap_or(0);
        s.unlock();
        o
    };
    if owner != 0 {
        let qt = super::queue::queue_table();
        qt.acquire();
        if let Some(qslot) = qt.slot_for_tid(owner) {
            let msg = BmoMsg::new(BmoMsgKind::EnterSizeMove, slot as u16, 0, 0, 0);
            let _ = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
        }
        qt.release();
    }
}

pub fn end_drag() {
    if !DRAG_ACTIVE.load(Ordering::Relaxed) { return; }
    let slot = DRAG_SLOT.load(Ordering::Relaxed);
    DRAG_ACTIVE.store(false, Ordering::Relaxed);
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.in_sizemove = false;
        w.flags.clear(wf::SIZEMOVE);
        w.dirty = true;
    }
    s.unlock();
    snap_to_edge(slot);
    let owner = {
        let s = super::state();
        s.lock();
        let o = s.windows.window(slot).map(|w| w.owner_tid).unwrap_or(0);
        s.unlock();
        o
    };
    if owner != 0 {
        let qt = super::queue::queue_table();
        qt.acquire();
        if let Some(qslot) = qt.slot_for_tid(owner) {
            let msg = BmoMsg::new(BmoMsgKind::ExitSizeMove, slot as u16, 0, 0, 0);
            let _ = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
        }
        qt.release();
    }
}

pub fn update_drag(mx: i32, my: i32) -> bool {
    if !DRAG_ACTIVE.load(Ordering::Relaxed) { return false; }
    let slot = DRAG_SLOT.load(Ordering::Relaxed);
    let ox = DRAG_OFFSET_X.load(Ordering::Relaxed);
    let oy = DRAG_OFFSET_Y.load(Ordering::Relaxed);
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.x = mx - ox;
        w.y = my - oy;
        w.dirty = true;
    }
    s.unlock();
    true
}

pub fn start_resize(slot: u32, edge: u32, mx: i32, my: i32) {
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.in_sizemove = true;
        w.flags.set(wf::SIZEMOVE);
        RESIZE_ORIG_X.store(w.x, Ordering::Relaxed);
        RESIZE_ORIG_Y.store(w.y, Ordering::Relaxed);
        RESIZE_ORIG_W.store(w.w, Ordering::Relaxed);
        RESIZE_ORIG_H.store(w.h, Ordering::Relaxed);
    }
    s.unlock();
    RESIZE_SLOT.store(slot, Ordering::Relaxed);
    RESIZE_EDGE.store(edge, Ordering::Relaxed);
    RESIZE_MOUSE_X0.store(mx, Ordering::Relaxed);
    RESIZE_MOUSE_Y0.store(my, Ordering::Relaxed);
    RESIZE_ACTIVE.store(true, Ordering::Relaxed);
    super::cursor::set(edge_cursor(edge));
}

pub fn end_resize() {
    if !RESIZE_ACTIVE.load(Ordering::Relaxed) { return; }
    let slot = RESIZE_SLOT.load(Ordering::Relaxed);
    RESIZE_ACTIVE.store(false, Ordering::Relaxed);
    super::cursor::set(super::cursor::id::ARROW);
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.in_sizemove = false;
        w.flags.clear(wf::SIZEMOVE);
        w.dirty = true;
    }
    s.unlock();
}

pub fn update_resize(mx: i32, my: i32) -> bool {
    if !RESIZE_ACTIVE.load(Ordering::Relaxed) { return false; }
    let slot = RESIZE_SLOT.load(Ordering::Relaxed);
    let edge = RESIZE_EDGE.load(Ordering::Relaxed);
    let ox = RESIZE_MOUSE_X0.load(Ordering::Relaxed);
    let oy = RESIZE_MOUSE_Y0.load(Ordering::Relaxed);
    let orig_x = RESIZE_ORIG_X.load(Ordering::Relaxed);
    let orig_y = RESIZE_ORIG_Y.load(Ordering::Relaxed);
    let orig_w = RESIZE_ORIG_W.load(Ordering::Relaxed);
    let orig_h = RESIZE_ORIG_H.load(Ordering::Relaxed);
    let dx = mx - ox;
    let dy = my - oy;
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        let min_w = 120;
        let min_h = 80;
        if edge & EDGE_LEFT != 0 {
            w.x = orig_x + dx;
            w.w = (orig_w - dx).max(min_w);
        }
        if edge & EDGE_RIGHT != 0 {
            w.w = (orig_w + dx).max(min_w);
        }
        if edge & EDGE_TOP != 0 {
            w.y = orig_y + dy;
            w.h = (orig_h - dy).max(min_h);
        }
        if edge & EDGE_BOTTOM != 0 {
            w.h = (orig_h + dy).max(min_h);
        }
        w.dirty = true;
    }
    s.unlock();
    true
}

pub fn snap_to_edge(slot: u32) {
    let s = super::state();
    s.lock();
    let (fw, fh) = unsafe { (crate::info::FB_WIDTH as i32, crate::info::FB_HEIGHT as i32) };
    if let Some(win) = s.windows.window_mut(slot) {
        if win.x < 16 { win.x = 0; }
        if win.y < 36 { win.y = 30; }
        if fw - (win.x + win.w) < 16 { win.x = fw - win.w; }
        if fh - (win.y + win.h) < 16 { win.y = fh - win.h; }
    }
    s.unlock();
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum TitleBtn { Close, Maximize, Minimize, None }

pub fn title_btn_hit_test(slot: u32, px: i32, py: i32) -> TitleBtn {
    let s = super::state();
    s.lock();
    let (wx, wy, style) = match s.windows.window(slot) {
        Some(w) => (w.x, w.y, w.style),
        None => { s.unlock(); return TitleBtn::None; }
    };
    s.unlock();
    if style & style::WS_CAPTION == 0 { return TitleBtn::None; }
    let btn_y = wy + 4;
    let btn_h = 28;
    let btn_r = 14;
    if py >= btn_y && py <= btn_y + btn_h {
        let cx = wx + 18;
        if (px - cx).unsigned_abs() <= btn_r as u32 { return TitleBtn::Close; }
        let cx2 = wx + 38;
        if (px - cx2).unsigned_abs() <= btn_r as u32 { return TitleBtn::Maximize; }
        let cx3 = wx + 58;
        if (px - cx3).unsigned_abs() <= btn_r as u32 { return TitleBtn::Minimize; }
    }
    TitleBtn::None
}

pub fn handle_title_btn_click(slot: u32, btn: TitleBtn) {
    match btn {
        TitleBtn::Close => {
            let s = super::state();
            s.lock();
            s.windows.free_window(slot);
            s.unlock();
        }
        TitleBtn::Maximize => maximize_window(slot),
        TitleBtn::Minimize => minimize_window(slot),
        TitleBtn::None => {}
    }
}

pub fn alt_tab() {
    let s = super::state();
    s.lock();
    let current_focus = s.windows.focus;
    let count = s.windows.visible_count();
    if count <= 1 { s.unlock(); return; }
    let mut found_next = false;
    let mut next_slot = WID_INVALID;
    let mut start_search = false;
    if current_focus == WID_INVALID || current_focus == s.windows.desktop {
        start_search = true;
    }
    s.windows.z_foreach_top_down(|slot| {
        if found_next || slot == s.windows.desktop { return; }
        if start_search {
            if let Some(w) = s.windows.window(slot) {
                if w.visible && slot != s.windows.desktop {
                    next_slot = slot;
                    found_next = true;
                }
            }
        } else if slot == current_focus {
            start_search = true;
        }
    });
    if !found_next {
        s.windows.z_foreach_top_down(|slot| {
            if found_next || slot == s.windows.desktop { return; }
            if let Some(w) = s.windows.window(slot) {
                if w.visible && slot != s.windows.desktop {
                    next_slot = slot;
                    found_next = true;
                }
            }
        });
    }
    s.unlock();
    if found_next && next_slot != WID_INVALID {
        raise_and_focus(next_slot);
    }
}

pub fn enter() -> ! {
    crate::cabina::info("bmo_api_v2.wm", "Entering Ring 3 BMO API desktop");
    crate::dev::console::serial_write("[bmo_api_v2] Entering desktop real (BMO API v3.0)\n");

    let _term = create_top_window("BMO Terminal", 60, 60, 720, 460);
    let _editor = create_top_window("Datos.md viewer", 120, 100, 620, 420);
    let _settings = create_top_window("Ajustes", 180, 140, 520, 380);

    let mut last_tick: u64 = 0;
    loop {
        let now = crate::cpu::rdtsc();
        super::input::poll_and_dispatch();
        process_message_queue();
        if now.wrapping_sub(last_tick) > 33_000_000 {
            super::paint_compositor::tick();
            super::timer::tick_global();
            last_tick = now;
        }
        if super::input::esc_pressed() {
            crate::cabina::info("bmo_api_v2.wm", "ESC pressed — return to welcome");
            crate::dev::console::serial_write("[bmo_api_v2] ESC — returning to welcome.\n");
            crate::desktop::welcome::run();
        }
        core::hint::spin_loop();
    }
}

fn process_message_queue() {
    let focused = {
        let s = super::state();
        s.lock();
        let f = s.windows.focus;
        s.unlock();
        f
    };
    if focused == WID_INVALID { return; }

    let s = super::state();
    s.lock();
    let (owner_tid, wnd_proc) = match s.windows.window(focused) {
        Some(w) => {
            let owner = w.owner_tid;
            let wp = if w.class_id != 0 {
                s.windows.class(w.class_id).map(|c| c.wnd_proc).unwrap_or(0)
            } else { 0 };
            (owner, wp)
        }
        None => { s.unlock(); return; }
    };
    s.unlock();

    if owner_tid != 0 && wnd_proc != 0 {
        let qt = super::queue::queue_table();
        qt.acquire();
        if let Some(slot) = qt.slot_for_tid(owner_tid) {
            let msg = BmoMsg::new(BmoMsgKind::Paint, focused as u16, 0, 0, 0);
            let _ = super::event::post_coalesced(&mut qt.queues[slot as usize], msg);
        }
        qt.release();
    } else {
        super::class::default_wnd_proc(focused, super::message::BmoMsgKind::Paint, 0, 0);
    }
}

pub fn is_dragging() -> bool { DRAG_ACTIVE.load(Ordering::Relaxed) }
pub fn is_resizing() -> bool { RESIZE_ACTIVE.load(Ordering::Relaxed) }

fn create_top_window(title: &'static str, x: i32, y: i32, w: i32, h: i32) -> u32 {
    let st = super::state();
    st.lock();
    let slot = match st.windows.alloc_window() {
        Some(s) => s,
        None => { st.unlock(); return WID_INVALID; }
    };
    let surf = st.surfaces.alloc(w as u16, h as u16, crate::bmo_abi::surface::BmoFormat::ARGB8 as u32, slot);
    {
        let win = match st.windows.window_mut(slot) {
            Some(w) => w,
            None => { st.unlock(); return WID_INVALID; }
        };
        win.x = x; win.y = y; win.w = w; win.h = h;
        win.style = style::WS_OVERLAPPEDWINDOW;
        win.flags.0 = wf::VISIBLE | wf::ENABLED;
        win.visible = true;
        win.surface = surf.unwrap_or(0);
        win.owner_tid = 1;
        let tbytes = title.as_bytes();
        let n = tbytes.len().min(63);
        for i in 0..n { win.title[i] = tbytes[i]; }
        win.title_len = n as u8;
    }
    st.windows.z_push_top(slot);
    st.windows.focus = slot;
    st.windows.active = slot;
    st.unlock();
    slot
}


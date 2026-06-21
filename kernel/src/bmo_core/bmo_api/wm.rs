//! v2.0 — Window manager: Z-order, focus, drag/resize, snap, modal.
//!
//! Componente central del BMO API. Mantiene la Z-list singly-linked,
//! el árbol parent/child, las reglas de focus, drag/resize con
//! snap-to-edge, y el ciclo modal para ventanas de diálogo.

#![allow(dead_code)]

#[allow(unused_imports)]
use super::window::{
    BmoWindow, BmoWindowFlags, WindowTable, style, wf, WID_INVALID, MAX_WINDOWS,
};
use super::message::{BmoMsg, BmoMsgKind};
#[allow(unused_imports)]
use super::handle::BmoHandle;
use super::surface;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

static DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
static DRAG_SLOT: AtomicU32 = AtomicU32::new(0);
static DRAG_OFFSET_X: AtomicI32 = AtomicI32::new(0);
static DRAG_OFFSET_Y: AtomicI32 = AtomicI32::new(0);

pub fn create_desktop_window() -> u32 {
    let (fbw, fbh) = unsafe { (crate::boot::info::FB_WIDTH as i32, crate::boot::info::FB_HEIGHT as i32) };
    let s = super::state();
    s.lock();
    let slot = s.windows.alloc_window().expect("no free window slot");
    {
        if let Some(w) = s.windows.window_mut(slot) {
            w.x = 0; w.y = 0;
            w.w = fbw;
            w.h = fbh;
            w.style = 0;
            w.style_ex = 0;
            w.flags.0 = wf::VISIBLE | wf::ENABLED;
            w.visible = true;
            w.title[0] = b'D'; w.title[1] = b'e'; w.title[2] = b's';
            w.title[3] = b'k'; w.title[4] = b't'; w.title[5] = b'o'; w.title[6] = b'p';
            w.title_len = 7;
            let surf = s.surfaces.alloc(
                fbw as u16, fbh as u16,
                surface::format::XRGB32, slot,
            );
            w.surface = surf.unwrap_or(0);
        }
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
    }
    s.unlock();
    true
}

pub fn snap_to_edge(slot: u32) {
    let s = super::state();
    s.lock();
    let (w, h) = unsafe { (crate::boot::info::FB_WIDTH as i32, crate::boot::info::FB_HEIGHT as i32) };
    if let Some(win) = s.windows.window_mut(slot) {
        if win.x < 16 { win.x = 0; }
        if win.y < 36 { win.y = 30; }
        if w - (win.x + win.w) < 16 { win.x = w - win.w; }
        if h - (win.y + win.h) < 16 { win.y = h - win.h; }
    }
    s.unlock();
}

pub fn enter() -> ! {
    crate::bmo_core::diag::info("bmo_api_v2.wm", "Entering Ring 3 BMO API desktop");
    crate::dev::console::serial_write("[bmo_api_v2] Entering desktop real (BMO API v2.0)\n");

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
            crate::bmo_core::diag::info("bmo_api_v2.wm", "ESC pressed — return to welcome");
            crate::dev::console::serial_write("[bmo_api_v2] ESC — returning to welcome.\n");
            crate::bmo_core::desktop::welcome::run();
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
    let cls_id = s.windows.window(focused).map(|w| w.class_id);
    let wnd_proc = cls_id.and_then(|cid| s.windows.class(cid).map(|c| c.wnd_proc));
    s.unlock();

    let wnd_proc = match wnd_proc { Some(wp) => wp, None => return };

    if wnd_proc == 0 {
        super::class::default_wnd_proc(focused, super::message::BmoMsgKind::Paint, 0, 0);
    }
}

pub fn is_dragging() -> bool { DRAG_ACTIVE.load(Ordering::Relaxed) }

fn create_top_window(title: &'static str, x: i32, y: i32, w: i32, h: i32) -> u32 {
    let st = super::state();
    st.lock();
    let slot = match st.windows.alloc_window() {
        Some(s) => s,
        None => { st.unlock(); return WID_INVALID; }
    };
    let surf = st.surfaces.alloc(w as u16, h as u16, surface::format::XRGB32, slot);
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

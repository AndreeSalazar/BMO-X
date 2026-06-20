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
use crate::boot_info;

/// Crea la ventana de escritorio (cubre todo el framebuffer).
pub fn create_desktop_window() -> u32 {
    let (fbw, fbh) = unsafe { (boot_info::FB_WIDTH as i32, boot_info::FB_HEIGHT as i32) };
    let s = super::state();
    s.lock();
    let slot = s.windows.alloc_window().expect("no free window slot");
    {
        let w = s.windows.window_mut(slot).unwrap();
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
        // Crea una surface del tamaño del desktop.
        let surf = s.surfaces.alloc(
            fbw as u16, fbh as u16,
            surface::format::XRGB32, slot,
        );
        w.surface = surf.unwrap_or(0);
    }
    s.windows.desktop = slot;
    s.windows.focus = slot;
    s.windows.active = slot;
    s.windows.z_push_top(slot);
    s.unlock();
    slot
}

/// Trae la ventana al tope (raise).
pub fn bring_to_front(slot: u32) {
    let s = super::state();
    s.lock();
    s.windows.z_remove(slot);
    s.windows.z_push_top(slot);
    s.windows.focus = slot;
    s.windows.active = slot;
    s.unlock();
}

/// Devuelve el slot de la ventana más alta bajo el punto (px, py).
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

/// Mueve una ventana al frente y le da foco. Usado por click en
/// title bar o por atajos de teclado (alt-tab).
pub fn raise_and_focus(slot: u32) {
    let s = super::state();
    s.lock();
    let prev = s.windows.focus;
    s.unlock();
    if prev != WID_INVALID && prev != slot {
        post_killfocus(prev);
    }
    bring_to_front(slot);
    post_setfocus(slot);
}

fn post_setfocus(slot: u32) {
    post_message_to_owner(slot, BmoMsgKind::SetFocus, 0, 0);
}
fn post_killfocus(slot: u32) {
    post_message_to_owner(slot, BmoMsgKind::KillFocus, 0, 0);
}
fn post_message_to_owner(slot: u32, kind: BmoMsgKind, wparam: u64, lparam: u64) {
    let s = super::state();
    s.lock();
    let owner_tid = s.windows.window(slot).map(|w| w.owner_tid).unwrap_or(0);
    s.unlock();
    if owner_tid == 0 { return; }
    let qt = super::queue::queue_table();
    qt.lock();
    if let Some(qslot) = qt.slot_for_tid(owner_tid) {
        let msg = BmoMsg::new(kind, slot as u16, 0, wparam, lparam);
        let _ = super::event::post_coalesced(&mut qt.queues[qslot as usize], msg);
    }
    qt.unlock();
}

/// Inicia drag: el WM entra en modal loop y emite MOUSEMOVE hasta LBUTTONUP.
pub fn start_drag(slot: u32, mx: i32, my: i32) {
    let s = super::state();
    s.lock();
    if let Some(w) = s.windows.window_mut(slot) {
        w.in_sizemove = true;
        w.flags.set(wf::SIZEMOVE);
    }
    s.unlock();
    crate::bmo_core::diag::info_u64("bmo_api_v2.wm", "drag start slot=", slot as u64);
    crate::bmo_core::diag::info_u64("bmo_api_v2.wm", "    at mx=", mx as u64);
    crate::bmo_core::diag::info_u64("bmo_api_v2.wm", "    my=", my as u64);
    let _ = (mx, my);
}

/// Snap a la ventana al borde más cercano si está a ≤ 16 px.
pub fn snap_to_edge(slot: u32) {
    let s = super::state();
    s.lock();
    let (w, h) = unsafe { (boot_info::FB_WIDTH as i32, boot_info::FB_HEIGHT as i32) };
    if let Some(win) = s.windows.window_mut(slot) {
        if win.x < 16 { win.x = 0; }
        if win.y < 36 { win.y = 30; }
        if w - (win.x + win.w) < 16 { win.x = w - win.w; }
        if h - (win.y + win.h) < 16 { win.y = h - win.h; }
    }
    s.unlock();
}

/// Entra al desktop real: lo llama `desktop::welcome::process_enter`
/// cuando el usuario escribe "Run". Crea ventanas built-in, las pone
/// en la Z-list y entra en el loop de pintado. Devuelve cuando el
/// usuario presiona ESC (igual que el viejo desktop stub).
pub fn enter() -> ! {
    crate::bmo_core::diag::info("bmo_api_v2.wm", "Entering Ring 3 BMO API desktop");
    crate::device::serial::serial_write("[bmo_api_v2] Entering desktop real (BMO API v2.0)\n");

    // Crea tres ventanas built-in para demostrar el WM.
    let _term = create_top_window("BMO Terminal", 60, 60, 720, 460);
    let _editor = create_top_window("Datos.md viewer", 120, 100, 620, 420);
    let _settings = create_top_window("Ajustes", 180, 140, 520, 380);

    // Loop principal: en v2.0 consume mensajes, procesa mouse y
    // hace repaint de las superficies modificadas. La integración
    // completa con wnd_proc Ring 3 está descrita en el spec §6 y
    // se completará cuando los Ring 3 programs estén listos.
    let mut last_tick: u64 = 0;
    loop {
        let now = crate::cpu::rdtsc();
        // Procesa input de PS/2 → eventos.
        super::input::poll_and_dispatch();
        // 30 Hz repaint (33 ms ≈ 1_000_000_000 ciclos @ 3 GHz).
        if now.wrapping_sub(last_tick) > 33_000_000 {
            super::paint_compositor::tick();
            super::timer::tick_global();
            last_tick = now;
        }
        // ESC → volver al welcome.
        if super::input::esc_pressed() {
            crate::bmo_core::diag::info("bmo_api_v2.wm", "ESC pressed — return to welcome");
            crate::device::serial::serial_write("[bmo_api_v2] ESC — returning to welcome.\n");
            crate::bmo_core::desktop::welcome::run();
        }
        core::hint::spin_loop();
    }
}

fn create_top_window(title: &'static str, x: i32, y: i32, w: i32, h: i32) -> u32 {
    let st = super::state();
    st.lock();
    let slot = match st.windows.alloc_window() {
        Some(s) => s,
        None => { st.unlock(); return WID_INVALID; }
    };
    let surf = st.surfaces.alloc(w as u16, h as u16, surface::format::XRGB32, slot);
    {
        let win = st.windows.window_mut(slot).unwrap();
        win.x = x; win.y = y; win.w = w; win.h = h;
        win.style = style::WS_OVERLAPPEDWINDOW;
        win.flags.0 = wf::VISIBLE | wf::ENABLED;
        win.visible = true;
        win.surface = surf.unwrap_or(0);
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

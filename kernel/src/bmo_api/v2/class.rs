//! v2.0 — Tabla de clases, default wnd_proc, clases built-in.

#![allow(dead_code)]

#[allow(unused_imports)]
use super::super::v2::window::{BmoClass, cs};
use super::super::v2::message::{BmoMsg, BmoMsgKind};

/// Nombres de las clases built-in (WNDCLASSEX-style).
pub mod names {
    pub const BMO_CLASS: &[u8]  = b"BmoClass\0";
    pub const BMO_BUTTON: &[u8] = b"BmoButton\0";
    pub const BMO_STATIC: &[u8] = b"BmoStatic\0";
    pub const BMO_EDIT: &[u8]   = b"BmoEdit\0";
    pub const BMO_LISTBOX: &[u8] = b"BmoListBox\0";
}

pub const CLASS_ID_BASE: u16 = 1; // IDs 1..=8 son built-in kernel-side.

/// Registra las clases built-in en la tabla. El `wnd_proc` de las clases
/// built-in es 0 → el kernel las despacha él mismo (no se llama a Ring 3).
pub fn register_builtin_classes() {
    let s = super::state();
    s.lock();
    // BMO_CLASS — clase genérica root.
    if let Some(slot) = s.windows.alloc_class() {
        if let Some(c) = s.windows.class_mut(slot) {
            copy_name(&mut c.name, &mut c.name_len, names::BMO_CLASS);
            c.style = cs::DBLCLKS;
            c.hbr_background = 1; // COLOR_WINDOW
        }
    }
    // BMO_BUTTON
    if let Some(slot) = s.windows.alloc_class() {
        if let Some(c) = s.windows.class_mut(slot) {
            copy_name(&mut c.name, &mut c.name_len, names::BMO_BUTTON);
            c.style = cs::DBLCLKS;
            c.hbr_background = 2;
        }
    }
    // BMO_STATIC
    if let Some(slot) = s.windows.alloc_class() {
        if let Some(c) = s.windows.class_mut(slot) {
            copy_name(&mut c.name, &mut c.name_len, names::BMO_STATIC);
            c.style = 0;
            c.hbr_background = 0;
        }
    }
    // BMO_EDIT
    if let Some(slot) = s.windows.alloc_class() {
        if let Some(c) = s.windows.class_mut(slot) {
            copy_name(&mut c.name, &mut c.name_len, names::BMO_EDIT);
            c.style = cs::DBLCLKS;
            c.hbr_background = 1;
        }
    }
    // BMO_LISTBOX
    if let Some(slot) = s.windows.alloc_class() {
        if let Some(c) = s.windows.class_mut(slot) {
            copy_name(&mut c.name, &mut c.name_len, names::BMO_LISTBOX);
            c.style = 0;
            c.hbr_background = 1;
        }
    }
    s.unlock();
}

fn copy_name(dst: &mut [u8; 32], dst_len: &mut u8, src: &[u8]) {
    let n = src.len().min(32);
    for i in 0..n { dst[i] = src[i]; }
    *dst_len = n as u8;
}

/// Default wnd_proc para ventanas que no proveen uno (BMO_DEFDLGPROC).
/// En el spec es una función Ring 0. En v2.0 lo implementamos como
/// un match sobre el kind. Devuelve 0 por defecto, 1 si procesó.
pub fn default_wnd_proc(_hwnd: u32, msg: BmoMsgKind, _wparam: u64, _lparam: u64) -> u64 {
    match msg {
        BmoMsgKind::NcPaint => 1,
        BmoMsgKind::NcCalcSize => 1,
        BmoMsgKind::EraseBkGnd => 1,
        BmoMsgKind::GetMinMaxInfo => 1,
        BmoMsgKind::KeyDown => 1, // traducción a CHAR
        _ => 0,
    }
}

/// Comprueba si un kind de mensaje debe ser traducido por el kernel
/// (pre-translate hook, como TranslateMessage en Win32).
pub fn translate(_msg: &BmoMsg) -> Option<BmoMsg> {
    // En v2.0 sólo KEYDOWN con scancode imprimible genera un CHAR.
    // El escaneo se hace en el input thread, no aquí.
    None
}

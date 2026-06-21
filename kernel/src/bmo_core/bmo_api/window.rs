//! v2.0 — Tabla de ventanas, clases, Z-order, parent/child tree.
//!
//! `WINDOWS[256]` es la tabla plana. El árbol se mantiene con índices
//! (no punteros) para que sea estable frente a movimientos de memoria.

#![allow(dead_code)]

pub const MAX_WINDOWS: usize = 256;
pub const MAX_CLASSES: usize = 32;
pub const WID_INVALID: u32 = 0xFFFFFFFF;

pub mod style {
    pub const WS_OVERLAPPED: u32 = 0x00000000;
    pub const WS_POPUP: u32 = 0x80000000;
    pub const WS_CHILD: u32 = 0x40000000;
    pub const WS_MINIMIZE: u32 = 0x20000000;
    pub const WS_VISIBLE: u32 = 0x10000000;
    pub const WS_DISABLED: u32 = 0x08000000;
    pub const WS_CLIPSIBLINGS: u32 = 0x04000000;
    pub const WS_CLIPCHILDREN: u32 = 0x02000000;
    pub const WS_MAXIMIZE: u32 = 0x01000000;
    pub const WS_CAPTION: u32 = 0x00C00000;
    pub const WS_BORDER: u32 = 0x00800000;
    pub const WS_DLGFRAME: u32 = 0x00400000;
    pub const WS_VSCROLL: u32 = 0x00200000;
    pub const WS_HSCROLL: u32 = 0x00100000;
    pub const WS_SYSMENU: u32 = 0x00080000;
    pub const WS_THICKFRAME: u32 = 0x00040000;
    pub const WS_GROUP: u32 = 0x00020000;
    pub const WS_TABSTOP: u32 = 0x00010000;
    pub const WS_MODAL: u32 = 0x00000400;
    pub const WS_OVERLAPPEDWINDOW: u32 =
        WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_THICKFRAME
        | WS_MINIMIZE | WS_MAXIMIZE;
}

pub mod wf {
    pub const VISIBLE: u32 = 0x00000001;
    pub const ENABLED: u32 = 0x00000002;
    pub const FOCUSED: u32 = 0x00000004;
    pub const CAPTURED: u32 = 0x00000008;
    pub const DIRTY: u32 = 0x00000010;
    pub const TOPMOST: u32 = 0x00000020;
    pub const TOOL: u32 = 0x00000040;
    pub const POPUP: u32 = 0x00000080;
    pub const MODAL: u32 = 0x00000100;
    pub const TRANSIENT: u32 = 0x00000200;
    pub const SIZEMOVE: u32 = 0x00000400;
    pub const DESTROYED: u32 = 0x80000000;
}

pub mod cs {
    pub const VREDRAW: u32 = 0x0001;
    pub const HREDRAW: u32 = 0x0002;
    pub const DBLCLKS: u32 = 0x0008;
    pub const OWNDC: u32 = 0x0020;
    pub const CLASSDC: u32 = 0x0040;
    pub const NOCLOSE: u32 = 0x0200;
    pub const SAVEBITS: u32 = 0x0800;
    pub const GLOBALCLASS: u32 = 0x4000;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BmoWindowFlags(pub u32);

impl BmoWindowFlags {
    pub const EMPTY: Self = Self(0);
    pub fn contains(self, mask: u32) -> bool { (self.0 & mask) == mask }
    pub fn set(&mut self, mask: u32) { self.0 |= mask; }
    pub fn clear(&mut self, mask: u32) { self.0 &= !mask; }
    pub fn replace(&mut self, mask: u32, value: bool) {
        if value { self.set(mask); } else { self.clear(mask); }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct BmoWindow {
    pub used: bool,
    pub id: u32,
    pub class_id: u16,
    pub generation: u16,
    pub owner_tid: u16,
    pub owner_pid: u16,
    pub flags: BmoWindowFlags,
    pub style: u32,
    pub style_ex: u32,
    pub x: i32, pub y: i32, pub w: i32, pub h: i32,
    pub cx: i32, pub cy: i32, pub cw: i32, pub ch: i32, // client rect
    pub parent: u32,
    pub owner: u32,
    pub first_child: u32,
    pub next_sibling: u32,
    pub prev_sibling: u32,
    pub z_order: i32,   // mayor = encima
    pub prev_z: u32,    // singly-linked Z (top → next)
    pub surface: u32,   // surface id
    pub dc: u32,        // DC id (válido durante paint)
    pub user_data: u64,
    pub last_paint_tick: u64,
    pub title: [u8; 64],
    pub title_len: u8,
    pub dirty: bool,
    pub visible: bool,
    pub enabled: bool,
    pub focus: bool,
    pub captured: bool,
    pub erase_pending: bool,
    pub in_sizemove: bool,
}

impl BmoWindow {
    pub const fn empty() -> Self {
        Self {
            used: false,
            id: 0, class_id: 0, generation: 0,
            owner_tid: 0, owner_pid: 0,
            flags: BmoWindowFlags::EMPTY,
            style: 0, style_ex: 0,
            x: 0, y: 0, w: 0, h: 0,
            cx: 0, cy: 0, cw: 0, ch: 0,
            parent: WID_INVALID, owner: WID_INVALID,
            first_child: WID_INVALID, next_sibling: WID_INVALID, prev_sibling: WID_INVALID,
            z_order: 0, prev_z: WID_INVALID,
            surface: 0, dc: 0,
            user_data: 0, last_paint_tick: 0,
            title: [0; 64], title_len: 0,
            dirty: true, visible: false, enabled: true, focus: false,
            captured: false, erase_pending: true, in_sizemove: false,
        }
    }
}

/// Descriptor de clase — incluye RIP del wnd_proc en Ring 3.
#[derive(Debug, Clone, Copy)]
pub struct BmoClass {
    pub used: bool,
    pub id: u16,
    pub magic: u32,             // BMO_WND_PROC_MAGIC cuando está "live"
    pub wnd_proc: u64,          // Ring 3 RIP del wnd_proc (0 = default kernel-side)
    pub style: u32,
    pub style_ex: u32,
    pub extra_bytes: u16,
    pub owner_pid: u16,
    pub hbr_background: u8,
    pub name: [u8; 32],
    pub name_len: u8,
}

impl BmoClass {
    pub const fn empty() -> Self {
        Self {
            used: false, id: 0, magic: 0, wnd_proc: 0,
            style: 0, style_ex: 0, extra_bytes: 0,
            owner_pid: 0, hbr_background: 0,
            name: [0; 32], name_len: 0,
        }
    }
}

pub type BmoClassRef = u16;

pub struct WindowTable {
    pub windows: [BmoWindow; MAX_WINDOWS],
    pub classes: [BmoClass; MAX_CLASSES],
    /// Singly-linked list top→bottom: head es la ventana más alta.
    pub z_head: u32,
    pub count: u32,
    pub focus: u32,
    pub capture: u32,
    pub active: u32,
    pub desktop: u32,
    pub modal: u32,
    pub next_id: u32,
    pub next_class_id: u16,
}

impl WindowTable {
    pub const fn new() -> Self {
        const WIN: BmoWindow = BmoWindow::empty();
        const CLS: BmoClass = BmoClass::empty();
        Self {
            windows: [WIN; MAX_WINDOWS],
            classes: [CLS; MAX_CLASSES],
            z_head: WID_INVALID,
            count: 0,
            focus: WID_INVALID, capture: WID_INVALID, active: WID_INVALID,
            desktop: WID_INVALID, modal: WID_INVALID,
            next_id: 1, next_class_id: 1,
        }
    }

    pub fn init(&mut self) {
        for w in self.windows.iter_mut() { *w = BmoWindow::empty(); }
        for c in self.classes.iter_mut() { *c = BmoClass::empty(); }
        self.z_head = WID_INVALID;
        self.count = 0;
        self.focus = WID_INVALID;
        self.capture = WID_INVALID;
        self.active = WID_INVALID;
        self.desktop = WID_INVALID;
        self.modal = WID_INVALID;
        self.next_id = 1;
        self.next_class_id = 1;
    }

    /// Reserva un slot de clase. Devuelve el class_id.
    pub fn alloc_class(&mut self) -> Option<u16> {
        for (i, c) in self.classes.iter_mut().enumerate() {
            if !c.used {
                c.used = true;
                c.id = self.next_class_id;
                c.magic = 0xB17D; // BMO_WND_PROC_MAGIC
                self.next_class_id = self.next_class_id.wrapping_add(1);
                return Some(i as u16);
            }
        }
        None
    }

    pub fn class(&self, id: u16) -> Option<&BmoClass> {
        self.classes.iter().find(|c| c.used && c.id == id)
    }
    pub fn class_mut(&mut self, id: u16) -> Option<&mut BmoClass> {
        self.classes.iter_mut().find(|c| c.used && c.id == id)
    }

    /// Reserva un slot de ventana. Devuelve el slot index.
    pub fn alloc_window(&mut self) -> Option<u32> {
        for (i, w) in self.windows.iter_mut().enumerate() {
            if !w.used {
                w.used = true;
                w.id = self.next_id;
                w.generation = w.generation.wrapping_add(1);
                w.parent = WID_INVALID;
                w.owner = WID_INVALID;
                w.first_child = WID_INVALID;
                w.next_sibling = WID_INVALID;
                w.prev_sibling = WID_INVALID;
                w.prev_z = WID_INVALID;
                w.flags = BmoWindowFlags::EMPTY;
                w.dirty = true;
                w.visible = false;
                w.enabled = true;
                w.erase_pending = true;
                w.in_sizemove = false;
                self.next_id = self.next_id.wrapping_add(1);
                self.count += 1;
                return Some(i as u32);
            }
        }
        None
    }

    pub fn free_window(&mut self, slot: u32) -> bool {
        if let Some(w) = self.windows.get_mut(slot as usize) {
            if !w.used { return false; }
            w.used = false;
            w.generation = w.generation.wrapping_add(1);
            w.visible = false;
            w.flags.0 = 0;
            self.count = self.count.saturating_sub(1);
            true
        } else { false }
    }

    pub fn window(&self, slot: u32) -> Option<&BmoWindow> {
        self.windows.get(slot as usize).and_then(|w| if w.used { Some(w) } else { None })
    }
    pub fn window_mut(&mut self, slot: u32) -> Option<&mut BmoWindow> {
        self.windows.get_mut(slot as usize).and_then(|w| if w.used { Some(w) } else { None })
    }

    /// Inserta la ventana en el tope de la Z-list. No hace coalesce
    /// de duplicados — el caller debe quitar primero si la ventana ya
    /// está en la lista.
    pub fn z_push_top(&mut self, slot: u32) {
        // Copiamos z_head primero para evitar el borrow conflictivo.
        let head = self.z_head;
        if let Some(w) = self.window_mut(slot) {
            w.prev_z = head;
        }
        self.z_head = slot;
    }

    /// Quita la ventana de la Z-list (no la libera).
    pub fn z_remove(&mut self, slot: u32) {
        let mut prev = WID_INVALID;
        let mut cur = self.z_head;
        while cur != WID_INVALID {
            if cur == slot {
                let next = self.windows[cur as usize].prev_z;
                if prev == WID_INVALID {
                    self.z_head = next;
                } else {
                    self.windows[prev as usize].prev_z = next;
                }
                self.windows[cur as usize].prev_z = WID_INVALID;
                return;
            }
            prev = cur;
            cur = self.windows[cur as usize].prev_z;
        }
    }

    /// Itera la Z-list de top a bottom.
    pub fn z_foreach_top_down<F: FnMut(u32)>(&self, mut f: F) {
        let mut cur = self.z_head;
        while cur != WID_INVALID {
            f(cur);
            cur = self.windows[cur as usize].prev_z;
        }
    }
}

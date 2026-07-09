//! BMO/BMO v1.8.8
//!
//! Desarrolado por Salazar.
//!
//! BMO API v2.0 â€” windowing API para BMO/BMO.
//!
//! Facade: un solo `BmoState` global contiene todas las tablas del
//! subsistema (handles, ventanas, clases, colas, superficies, timers).
//! Se inicializa con `init()` desde `boot::phase5` y se mantiene vivo
//! durante toda la sesiÃ³n.
//!
//! Syscall ABI: 0x100..0x1FF (256 nÃºmeros), convenciÃ³n System V AMD64.
//! Ver `docs/BMO_API_V2_SPEC.md` para el spec completo.
//!
//! MÃ³dulos:
//!   handle         : Handle table con generation counter
//!   window         : Windows + classes + Z-order + parent/child tree
//!   message        : bmo_msg + BMO_MSG_* enum
//!   queue          : SPSC ring per-thread (64 mensajes) con AtomicU8 lock
//!   event          : MouseMove dedup, paint-region coalesce
//!   surface        : Offscreen surfaces con pixel pool allocation
//!   draw           : DC + primitives con AtomicU8 lock
//!   class          : Class table + default wnd_proc
//!   wm             : Z-order, focus, drag/resize, snap, modal
//!   timer          : Timer wheel (1 ms)
//!   input          : PS/2 + USB HID â†’ events con AtomicBool state
//!   cursor         : 16 builtin cursor sprites con AtomicBool state
//!   paint_compositor : Dirty-region tracking + blit
//!   syscall        : Dispatcher 0x100..0x1FF

#![allow(dead_code)]

pub mod handle;
pub mod window;
pub mod class;
pub mod message;
pub mod queue;
pub mod event;
pub mod surface;
pub mod draw;
pub mod wm;
pub mod timer;
pub mod input;
pub mod cursor;
pub mod paint_compositor;
pub mod syscall;
pub mod taskbar;

// â”€â”€ Estado global â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€â”€
use handle::HandleTable;
use window::WindowTable;
use surface::SurfaceTable;
use timer::TimerWheel;
use core::sync::atomic::{AtomicU8, Ordering};

/// Estado global del subsistema BMO API v2.
///
/// Vive en una static mutable conocida por el kernel. Todos los accesos
/// estÃ¡n protegidos por `wm_lock` (spinlock atÃ³mico) en operaciones
/// que tocan mÃ¡s de una tabla.
pub struct BmoState {
    pub handles: HandleTable,
    pub windows: WindowTable,
    pub surfaces: SurfaceTable,
    pub timers: TimerWheel,
    wm_lock: AtomicU8,
    pub initialized: bool,
}

impl BmoState {
    pub const fn new() -> Self {
        Self {
            handles: HandleTable::new(),
            windows: WindowTable::new(),
            surfaces: SurfaceTable::new(),
            timers: TimerWheel::new(),
            wm_lock: AtomicU8::new(0),
            initialized: false,
        }
    }

    pub fn lock(&self) {
        loop {
            match self.wm_lock.compare_exchange(0, 1, Ordering::Acquire, Ordering::Relaxed) {
                Ok(_) => return,
                Err(_) => core::hint::spin_loop(),
            }
        }
    }

    pub fn unlock(&self) {
        self.wm_lock.store(0, Ordering::Release);
    }
}

static mut BMO_STATE: BmoState = BmoState::new();

/// Get mutable access to the global BMO state.
///
/// # SAFETY (UB justification)
///
/// Returns `&'static mut` to a `static mut`. In general Rust, creating
/// multiple `&mut` references to the same data is UB. BMO is single-threaded
/// and `state()` is only called from `init()` (boot, once) and `tick()`
/// (main loop). These never overlap.
///
/// TODO(v2.0): Replace with `UnsafeCell<BmoState>` and return `*mut BmoState`.
pub fn state() -> &'static mut BmoState {
    unsafe { &mut BMO_STATE }
}

pub fn init() {
    let s = state();
    if s.initialized { return; }
    s.handles.init();
    s.windows.init();
    s.surfaces.init();
    s.timers.init();
    class::register_builtin_classes();
    wm::create_desktop_window();
    cursor::init();
    s.initialized = true;
    crate::cabina::info("bmo_api_v2", "BMO API v2.0 initialized â€” 16 modules, 256 syscalls");
}

pub fn tick() {
    let s = state();
    if !s.initialized { return; }
    timer::tick_global();
    paint_compositor::tick();
}

pub fn dispatch_syscall(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    syscall::dispatch(nr, a0, a1, a2, a3, a4, a5)
}



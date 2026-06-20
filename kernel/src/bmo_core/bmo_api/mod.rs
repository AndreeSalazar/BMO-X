//! BMO API v2.0 — windowing API para FastOS/BMO.
//!
//! Facade: un solo `BmoState` global contiene todas las tablas del
//! subsistema (handles, ventanas, clases, colas, superficies, timers).
//! Se inicializa con `init()` desde `boot::phase5` y se mantiene vivo
//! durante toda la sesión.
//!
//! Syscall ABI: 0x100..0x1FF (256 números), convención System V AMD64.
//! Ver `docs/BMO_API_V2_SPEC.md` para el spec completo.
//!
//! Módulos:
//!   handle         : Handle table con generation counter
//!   window         : Windows + classes + Z-order + parent/child tree
//!   message        : bmo_msg + BMO_MSG_* enum
//!   queue          : SPSC ring per-thread (64 mensajes)
//!   event          : MouseMove dedup, paint-region coalesce
//!   surface        : Offscreen surfaces (1:1 ventana↔surface)
//!   draw           : DC + primitives
//!   class          : Class table + default wnd_proc
//!   wm             : Z-order, focus, drag/resize, snap, modal
//!   timer          : Timer wheel (1 ms)
//!   input          : PS/2 + USB HID → events
//!   cursor         : 16 builtin cursor sprites
//!   paint_compositor : Dirty-region tracking + blit
//!   syscall        : Dispatcher 0x100..0x1FF
//!
//! Re-exports de alto nivel para que el resto del kernel no tenga que
//! importar submódulos.

#![allow(dead_code)]
#![allow(static_mut_refs)]

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

// ── Estado global ──────────────────────────────────────────────────
use handle::HandleTable;
use window::WindowTable;
use surface::SurfaceTable;
use timer::TimerWheel;

/// Estado global del subsistema BMO API v2.
///
/// Vive en una static mutable conocida por el kernel. Todos los accesos
/// están protegidos por `wm_lock` (un spinlock simple) en operaciones
/// que tocan más de una tabla.
pub struct BmoState {
    pub handles: HandleTable,
    pub windows: WindowTable,
    pub surfaces: SurfaceTable,
    pub timers: TimerWheel,
    pub wm_lock: u8, // 0 = unlocked, 1 = locked (simple spinlock)
    pub initialized: bool,
}

impl BmoState {
    pub const fn new() -> Self {
        Self {
            handles: HandleTable::new(),
            windows: WindowTable::new(),
            surfaces: SurfaceTable::new(),
            timers: TimerWheel::new(),
            wm_lock: 0,
            initialized: false,
        }
    }

    /// Spinlock acquire. v2.0 es single-CPU en la mayoría del código,
    /// pero SMP boot ya tiene APs levantados — usamos el patrón
    /// `lock: test-and-set + pause`.
    pub fn lock(&mut self) {
        // Minimal CAS spinlock. En ARM sería LDREX/STREX; en x86-64
        // un simple `lock bts` es suficiente.
        loop {
            let prev = self.wm_lock;
            if prev == 0 {
                self.wm_lock = 1;
                return;
            }
            core::hint::spin_loop();
        }
    }

    pub fn unlock(&mut self) {
        self.wm_lock = 0;
    }
}

#[inline]
pub fn state() -> &'static mut BmoState {
    unsafe { &mut BMO_STATE }
}

static mut BMO_STATE: BmoState = BmoState::new();

/// Inicializa el subsistema. Llamar desde `boot::phase5` después del
/// scheduler y antes de entrar al desktop.
pub fn init() {
    let s = state();
    if s.initialized { return; }
    s.handles.init();
    s.windows.init();
    s.surfaces.init();
    s.timers.init();
    // Registra clases built-in (BmoClass, BmoButton, BmoStatic, BmoEdit).
    class::register_builtin_classes();
    // Crea la ventana de escritorio (cubre todo el framebuffer).
    wm::create_desktop_window();
    // Cursor por defecto.
    cursor::init();
    s.initialized = true;
    crate::bmo_core::diag::info("bmo_api_v2", "BMO API v2.0 initialized — 16 modules, 256 syscalls");
}

/// Tick periódico llamado desde el scheduler. Procesa timers y el
/// compositor de pintado. ~10 ms de granularidad es suficiente.
pub fn tick() {
    let s = state();
    if !s.initialized { return; }
    s.timers.tick();
    paint_compositor::tick();
}

/// Llamado por `crate::arch::syscall_entry` cuando el nr está en 0x100..0x1FF.
/// Devuelve el valor a poner en RAX.
pub fn dispatch_syscall(nr: u16, a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> u64 {
    syscall::dispatch(nr, a0, a1, a2, a3, a4, a5)
}

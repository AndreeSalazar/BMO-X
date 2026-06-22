//! `bmo_core::diag` — Shim de compatibilidad.
//!
//! v1.8.8: el sistema de diagnóstico se movió a `cabina` (módulo hermano
//! de nivel superior). Este shim re-exporta la API de cabina para que
//! el código Ring 0/BMO Core siga funcionando con `bmo_core::diag::*`.
//!
//! **En v1.9** se eliminará este shim y se migrarán todos los call sites
//! a `crate::cabina::*` directamente.

#![allow(dead_code)]


// ── Re-exports directos de cabina ─────────────────────────────────

pub use crate::cabina::{
    Severity, Event, Layer, Entity,
    info, warn, fault, trace, panic_msg, assert,
    info_u64, warn_u64, fault_u64, trace_u64,
    event as event_fn, event_u64,
    emit, emit_layer, emit_full,
    is_ready, boot_ready, mark_boot_ready,
    init, tick, tick_refresh,
    toggle_overlay, set_overlay_enabled, is_overlay_enabled, overlay_enabled,
    paint_overlay, current_tab, cycle_tab, cycle_query,
    read_cr3_into_serial,
    persistent_target_path, persistent_pending_bytes, persistent_dropped_bytes,
    copy_persistent_pending, ack_persistent_bytes,
};

// ── Módulo telemetry (API legacy con struct plana) ───────────────

/// Sub-structs de telemetría legacy (para `diag::telemetry::t().cpu.X`).
pub mod telemetry {
    use core::sync::atomic::{AtomicU64, Ordering};

    /// Telemetry handle (API legacy de diag). Devuelve refs a contadores.
    pub struct Telemetry {
        pub cpu: CpuCounters,
        pub mem: MemCounters,
        pub sched: SchedCounters,
    }

    /// Counters de CPU (legacy).
    pub struct CpuCounters {
        pub interrupts: AtomicU64,
        pub timer_ticks: AtomicU64,
        pub page_faults: AtomicU64,
        pub gp_faults: AtomicU64,
        pub nm_faults: AtomicU64,
        pub df_faults: AtomicU64,
        pub ud_faults: AtomicU64,
        pub mc_faults: AtomicU64,
        pub other_faults: AtomicU64,
    }

    impl CpuCounters {
        /// Incrementa interrupts.
        pub fn inc_interrupts(&self) { self.interrupts.fetch_add(1, Ordering::Relaxed); }
        /// Incrementa page_faults.
        pub fn inc_page_faults(&self) { self.page_faults.fetch_add(1, Ordering::Relaxed); }
    }

    /// Counters de memoria (legacy).
    pub struct MemCounters {
        pub allocs: AtomicU64,
        pub frees: AtomicU64,
        pub heap_used: AtomicU64,
    }

    /// Counters de scheduler (legacy).
    pub struct SchedCounters {
        pub ctx_switches: AtomicU64,
    }

    impl SchedCounters {
        /// Registra un context switch.
        pub fn record_context_switch(&self) {
            self.ctx_switches.fetch_add(1, Ordering::Relaxed);
        }
    }

    static mut T: Option<Telemetry> = None;
    static mut INIT: bool = false;

    /// Devuelve el handle de telemetría legacy. Inicializa en primer uso.
    pub fn t() -> &'static Telemetry {
        unsafe {
            if !INIT {
                T = Some(Telemetry {
                    cpu: CpuCounters {
                        interrupts: AtomicU64::new(0),
                        timer_ticks: AtomicU64::new(0),
                        page_faults: AtomicU64::new(0),
                        gp_faults: AtomicU64::new(0),
                        nm_faults: AtomicU64::new(0),
                        df_faults: AtomicU64::new(0),
                        ud_faults: AtomicU64::new(0),
                        mc_faults: AtomicU64::new(0),
                        other_faults: AtomicU64::new(0),
                    },
                    mem: MemCounters {
                        allocs: AtomicU64::new(0),
                        frees: AtomicU64::new(0),
                        heap_used: AtomicU64::new(0),
                    },
                    sched: SchedCounters {
                        ctx_switches: AtomicU64::new(0),
                    },
                });
                INIT = true;
            }
            T.as_ref().unwrap()
        }
    }
}

// ── Módulo overlay (legacy con set_target_override) ─────────────

pub mod overlay {
    use super::set_target_override_impl as _set_target;

    /// Cambia el framebuffer destino del overlay.
    /// v1.8.8: stub — la cabina siempre dibuja sobre el FB principal.
    pub fn set_target_override(target: Option<(*mut u32, usize, usize, usize)>) {
        let _ = target;
        // No-op en cabina. La pint siempre usa el FB GOP.
        let _ = _set_target(0);
    }
}

#[inline(always)]
fn set_target_override_impl(_x: usize) -> usize { 0 }

// ── Macros de compatibilidad ─────────────────────────────────────

#[macro_export]
macro_rules! diag_info {
    ($module:expr, $message:expr) => {
        $crate::cabina::info($module, $message)
    };
}

#[macro_export]
macro_rules! diag_warn {
    ($module:expr, $message:expr) => {
        $crate::cabina::warn($module, $message)
    };
}

#[macro_export]
macro_rules! diag_fault {
    ($module:expr, $message:expr) => {
        $crate::cabina::fault($module, $message)
    };
}

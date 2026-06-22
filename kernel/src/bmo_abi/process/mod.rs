//! `bmo_abi::process` — Procesos, threads y scheduling hints.
//!
//! Define los **datos** que las funciones `proc_*` y `thread_*`
//! (declaradas en `crate::bmo_abi::syscalls`) reciben o devuelven.
//!
//! ## Modelo
//!
//! - Cada proceso tiene un **PID** y un **parent PID**.
//! - Cada thread tiene un **TID** y un **TLS base**.
//! - Los IDs son **u32** con un slot reservado para el kernel (0).
//!
//! ## Syscalls
//!
//! - `NR_PROC_SPAWN` (0x180) → `bmo_proc_spawn(path, argv) -> BmoHandle`
//! - `NR_PROC_EXIT`  (0x181) → `bmo_proc_exit(code)`
//! - `NR_PROC_GET_PID` (0x182) → `bmo_proc_get_pid() -> u32`
//! - `NR_PROC_GET_TID` (0x183) → `bmo_proc_get_tid() -> u32`
//! - `NR_PROC_YIELD`  (0x184) → `bmo_proc_yield()`
//! - `NR_THREAD_CREATE` (0x185) → `bmo_thread_create(fn, arg) -> BmoHandle`
//! - `NR_THREAD_EXIT`  (0x186) → `bmo_thread_exit(code)`
//! - `NR_THREAD_JOIN`  (0x187) → `bmo_thread_join(t, code_out)`
//! - `NR_THREAD_SELF`  (0x188) → `bmo_thread_self() -> u32`

#![allow(dead_code)]

use crate::bmo_abi::fundamentals::handle::BmoHandle;

// ─── IDs ───────────────────────────────────────────────────────────

/// Process ID.
pub type BmoPid = u32;

/// Thread ID.
pub type BmoTid = u32;

/// Reserved IDs.
pub const PID_KERNEL: BmoPid = 0;
pub const PID_INVALID: BmoPid = u32::MAX;
pub const TID_INVALID: BmoTid = u32::MAX;

// ─── Process info ──────────────────────────────────────────────────

/// Estado de un proceso.
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BmoProcState {
    #[default]
    /// Creando, aún no corriendo.
    Creating = 0,
    /// Listo para correr.
    Ready    = 1,
    /// Corriendo.
    Running  = 2,
    /// Bloqueado en I/O, IPC, etc.
    Blocked  = 3,
    /// Terminado, esperando a que el padre lo recoja.
    Zombie   = 4,
    /// Terminado y recogido.
    Dead     = 5,
}

/// Información de un proceso. Tamaño: 64 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoProcessInfo {
    pub pid: BmoPid,
    pub ppid: BmoPid,
    pub state: BmoProcState,
    pub _pad0: u32,
    /// Exit code si está Zombie/Dead.
    pub exit_code: i32,
    /// Número de threads vivos.
    pub nthreads: u32,
    /// Handle al thread principal.
    pub main_thread: BmoHandle,
    /// Handle al port IPC de control.
    pub control_port: BmoHandle,
    /// Nombre del proceso (UTF-8, null-terminated, max 32 bytes).
    pub name: [u8; 32],
    pub _pad1: [u8; 8],
}

// ─── Thread info ───────────────────────────────────────────────────

/// Información de un thread. Tamaño: 48 bytes.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoThreadInfo {
    pub tid: BmoTid,
    pub pid: BmoPid,
    pub state: BmoProcState,
    /// Prioridad (0 = lowest, 255 = realtime).
    pub priority: u8,
    /// CPU affinity (máscara de cores).
    pub affinity: u8,
    pub _pad0: [u8; 2],
    /// Stack base pointer.
    pub stack_base: u64,
    /// Stack size.
    pub stack_size: u64,
    /// TLS base pointer.
    pub tls_base: u64,
}

/// Argumento para `bmo_thread_create`.
///
/// El thread empieza en `fn_ptr(arg: usize)` y se une con
/// `bmo_thread_join`.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct BmoThreadCreateInfo {
    /// Puntero a la función entry: `extern "sysv64" fn(arg: usize) -> i32`.
    pub fn_ptr: u64,
    /// Argumento pasado a la función.
    pub arg: u64,
    /// Stack size (0 = default, ~1 MB).
    pub stack_size: u64,
    /// Priority (0 = default).
    pub priority: u8,
    /// CPU affinity (0xFF = any).
    pub affinity: u8,
    pub _pad: [u8; 6],
}

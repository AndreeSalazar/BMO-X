//! Fixed-capacity scheduler with real context switching at trap boundaries.
//!
//! Design rule: **a context switch only ever happens at a trap boundary**
//! (timer IRQ or SYSCALL). Voluntary operations from kernel tasks just mark
//! state and park in a `hlt` loop; the next trap commits the switch through
//! the unified frame. SYSCALLs from Ring 3 are themselves trap frames, so
//! YIELD/WAIT/EXIT switch immediately and correctly from the dispatcher.
//!
//! A running context is captured into its task by the trap stub writing
//! `percpu.trap_rsp`; `schedule_locked` stores that into the outgoing task
//! and publishes the next task's `context_rsp` back to `percpu.trap_rsp`,
//! which the trap epilogue restores.


//! # ** LOS CARRILES (L6g)
//!
//! ```text
//!    roja.rs    lo que CAMBIA el estado: el cambio de contexto, `reap`,
//!               `spawn`, `exit`. Si falla, no hay a quien darle el CPU
//!    verde.rs   lo que solo MIRA, y los numeros del TSC
//! ```
//!
//! *** DOS carriles y no tres, a proposito. Aqui no hay un "va a cambiar y
//! arrastra" con masa propia -- hay lo que escribe la tabla y lo que la lee.
//! **Un modulo lleva los carriles que TIENE**: tres ficheros donde solo hay dos
//! lineas de verdad es la aguja mejor escondida.
//!
//! [!] Fuera no cambia nada: `pub use` deja el modulo con la misma cara.

mod roja;
mod verde;

pub use roja::{
    delante, exit_and_park, exit_current, init, kill_current_and_pick, on_timer, park_until,
    spawn_kernel, spawn_user, terminar, wait_current, wait_current_checked, wake_by_key,
    yield_current, Task,
};
pub use verde::{
    context_rsp_of, counts, cr3_de_pid, current_pid, current_state, current_tid, duenno_de_pila,
    hay_hueco, huecos_libres, ns_to_tsc, pid_de, quien_corre, rdtsc, rdtsc_serial, switch_snap,
    tid_de, tid_state, tsc_freq, user_switches, vive, TaskState, DEFAULT_QUANTUM_TICKS, MAX_TASKS,
    QUANTUM_DELANTE,
};


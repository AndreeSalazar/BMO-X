//! Single syscall dispatch point + named wrappers for Ring 3.
//!
//! `bmo_syscall` is the ONLY function that executes the `syscall` instruction,
//! exported as `extern "C"` for the C/COBOL frontends. Internally it delegates
//! to `bmo_abi::syscalls::syscall6`.
//!
//! Los envoltorios de abajo son operaciones sobre las DOS puertas (`INVOKE`,
//! `WAIT`). Hasta el 2026-09-19 habia treinta mas sobre la tabla v1
//! (0x100..0x1FF) -- `fs_read`, `wm_create_window`, `draw_rect`... -- que el
//! kernel no despacha: se fueron con la tabla.

use bmo_abi::syscalls::{self, syscall6, SyscallResult};

#[inline]
fn value_or_code(result: SyscallResult) -> u64 {
    if result.is_ok() { result.value() } else { result.code() as u64 }
}

/// The single syscall dispatch point, exported for C/COBOL.
///
/// ```c
/// u64 bmo_syscall(u32 nr, u64 a0, u64 a1, u64 a2, u64 a3, u64 a4, u64 a5);
/// ```
///
/// Returns the `value` (RDX) on success (RAX==0), or the `code` (RAX) on error.
/// This ensures callers get the actual pointer/handle, not the status code.
#[no_mangle]
pub unsafe extern "C" fn bmo_syscall(
    nr: u32,
    a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64,
) -> u64 {
    value_or_code(syscall6(nr, a0, a1, a2, a3, a4, a5))
}

// --- Process / Thread ------------------------------------------------

pub unsafe fn proc_exit(code: u32) -> u64 {
    value_or_code(syscalls::surface::invoke(
        syscalls::surface::CURRENT_TASK,
        syscalls::surface::task_op::EXIT,
        code as u64,
        0,
        0,
        0,
    ))
}

pub unsafe fn proc_get_pid() -> u64 {
    value_or_code(syscalls::surface::invoke(
        syscalls::surface::CURRENT_TASK,
        syscalls::surface::task_op::GET_PID,
        0,
        0,
        0,
        0,
    ))
}

pub unsafe fn proc_get_tid() -> u64 {
    value_or_code(syscalls::surface::invoke(
        syscalls::surface::CURRENT_TASK,
        syscalls::surface::task_op::GET_TID,
        0,
        0,
        0,
        0,
    ))
}

pub unsafe fn proc_yield() -> u64 {
    value_or_code(syscalls::surface::invoke(
        syscalls::surface::CURRENT_TASK,
        syscalls::surface::task_op::YIELD,
        0,
        0,
        0,
        0,
    ))
}

// --- BMO Channel (ABI v2: capability-addressed IPC) ------------------

/// Discover this process's estuary capability for channel `index`.
/// Returns the handle, or an error code (3 = NEEDS_CAP) as value.
pub unsafe fn channel_open(index: u64) -> u64 {
    value_or_code(syscalls::surface::invoke(
        syscalls::surface::CURRENT_TASK,
        syscalls::surface::task_op::CHANNEL_OPEN,
        index,
        0,
        0,
        0,
    ))
}

/// Completion-side sequence of the estuary behind `handle` -- the value
/// `channel_wait` compares against.
pub unsafe fn channel_seq(handle: u64) -> u64 {
    value_or_code(syscalls::surface::invoke(
        handle,
        syscalls::surface::channel_op::GET_SEQ,
        0,
        0,
        0,
        0,
    ))
}

/// Notify Ring 0 after publishing submissions. Returns requests serviced.
pub unsafe fn channel_kick(handle: u64, published_seq: u64) -> u64 {
    value_or_code(syscalls::surface::channel_kick(handle, published_seq))
}

/// Block until the estuary's completion sequence moves past
/// `observed_seq` or `timeout_ns` elapses (0 = no timeout).
pub unsafe fn channel_wait(handle: u64, observed_seq: u64, timeout_ns: u64) -> u64 {
    value_or_code(syscalls::surface::wait(handle, observed_seq, timeout_ns))
}

/// Pure timed sleep through the v2 WAIT surface.
pub unsafe fn sleep_ns(timeout_ns: u64) -> u64 {
    value_or_code(syscalls::surface::wait(0, 0, timeout_ns))
}

// --- Memoria ---------------------------------------------------------

/// **Pide un bloque a `KIND_MEMORIA`** y devuelve su direccion, o nulo.
///
/// Dos puertas, las mismas que emite `malloc` en BMO C: `MEMORIA_PEDIR` da el
/// handle del bloque y `MEM_OP_BASE` su direccion. No hay forma de devolverlo
/// --el kernel entrega bloques grandes y el asignador vive encima-- y cada
/// proceso tiene un tope de peticiones, asi que quien llama pide POCAS veces.
///
/// [!] Se mira el CODIGO de cada respuesta, no el valor: hasta el 2026-09-19
/// esto era `bmo_syscall(0x190, ...)`, que el kernel contesta con `rax = 10`,
/// y `value_or_code` devolvia ese 10 como si fuera una direccion.
pub unsafe fn memoria_pedir(bytes: u64) -> *mut u8 {
    use syscalls::surface::{invoke, CURRENT_TASK, MEM_OP_BASE, TASK_OP_MEMORIA_PEDIR};
    let bloque = invoke(CURRENT_TASK, TASK_OP_MEMORIA_PEDIR, bytes, 0, 0, 0);
    if !bloque.is_ok() {
        return core::ptr::null_mut();
    }
    let base = invoke(bloque.value(), MEM_OP_BASE, 0, 0, 0, 0);
    if !base.is_ok() {
        return core::ptr::null_mut();
    }
    base.value() as *mut u8
}

// --- Consola ---------------------------------------------------------

/// **Escribe `len` bytes en la consola del kernel**, de ocho en ocho.
///
/// `CONSOLE_WRITE` recibe el texto POR VALOR: ocho bytes empaquetados en
/// little-endian dentro del argumento, y un cero corta. Un trozo corto se
/// rellena con ceros, que es justo ese corte.
pub unsafe fn consola_escribir(msg: *const u8, len: u64) {
    use syscalls::surface::{invoke, CURRENT_TASK, TASK_OP_CONSOLE_WRITE};
    let mut hecho = 0u64;
    while hecho < len {
        let mut palabra = [0u8; 8];
        let trozo = core::cmp::min(8, len - hecho) as usize;
        core::ptr::copy_nonoverlapping(msg.add(hecho as usize), palabra.as_mut_ptr(), trozo);
        let _ = invoke(CURRENT_TASK, TASK_OP_CONSOLE_WRITE, u64::from_le_bytes(palabra), 0, 0, 0);
        hecho += trozo as u64;
    }
}

//! Single syscall dispatch point + named wrappers for Ring 3.
//!
//! `bmo_syscall` is the ONLY function that executes the `syscall` instruction,
//! exported as `extern "C"` for the C/COBOL frontends. Internally it delegates
//! to `bmo_abi::syscalls::syscall6`.
//!
//! All category wrappers (process, memory, fs, etc.) are thin Rust functions
//! that call `bmo_syscall` with the appropriate NR constant.

use bmo_abi::syscalls::{self, syscall6};

/// The single syscall dispatch point, exported for C/COBOL.
///
/// ```c
/// u64 bmo_syscall(u32 nr, u64 a0, u64 a1, u64 a2, u64 a3, u64 a4, u64 a5);
/// ```
#[no_mangle]
pub unsafe extern "C" fn bmo_syscall(
    nr: u32,
    a0: u64, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64,
) -> u64 {
    syscall6(nr, a0, a1, a2, a3, a4, a5).code() as u64
}

// ─── Process / Thread ────────────────────────────────────────────────

pub unsafe fn proc_exit(code: u32) -> u64 {
    bmo_syscall(syscalls::NR_PROC_EXIT, code as u64, 0, 0, 0, 0, 0)
}

pub unsafe fn proc_get_pid() -> u64 {
    bmo_syscall(syscalls::NR_PROC_GET_PID, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn proc_get_tid() -> u64 {
    bmo_syscall(syscalls::NR_PROC_GET_TID, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn proc_yield() -> u64 {
    bmo_syscall(syscalls::NR_PROC_YIELD, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn proc_spawn(path: *const u8, path_len: u64) -> u64 {
    bmo_syscall(syscalls::NR_PROC_SPAWN, path as u64, path_len, 0, 0, 0, 0)
}

// ─── Memory ──────────────────────────────────────────────────────────

pub unsafe fn mem_alloc(size: u64) -> *mut u8 {
    bmo_syscall(syscalls::NR_MEM_ALLOC, size, 0, 0, 0, 0, 0) as *mut u8
}

pub unsafe fn mem_free(ptr: *mut u8, size: u64) -> u64 {
    bmo_syscall(syscalls::NR_MEM_FREE, ptr as u64, size, 0, 0, 0, 0)
}

pub unsafe fn mem_map(phys: u64, size: u64) -> *mut u8 {
    bmo_syscall(syscalls::NR_MEM_MAP, phys, size, 0, 0, 0, 0) as *mut u8
}

pub unsafe fn mem_unmap(virt: *mut u8, size: u64) -> u64 {
    bmo_syscall(syscalls::NR_MEM_UNMAP, virt as u64, size, 0, 0, 0, 0)
}

// ─── Filesystem ──────────────────────────────────────────────────────

pub unsafe fn fs_open(path: *const u8, path_len: u64, flags: u64) -> u64 {
    bmo_syscall(syscalls::NR_FS_OPEN, path as u64, path_len, flags, 0, 0, 0)
}

pub unsafe fn fs_close(fd: u64) -> u64 {
    bmo_syscall(syscalls::NR_FS_CLOSE, fd, 0, 0, 0, 0, 0)
}

pub unsafe fn fs_read(fd: u64, buf: *mut u8, count: u64) -> u64 {
    bmo_syscall(syscalls::NR_FS_READ, fd, buf as u64, count, 0, 0, 0)
}

pub unsafe fn fs_write(fd: u64, buf: *const u8, count: u64) -> u64 {
    bmo_syscall(syscalls::NR_FS_WRITE, fd, buf as u64, count, 0, 0, 0)
}

pub unsafe fn fs_seek(fd: u64, offset: i64, whence: u64) -> u64 {
    bmo_syscall(syscalls::NR_FS_SEEK, fd, offset as u64, whence, 0, 0, 0)
}

// ─── Time ────────────────────────────────────────────────────────────

pub unsafe fn time_now_ns() -> u64 {
    bmo_syscall(syscalls::NR_TIME_NOW_NS, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn time_sleep_ms(ms: u64) -> u64 {
    bmo_syscall(syscalls::NR_TIME_SLEEP_MS, ms, 0, 0, 0, 0, 0)
}

// ─── Diagnostics ─────────────────────────────────────────────────────

pub unsafe fn debug_print(msg: *const u8, len: u64) -> u64 {
    bmo_syscall(syscalls::NR_DEBUG_PRINT, msg as u64, len, 0, 0, 0, 0)
}

pub unsafe fn debug_panic(msg: *const u8, len: u64) -> ! {
    bmo_syscall(syscalls::NR_DEBUG_PANIC, msg as u64, len, 0, 0, 0, 0);
    loop { core::arch::asm!("cli; hlt") }
}

// ─── Window Manager ──────────────────────────────────────────────────

pub unsafe fn wm_create_window(x: i64, y: i64, w: u64, h: u64) -> u64 {
    bmo_syscall(syscalls::NR_WM_CREATE_WINDOW, x as u64, y as u64, w, h, 0, 0)
}

pub unsafe fn wm_destroy_window(win: u64) -> u64 {
    bmo_syscall(syscalls::NR_WM_DESTROY_WINDOW, win, 0, 0, 0, 0, 0)
}

pub unsafe fn wm_set_title(win: u64, title: *const u8, len: u64) -> u64 {
    bmo_syscall(syscalls::NR_WM_SET_TITLE, win, title as u64, len, 0, 0, 0)
}

// ─── Drawing ─────────────────────────────────────────────────────────

pub unsafe fn draw_rect(x: i64, y: i64, w: u64, h: u64, color: u32) -> u64 {
    bmo_syscall(syscalls::NR_DRAW_RECT, x as u64, y as u64, w, h, color as u64, 0)
}

pub unsafe fn draw_text(x: i64, y: i64, text: *const u8, len: u64, color: u32) -> u64 {
    bmo_syscall(syscalls::NR_DRAW_TEXT, x as u64, y as u64, text as u64, len, color as u64, 0)
}

pub unsafe fn draw_pixel(x: i64, y: i64, color: u32) -> u64 {
    bmo_syscall(syscalls::NR_DRAW_PIXEL, x as u64, y as u64, color as u64, 0, 0, 0)
}

// ─── Input ───────────────────────────────────────────────────────────

pub unsafe fn poll_key() -> u64 {
    bmo_syscall(syscalls::NR_POLL_KEY, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn poll_event() -> u64 {
    bmo_syscall(syscalls::NR_POLL_EVENT, 0, 0, 0, 0, 0, 0)
}

// ─── IPC ─────────────────────────────────────────────────────────────

pub unsafe fn ipc_port_create() -> u64 {
    bmo_syscall(syscalls::NR_IPC_PORT_CREATE, 0, 0, 0, 0, 0, 0)
}

pub unsafe fn ipc_port_send(port: u64, data: *const u8, len: u64) -> u64 {
    bmo_syscall(syscalls::NR_IPC_PORT_SEND, port, data as u64, len, 0, 0, 0)
}

pub unsafe fn ipc_port_recv(port: u64, buf: *mut u8, max: u64) -> u64 {
    bmo_syscall(syscalls::NR_IPC_PORT_RECV, port, buf as u64, max, 0, 0, 0)
}

pub unsafe fn ipc_port_close(port: u64) -> u64 {
    bmo_syscall(syscalls::NR_IPC_PORT_CLOSE, port, 0, 0, 0, 0, 0)
}

// ─── Compositor ──────────────────────────────────────────────────────

pub unsafe fn compositor_present(win: u64) -> u64 {
    bmo_syscall(syscalls::NR_COMPOSITOR_PRESENT, win, 0, 0, 0, 0, 0)
}

pub unsafe fn compositor_begin_frame(win: u64) -> u64 {
    bmo_syscall(syscalls::NR_COMPOSITOR_BEGIN_FRAME, win, 0, 0, 0, 0, 0)
}

pub unsafe fn compositor_end_frame(win: u64) -> u64 {
    bmo_syscall(syscalls::NR_COMPOSITOR_END_FRAME, win, 0, 0, 0, 0, 0)
}

// ─── Audio ───────────────────────────────────────────────────────────

pub unsafe fn audio_beep(freq: u32, dur_ms: u32) -> u64 {
    bmo_syscall(syscalls::NR_AUDIO_BEEP, freq as u64, dur_ms as u64, 0, 0, 0, 0)
}

// ─── Surface ─────────────────────────────────────────────────────────

pub unsafe fn surface_map(phys: u64, w: u64, h: u64) -> *mut u8 {
    bmo_syscall(syscalls::NR_SURFACE_MAP, phys, w, h, 0, 0, 0) as *mut u8
}

pub unsafe fn surface_unmap(ptr: *mut u8) -> u64 {
    bmo_syscall(syscalls::NR_SURFACE_UNMAP, ptr as u64, 0, 0, 0, 0, 0)
}

pub unsafe fn surface_present(surf: u64, win: u64) -> u64 {
    bmo_syscall(syscalls::NR_SURFACE_PRESENT, surf, win, 0, 0, 0, 0)
}

// ─── BEFCore ─────────────────────────────────────────────────────────

pub unsafe fn befcore_send(target: u64, msg: *const u8, len: u64) -> u64 {
    bmo_syscall(syscalls::NR_BEFCORE_SEND, target, msg as u64, len, 0, 0, 0)
}

pub unsafe fn befcore_recv(buf: *mut u8, max: u64) -> u64 {
    bmo_syscall(syscalls::NR_BEFCORE_RECV, buf as u64, max, 0, 0, 0, 0)
}

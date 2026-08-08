//! `bmo_abi::syscalls` -- Tabla unica de syscall numbers 0x100..0x1FF.
//!
//! **Constants y `name()` generados automaticamente** desde
//! `Semantic_ASM/bmo/*.toml` por `build.rs`.
//!
//! Este archivo conserva los wrappers arquitectura-especificos
//! (`syscall0..6`, `SyscallResult`) y los helpers.
//!
//! ## Layout
//!
//! | Rango | Owner | Notas |
//! |-------|-------|-------|
//! | 0x100..0x10F | WM | Window create/destroy/show/hide/title/bounds/clip |
//! | 0x110..0x119 | Draw | Clear/pixel/line/rect/circle/text/blit/gradient/round |
//! | 0x120..0x125 | WinPaint | fill_rect/draw_text/draw_pixel/line/blit/circle |
//! | 0x130..0x134 | Compositor | begin_frame/end_frame/present/set_target/flush |
//! | 0x140..0x149 | FS | open/close/read/write/seek/stat/mkdir/readdir/delete/mount |
//! | 0x150..0x153 | Time | now_ns/now_us/sleep_ns/sleep_ms |
//! | 0x160..0x162 | Input | poll_key/poll_mouse/poll_event |
//! | 0x170..0x173 | Audio | play/stop/beep/load_wave |
//! | 0x180..0x188 | Process | spawn/exit/get_pid/get_tid/yield/thread_create/thread_exit/thread_join/thread_self |
//! | 0x190..0x197 | Memory | alloc/free/map/unmap + BEFCore send/recv/poll |
//! | 0x1A0..0x1A3 | IPC | port_create/port_send/port_recv/port_close |
//! | 0x1C0..0x1C2 | Surface | map/unmap/present |
//! | 0x1F0..0x1F3 | Diagnostics | print/trace/assert/panic |

// ===========================================================================
//  Constants + name() -- EMBEDIDOS desde asm::defs
// ===========================================================================
mod generated;
pub use generated::*;

/// Minimal BMO ABI v2 kernel surface. The generated table above is retained
/// only while v1 producers and runtimes are migrated.
pub mod surface;

// ===========================================================================
//  Syscall wrappers (x86_64). Arquitectura-especificos.
//  Cuando se anada ARM: arch/aarch64.rs, arch/riscv64.rs, etc.
// ===========================================================================

/// Resultado de un syscall: (code, value) = (RAX, RDX).
///
/// El kernel siempre devuelve `BmoStatus` en RAX:RDX, donde:
/// - RAX bits [31:0] = codigo de estado (0 = OK)
/// - RDX = valor adicional (handle, contador, etc.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyscallResult(pub u64, pub u64);

impl SyscallResult {
    pub fn code(&self) -> u32 {
        self.0 as u32
    }
    pub fn flags(&self) -> u32 {
        (self.0 >> 32) as u32
    }
    pub fn value(&self) -> u64 {
        self.1
    }
    pub fn is_ok(&self) -> bool {
        self.code() == 0
    }
    pub fn status(&self) -> crate::fundamentals::status::BmoStatus {
        crate::fundamentals::status::BmoStatus::from_registers(self.0, self.1)
    }
}

/// Syscall con 0 argumentos.
#[inline(always)]
pub unsafe fn syscall0(nr: u32) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 1 argumento.
#[inline(always)]
pub unsafe fn syscall1(nr: u32, a1: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 2 argumentos.
#[inline(always)]
pub unsafe fn syscall2(nr: u32, a1: u64, a2: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        in("rsi") a2,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 3 argumentos.
#[inline(always)]
pub unsafe fn syscall3(nr: u32, a1: u64, a2: u64, a3: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 4 argumentos.
#[inline(always)]
pub unsafe fn syscall4(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 5 argumentos.
#[inline(always)]
pub unsafe fn syscall5(nr: u32, a1: u64, a2: u64, a3: u64, a4: u64, a5: u64) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

/// Syscall con 6 argumentos.
#[inline(always)]
pub unsafe fn syscall6(
    nr: u32,
    a1: u64,
    a2: u64,
    a3: u64,
    a4: u64,
    a5: u64,
    a6: u64,
) -> SyscallResult {
    let code: u64;
    let value: u64;
    core::arch::asm!(
        "syscall",
        in("rax") nr as u64,
        in("rdi") a1,
        in("rsi") a2,
        in("rdx") a3,
        in("r10") a4,
        in("r8") a5,
        in("r9") a6,
        lateout("rax") code,
        lateout("rdx") value,
        lateout("rcx") _,
        lateout("r11") _,
        options(nostack, preserves_flags),
    );
    SyscallResult(code, value)
}

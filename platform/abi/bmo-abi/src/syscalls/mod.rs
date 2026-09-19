//! `bmo_abi::syscalls` -- LAS DOS PUERTAS de BMO-X y la instruccion que las cruza.
//!
//! ```text
//!    rax = 0x00   INVOKE(capability, operacion, a0, a1, a2, a3)
//!    rax = 0x02   WAIT(esperable, secuencia_vista, plazo_ns)
//!    rax = 0x01   RESERVADO -- contesta ERROR_UNSUPPORTED, no se reutiliza
//! ```
//!
//! Todo lo demas es una OPERACION sobre una capability (`surface::`), no un
//! numero de syscall. Argumentos en `rdi, rsi, rdx, r10, r8, r9` (`rcx` y `r11`
//! los destruye la propia instruccion) y el resultado en `rax:rdx` =
//! `BmoStatus` (codigo | banderas << 32, valor).
//!
//! # [!] 2026-09-19: la tabla v1 (0x100..0x1FF) ya no existe
//!
//! Eran 109 nombres (`bmo_mem_alloc` 0x190, `bmo_exit` 0x181...) que el kernel
//! no despachaba desde que se congelaron las dos puertas: todos contestaban
//! `rax = 10`. Y ese 10 **no es cero** -- el `malloc` de `stdlib/heap` lo
//! tomaba por una direccion y escribia en `0xA`. La tabla alimentaba a los
//! frontends de C y COBOL y a `bmo-rt`; se fue con todos sus usuarios en el
//! mismo commit, y un nombre v1 es ahora un error de compilacion.

/// Las dos puertas y las operaciones de cada capability.
pub mod surface;

// ===========================================================================
//  Syscall wrappers (x86_64). Este repositorio es SOLO x86-64 (2026-09-18,
//  `toolchain/tools/isa`): otra CPU es otro repositorio, no un `arch/` aqui.
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

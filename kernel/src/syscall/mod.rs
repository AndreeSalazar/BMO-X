//! Tabla de syscalls de FastOS — puente Ring 3 → Ring 0.
//!
//! Spec: `FastOS_Syscall_Table_Spec.md`. ABI: `syscall`/`sysret`,
//! número de syscall en RAX, args en RDI, RSI, RDX, R10, R8, R9.
//! Resultado en RAX (negativo = errno; ≥0 = OK).

#![allow(dead_code)]

#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Syscall {
    // — Procesos / threads —
    ProcessExit         = 0x00,
    ThreadCreate        = 0x01,
    ThreadExit          = 0x02,
    ThreadYield         = 0x03,
    FutexWait           = 0x04,
    FutexWake           = 0x05,

    // — Memoria —
    Mmap                = 0x10,
    Munmap              = 0x11,
    Mprotect            = 0x12,

    // — VFS —
    FileOpen            = 0x20,
    FileRead            = 0x21,
    FileWrite           = 0x22,
    FileClose           = 0x23,
    FileSeek            = 0x24,
    FileStat            = 0x25,

    // — IPC —
    PortCreate          = 0x30,
    PortSend            = 0x31,
    PortRecv            = 0x32,

    // — BareX bridges (paso a Ring 3 service) —
    BarexGfxSubmit      = 0x40,
    BarexAudioSubmit    = 0x41,
    BarexInputPoll      = 0x42,
    BarexNetSubmit      = 0x43,

    // — Time —
    ClockGetTime        = 0x50,
    NanoSleep           = 0x51,

    // — Debug —
    DebugPrint          = 0xF0,
}

#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallFrame {
    pub rax: u64,
    pub rdi: u64,
    pub rsi: u64,
    pub rdx: u64,
    pub r10: u64,
    pub r8:  u64,
    pub r9:  u64,
}

/// Despachador. La instalación del MSR `IA32_LSTAR` se hará en `arch::x86_64::cpu`.
pub fn dispatch(_frame: &mut SyscallFrame) {
    // TODO: enrutar por número de syscall.
}

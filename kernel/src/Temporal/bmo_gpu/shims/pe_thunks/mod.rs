//! PE Win32 import → BMO backend dispatcher (inspired by Wine/Proton).
//!
//! v1.7.9: thin stub. The actual ntdll/kernel32 implementations lived
//! in `bmo_abi::interop::win32` (now removed). They were Windows NT
//! syscall shims — not applicable to a native OS. In v2.0, the ntdll/
//! kernel32 functions will be reimplemented as direct BMO API calls
//! (no Windows NT compatibility layer needed).

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;

/// Where a Win32 PE import should be redirected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThunkTarget {
    /// Stub silencioso (devuelve 0 / éxito).
    SilentStub,
    /// Stub que registra en serial y devuelve 0.
    LogStub,
    /// Backend real BareX o syscall FastOS.
    BarexGraphics,
    BarexAudio,
    BarexInput,
    BarexNet,
    SyscallVfs,
    SyscallProcess,
    SyscallTime,
    SyscallMemory,
    /// Real ntdll implementation (v2.0: BMO API call).
    NtdllGateway,
    /// Real kernel32 implementation (v2.0: BMO API call).
    Kernel32Impl,
    /// Proton-style DXGI/D3D12 → BareX graphics.
    ProtonDxvk,
    ProtonVkd3d,
    ProtonD8vk,
}

/// One entry in the thunk table: `(dll, fn_name, target)`.
#[derive(Debug, Clone, Copy)]
pub struct ThunkEntry {
    pub dll: &'static str,
    pub name: &'static str,
    pub target: ThunkTarget,
}

/// Thunk table. Stub: just maps every import to SilentStub for now.
/// v2.0: replace each entry with the actual BMO API call.
pub static THUNK_TABLE: &[ThunkEntry] = &[
    ThunkEntry { dll: "ntdll.dll",   name: "NtAllocateVirtualMemory",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtFreeVirtualMemory",         target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtProtectVirtualMemory",      target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtReadFile",                  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtWriteFile",                 target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtClose",                     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtCreateFile",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtTerminateProcess",          target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtCreateThreadEx",            target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtTerminateThread",           target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtQuerySystemTime",           target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "ntdll.dll",   name: "NtQueryPerformanceCounter",   target: ThunkTarget::SilentStub },

    ThunkEntry { dll: "kernel32.dll", name: "ExitProcess",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcess",          target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThread",           target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcessId",        target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThreadId",         target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "Sleep",                      target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount",               target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount64",             target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceCounter",    target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceFrequency",  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualAlloc",               target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualFree",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualProtect",             target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "CreateFileA",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "CreateFileW",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "ReadFile",                   target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "WriteFile",                  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "CloseHandle",                target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetLastError",               target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "SetLastError",               target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringA",         target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringW",         target: ThunkTarget::LogStub },
];

/// Resolve an import to its real function pointer.
pub fn resolve_fn(dll: &str, name: &str) -> (ThunkTarget, u64) {
    let target = resolve(dll, name);
    let fn_ptr = get_fn_ptr(target, dll, name);
    (target, fn_ptr)
}

fn get_fn_ptr(target: ThunkTarget, _dll: &str, _name: &str) -> u64 {
    match target {
        ThunkTarget::SilentStub
        | ThunkTarget::LogStub
        | ThunkTarget::BarexGraphics
        | ThunkTarget::BarexAudio
        | ThunkTarget::BarexInput
        | ThunkTarget::BarexNet
        | ThunkTarget::SyscallVfs
        | ThunkTarget::SyscallProcess
        | ThunkTarget::SyscallTime
        | ThunkTarget::SyscallMemory
        | ThunkTarget::NtdllGateway
        | ThunkTarget::Kernel32Impl
        | ThunkTarget::ProtonDxvk
        | ThunkTarget::ProtonVkd3d
        | ThunkTarget::ProtonD8vk => silent_stub as *const () as u64,
    }
}

/// Resolve a `(dll_name, fn_name)` to its `ThunkTarget`.
pub fn resolve(dll: &str, name: &str) -> ThunkTarget {
    for e in THUNK_TABLE {
        if e.dll.eq_ignore_ascii_case(dll) && e.name == name {
            return e.target;
        }
    }
    ThunkTarget::SilentStub
}

/// Number of entries in the master table.
pub const fn thunk_table_len() -> usize { THUNK_TABLE.len() }

/// Silent universal stub. Any unresolved import points here.
#[allow(unused)]
pub extern "C" fn silent_stub(_a: bx_u64, _b: bx_u64, _c: bx_u64, _d: bx_u64) -> bx_u64 { 0 }

/// Stub that logs the import name and returns 0.
#[allow(unused)]
pub extern "C" fn log_stub(_a: bx_u64) -> bx_u64 {
    crate::cabina::warn("bmo_gpu", "PE import called (not yet implemented)");
    0
}


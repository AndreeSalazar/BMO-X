//! PE Win32 import → BMO backend dispatcher (inspired by Wine/Proton).
//!
//! Architecture (analogous to Wine/Proton):
//!   PE app → ntdll.dll (gateway)    → bmo_abi::interop::win32::ntdll_*
//!   PE app → kernel32.dll (wrapper) → bmo_abi::interop::win32::kernel32_*
//!   PE app → d3d9/10/11/12.dll     → BareX graphics (via DXVK-style layer)
//!   PE app → xinput/xaudio          → BareX input/audio
//!   PE app → ws2_32                 → BareX net
//!
//! What is **NOT** here (eliminated legacy):
//!   - msvcrt.dll: BMO has its own C runtime; PE binaries using msvcrt
//!     must be recompiled or shimmed in Ring 3.
//!   - user32.dll / gdi32.dll: BMO has its own desktop; the PE app
//!     uses BareX directly via BMO ABI, not Win32 GUI.
//!   - ntdll!Rtl*: trivial RtlXXX memory helpers; not needed for the
//!     BMO PE loader flow.
//!   - kernel32!LoadLibrary/GetModuleHandle/GetProcAddress: BMO loads
//!     BEF binaries directly, not PE DLLs by name.

#![allow(dead_code)]

use crate::bmo_core::bmo_abi::interop::win32;
use crate::bmo_core::bmo_abi::primitives::bx_u64;

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
    /// Real ntdll implementation (in `bmo_abi::interop::win32`).
    NtdllGateway,
    /// Real kernel32 implementation (in `bmo_abi::interop::win32`).
    Kernel32Impl,
    /// Proton DXVK — DirectX 9/10/11 → Vulkan → BareX.
    ProtonDxvk,
    /// Proton VKD3D — DirectX 12 → Vulkan → BareX.
    ProtonVkd3d,
    /// Proton D8VK — DirectX 8 → Vulkan → BareX.
    ProtonD8vk,
}

/// One entry in the thunk table: `(dll, fn_name, target)`.
#[derive(Debug, Clone, Copy)]
pub struct ThunkEntry {
    pub dll: &'static str,
    pub name: &'static str,
    pub target: ThunkTarget,
}

/// Thunk table. If a PE imports something not in this table, it gets a
/// `SilentStub` to avoid crash. The log indicates what API was missing.
pub static THUNK_TABLE: &[ThunkEntry] = &[
    // ─── ntdll.dll (NT gateway — lowest level) ─────────────────────
    ThunkEntry { dll: "ntdll.dll", name: "NtAllocateVirtualMemory", target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtFreeVirtualMemory",     target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtProtectVirtualMemory",  target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtReadFile",              target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtWriteFile",             target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtClose",                 target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtCreateFile",            target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtTerminateProcess",      target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtCreateThreadEx",        target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtTerminateThread",       target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtQuerySystemTime",       target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "NtQueryPerformanceCounter", target: ThunkTarget::NtdllGateway },

    // ─── kernel32.dll (processes, threads, memory, files) ──────────
    ThunkEntry { dll: "kernel32.dll", name: "ExitProcess",          target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcess",    target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThread",     target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcessId",  target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThreadId",   target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "Sleep",                target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount",         target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount64",       target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceCounter", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceFrequency", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetSystemTimeAsFileTime", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualAlloc",         target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualFree",          target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "VirtualProtect",       target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "HeapCreate",           target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "HeapAlloc",            target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "HeapFree",             target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "GetProcessHeap",       target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "kernel32.dll", name: "CreateFileA",          target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "CreateFileW",          target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "ReadFile",             target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "WriteFile",            target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "CloseHandle",          target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "GetLastError",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "SetLastError",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringA",   target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringW",   target: ThunkTarget::LogStub },

    // ─── user32.dll (legacy — not implemented; BMO has its own UI) ─
    ThunkEntry { dll: "user32.dll", name: "MessageBoxA",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "MessageBoxW",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "CreateWindowExA",      target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "CreateWindowExW",      target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "ShowWindow",           target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "GetMessageA",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "GetMessageW",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "PeekMessageA",         target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "PeekMessageW",         target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "DispatchMessageA",     target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "DispatchMessageW",     target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "TranslateMessage",     target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "GetSystemMetrics",     target: ThunkTarget::LogStub },

    // ─── d3d12.dll / dxgi.dll → Proton VKD3D → BareX graphics ───
    ThunkEntry { dll: "d3d12.dll", name: "D3D12CreateDevice",       target: ThunkTarget::ProtonVkd3d },
    ThunkEntry { dll: "d3d12.dll", name: "D3D12GetDebugInterface",  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory",       target: ThunkTarget::ProtonVkd3d },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory2",      target: ThunkTarget::ProtonVkd3d },

    // ─── d3d11.dll → Proton DXVK → BareX graphics ───────────────
    ThunkEntry { dll: "d3d11.dll", name: "D3D11CreateDevice",       target: ThunkTarget::ProtonDxvk },
    ThunkEntry { dll: "d3d11.dll", name: "D3D11CreateDeviceAndSwapChain", target: ThunkTarget::ProtonDxvk },

    // ─── d3d9.dll → Proton DXVK/D8VK → BareX graphics ───────────
    ThunkEntry { dll: "d3d9.dll", name: "Direct3DCreate9",          target: ThunkTarget::ProtonDxvk },
    ThunkEntry { dll: "d3d9.dll", name: "Direct3DCreate9Ex",        target: ThunkTarget::ProtonDxvk },

    // ─── d3d8.dll → Proton D8VK → BareX graphics ────────────────
    ThunkEntry { dll: "d3d8.dll", name: "Direct3DCreate8",          target: ThunkTarget::ProtonD8vk },

    // ─── xinput1_4.dll → BareX input ──────────────────────────────
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputGetState",      target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputSetState",      target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputGetCapabilities", target: ThunkTarget::BarexInput },

    // ─── xaudio2_9.dll → BareX audio ──────────────────────────────
    ThunkEntry { dll: "xaudio2_9.dll", name: "XAudio2Create",       target: ThunkTarget::BarexAudio },

    // ─── ws2_32.dll → BareX net ───────────────────────────────────
    ThunkEntry { dll: "ws2_32.dll", name: "WSAStartup",             target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "WSACleanup",             target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "socket",                 target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "bind",                   target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "listen",                 target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "accept",                 target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "connect",                target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "send",                   target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "recv",                   target: ThunkTarget::BarexNet },
    ThunkEntry { dll: "ws2_32.dll", name: "closesocket",            target: ThunkTarget::BarexNet },
];

/// Resolve an import to its real function pointer.
pub fn resolve_fn(dll: &str, name: &str) -> (ThunkTarget, u64) {
    let target = resolve(dll, name);
    let fn_ptr = get_fn_ptr(target, dll, name);
    (target, fn_ptr)
}

fn get_fn_ptr(target: ThunkTarget, _dll: &str, name: &str) -> u64 {
    match target {
        ThunkTarget::SilentStub => silent_stub as *const () as u64,
        ThunkTarget::LogStub => log_stub as *const () as u64,

        ThunkTarget::NtdllGateway => resolve_ntdll_fn(name),
        ThunkTarget::Kernel32Impl => resolve_kernel32_fn(name),

        ThunkTarget::SyscallProcess => win32::ntdll_syscalls::NtTerminateProcess as *const () as u64,
        ThunkTarget::SyscallTime => win32::kernel32_thread::Sleep as *const () as u64,
        ThunkTarget::SyscallMemory => win32::kernel32_memory::VirtualAlloc as *const () as u64,
        ThunkTarget::SyscallVfs => win32::kernel32_file::CreateFileA as *const () as u64,

        ThunkTarget::BarexGraphics | ThunkTarget::BarexAudio
        | ThunkTarget::BarexInput | ThunkTarget::BarexNet
        | ThunkTarget::ProtonDxvk | ThunkTarget::ProtonVkd3d
        | ThunkTarget::ProtonD8vk => silent_stub as *const () as u64,
    }
}

fn resolve_ntdll_fn(name: &str) -> u64 {
    use win32::ntdll_syscalls as s;
    match name {
        "NtAllocateVirtualMemory" => s::NtAllocateVirtualMemory as *const () as u64,
        "NtFreeVirtualMemory" => s::NtFreeVirtualMemory as *const () as u64,
        "NtProtectVirtualMemory" => s::NtProtectVirtualMemory as *const () as u64,
        "NtCreateFile" => s::NtCreateFile as *const () as u64,
        "NtReadFile" => s::NtReadFile as *const () as u64,
        "NtWriteFile" => s::NtWriteFile as *const () as u64,
        "NtClose" => s::NtClose as *const () as u64,
        "NtTerminateProcess" => s::NtTerminateProcess as *const () as u64,
        "NtCreateThreadEx" => s::NtCreateThreadEx as *const () as u64,
        "NtTerminateThread" => s::NtTerminateThread as *const () as u64,
        "NtQuerySystemTime" => s::NtQuerySystemTime as *const () as u64,
        "NtQueryPerformanceCounter" => s::NtQueryPerformanceCounter as *const () as u64,
        _ => silent_stub as *const () as u64,
    }
}

fn resolve_kernel32_fn(name: &str) -> u64 {
    use win32::{kernel32_process as proc, kernel32_file as file};
    match name {
        "GetCurrentProcess" => proc::GetCurrentProcess as *const () as u64,
        "GetCurrentThread" => proc::GetCurrentThread as *const () as u64,
        "GetCurrentProcessId" => proc::GetCurrentProcessId as *const () as u64,
        "GetCurrentThreadId" => proc::GetCurrentThreadId as *const () as u64,
        "GetLastError" => file::GetLastError as *const () as u64,
        "SetLastError" => file::SetLastError as *const () as u64,
        _ => silent_stub as *const () as u64,
    }
}

/// Resolve a `(dll_name, fn_name)` to its `ThunkTarget`. If not in the
/// table, returns `SilentStub` to avoid crash.
pub fn resolve(dll: &str, name: &str) -> ThunkTarget {
    for e in THUNK_TABLE {
        if eq_ascii_ci(e.dll, dll) && e.name == name {
            return e.target;
        }
    }
    ThunkTarget::SilentStub
}

/// Number of entries in the master table.
pub const fn thunk_table_len() -> usize { THUNK_TABLE.len() }

/// Case-insensitive comparison for DLL names (Win32 treats them this way).
fn eq_ascii_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    let aa = a.as_bytes(); let bb = b.as_bytes();
    for i in 0..aa.len() {
        let ca = aa[i].to_ascii_lowercase();
        let cb = bb[i].to_ascii_lowercase();
        if ca != cb { return false; }
    }
    true
}

/// Silent universal stub. Any unresolved import points here.
#[allow(unused)]
pub extern "C" fn silent_stub(_a: bx_u64, _b: bx_u64, _c: bx_u64, _d: bx_u64) -> bx_u64 { 0 }

/// Stub that logs the import name and returns 0.
#[allow(unused)]
pub extern "C" fn log_stub(_a: bx_u64) -> bx_u64 {
    // TODO: serial::log("[devour-pe] unresolved import called");
    0
}

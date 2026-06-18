//! Fake-DLLs Win32 — tabla de funciones que el devour-loader provee a
//! binarios PE para que **crean** que están corriendo sobre Windows.
//!
//! El loader, al resolver un import como `kernel32!ExitProcess`, busca
//! aquí y obtiene un puntero a un wrapper Rust que traduce a syscall
//! FastOS / BareX. Si la función no está en la tabla, se asigna un stub
//! "log+return 0" que evita crashes inmediatos.
//!
//! Architecture (inspired by Wine/Proton):
//!   PE app → ntdll.dll (gateway) → BMO syscall → kernel
//!   PE app → kernel32.dll → ntdll.dll → BMO syscall
//!   PE app → user32.dll → win32k (desktop compositor)
//!   PE app → d3d12.dll → VKD3D → BareX graphics

#![allow(dead_code)]

use crate::bmo_abi::primitives::bx_u64;
use crate::windows_compat;

/// A qué backend del kernel se redirige la función importada.
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
    /// Real windows_compat implementation (ntdll gateway).
    NtdllGateway,
    /// Real windows_compat implementation (kernel32).
    Kernel32Impl,
    /// Real windows_compat implementation (msvcrt).
    MsvcrtImpl,
    /// Real windows_compat implementation (user32).
    User32Impl,
    /// Real windows_compat implementation (gdi32).
    Gdi32Impl,
    /// Proton DXVK — DirectX 9/10/11 → Vulkan → BareX.
    ProtonDxvk,
    /// Proton VKD3D — DirectX 12 → Vulkan → BareX.
    ProtonVkd3d,
    /// Proton D8VK — DirectX 8 → Vulkan → BareX.
    ProtonD8vk,
}

/// Una entrada de la tabla — `(dll, fn_name, target)`.
#[derive(Debug, Clone, Copy)]
pub struct ThunkEntry {
    pub dll: &'static str,
    pub name: &'static str,
    pub target: ThunkTarget,
}

/// Tabla maestra de fake-DLLs. **Estática, ordenada por DLL.**
///
/// Si un PE importa algo que no está aquí, se le da un `SilentStub` para
/// evitar el crash; el log indica qué API faltó (útil para iterar
/// compatibilidad).
pub static THUNK_TABLE: &[ThunkEntry] = &[
    // ─── ntdll.dll (NT gateway — lowest level) ─────────────────────────
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
    ThunkEntry { dll: "ntdll.dll", name: "RtlAddFunctionTable",     target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "RtlDeleteFunctionTable",  target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "RtlZeroMemory",           target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "RtlCopyMemory",           target: ThunkTarget::NtdllGateway },
    ThunkEntry { dll: "ntdll.dll", name: "RtlFillMemory",           target: ThunkTarget::NtdllGateway },

    // ─── kernel32.dll (processes, threads, memory, files) ──────────────
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
    ThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleA",     target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleW",     target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "GetProcAddress",       target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "LoadLibraryA",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "LoadLibraryW",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "FreeLibrary",          target: ThunkTarget::Kernel32Impl },
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
    ThunkEntry { dll: "kernel32.dll", name: "SetFilePointer",       target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "SetFilePointerEx",     target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "GetFileSize",          target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "DeleteFileA",          target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "kernel32.dll", name: "GetLastError",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "SetLastError",         target: ThunkTarget::Kernel32Impl },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringA",   target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringW",   target: ThunkTarget::LogStub },

    // ─── msvcrt.dll (C runtime) ────────────────────────────────────────
    ThunkEntry { dll: "msvcrt.dll", name: "_initterm",              target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "_initterm_e",            target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "__security_init_cookie", target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "__GSHandlerCheck",       target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "__CxxFrameHandler3",     target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "__chkstk",               target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "malloc",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "free",                   target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "realloc",                target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "calloc",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "strlen",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "strcpy",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "strcmp",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "memcpy",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "memset",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "memcmp",                 target: ThunkTarget::MsvcrtImpl },
    ThunkEntry { dll: "msvcrt.dll", name: "exit",                   target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "msvcrt.dll", name: "atoi",                   target: ThunkTarget::MsvcrtImpl },

    // ─── user32.dll (windows, messages, input legacy) ──────────────────
    ThunkEntry { dll: "user32.dll", name: "MessageBoxA",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "MessageBoxW",          target: ThunkTarget::LogStub },
    ThunkEntry { dll: "user32.dll", name: "CreateWindowExA",      target: ThunkTarget::BarexGraphics },
    ThunkEntry { dll: "user32.dll", name: "CreateWindowExW",      target: ThunkTarget::BarexGraphics },
    ThunkEntry { dll: "user32.dll", name: "ShowWindow",           target: ThunkTarget::BarexGraphics },
    ThunkEntry { dll: "user32.dll", name: "GetMessageA",          target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "user32.dll", name: "GetMessageW",          target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "user32.dll", name: "PeekMessageA",         target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "user32.dll", name: "PeekMessageW",         target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "user32.dll", name: "DispatchMessageA",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "user32.dll", name: "DispatchMessageW",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "user32.dll", name: "TranslateMessage",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "user32.dll", name: "GetSystemMetrics",     target: ThunkTarget::User32Impl },

    // ─── d3d12.dll / dxgi.dll → Proton VKD3D → BareX graphics ─────────
    ThunkEntry { dll: "d3d12.dll", name: "D3D12CreateDevice",       target: ThunkTarget::ProtonVkd3d },
    ThunkEntry { dll: "d3d12.dll", name: "D3D12GetDebugInterface",  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory",       target: ThunkTarget::ProtonVkd3d },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory2",      target: ThunkTarget::ProtonVkd3d },

    // ─── d3d11.dll → Proton DXVK → BareX graphics ─────────────────────
    ThunkEntry { dll: "d3d11.dll", name: "D3D11CreateDevice",       target: ThunkTarget::ProtonDxvk },
    ThunkEntry { dll: "d3d11.dll", name: "D3D11CreateDeviceAndSwapChain", target: ThunkTarget::ProtonDxvk },

    // ─── d3d9.dll → Proton DXVK/D8VK → BareX graphics ────────────────
    ThunkEntry { dll: "d3d9.dll", name: "Direct3DCreate9",          target: ThunkTarget::ProtonDxvk },
    ThunkEntry { dll: "d3d9.dll", name: "Direct3DCreate9Ex",        target: ThunkTarget::ProtonDxvk },

    // ─── d3d8.dll → Proton D8VK → BareX graphics ──────────────────────
    ThunkEntry { dll: "d3d8.dll", name: "Direct3DCreate8",          target: ThunkTarget::ProtonD8vk },

    // ─── xinput1_4.dll → BareX input ──────────────────────────────────
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputGetState",      target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputSetState",      target: ThunkTarget::BarexInput },
    ThunkEntry { dll: "xinput1_4.dll", name: "XInputGetCapabilities", target: ThunkTarget::BarexInput },

    // ─── xaudio2_9.dll → BareX audio ──────────────────────────────────
    ThunkEntry { dll: "xaudio2_9.dll", name: "XAudio2Create",       target: ThunkTarget::BarexAudio },

    // ─── ws2_32.dll → BareX net ───────────────────────────────────────
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

/// Resolve an import to its real function pointer (runtime resolution).
///
/// This is called during PE import resolution. It returns the actual
/// function pointer from windows_compat that should be used.
pub fn resolve_fn(dll: &str, name: &str) -> (ThunkTarget, u64) {
    let target = resolve(dll, name);
    let fn_ptr = get_fn_ptr(target, dll, name);
    (target, fn_ptr)
}

/// Get the real function pointer for a given target.
///
/// This uses runtime resolution to avoid const-eval issues with
/// function pointer casts.
fn get_fn_ptr(target: ThunkTarget, dll: &str, name: &str) -> u64 {
    match target {
        ThunkTarget::SilentStub => silent_stub as u64,
        ThunkTarget::LogStub => log_stub as u64,

        ThunkTarget::NtdllGateway => resolve_ntdll_fn(name),
        ThunkTarget::Kernel32Impl => resolve_kernel32_fn(name),
        ThunkTarget::MsvcrtImpl => resolve_msvcrt_fn(name),
        ThunkTarget::User32Impl => resolve_user32_fn(name),
        ThunkTarget::Gdi32Impl => silent_stub as u64, // TODO

        ThunkTarget::SyscallProcess => windows_compat::ntdll::syscalls::NtTerminateProcess as u64,
        ThunkTarget::SyscallTime => windows_compat::kernel32::time::GetTickCount as u64,
        ThunkTarget::SyscallMemory => windows_compat::kernel32::memory::VirtualAlloc as u64,
        ThunkTarget::SyscallVfs => windows_compat::kernel32::file::CreateFileA as u64,

        ThunkTarget::BarexGraphics => silent_stub as u64,
        ThunkTarget::BarexAudio => silent_stub as u64,
        ThunkTarget::BarexInput => silent_stub as u64,
        ThunkTarget::BarexNet => silent_stub as u64,

        ThunkTarget::ProtonDxvk => silent_stub as u64,
        ThunkTarget::ProtonVkd3d => silent_stub as u64,
        ThunkTarget::ProtonD8vk => silent_stub as u64,
    }
}

/// Resolve ntdll function by name.
fn resolve_ntdll_fn(name: &str) -> u64 {
    match name {
        "NtAllocateVirtualMemory" => windows_compat::ntdll::syscalls::NtAllocateVirtualMemory as u64,
        "NtFreeVirtualMemory" => windows_compat::ntdll::syscalls::NtFreeVirtualMemory as u64,
        "NtProtectVirtualMemory" => windows_compat::ntdll::syscalls::NtProtectVirtualMemory as u64,
        "NtCreateFile" => windows_compat::ntdll::syscalls::NtCreateFile as u64,
        "NtReadFile" => windows_compat::ntdll::syscalls::NtReadFile as u64,
        "NtWriteFile" => windows_compat::ntdll::syscalls::NtWriteFile as u64,
        "NtClose" => windows_compat::ntdll::syscalls::NtClose as u64,
        "NtTerminateProcess" => windows_compat::ntdll::syscalls::NtTerminateProcess as u64,
        "NtCreateThreadEx" => windows_compat::ntdll::syscalls::NtCreateThreadEx as u64,
        "NtTerminateThread" => windows_compat::ntdll::syscalls::NtTerminateThread as u64,
        "NtQuerySystemTime" => windows_compat::ntdll::syscalls::NtQuerySystemTime as u64,
        "NtQueryPerformanceCounter" => windows_compat::ntdll::syscalls::NtQueryPerformanceCounter as u64,
        "RtlAddFunctionTable" => windows_compat::ntdll::rtl::RtlAddFunctionTable as u64,
        "RtlDeleteFunctionTable" => windows_compat::ntdll::rtl::RtlDeleteFunctionTable as u64,
        "RtlZeroMemory" => windows_compat::ntdll::rtl::RtlZeroMemory as u64,
        "RtlCopyMemory" => windows_compat::ntdll::rtl::RtlCopyMemory as u64,
        "RtlFillMemory" => windows_compat::ntdll::rtl::RtlFillMemory as u64,
        _ => silent_stub as u64,
    }
}

/// Resolve kernel32 function by name.
fn resolve_kernel32_fn(name: &str) -> u64 {
    match name {
        "GetCurrentProcess" => windows_compat::kernel32::process::GetCurrentProcess as u64,
        "GetCurrentThread" => windows_compat::kernel32::process::GetCurrentThread as u64,
        "GetCurrentProcessId" => windows_compat::kernel32::process::GetCurrentProcessId as u64,
        "GetCurrentThreadId" => windows_compat::kernel32::process::GetCurrentThreadId as u64,
        "GetModuleHandleA" => windows_compat::kernel32::module::GetModuleHandleA as u64,
        "GetModuleHandleW" => windows_compat::kernel32::module::GetModuleHandleW as u64,
        "GetProcAddress" => windows_compat::kernel32::module::GetProcAddress as u64,
        "LoadLibraryA" => windows_compat::kernel32::module::LoadLibraryA as u64,
        "LoadLibraryW" => windows_compat::kernel32::module::LoadLibraryW as u64,
        "FreeLibrary" => windows_compat::kernel32::module::FreeLibrary as u64,
        "GetLastError" => windows_compat::kernel32::file::GetLastError as u64,
        "SetLastError" => windows_compat::kernel32::file::SetLastError as u64,
        _ => silent_stub as u64,
    }
}

/// Resolve msvcrt function by name.
fn resolve_msvcrt_fn(name: &str) -> u64 {
    match name {
        "_initterm" => windows_compat::msvcrt::init::_initterm as u64,
        "_initterm_e" => windows_compat::msvcrt::init::_initterm_e as u64,
        "__security_init_cookie" => windows_compat::msvcrt::init::__security_init_cookie as u64,
        "__GSHandlerCheck" => windows_compat::msvcrt::init::__GSHandlerCheck as u64,
        "__CxxFrameHandler3" => windows_compat::msvcrt::init::__CxxFrameHandler3 as u64,
        "__chkstk" => windows_compat::msvcrt::init::__chkstk as u64,
        "malloc" => windows_compat::msvcrt::memory::malloc as u64,
        "free" => windows_compat::msvcrt::memory::free as u64,
        "realloc" => windows_compat::msvcrt::memory::realloc as u64,
        "calloc" => windows_compat::msvcrt::memory::calloc as u64,
        "strlen" => windows_compat::msvcrt::string::strlen as u64,
        "strcpy" => windows_compat::msvcrt::string::strcpy as u64,
        "strcmp" => windows_compat::msvcrt::string::strcmp as u64,
        "memcpy" => windows_compat::msvcrt::string::memcpy as u64,
        "memset" => windows_compat::msvcrt::string::memset as u64,
        "memcmp" => windows_compat::msvcrt::string::memcmp as u64,
        "atoi" => windows_compat::msvcrt::stdlib::atoi as u64,
        _ => silent_stub as u64,
    }
}

/// Resolve user32 function by name.
fn resolve_user32_fn(name: &str) -> u64 {
    match name {
        "GetSystemMetrics" => windows_compat::user32::metrics::GetSystemMetrics as u64,
        _ => silent_stub as u64,
    }
}

/// Resuelve una import `(dll_name, fn_name)` a su `ThunkTarget`.
/// Si no está en la tabla, devuelve `SilentStub` para evitar crash.
pub fn resolve(dll: &str, name: &str) -> ThunkTarget {
    for e in THUNK_TABLE {
        if eq_ascii_ci(e.dll, dll) && e.name == name {
            return e.target;
        }
    }
    ThunkTarget::SilentStub
}

/// Cuántas entradas tiene la tabla maestra (para reporte).
pub const fn thunk_table_len() -> usize { THUNK_TABLE.len() }

/// Comparación case-insensitive para nombres de DLL (Win32 los trata así).
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

/// Stub silencioso universal. Cualquier import no resuelta apunta aquí.
/// Toma cualquier args (BMO ABI) y devuelve 0 = "éxito" / NULL.
#[allow(unused)]
pub extern "C" fn silent_stub(_a: bx_u64, _b: bx_u64, _c: bx_u64, _d: bx_u64) -> bx_u64 { 0 }

/// Stub que registra el nombre y devuelve 0.
#[allow(unused)]
pub extern "C" fn log_stub(_a: bx_u64) -> bx_u64 {
    // TODO: serial::log("[devour-pe] unresolved import called");
    0
}

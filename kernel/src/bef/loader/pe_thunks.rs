//! Fake-DLLs Win32 — tabla de funciones que el devour-loader provee a
//! binarios PE para que **crean** que están corriendo sobre Windows.
//!
//! El loader, al resolver un import como `kernel32!ExitProcess`, busca
//! aquí y obtiene un puntero a un wrapper Rust que traduce a syscall
//! FastOS / BareX. Si la función no está en la tabla, se asigna un stub
//! "log+return 0" que evita crashes inmediatos.

#![allow(dead_code)]

use crate::barex::abi::primitives::bx_u64;

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
    // ─── kernel32.dll (procesos, threads, memoria, archivos) ───────────
    ThunkEntry { dll: "kernel32.dll", name: "ExitProcess",          target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcess",    target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThread",     target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentProcessId",  target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "GetCurrentThreadId",   target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "kernel32.dll", name: "Sleep",                target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount",         target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetTickCount64",       target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceCounter", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceFrequency", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetSystemTimeAsFileTime", target: ThunkTarget::SyscallTime },
    ThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleA",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleW",     target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "GetProcAddress",       target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "LoadLibraryA",         target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "LoadLibraryW",         target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "kernel32.dll", name: "FreeLibrary",          target: ThunkTarget::SilentStub },
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
    ThunkEntry { dll: "kernel32.dll", name: "GetLastError",         target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "SetLastError",         target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringA",   target: ThunkTarget::LogStub },
    ThunkEntry { dll: "kernel32.dll", name: "OutputDebugStringW",   target: ThunkTarget::LogStub },

    // ─── user32.dll (ventanas, mensajes, input legacy) ─────────────────
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

    // ─── ntdll.dll (interfaz de syscalls) ──────────────────────────────
    ThunkEntry { dll: "ntdll.dll", name: "NtAllocateVirtualMemory", target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "ntdll.dll", name: "NtFreeVirtualMemory",     target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "ntdll.dll", name: "NtProtectVirtualMemory",  target: ThunkTarget::SyscallMemory },
    ThunkEntry { dll: "ntdll.dll", name: "NtReadFile",              target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "ntdll.dll", name: "NtWriteFile",             target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "ntdll.dll", name: "NtClose",                 target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "ntdll.dll", name: "NtCreateFile",            target: ThunkTarget::SyscallVfs },
    ThunkEntry { dll: "ntdll.dll", name: "NtTerminateProcess",      target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "ntdll.dll", name: "NtCreateThreadEx",        target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "ntdll.dll", name: "NtWaitForSingleObject",   target: ThunkTarget::SyscallProcess },
    ThunkEntry { dll: "ntdll.dll", name: "NtQuerySystemTime",       target: ThunkTarget::SyscallTime },

    // ─── d3d12.dll / dxgi.dll → BareX graphics ─────────────────────────
    ThunkEntry { dll: "d3d12.dll", name: "D3D12CreateDevice",       target: ThunkTarget::BarexGraphics },
    ThunkEntry { dll: "d3d12.dll", name: "D3D12GetDebugInterface",  target: ThunkTarget::SilentStub },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory",       target: ThunkTarget::BarexGraphics },
    ThunkEntry { dll: "dxgi.dll",  name: "CreateDXGIFactory2",      target: ThunkTarget::BarexGraphics },

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

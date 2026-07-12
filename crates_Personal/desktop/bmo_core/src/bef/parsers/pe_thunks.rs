#![allow(dead_code)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeThunkTarget {
    SilentStub,
    ProcessExit,
    DebugPrint,
    SyscallVfs,
    SyscallMemory,
    SyscallTime,
}

#[derive(Debug, Clone, Copy)]
pub struct PeThunkEntry {
    pub dll: &'static str,
    pub name: &'static str,
    pub target: PeThunkTarget,
}

pub static THUNK_TABLE: &[PeThunkEntry] = &[
    // kernel32.dll
    PeThunkEntry { dll: "kernel32.dll", name: "ExitProcess",             target: PeThunkTarget::ProcessExit },
    PeThunkEntry { dll: "kernel32.dll", name: "TerminateProcess",        target: PeThunkTarget::ProcessExit },
    PeThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleA",        target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleW",        target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetProcAddress",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetStdHandle",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "WriteFile",               target: PeThunkTarget::DebugPrint },
    PeThunkEntry { dll: "kernel32.dll", name: "ReadFile",                target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "HeapAlloc",               target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "kernel32.dll", name: "HeapFree",                target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "kernel32.dll", name: "VirtualAlloc",            target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "kernel32.dll", name: "VirtualFree",             target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "kernel32.dll", name: "GetLastError",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "SetLastError",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetCommandLineA",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetCommandLineW",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetEnvironmentStringsW",  target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "FreeEnvironmentStringsW", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "SetHandleInformation",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetStartupInfoA",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetModuleHandleExW",      target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "LCMapStringEx",           target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "InitializeSListHead",     target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "FlushProcessWriteBuffers", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetSystemInfo",           target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "GetNativeSystemInfo",     target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "IsProcessorFeaturePresent", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceCounter", target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "kernel32.dll", name: "QueryPerformanceFrequency", target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "kernel32.dll", name: "GetTickCount",            target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "kernel32.dll", name: "GetTickCount64",          target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "kernel32.dll", name: "Sleep",                   target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "kernel32.dll", name: "CreateFileA",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "CreateFileW",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "CloseHandle",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "DeleteFileA",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "DeleteFileW",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "MoveFileA",               target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "MoveFileW",               target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "CopyFileA",               target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "CopyFileW",               target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "SetFilePointer",          target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "SetFilePointerEx",        target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "GetFileSize",             target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "kernel32.dll", name: "GetFileSizeEx",           target: PeThunkTarget::SyscallVfs },

    // ntdll.dll
    PeThunkEntry { dll: "ntdll.dll", name: "NtWriteFile",               target: PeThunkTarget::DebugPrint },
    PeThunkEntry { dll: "ntdll.dll", name: "NtTerminateProcess",        target: PeThunkTarget::ProcessExit },
    PeThunkEntry { dll: "ntdll.dll", name: "NtAllocateVirtualMemory",   target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "ntdll.dll", name: "NtFreeVirtualMemory",       target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "ntdll.dll", name: "NtCreateFile",              target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "ntdll.dll", name: "NtClose",                   target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "ntdll.dll", name: "NtDeviceIoControlFile",     target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "NtWaitForSingleObject",     target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "NtQueryInformationProcess", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "NtCreateThreadEx",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "NtDelayExecution",          target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "ntdll.dll", name: "NtQueryPerformanceCounter", target: PeThunkTarget::SyscallTime },
    PeThunkEntry { dll: "ntdll.dll", name: "NtSetInformationFile",      target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "ntdll.dll", name: "NtQueryVolumeInformationFile", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "NtQueryDirectoryFile",      target: PeThunkTarget::SyscallVfs },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlNtStatusToDosError",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlAllocateHeap",          target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlFreeHeap",              target: PeThunkTarget::SyscallMemory },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlEnterCriticalSection",   target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlLeaveCriticalSection",   target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlInitializeCriticalSection", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlDeleteCriticalSection",  target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlLookupFunctionEntry",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlVirtualUnwind",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlCaptureContext",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlUnwind",                 target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlRaiseException",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlRaiseStatus",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlGetCurrentPeb",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlGetNtVersionNumbers",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlImageNtHeader",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlImageDirectoryEntryToData", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlPcToFileHeader",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlAddFunctionTable",       target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlDeleteFunctionTable",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "RtlInstallFunctionTableCallback", target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "memset",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "memcpy",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "memcmp",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "wcslen",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "wcscmp",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "ntdll.dll", name: "wcscpy",                    target: PeThunkTarget::SilentStub },

    // user32.dll - all stubs
    PeThunkEntry { dll: "user32.dll", name: "CreateWindowExA",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "CreateWindowExW",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "DestroyWindow",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "DefWindowProcA",           target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "DefWindowProcW",           target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "DispatchMessageA",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "DispatchMessageW",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "GetMessageA",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "GetMessageW",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "TranslateMessage",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "RegisterClassExA",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "RegisterClassExW",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "ShowWindow",               target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "UpdateWindow",             target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "BeginPaint",               target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "EndPaint",                 target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "PostQuitMessage",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "MessageBoxA",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "MessageBoxW",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "LoadCursorA",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "LoadCursorW",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "LoadIconA",                target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "LoadIconW",                target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "GetDC",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "ReleaseDC",                target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "GetClientRect",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "AdjustWindowRect",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "PeekMessageA",             target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "PeekMessageW",             target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "SendMessageA",             target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "user32.dll", name: "SendMessageW",             target: PeThunkTarget::SilentStub },

    // gdi32.dll - all stubs
    PeThunkEntry { dll: "gdi32.dll", name: "CreateCompatibleDC",        target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "DeleteDC",                  target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "BitBlt",                    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "StretchDIBits",             target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "SetDIBitsToDevice",         target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "GetStockObject",            target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "SelectObject",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "DeleteObject",              target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "CreateSolidBrush",          target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "CreateFontA",               target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "CreateFontW",               target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "CreateCompatibleBitmap",    target: PeThunkTarget::SilentStub },
    PeThunkEntry { dll: "gdi32.dll", name: "GetDeviceCaps",             target: PeThunkTarget::SilentStub },
];

/// Look up PE thunk target by DLL + function name.
pub fn resolve(dll: &str, name: &str) -> PeThunkTarget {
    let dll_normalized = normalize_dll_name(dll);
    for e in THUNK_TABLE {
        if eq_ci(e.dll, &dll_normalized) && eq_ci(e.name, name) {
            return e.target;
        }
    }
    PeThunkTarget::SilentStub
}

fn normalize_dll_name(dll: &str) -> alloc::string::String {
    let basename = dll.rsplit('\\').next().unwrap_or(dll).rsplit('/').next().unwrap_or(dll);
    basename.to_lowercase()
}

fn eq_ci(a: &str, b: &str) -> bool {
    if a.len() != b.len() { return false; }
    a.bytes().zip(b.bytes()).all(|(x, y)| x.eq_ignore_ascii_case(&y))
}

/// Return a function pointer matching the PE thunk target.
pub fn resolve_fn_ptr(dll: &str, name: &str) -> Option<*const ()> {
    let target = resolve(dll, name);
    match target {
        PeThunkTarget::ProcessExit => Some(exit_process as *const ()),
        PeThunkTarget::DebugPrint => Some(write_file as *const ()),
        PeThunkTarget::SyscallVfs => Some(vfs_stub as *const ()),
        PeThunkTarget::SyscallMemory => Some(memory_stub as *const ()),
        PeThunkTarget::SyscallTime => Some(time_stub as *const ()),
        PeThunkTarget::SilentStub => Some(silent_stub as *const ()),
    }
}

// ─── Thunk implementations ──────────────────────────────────────────
//
// These are extern "C" functions that the IAT entries point to.
// The PE caller uses the Win64 calling convention (RCX, RDX, R8, R9),
// but our extern "C" uses System V (RDI, RSI, RDX, RCX, R8, R9).
//
// To work around this mismatch, functions that NEED actual args
// read them directly from the Windows-ABI registers via inline asm.
// Stub functions ignore all args and just return 0 / halt.

/// PE kernel32!ExitProcess / ntdll!NtTerminateProcess
/// Win64: RCX = exit code
#[no_mangle]
pub unsafe extern "C" fn exit_process() -> ! {
    loop { core::arch::asm!("hlt", options(nomem, nostack)); }
}

/// PE kernel32!WriteFile / ntdll!NtWriteFile
/// Win64: RCX=handle, RDX=buf, R8=count, R9=bytes_written, [rsp+40]=overlapped
/// Reads buf + count directly from Windows registers.
#[no_mangle]
pub unsafe extern "C" fn write_file() -> u32 {
    let ptr: u64;
    let len: u32;
    core::arch::asm!(
        "mov {}, rdx",
        "mov {:e}, r8d",
        out(reg) ptr,
        out(reg) len,
        options(preserves_flags, nostack),
    );
    if ptr != 0 && len > 0 {
        let slice = core::slice::from_raw_parts(ptr as *const u8, len as usize);
        crate::cabina::info("pe", core::str::from_utf8_unchecked(slice));
    }
    len
}

/// PE stub for filesystem functions.
#[no_mangle]
pub unsafe extern "C" fn vfs_stub() -> u64 {
    u64::MAX
}

/// PE stub for memory functions.
#[no_mangle]
pub unsafe extern "C" fn memory_stub() -> u64 {
    0
}

/// PE stub for time functions.
#[no_mangle]
pub unsafe extern "C" fn time_stub() -> u64 {
    0
}

/// PE stub for unsupported functions.
#[no_mangle]
pub extern "C" fn silent_stub() -> u64 {
    0
}

//! Win32 NT-level interop surface.
//!
//! The minimum subset of Windows NT that BMO implements natively in Ring 0
//! to load and run Windows PE binaries. Only the essential syscalls and
//! wrappers — no GUI, no GDI, no COM, no shell.
//!
//! ## What is here
//!
//! - `ntdll_syscalls` — Table mapping NT syscall numbers to BMO syscalls.
//! - `ntdll_memory`   — NtAllocateVirtualMemory, NtFreeVirtualMemory, etc.
//! - `ntdll_file`     — NtCreateFile, NtReadFile, NtWriteFile, etc.
//! - `ntdll`          — Aggregator with NTSTATUS types and init.
//! - `kernel32_memory` — VirtualAlloc, VirtualFree, HeapAlloc, etc.
//! - `kernel32_thread`  — CreateThread, Sleep, TlsAlloc, etc.
//! - `kernel32_process` — GetCurrentProcessId, ExitProcess, etc.
//! - `kernel32_file`    — CreateFileA, ReadFile, WriteFile, etc.
//! - `kernel32`         — Aggregator with init.
//!
//! ## What is NOT here (eliminated as legacy)
//!
//! - `kernel32_string`   (lstrcpy)        — trivial, BMO strings are BMO.
//! - `kernel32_env`      (GetCommandLine) — BMO has its own env.
//! - `kernel32_time`     (GetSystemTime)  — see `bmo_abi::time`.
//! - `kernel32_module`   (LoadLibraryA)   — BMO loads BEF directly.
//! - `ntdll_rtl`         (RtlAllocateHeap) — duplicate of kernel32_memory.
//! - `ntdll_objects`     (NT object APIs) — BMO has its own objects.
//! - `ntdll_thread`      (3 lines)        — covered by kernel32_thread.
//! - `ntdll_process`     (3 lines)        — covered by kernel32_process.

#![allow(dead_code)]

pub mod ntdll;
pub mod ntdll_syscalls;
pub mod ntdll_memory;
pub mod ntdll_file;

pub mod kernel32;
pub mod kernel32_memory;
pub mod kernel32_thread;
pub mod kernel32_process;
pub mod kernel32_file;

/// Initialize the Win32 NT interop surface.
pub fn init() {
    ntdll::init();
    crate::diag::info("bmo_abi::interop::win32", "Win32 NT interop initialized");
}

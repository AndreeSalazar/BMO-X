//! kernel32.dll compatibility — Process, Memory, Thread, File.
//!
//! Maps Win32 kernel32 functions to BMO syscalls and barex functions.
//!
//! Sub-files in this directory:
//!   - `kernel32_process.rs` — GetCurrentProcessId, ExitProcess, etc.
//!   - `kernel32_thread.rs`  — CreateThread, Sleep, etc.
//!   - `kernel32_memory.rs`  — VirtualAlloc, HeapAlloc, etc.
//!   - `kernel32_file.rs`    — CreateFile, ReadFile, WriteFile, etc.
//!
//! (No more module, string, env, time: those are BMO native now.
//!  Eliminated as legacy.)

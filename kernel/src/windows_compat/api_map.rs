//! Win32 → BMO API Mapping Table.
//!
//! This file maps Windows API functions to their BMO equivalents.
//! Sources: Wine, ReactOS, MSDN, ntdoc.
//!
//! Format:
//!   ("dll", "FunctionName") => BmoTarget,
//!
//! Coverage: ~2,000 functions across 40+ DLLs.
//! Current implementation: P0-P1 tier only.

#![allow(dead_code)]

/// BMO backend target for a Win32 function.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BmoTarget {
    /// Maps to a BMO syscall (number).
    Syscall(u16),
    /// Maps to a BMO barex module function.
    Barex(&'static str),
    /// Maps to a windows_compat implementation function.
    Compat(&'static str),
    /// Not implemented yet (returns 0 / no-op).
    Stub,
    /// Not applicable (does not exist in BMO).
    NotApplicable,
}

// ════════════════════════════════════════════════════════════════════════
// KERNEL32.DLL — Process, Memory, File, Thread, Module, String, Env
// ════════════════════════════════════════════════════════════════════════

// ─── Process ────────────────────────────────────────────────────────────
pub const KERNEL32_PROCESS: &[(&str, BmoTarget)] = &[
    ("ExitProcess",              BmoTarget::Syscall(0x00)),           // ProcessExit
    ("GetCurrentProcess",        BmoTarget::Compat("kernel32::process::current_process")),
    ("GetCurrentProcessId",      BmoTarget::Compat("kernel32::process::current_pid")),
    ("GetCurrentThread",         BmoTarget::Compat("kernel32::process::current_thread")),
    ("GetCurrentThreadId",       BmoTarget::Compat("kernel32::process::current_tid")),
    ("TerminateProcess",         BmoTarget::Syscall(0x00)),           // ExitProcess
    ("GetExitCodeProcess",       BmoTarget::Compat("kernel32::process::exit_code")),
    ("WaitForSingleObject",      BmoTarget::Syscall(0x03)),           // Yield (simplified)
    ("WaitForMultipleObjects",   BmoTarget::Compat("kernel32::thread::wait_multi")),
    ("CreateProcessA",           BmoTarget::Compat("kernel32::process::create_a")),
    ("CreateProcessW",           BmoTarget::Compat("kernel32::process::create_w")),
    ("OpenProcess",              BmoTarget::Compat("kernel32::process::open")),
    ("GetCommandLineA",          BmoTarget::Compat("kernel32::env::command_line_a")),
    ("GetCommandLineW",          BmoTarget::Compat("kernel32::env::command_line_w")),
];

// ─── Memory ─────────────────────────────────────────────────────────────
pub const KERNEL32_MEMORY: &[(&str, BmoTarget)] = &[
    ("VirtualAlloc",             BmoTarget::Syscall(0x10)),           // Mmap
    ("VirtualFree",              BmoTarget::Syscall(0x11)),           // Munmap
    ("VirtualProtect",           BmoTarget::Syscall(0x12)),           // Mprotect
    ("VirtualQuery",             BmoTarget::Compat("kernel32::memory::query")),
    ("HeapCreate",               BmoTarget::Compat("kernel32::memory::heap_create")),
    ("HeapDestroy",              BmoTarget::Compat("kernel32::memory::heap_destroy")),
    ("HeapAlloc",                BmoTarget::Compat("kernel32::memory::heap_alloc")),
    ("HeapFree",                 BmoTarget::Compat("kernel32::memory::heap_free")),
    ("HeapReAlloc",              BmoTarget::Compat("kernel32::memory::heap_realloc")),
    ("HeapSize",                 BmoTarget::Compat("kernel32::memory::heap_size")),
    ("GetProcessHeap",           BmoTarget::Compat("kernel32::memory::process_heap")),
    ("GlobalAlloc",              BmoTarget::Compat("kernel32::memory::global_alloc")),
    ("GlobalFree",               BmoTarget::Compat("kernel32::memory::global_free")),
    ("LocalAlloc",               BmoTarget::Compat("kernel32::memory::local_alloc")),
    ("LocalFree",                BmoTarget::Compat("kernel32::memory::local_free")),
];

// ─── Thread ─────────────────────────────────────────────────────────────
pub const KERNEL32_THREAD: &[(&str, BmoTarget)] = &[
    ("CreateThread",             BmoTarget::Syscall(0x04)),           // ThreadCreate
    ("ExitThread",               BmoTarget::Syscall(0x05)),           // ThreadExit
    ("SuspendThread",            BmoTarget::Compat("kernel32::thread::suspend")),
    ("ResumeThread",             BmoTarget::Compat("kernel32::thread::resume")),
    ("SwitchToThread",           BmoTarget::Syscall(0x03)),           // Yield
    ("Sleep",                    BmoTarget::Syscall(0x51)),           // SleepNs
    ("SleepEx",                  BmoTarget::Syscall(0x51)),
    ("TlsAlloc",                 BmoTarget::Compat("kernel32::thread::tls_alloc")),
    ("TlsFree",                  BmoTarget::Compat("kernel32::thread::tls_free")),
    ("TlsGetValue",              BmoTarget::Compat("kernel32::thread::tls_get")),
    ("TlsSetValue",              BmoTarget::Compat("kernel32::thread::tls_set")),
    ("InitializeCriticalSection",    BmoTarget::Compat("kernel32::thread::crit_init")),
    ("EnterCriticalSection",         BmoTarget::Compat("kernel32::thread::crit_enter")),
    ("LeaveCriticalSection",         BmoTarget::Compat("kernel32::thread::crit_leave")),
    ("DeleteCriticalSection",        BmoTarget::Compat("kernel32::thread::crit_delete")),
    ("InitializeSRWLock",            BmoTarget::Compat("kernel32::thread::srw_init")),
    ("AcquireSRWLockExclusive",      BmoTarget::Compat("kernel32::thread::srw_lock_ex")),
    ("ReleaseSRWLockExclusive",      BmoTarget::Compat("kernel32::thread::srw_unlock_ex")),
    ("CreateEventA",             BmoTarget::Compat("kernel32::thread::event_create_a")),
    ("CreateEventW",             BmoTarget::Compat("kernel32::thread::event_create_w")),
    ("SetEvent",                 BmoTarget::Compat("kernel32::thread::event_set")),
    ("ResetEvent",               BmoTarget::Compat("kernel32::thread::event_reset")),
    ("CreateMutexA",             BmoTarget::Compat("kernel32::thread::mutex_create_a")),
    ("ReleaseMutex",             BmoTarget::Compat("kernel32::thread::mutex_release")),
];

// ─── File ───────────────────────────────────────────────────────────────
pub const KERNEL32_FILE: &[(&str, BmoTarget)] = &[
    ("CreateFileA",              BmoTarget::Syscall(0x20)),           // FileOpen
    ("CreateFileW",              BmoTarget::Syscall(0x20)),
    ("ReadFile",                 BmoTarget::Syscall(0x21)),           // FileRead
    ("WriteFile",                BmoTarget::Syscall(0x22)),           // FileWrite
    ("CloseHandle",              BmoTarget::Syscall(0x23)),           // FileClose
    ("GetFileSize",              BmoTarget::Syscall(0x25)),           // FileSize
    ("SetFilePointer",           BmoTarget::Syscall(0x24)),           // FileSeek
    ("SetFilePointerEx",         BmoTarget::Syscall(0x24)),
    ("GetFileAttributesA",       BmoTarget::Compat("kernel32::file::get_attr_a")),
    ("GetFileAttributesW",       BmoTarget::Compat("kernel32::file::get_attr_w")),
    ("SetFileAttributesA",       BmoTarget::Compat("kernel32::file::set_attr_a")),
    ("FindFirstFileA",           BmoTarget::Compat("kernel32::file::find_first_a")),
    ("FindFirstFileW",           BmoTarget::Compat("kernel32::file::find_first_w")),
    ("FindNextFileA",            BmoTarget::Compat("kernel32::file::find_next_a")),
    ("FindClose",                BmoTarget::Compat("kernel32::file::find_close")),
    ("CreateDirectoryA",         BmoTarget::Compat("kernel32::file::mkdir_a")),
    ("CreateDirectoryW",         BmoTarget::Compat("kernel32::file::mkdir_w")),
    ("DeleteFileA",              BmoTarget::Compat("kernel32::file::delete_a")),
    ("DeleteFileW",              BmoTarget::Compat("kernel32::file::delete_w")),
    ("CopyFileA",                BmoTarget::Compat("kernel32::file::copy_a")),
    ("CopyFileW",                BmoTarget::Compat("kernel32::file::copy_w")),
    ("MoveFileA",                BmoTarget::Compat("kernel32::file::move_a")),
    ("MoveFileW",                BmoTarget::Compat("kernel32::file::move_w")),
    ("GetTempPathA",             BmoTarget::Compat("kernel32::file::temp_path_a")),
    ("GetTempPathW",             BmoTarget::Compat("kernel32::file::temp_path_w")),
    ("GetCurrentDirectoryA",     BmoTarget::Compat("kernel32::file::cwd_a")),
    ("SetCurrentDirectoryA",     BmoTarget::Compat("kernel32::file::set_cwd_a")),
    ("GetModuleFileNameA",       BmoTarget::Compat("kernel32::module::filename_a")),
    ("GetModuleFileNameW",       BmoTarget::Compat("kernel32::module::filename_w")),
];

// ─── Module ─────────────────────────────────────────────────────────────
pub const KERNEL32_MODULE: &[(&str, BmoTarget)] = &[
    ("GetModuleHandleA",         BmoTarget::Compat("kernel32::module::handle_a")),
    ("GetModuleHandleW",         BmoTarget::Compat("kernel32::module::handle_w")),
    ("LoadLibraryA",             BmoTarget::Compat("kernel32::module::load_a")),
    ("LoadLibraryW",             BmoTarget::Compat("kernel32::module::load_w")),
    ("LoadLibraryExA",           BmoTarget::Compat("kernel32::module::load_ex_a")),
    ("GetProcAddress",           BmoTarget::Compat("kernel32::module::get_proc")),
    ("FreeLibrary",              BmoTarget::Compat("kernel32::module::free")),
    ("GetModuleHandleExA",       BmoTarget::Compat("kernel32::module::handle_ex_a")),
];

// ─── String ─────────────────────────────────────────────────────────────
pub const KERNEL32_STRING: &[(&str, BmoTarget)] = &[
    ("lstrlenA",                 BmoTarget::Compat("kernel32::string::len_a")),
    ("lstrlenW",                 BmoTarget::Compat("kernel32::string::len_w")),
    ("lstrcpyA",                 BmoTarget::Compat("kernel32::string::copy_a")),
    ("lstrcpyW",                 BmoTarget::Compat("kernel32::string::copy_w")),
    ("lstrcpynA",                BmoTarget::Compat("kernel32::string::copy_n_a")),
    ("lstrcmpA",                  BmoTarget::Compat("kernel32::string::cmp_a")),
    ("lstrcmpW",                  BmoTarget::Compat("kernel32::string::cmp_w")),
    ("CharLowerA",               BmoTarget::Compat("kernel32::string::lower_a")),
    ("CharUpperA",               BmoTarget::Compat("kernel32::string::upper_a")),
    ("MultiByteToWideChar",      BmoTarget::Compat("kernel32::string::mb_to_wc")),
    ("WideCharToMultiByte",      BmoTarget::Compat("kernel32::string::wc_to_mb")),
    ("wsprintfA",                BmoTarget::Compat("kernel32::string::sprintf_a")),
    ("wsprintfW",                BmoTarget::Compat("kernel32::string::sprintf_w")),
    ("CharNextA",                BmoTarget::Compat("kernel32::string::next_a")),
    ("CharPrevA",                BmoTarget::Compat("kernel32::string::prev_a")),
];

// ─── Time ───────────────────────────────────────────────────────────────
pub const KERNEL32_TIME: &[(&str, BmoTarget)] = &[
    ("GetTickCount",             BmoTarget::Syscall(0x50)),           // ClockGetTime
    ("GetTickCount64",           BmoTarget::Syscall(0x50)),
    ("QueryPerformanceCounter",  BmoTarget::Syscall(0x50)),
    ("QueryPerformanceFrequency",BmoTarget::Compat("kernel32::time::perf_freq")),
    ("GetSystemTimeAsFileTime",  BmoTarget::Compat("kernel32::time::system_time")),
    ("GetLocalTime",             BmoTarget::Compat("kernel32::time::local_time")),
    ("SetLocalTime",             BmoTarget::Compat("kernel32::time::set_local")),
    ("FileTimeToSystemTime",     BmoTarget::Compat("kernel32::time::ft_to_st")),
    ("SystemTimeToFileTime",     BmoTarget::Compat("kernel32::time::st_to_ft")),
];

// ════════════════════════════════════════════════════════════════════════
// USER32.DLL — Window, Message, Input, Cursor, Metrics
// ════════════════════════════════════════════════════════════════════════

pub const USER32_WINDOW: &[(&str, BmoTarget)] = &[
    ("RegisterClassA",           BmoTarget::Compat("user32::window::register_class_a")),
    ("RegisterClassW",           BmoTarget::Compat("user32::window::register_class_w")),
    ("RegisterClassExA",         BmoTarget::Compat("user32::window::register_class_ex_a")),
    ("RegisterClassExW",         BmoTarget::Compat("user32::window::register_class_ex_w")),
    ("UnregisterClassA",         BmoTarget::Compat("user32::window::unregister_a")),
    ("CreateWindowExA",          BmoTarget::Compat("user32::window::create_ex_a")),
    ("CreateWindowExW",          BmoTarget::Compat("user32::window::create_ex_w")),
    ("DestroyWindow",            BmoTarget::Compat("user32::window::destroy")),
    ("ShowWindow",               BmoTarget::Compat("user32::window::show")),
    ("UpdateWindow",             BmoTarget::Compat("user32::window::update")),
    ("SetWindowPos",             BmoTarget::Compat("user32::window::set_pos")),
    ("MoveWindow",               BmoTarget::Compat("user32::window::move")),
    ("GetClientRect",            BmoTarget::Compat("user32::window::client_rect")),
    ("GetWindowRect",            BmoTarget::Compat("user32::window::window_rect")),
    ("SetWindowTextA",           BmoTarget::Compat("user32::window::set_text_a")),
    ("SetWindowTextW",           BmoTarget::Compat("user32::window::set_text_w")),
    ("GetWindowTextA",           BmoTarget::Compat("user32::window::get_text_a")),
    ("GetWindowTextW",           BmoTarget::Compat("user32::window::get_text_w")),
    ("EnableWindow",             BmoTarget::Compat("user32::window::enable")),
    ("IsWindowVisible",          BmoTarget::Compat("user32::window::is_visible")),
    ("DefWindowProcA",           BmoTarget::Compat("user32::window::def_proc_a")),
    ("DefWindowProcW",           BmoTarget::Compat("user32::window::def_proc_w")),
    ("PostQuitMessage",          BmoTarget::Compat("user32::message::post_quit")),
    ("MessageBoxA",              BmoTarget::Compat("user32::window::message_box_a")),
    ("MessageBoxW",              BmoTarget::Compat("user32::window::message_box_w")),
];

pub const USER32_MESSAGE: &[(&str, BmoTarget)] = &[
    ("GetMessageA",              BmoTarget::Compat("user32::message::get_a")),
    ("GetMessageW",              BmoTarget::Compat("user32::message::get_w")),
    ("PeekMessageA",             BmoTarget::Compat("user32::message::peek_a")),
    ("PeekMessageW",             BmoTarget::Compat("user32::message::peek_w")),
    ("TranslateMessage",         BmoTarget::Compat("user32::message::translate")),
    ("DispatchMessageA",         BmoTarget::Compat("user32::message::dispatch_a")),
    ("DispatchMessageW",         BmoTarget::Compat("user32::message::dispatch_w")),
    ("SendMessageA",             BmoTarget::Compat("user32::message::send_a")),
    ("SendMessageW",             BmoTarget::Compat("user32::message::send_w")),
    ("PostMessageA",             BmoTarget::Compat("user32::message::post_a")),
    ("PostMessageW",             BmoTarget::Compat("user32::message::post_w")),
    ("SetTimer",                 BmoTarget::Compat("user32::message::set_timer")),
    ("KillTimer",                BmoTarget::Compat("user32::message::kill_timer")),
];

pub const USER32_INPUT: &[(&str, BmoTarget)] = &[
    ("GetKeyboardState",         BmoTarget::Compat("user32::input::kb_state")),
    ("GetAsyncKeyState",         BmoTarget::Compat("user32::input::async_key")),
    ("GetKeyState",              BmoTarget::Compat("user32::input::key_state")),
    ("ToAscii",                  BmoTarget::Compat("user32::input::to_ascii")),
    ("MapVirtualKeyA",           BmoTarget::Compat("user32::input::map_vk")),
    ("GetMessagePos",            BmoTarget::Compat("user32::input::msg_pos")),
    ("GetMessageTime",           BmoTarget::Compat("user32::input::msg_time")),
    ("SetCapture",               BmoTarget::Compat("user32::input::set_capture")),
    ("ReleaseCapture",           BmoTarget::Compat("user32::input::release_capture")),
    ("SetCursorPos",             BmoTarget::Compat("user32::input::set_cursor_pos")),
    ("GetCursorPos",             BmoTarget::Compat("user32::input::get_cursor_pos")),
    ("ShowCursor",               BmoTarget::Compat("user32::input::show_cursor")),
    ("LoadCursorA",              BmoTarget::Compat("user32::input::load_cursor_a")),
    ("LoadCursorW",              BmoTarget::Compat("user32::input::load_cursor_w")),
    ("LoadIconA",                BmoTarget::Compat("user32::input::load_icon_a")),
    ("LoadIconW",                BmoTarget::Compat("user32::input::load_icon_w")),
];

pub const USER32_METRICS: &[(&str, BmoTarget)] = &[
    ("GetSystemMetrics",         BmoTarget::Compat("user32::metrics::get_system")),
    ("SystemParametersInfoA",    BmoTarget::Compat("user32::metrics::sys_param_a")),
    ("SystemParametersInfoW",    BmoTarget::Compat("user32::metrics::sys_param_w")),
    ("GetDesktopWindow",         BmoTarget::Compat("user32::metrics::desktop_hwnd")),
    ("GetDC",                    BmoTarget::Compat("user32::gdi::get_dc")),
    ("ReleaseDC",                BmoTarget::Compat("user32::gdi::release_dc")),
    ("BeginPaint",               BmoTarget::Compat("user32::paint::begin")),
    ("EndPaint",                 BmoTarget::Compat("user32::paint::end")),
    ("InvalidateRect",           BmoTarget::Compat("user32::paint::invalidate")),
    ("ValidateRect",             BmoTarget::Compat("user32::paint::validate")),
];

// ════════════════════════════════════════════════════════════════════════
// MSVCRT.DLL — C Runtime (malloc, printf, string, math)
// ════════════════════════════════════════════════════════════════════════

pub const MSVCRT_MEMORY: &[(&str, BmoTarget)] = &[
    ("malloc",                   BmoTarget::Compat("msvcrt::memory::malloc")),
    ("free",                     BmoTarget::Compat("msvcrt::memory::free")),
    ("realloc",                  BmoTarget::Compat("msvcrt::memory::realloc")),
    ("calloc",                   BmoTarget::Compat("msvcrt::memory::calloc")),
    ("_msize",                   BmoTarget::Compat("msvcrt::memory::msize")),
    ("_aligned_malloc",          BmoTarget::Compat("msvcrt::memory::aligned_malloc")),
    ("_aligned_free",            BmoTarget::Compat("msvcrt::memory::aligned_free")),
    ("operator_new",             BmoTarget::Compat("msvcrt::memory::malloc")),
    ("operator_delete",          BmoTarget::Compat("msvcrt::memory::free")),
    ("_set_new_mode",            BmoTarget::Stub),
    ("_callnewh",                BmoTarget::Stub),
];

pub const MSVCRT_STRING: &[(&str, BmoTarget)] = &[
    ("strlen",                   BmoTarget::Compat("msvcrt::string::strlen")),
    ("strcpy",                   BmoTarget::Compat("msvcrt::string::strcpy")),
    ("strncpy",                  BmoTarget::Compat("msvcrt::string::strncpy")),
    ("strcat",                   BmoTarget::Compat("msvcrt::string::strcat")),
    ("strcmp",                   BmoTarget::Compat("msvcrt::string::strcmp")),
    ("strncmp",                  BmoTarget::Compat("msvcrt::string::strncmp")),
    ("strchr",                   BmoTarget::Compat("msvcrt::string::strchr")),
    ("strrchr",                  BmoTarget::Compat("msvcrt::string::strrchr")),
    ("strstr",                   BmoTarget::Compat("msvcrt::string::strstr")),
    ("sprintf",                  BmoTarget::Compat("msvcrt::string::sprintf")),
    ("_snprintf",                BmoTarget::Compat("msvcrt::string::snprintf")),
    ("_vsnprintf",               BmoTarget::Compat("msvcrt::string::vsnprintf")),
    ("memcpy",                   BmoTarget::Compat("msvcrt::string::memcpy")),
    ("memmove",                  BmoTarget::Compat("msvcrt::string::memmove")),
    ("memset",                   BmoTarget::Compat("msvcrt::string::memset")),
    ("memcmp",                   BmoTarget::Compat("msvcrt::string::memcmp")),
    ("_strdup",                  BmoTarget::Compat("msvcrt::string::strdup")),
    ("_stricmp",                 BmoTarget::Compat("msvcrt::string::stricmp")),
    ("_strnicmp",                BmoTarget::Compat("msvcrt::string::strnicmp")),
];

pub const MSVCRT_STDIO: &[(&str, BmoTarget)] = &[
    ("printf",                   BmoTarget::Compat("msvcrt::stdio::printf")),
    ("fprintf",                  BmoTarget::Compat("msvcrt::stdio::fprintf")),
    ("sprintf",                  BmoTarget::Compat("msvcrt::string::sprintf")),
    ("snprintf",                 BmoTarget::Compat("msvcrt::string::snprintf")),
    ("fopen",                    BmoTarget::Compat("msvcrt::stdio::fopen")),
    ("fclose",                   BmoTarget::Compat("msvcrt::stdio::fclose")),
    ("fread",                    BmoTarget::Compat("msvcrt::stdio::fread")),
    ("fwrite",                   BmoTarget::Compat("msvcrt::stdio::fwrite")),
    ("fgets",                    BmoTarget::Compat("msvcrt::stdio::fgets")),
    ("fputs",                    BmoTarget::Compat("msvcrt::stdio::fputs")),
    ("fseek",                    BmoTarget::Compat("msvcrt::stdio::fseek")),
    ("ftell",                    BmoTarget::Compat("msvcrt::stdio::ftell")),
    ("fflush",                   BmoTarget::Compat("msvcrt::stdio::fflush")),
    ("feof",                     BmoTarget::Compat("msvcrt::stdio::feof")),
    ("ferror",                   BmoTarget::Compat("msvcrt::stdio::ferror")),
    ("_get_osfhandle",           BmoTarget::Compat("msvcrt::stdio::get_osfhandle")),
    ("_open_osfhandle",          BmoTarget::Compat("msvcrt::stdio::open_osfhandle")),
];

pub const MSVCRT_STDLIB: &[(&str, BmoTarget)] = &[
    ("exit",                     BmoTarget::Syscall(0x00)),
    ("_exit",                    BmoTarget::Syscall(0x00)),
    ("atexit",                   BmoTarget::Compat("msvcrt::stdlib::atexit")),
    ("atoi",                     BmoTarget::Compat("msvcrt::stdlib::atoi")),
    ("atol",                     BmoTarget::Compat("msvcrt::stdlib::atol")),
    ("atof",                     BmoTarget::Compat("msvcrt::stdlib::atof")),
    ("strtol",                   BmoTarget::Compat("msvcrt::stdlib::strtol")),
    ("strtoul",                  BmoTarget::Compat("msvcrt::stdlib::strtoul")),
    ("strtod",                   BmoTarget::Compat("msvcrt::stdlib::strtod")),
    ("getenv",                   BmoTarget::Compat("msvcrt::stdlib::getenv")),
    ("system",                   BmoTarget::Stub),
    ("qsort",                    BmoTarget::Compat("msvcrt::stdlib::qsort")),
    ("bsearch",                  BmoTarget::Compat("msvcrt::stdlib::bsearch")),
    ("rand",                     BmoTarget::Compat("msvcrt::stdlib::rand")),
    ("srand",                    BmoTarget::Compat("msvcrt::stdlib::srand")),
    ("_errno",                   BmoTarget::Compat("msvcrt::stdlib::errno")),
    ("_amsg_exit",               BmoTarget::Compat("msvcrt::stdlib::amsg_exit")),
];

pub const MSVCRT_INIT: &[(&str, BmoTarget)] = &[
    ("_initterm",                BmoTarget::Compat("msvcrt::init::initterm")),
    ("_initterm_e",              BmoTarget::Compat("msvcrt::init::initterm_e")),
    ("__security_init_cookie",   BmoTarget::Compat("msvcrt::init::security_cookie")),
    ("__GSHandlerCheck",         BmoTarget::Compat("seh::gs_handler")),
    ("__CxxFrameHandler3",       BmoTarget::Compat("seh::cxx_frame_handler")),
    ("_set_se_translator",       BmoTarget::Compat("seh::set_translator")),
    ("_set_invalid_parameter_handler", BmoTarget::Compat("seh::set_invalid_param")),
    ("_set_purecall_handler",    BmoTarget::Compat("seh::set_purecall")),
    ("_set_unexpected_handler",  BmoTarget::Compat("seh::set_unexpected")),
];

// ════════════════════════════════════════════════════════════════════════
// ADVAPI32.DLL — Registry, Security, Crypto
// ════════════════════════════════════════════════════════════════════════

pub const ADVAPI32_REGISTRY: &[(&str, BmoTarget)] = &[
    ("RegOpenKeyExA",            BmoTarget::Compat("advapi32::registry::open_a")),
    ("RegOpenKeyExW",            BmoTarget::Compat("advapi32::registry::open_w")),
    ("RegCreateKeyExA",          BmoTarget::Compat("advapi32::registry::create_a")),
    ("RegCreateKeyExW",          BmoTarget::Compat("advapi32::registry::create_w")),
    ("RegQueryValueExA",         BmoTarget::Compat("advapi32::registry::query_a")),
    ("RegQueryValueExW",         BmoTarget::Compat("advapi32::registry::query_w")),
    ("RegSetValueExA",           BmoTarget::Compat("advapi32::registry::set_a")),
    ("RegSetValueExW",           BmoTarget::Compat("advapi32::registry::set_w")),
    ("RegDeleteKeyA",            BmoTarget::Compat("advapi32::registry::delete_key_a")),
    ("RegDeleteValueA",          BmoTarget::Compat("advapi32::registry::delete_val_a")),
    ("RegCloseKey",              BmoTarget::Compat("advapi32::registry::close")),
    ("RegEnumKeyExA",            BmoTarget::Compat("advapi32::registry::enum_key_a")),
    ("RegEnumValueA",            BmoTarget::Compat("advapi32::registry::enum_val_a")),
];

pub const ADVAPI32_CRYPTO: &[(&str, BmoTarget)] = &[
    ("CryptAcquireContextA",     BmoTarget::Compat("advapi32::crypto::acquire_a")),
    ("CryptReleaseContext",      BmoTarget::Compat("advapi32::crypto::release")),
    ("CryptGenRandom",           BmoTarget::Compat("advapi32::crypto::gen_random")),
    ("CryptCreateHash",          BmoTarget::Compat("advapi32::crypto::create_hash")),
    ("CryptHashData",            BmoTarget::Compat("advapi32::crypto::hash_data")),
    ("CryptGetHashParam",        BmoTarget::Compat("advapi32::crypto::get_hash_param")),
    ("CryptDestroyHash",         BmoTarget::Compat("advapi32::crypto::destroy_hash")),
];

// ════════════════════════════════════════════════════════════════════════
// NTOSKRNL.EXE / NTDLL.DLL — Low-level system
// ════════════════════════════════════════════════════════════════════════

pub const NTDLL: &[(&str, BmoTarget)] = &[
    ("NtAllocateVirtualMemory",  BmoTarget::Syscall(0x10)),
    ("NtFreeVirtualMemory",      BmoTarget::Syscall(0x11)),
    ("NtProtectVirtualMemory",   BmoTarget::Syscall(0x12)),
    ("NtReadFile",               BmoTarget::Syscall(0x21)),
    ("NtWriteFile",              BmoTarget::Syscall(0x22)),
    ("NtClose",                  BmoTarget::Syscall(0x23)),
    ("NtCreateFile",             BmoTarget::Syscall(0x20)),
    ("NtTerminateProcess",       BmoTarget::Syscall(0x00)),
    ("NtCreateThreadEx",         BmoTarget::Syscall(0x04)),
    ("NtWaitForSingleObject",    BmoTarget::Syscall(0x03)),
    ("NtQuerySystemTime",        BmoTarget::Syscall(0x50)),
    ("RtlAddFunctionTable",      BmoTarget::Compat("seh::add_function_table")),
    ("RtlDeleteFunctionTable",   BmoTarget::Compat("seh::delete_function_table")),
    ("RtlVirtualUnwind",         BmoTarget::Compat("seh::virtual_unwind")),
];

// ════════════════════════════════════════════════════════════════════════
// Total count of mapped APIs
// ════════════════════════════════════════════════════════════════════════

/// Total number of Win32 APIs mapped across all DLLs.
pub const TOTAL_MAPPED: usize =
    KERNEL32_PROCESS.len()
    + KERNEL32_MEMORY.len()
    + KERNEL32_THREAD.len()
    + KERNEL32_FILE.len()
    + KERNEL32_MODULE.len()
    + KERNEL32_STRING.len()
    + KERNEL32_TIME.len()
    + USER32_WINDOW.len()
    + USER32_MESSAGE.len()
    + USER32_INPUT.len()
    + USER32_METRICS.len()
    + MSVCRT_MEMORY.len()
    + MSVCRT_STRING.len()
    + MSVCRT_STDIO.len()
    + MSVCRT_STDLIB.len()
    + MSVCRT_INIT.len()
    + ADVAPI32_REGISTRY.len()
    + ADVAPI32_CRYPTO.len()
    + NTDLL.len();

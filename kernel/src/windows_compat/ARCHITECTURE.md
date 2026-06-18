# Windows Compatibility Layer — Architecture (Based on Wine + ReactOS)

## Key Insights from Wine/ReactOS Research

### 1. Wine's PE/Unix Split (Most Important Pattern)

Wine separates Windows PE code from Unix-native code:

```
┌─────────────────────────────────────────────────────┐
│  PE Side (Windows-compatible)                       │
│  - stdcall/fastcall calling conventions             │
│  - Windows data structures (UNICODE_STRING, etc.)   │
│  - Thin stubs that forward to Unix side             │
├─────────────────────────────────────────────────────┤
│  Syscall Dispatcher (__wine_syscall_dispatcher)     │
│  - Converts Windows stack → Unix stack              │
│  - Looks up function in System Service Table        │
│  - Calls Unix function                              │
│  - Restores context and returns                     │
├─────────────────────────────────────────────────────┤
│  Unix Side (Native)                                 │
│  - libc, POSIX, host OS APIs                        │
│  - Real implementations                             │
└─────────────────────────────────────────────────────┘
```

**For FastOS**: Our BEF devour is the PE side. Our windows_compat is the Unix side.
The PE thunks in `pe_thunks.rs` are the thin stubs. The `windows_compat/` modules
are the real implementations.

### 2. Wine's Layering (Bottom-Up)

```
ntdll.dll      ← Lowest level: NT syscalls (NtCreateFile, NtReadFile, etc.)
    ↓
kernelbase.dll ← High-level Win32 (CreateFile, ReadFile, etc.)
    ↓
kernel32.dll   ← CRT, process, thread, file
user32.dll     ← Windows, messages, input
gdi32.dll      ← Drawing, fonts, bitmaps
    ↓
win32u.dll     ← Kernel-mode UI (replaces win32k.sys)
```

**Key**: `ntdll.dll` is the GATEWAY. All Windows apps go through it.
In Wine, ntdll.dll → __wine_syscall_dispatcher → Unix side.

**For FastOS**: Our ntdll.rs in windows_compat should be the central dispatcher.
All Win32 calls should route through it to BMO syscalls.

### 3. ReactOS's Modular Organization

ReactOS organizes into independent sets:

```
sdk/          ← Global dependencies, import libs, CRT, headers
ntcore/       ← Minimal bootable system (ntoskrnl, hal, ntdll)
win32core/    ← win32k, user32, gdi32, kernel32, advapi32
gui/          ← Shell, apps, additional DLLs
```

**For FastOS**: Our windows_compat should follow similar sets:
- `core/` → ntdll, kernel32 basics (process, memory, thread)
- `gui/` → user32, gdi32, win32k equivalent
- `ext/` → shell32, advapi32, comctl32, ole32
- `crt/` → msvcrt, vcruntime

### 4. ReactOS's kernel32 File Organization

ReactOS organizes kernel32 by functional area:

```
kernel32/
├── debug/      break.c, debugger.c, output.c
├── except/     except.c
├── file/       backup.c, create.c, dir.c, file.c, find.c, ...
├── mem/        global.c, heap.c, local.c, virtual.c
├── misc/       atom.c, console.c, env.c, error.c, handle.c, ...
├── process/    cmdline.c, create.c, proc.c
├── string/     lstring.c
├── synch/      critical.c, event.c, mutex.c, sem.c, wait.c
└── thread/     fiber.c, tls.c, thread.c
```

**This is exactly what we need.** Let's adopt this structure.

### 5. Wine's Syscall Dispatcher Pattern

```c
// Wine's syscall flow:
// 1. App calls NtWriteFile() in ntdll.dll
// 2. ntdll.dll has a thunk that calls __wine_syscall_dispatcher
// 3. Dispatcher saves Windows context
// 4. Dispatcher switches to Unix stack
// 5. Dispatcher looks up NtWriteFile in syscalls[] table
// 6. Dispatcher calls Unix implementation
// 7. Dispatcher restores Windows context
// 8. Dispatcher returns to app

// The key data structure:
static void * const syscalls[] = {
    NtAcceptConnectPort,     // 0x000
    NtAccessCheck,           // 0x001
    NtAccessCheckAndAuditAlarm, // 0x002
    ...
    NtWriteFile,             // 0x015
    ...
};
```

**For FastOS**: We need a similar syscall table mapping NT numbers → BMO functions.

### 6. ReactOS's Win32 Subsystem (win32ss)

```
win32ss/
├── gdi/          ← GDI kernel-mode implementation
│   ├── ntgdi/    ← NT GDI functions
│   └── eng/      ← Graphics engine
├── user/         ← USER kernel-mode implementation
│   └── ntuser/   ← NT USER functions
├── win32u/       ← User-mode bridge
└── drivers/      ← Video drivers
```

**Key**: user32.dll and gdi32.dll are thin wrappers around win32k.sys.

**For FastOS**: Our BMO desktop IS win32k.sys. user32/gdi32 just call into it.

## Recommended Reorganization

Based on these patterns, here's how to reorganize `windows_compat/`:

```
windows_compat/
├── mod.rs                    ← Root, init(), version
├── README.md                 ← Architecture doc
├── api_map.rs                ← Master Win32→BMO mapping
│
├── ntdll/                    ← Gateway layer (like Wine's ntdll)
│   ├── mod.rs                ← Module root, syscall dispatch
│   ├── syscalls.rs           ← NT syscall table (Nt* functions)
│   ├── memory.rs             ← NtAllocateVirtualMemory, etc.
│   ├── file.rs               ← NtCreateFile, NtReadFile, etc.
│   ├── thread.rs             ← NtCreateThread, NtTerminateThread, etc.
│   ├── process.rs            ← NtCreateProcess, NtTerminateProcess, etc.
│   ├── objects.rs            ← NtOpenSection, NtMapViewOfSection, etc.
│   └── rtl.rs                ← RTL_* functions (strings, lists, etc.)
│
├── kernel32/                 ← High-level Win32 (like ReactOS)
│   ├── mod.rs
│   ├── process/              ← Process management
│   │   ├── mod.rs
│   │   ├── create.c          ← CreateProcess, CreateProcessA/W
│   │   ├── cmdline.c         ← GetCommandLine, CommandLineToArgvW
│   │   └── proc.c            ← GetCurrentProcess, TerminateProcess
│   ├── thread/               ← Thread management
│   │   ├── mod.rs
│   │   ├── thread.c          ← CreateThread, ExitThread
│   │   ├── tls.c             ← TlsAlloc, TlsGetValue, TlsSetValue
│   │   ├── fiber.c           ← CreateFiber, SwitchToFiber
│   │   └── synch.c           ← Sleep, WaitForSingleObject
│   ├── memory/               ← Memory management
│   │   ├── mod.rs
│   │   ├── virtual.c         ← VirtualAlloc, VirtualFree
│   │   ├── heap.c            ← HeapCreate, HeapAlloc, HeapFree
│   │   ├── global.c          ← GlobalAlloc, GlobalFree
│   │   └── local.c           ← LocalAlloc, LocalFree
│   ├── file/                 ← File I/O
│   │   ├── mod.rs
│   │   ├── create.c          ← CreateFile, DeleteFile
│   │   ├── read.c            ← ReadFile, ReadFileEx
│   │   ├── write.c           ← WriteFile, WriteFileEx
│   │   ├── seek.c            ← SetFilePointer, SetFilePointerEx
│   │   ├── find.c            ← FindFirstFile, FindNextFile
│   │   ├── dir.c             ← CreateDirectory, RemoveDirectory
│   │   ├── copy.c            ← CopyFile, MoveFile
│   │   └── info.c            ← GetFileSize, GetFileAttributes
│   ├── module/               ← DLL management
│   │   ├── mod.rs
│   │   ├── load.c            ← LoadLibrary, LoadLibraryEx
│   │   ├── proc.c            ← GetProcAddress
│   │   └── handle.c          ← GetModuleHandle, FreeLibrary
│   ├── string/               ← String operations
│   │   ├── mod.rs
│   │   ├── ansi.c            ← lstrlenA, lstrcpyA, lstrcmpA
│   │   ├── wide.c            ← lstrlenW, lstrcpyW, lstrcmpW
│   │   ├── unicode.c         ← MultiByteToWideChar, WideCharToMultiByte
│   │   └── fmt.c             ← wsprintf, wvsprintf
│   ├── env/                  ← Environment
│   │   ├── mod.rs
│   │   ├── cmdline.c         ← GetCommandLineA/W
│   │   ├── envvar.c          ← GetEnvironmentVariable, SetEnvironmentVariable
│   │   └── paths.c           ← GetCurrentDirectory, GetTempPath
│   └── time/                 ← Time functions
│       ├── mod.rs
│       ├── perf.c            ← QueryPerformanceCounter/Frequency
│       ├── tick.c            ← GetTickCount, GetTickCount64
│       └── filetime.c        ← GetSystemTimeAsFileTime, FileTimeToSystemTime
│
├── user32/                   ← Window management (like ReactOS win32ss/user)
│   ├── mod.rs
│   ├── window.rs             ← RegisterClass, CreateWindow, DestroyWindow
│   ├── message.rs            ← GetMessage, DispatchMessage, PostMessage
│   ├── paint.rs              ← BeginPaint, EndPaint, InvalidateRect
│   ├── input.rs              ← GetKeyboardState, ToAscii, MapVirtualKey
│   ├── cursor.rs             ← LoadCursor, ShowCursor, SetCursorPos
│   ├── metrics.rs            ← GetSystemMetrics, SystemParametersInfo
│   └── dialog.rs             ← DialogBox, CreateDialog, EndDialog
│
├── gdi32/                    ← Graphics (like ReactOS win32ss/gdi)
│   ├── mod.rs
│   ├── device.rs             ← CreateDC, CreateCompatibleDC, DeleteDC
│   ├── bitmap.rs             ← CreateBitmap, BitBlt, StretchBlt
│   ├── text.rs               ← TextOut, DrawText, GetTextExtentPoint
│   ├── font.rs               ← CreateFont, SelectObject, GetCharWidth
│   ├── brush.rs              ← CreateSolidBrush, FillRect, PatBlt
│   ├── pen.rs                ← CreatePen, LineTo, Rectangle
│   └── region.rs             ← CreateRectRgn, CombineRgn
│
├── msvcrt/                   ← C Runtime
│   ├── mod.rs
│   ├── memory.rs             ← malloc, free, realloc, calloc
│   ├── string.rs             ← strlen, strcmp, strcpy, memcpy, memset
│   ├── stdio.rs              ← printf, fprintf, fopen, fclose
│   ├── stdlib.rs             ← exit, atoi, getenv, qsort
│   ├── math.rs               ← sin, cos, sqrt, floor, ceil
│   └── init.rs               ← _initterm, __security_init_cookie
│
├── advapi32/                 ← Registry, Security
│   ├── mod.rs
│   ├── registry.rs           ← RegOpenKey, RegQueryValue, RegSetValue
│   └── crypto.rs             ← CryptAcquireContext, CryptGenRandom
│
├── shell32/                  ← Shell operations
│   ├── mod.rs
│   ├── path.rs               ← SHGetFolderPath, SHGetSpecialFolderPath
│   └── execute.rs            ← ShellExecute, ShellExecuteEx
│
├── comctl32/                 ← Common controls
│   └── mod.rs                ← InitCommonControlsEx
│
├── ole32/                    ← COM/OLE
│   ├── mod.rs                ← CoInitialize, CoCreateInstance
│   └── memory.rs             ← CoTaskMemAlloc, CoTaskMemFree
│
└── seh/                      ← Exception handling
    ├── mod.rs                ← SEH/VEH dispatch
    ├── unwind.rs             ← RUNTIME_FUNCTION, .pdata parsing
    └── cookie.rs             ← __security_init_cookie, GS handler
```

## Implementation Priority (Based on Wine/ReactOS Patterns)

### Phase 1: Gateway Layer (ntdll)
- [ ] Create `ntdll/syscalls.rs` with NT syscall table
- [ ] Implement ntdll→BMO syscall dispatcher
- [ ] Map Nt* functions to BMO syscalls

### Phase 2: Core Runtime (kernel32 basics)
- [ ] kernel32/process: CreateProcess, TerminateProcess
- [ ] kernel32/thread: CreateThread, ExitThread, Sleep
- [ ] kernel32/memory: VirtualAlloc, VirtualFree, HeapCreate
- [ ] kernel32/file: CreateFile, ReadFile, WriteFile
- [ ] msvcrt: malloc, free, printf, exit

### Phase 3: GUI Layer (user32 + gdi32)
- [ ] user32/window: RegisterClass, CreateWindow, DefWindowProc
- [ ] user32/message: GetMessage, DispatchMessage, PostMessage
- [ ] gdi32/text: TextOut, DrawText
- [ ] gdi32/device: CreateDC, CreateCompatibleDC

### Phase 4: Extensions (advapi32, shell32, etc.)
- [ ] advapi32/registry: RegOpenKey, RegQueryValue
- [ ] shell32/path: SHGetFolderPath
- [ ] seh: SEH/VEH exception handling

## Key Differences: Wine vs FastOS

| Aspect | Wine | FastOS |
|---|---|---|
| Host OS | Linux/macOS/BSD | Bare metal (no host) |
| Syscall target | POSIX/libc | BMO syscalls |
| Memory model | Virtual memory (mmap) | Page allocator + heap |
| Process model | Unix processes | BMO processes (Ring 0/3) |
| Graphics | X11/Wayland | GOP framebuffer |
| Networking | BSD sockets | BMO net stack |
| Threading | pthreads | BMO threads |

**Advantage**: We don't need to translate to POSIX — we can implement directly
in BMO. Wine has an extra layer of indirection that we don't need.

## References

- Wine source: https://github.com/wine-mirror/wine
- ReactOS source: https://github.com/reactos/reactos
- Wine PE/Unix split: https://deepwiki.com/wine-mirror/wine/1.2-windows-api-implementation-layer
- Wine syscall dispatcher: https://blog.hiler.eu/wine-pe-to-unix/
- ReactOS architecture: https://reactos.org/architecture/
- ReactOS kernel32: https://github.com/reactos/reactos/tree/master/sdk/lib/3rdparty
